use self::handlebars::{Handlebars, Helper, Output, RenderContext, RenderError};
use super::*;
use crate::data_types::{User, UserId};
use anyhow::Context;
use rocket::{
    http::{Cookie, CookieJar},
    request::FromRequest,
};
use std::collections::HashMap;

pub struct AcceptedLanguages(Vec<String>);

#[async_trait]
impl<'a, 'r> FromRequest<'a, 'r> for AcceptedLanguages {
    type Error = std::convert::Infallible;

    async fn from_request(request: &'a Request<'r>) -> request::Outcome<Self, Self::Error> {
        let mut res: Vec<(String, f32)> = request
            .headers()
            .get("accept-language")
            .map(|header| {
                header.split(',').map(str::trim).filter_map(|x| {
                    let mut parts = x.split(';').map(str::trim);
                    let lang = parts.find(|x| !x.starts_with("q="))?;
                    let qualifier: f32 = parts
                        .find(|x| x.starts_with("q="))
                        .map(|x| x[2..].parse().ok())
                        .flatten()
                        .unwrap_or(1.);

                    Some((lang.to_string(), qualifier))
                })
            })
            .flatten()
            .collect();

        res.sort_by(|(_, qualifier_a), (_, qualifier_b)| {
            qualifier_b.partial_cmp(qualifier_a).unwrap_or(std::cmp::Ordering::Equal)
        });

        request::Outcome::Success(AcceptedLanguages(
            res.into_iter().map(|(lang, _)| lang).collect(),
        ))
    }
}

#[derive(Debug, Clone, Copy, serde::Deserialize, serde::Serialize, PartialEq, Eq, Hash)]
pub enum ApplicationLanguage {
    English,
    German,
}

impl std::fmt::Display for ApplicationLanguage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Self::English => write!(f, "en"),
            Self::German => write!(f, "de"),
        }
    }
}

use std::convert::TryFrom;

impl TryFrom<&str> for ApplicationLanguage {
    type Error = ();

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value.split('-').next() {
            Some("en") => Ok(Self::English),
            Some("de") => Ok(Self::German),

            _ => Err(()),
        }
    }
}

#[async_trait]
impl<'a, 'r> FromRequest<'a, 'r> for ApplicationLanguage {
    type Error = std::convert::Infallible;

    async fn from_request(request: &'a Request<'r>) -> request::Outcome<Self, Self::Error> {
        match AcceptedLanguages::from_request(request).await {
            outcome::Outcome::Success(AcceptedLanguages(accepted_languages)) => {
                outcome::Outcome::Success(
                    accepted_languages
                        .into_iter()
                        .find_map(|lang| ApplicationLanguage::try_from(lang.as_str()).ok())
                        .unwrap_or(ApplicationLanguage::German),
                )
            }

            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Auth {
    user: User,
    cookie: Cookie<'static>,
}

const AUTH_COOKIE: &str = "authentication";

#[async_trait]
impl<'a, 'r> FromRequest<'a, 'r> for Auth {
    type Error = anyhow::Error;

    async fn from_request(request: &'a Request<'r>) -> request::Outcome<Self, Self::Error> {
        let run = || -> anyhow::Result<Option<Self>> {
            let cookie = if let Some(cookie) = request.cookies().get_private(AUTH_COOKIE) {
                cookie
            } else {
                return Ok(None);
            };

            let user_id: UserId = cookie
                .value()
                .parse::<uuid::Uuid>()
                .context("user is logged in as an invalid UUID")?
                .into();

            let db = request.managed_state::<Database>().unwrap();

            db.users()
                .unwrap()
                .get(user_id)
                .unwrap()
                .context("user is logged in as a User that does not exist")
                .map(|user| Some(Auth { user, cookie }))
        };
        match run() {
            Ok(Some(res)) => request::Outcome::Success(res),
            Ok(None) => request::Outcome::Forward(()),
            Err(err) => request::Outcome::Failure((rocket::http::Status::Forbidden, err)),
        }
    }
}

impl Auth {
    pub fn deauthorize(self, cookies: &CookieJar) {
        cookies.remove_private(self.cookie);
    }

    pub fn authorize(cookies: &CookieJar, user_id: UserId, remember: bool) {
        let mut cookie = Cookie::new(AUTH_COOKIE, user_id.0.to_string());
        cookie.set_expires(match remember {
            true => Some(
                time::OffsetDateTime::now_utc()
                    + std::time::Duration::from_secs(60 * 60 * 24 * 365 * 10), // in 10 years
            ),
            false => None,
        });
        cookies.add_private(cookie);
    }
}

pub struct ContextTemplate {
    name: std::borrow::Cow<'static, str>,
    context: Option<serde_json::Value>,
}

impl ContextTemplate {
    pub fn render<S, C>(name: S, context: C) -> Self
    where
        S: Into<std::borrow::Cow<'static, str>>,
        C: serde::Serialize,
    {
        Self { name: name.into(), context: context.serialize(serde_json::value::Serializer).ok() }
    }
}
#[derive(serde::Serialize, Debug)]
struct LocaleContext {
    locale: ApplicationLanguage,
    ctx: Option<serde_json::Value>,
}

impl<'r, 'o: 'r> rocket::response::Responder<'r, 'o> for ContextTemplate {
    fn respond_to(self, request: &'r Request<'_>) -> response::Result<'o> {
        let locale = if let outcome::Outcome::Success(locale) =
            futures::executor::block_on(ApplicationLanguage::from_request(request))
        {
            locale
        } else {
            unreachable!()
        };

        let auth: Option<User> = if let outcome::Outcome::Success(auth) =
            futures::executor::block_on(Auth::from_request(request))
        {
            Some(auth.user)
        } else {
            None
        };

        info!(
            "Rendering template \x1b[32m{}\x1b[0m with locale \x1b[33m{:?}\x1b[0m.",
            self.name, locale
        );
        let mut map = if let Some(serde_json::Value::Object(map)) = self.context {
            map
        } else {
            panic!("The context to a `ContextTemplate` must be a `serde_json::Value::Object`")
        };

        assert!(
            map.insert("__locale".to_string(), serde_json::to_value(locale).unwrap()).is_none(),
            "context already had `__locale`"
        );

        assert!(
            map.insert("__auth".to_string(), serde_json::to_value(auth).unwrap()).is_none(),
            "context already had `__auth`"
        );

        assert!(
            map.insert(
                "__uri".to_string(),
                serde_json::to_value(request.uri().to_string()).unwrap()
            )
            .is_none(),
            "context already had `__uri`"
        );

        let context = serde_json::Value::Object(map);
        Template::render(self.name, context).respond_to(request)
    }
}

#[catch(404)]
pub fn not_found(req: &Request<'_>) -> ContextTemplate {
    ContextTemplate::render("error/404", hashmap! {
        "path" => req.uri().path(),
    })
}

#[derive(Debug)]
struct HelperError(std::borrow::Cow<'static, str>);

impl std::fmt::Display for HelperError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Helper Error: {:?}", self.0)
    }
}

impl std::error::Error for HelperError {}

impl From<HelperError> for RenderError {
    fn from(error: HelperError) -> Self {
        Self::from_error("helper error", error)
    }
}

#[throws(RenderError)]
pub fn user_name_helper(
    helper: &Helper<'_, '_>,
    _handlebars: &Handlebars,
    context: &handlebars::Context,
    _render_context: &mut RenderContext<'_, '_>,
    out: &mut dyn Output,
) {
    let mut params = helper.params().iter();

    let user_id = params
        .next()
        .ok_or_else(|| HelperError("`user_name` helper needs at least one argument".into()))?
        .render();

    let context = if let Some(context) = params.next() {
        context.value()
    } else {
        context
            .data()
            .get("user_name_map")
            .ok_or_else(|| HelperError("`user_name_map` not found in context".into()))?
    };

    let user_name = context
        .get(&user_id)
        .ok_or_else(|| HelperError(format!("missing id {} in `user_name_map`", user_id).into()))?;

    out.write(user_name.as_str().unwrap())?;
}

#[throws(RenderError)]
pub fn connector_name_helper(
    helper: &Helper<'_, '_>,
    _handlebars: &Handlebars,
    context: &handlebars::Context,
    _render_context: &mut RenderContext<'_, '_>,
    out: &mut dyn Output,
) {
    let mut params = helper.params().iter();

    let connector_id = params
        .next()
        .ok_or_else(|| HelperError("`connector_name` helper needs at least one argument".into()))?
        .render();

    let context = if let Some(context) = params.next() {
        context.value()
    } else {
        context
            .data()
            .get("connector_name_map")
            .ok_or_else(|| HelperError("`connector_name_map` not found in context".into()))?
    };

    let connector_name = context.get(&connector_id).ok_or_else(|| {
        HelperError(format!("missing id {} in `connector_name_map`", connector_id).into())
    })?;

    out.write(connector_name.as_str().unwrap())?;
}

#[throws(RenderError)]
pub fn localisation_helper(
    helper: &Helper<'_, '_>,
    _handlebars: &Handlebars,
    context: &handlebars::Context,
    _render_context: &mut RenderContext<'_, '_>,
    out: &mut dyn Output,
) {
    use rocket_contrib::templates::handlebars::RenderError;

    type Locl = HashMap<ApplicationLanguage, HashMap<String, String>>;

    static LOCL: once_cell::sync::OnceCell<Locl> = once_cell::sync::OnceCell::new();
    /*
    if let Some(param) = h.param(0) {
        out.write("<b><i>")?;
        out.write(&param.value().render())?;
        out.write("</b></i>")?;
    }
    */

    let locl = LOCL.get();

    let locl: &Locl = if let Some(locl) = locl {
        locl
    } else {
        let locl = serde_json::from_reader::<_, Locl>(
            std::fs::File::open(rocket_contrib::crate_relative!("./localisation.json"))
                .map_err(|_| RenderError::new("Failed read localisation data"))?,
        )
        .map_err(|_| RenderError::new("Failed to parse localisation data"))?;

        let mut keys: Option<Vec<_>> = None;
        for language in locl.values() {
            if let Some(ref keys) = keys {
                let mut new_keys = language.keys().collect::<Vec<_>>();
                new_keys.sort();
                assert_eq!(keys, &new_keys, "All locales must have the same localisation keys");
            } else {
                let mut new_keys: Vec<_> = language.keys().collect();
                new_keys.sort();
                keys = Some(new_keys);
            }
        }

        LOCL.set(locl).unwrap();
        LOCL.get().unwrap()
    };

    let locale: ApplicationLanguage = serde_json::from_value(context.data()["__locale"].clone())?;

    let lang = locl
        .get(&locale)
        .ok_or_else(|| RenderError::new(&format!("missing tanslation \"{}\"", locale)))?;

    if let Some(param) = helper.params().first() {
        let tag: String = param.render();
        out.write(
            lang.get(&tag)
                .ok_or_else(|| RenderError::new(&format!("missing tanslation \"{}\"", tag)))?,
        )?;
    }
}

#[throws(RenderError)]
pub fn debug(
    helper: &Helper<'_, '_>,
    _handlebars: &Handlebars,
    context: &handlebars::Context,
    _render_context: &mut RenderContext<'_, '_>,
    out: &mut dyn Output,
) {
    out.write("<pre>")?;
    out.write(&format!("helper: {:#?}\n<hr />", helper))?;
    out.write(&format!("context: {:#?}", context))?;
    out.write("</pre>")?;
}

pub fn setup_handlebars(handlebars: &mut handlebars::Handlebars<'static>) {
    handlebars.set_strict_mode(true);
    handlebars.source_map_enabled(true);
    handlebars.register_helper("locl", Box::new(localisation_helper));
    handlebars.register_helper("user_name", Box::new(user_name_helper));
    handlebars.register_helper("connector_name", Box::new(connector_name_helper));
    handlebars.register_helper("dbg", Box::new(debug));
}
