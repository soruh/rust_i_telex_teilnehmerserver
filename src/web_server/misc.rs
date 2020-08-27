use self::handlebars::{Context, Handlebars, Helper, Output, RenderContext, RenderError};
use super::*;
use rocket::request::FromRequest;
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

pub struct LocaleTemplate {
    name: std::borrow::Cow<'static, str>,
    context: Option<serde_json::Value>,
}

impl LocaleTemplate {
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

impl<'r, 'o: 'r> rocket::response::Responder<'r, 'o> for LocaleTemplate {
    fn respond_to(self, request: &'r Request<'_>) -> response::Result<'o> {
        // TODO: what impact does this `block_on` have on performance?
        match futures::executor::block_on(ApplicationLanguage::from_request(request)) {
            outcome::Outcome::Success(locale) => {
                info!(
                    "Rendering template \x1b[32m{}\x1b[0m with locale \x1b[33m{:?}\x1b[0m.",
                    self.name, locale
                );
                let mut map = if let Some(serde_json::Value::Object(map)) = self.context {
                    map
                } else {
                    panic!(
                        "The context to a `LocaleTemplate` must be a `serde_json::Value::Object`"
                    )
                };
                map.insert("__locale".to_string(), serde_json::to_value(locale).unwrap());
                let context = serde_json::Value::Object(map);
                Template::render(self.name, context).respond_to(request)
            }

            _ => unreachable!(),
        }
    }
}

#[catch(404)]
pub fn not_found(req: &Request<'_>) -> LocaleTemplate {
    LocaleTemplate::render("error/404", hashmap! {
        "path" => req.uri().path(),
    })
}

#[throws(RenderError)]
pub fn user_name_helper(
    helper: &Helper<'_, '_>,
    _handlebars: &Handlebars,
    context: &Context,
    _render_context: &mut RenderContext<'_, '_>,
    out: &mut dyn Output,
) {
    // TODO: convert errors to `RenderError`

    assert_eq!(
        context.data().get("show_owner"),
        Some(&serde_json::json!(true)),
        "You can only use this helper when showing connector owners"
    );

    let user_id = helper.params().first().expect("`user_name_helper` needs one argument").render();

    let user_name = context
        .data()
        .get("user_name_map")
        .expect("missing `user_name_map`")
        .get(user_id)
        .expect("missing id in user_name_map");

    out.write(user_name.as_str().unwrap())?;
}

#[throws(RenderError)]
pub fn connector_name_helper(
    helper: &Helper<'_, '_>,
    _handlebars: &Handlebars,
    context: &Context,
    _render_context: &mut RenderContext<'_, '_>,
    out: &mut dyn Output,
) {
    // TODO: convert errors to `RenderError`

    assert_eq!(
        context.data().get("show_connector"),
        Some(&serde_json::json!(true)),
        "You can only use this helper when showing machine connectors"
    );

    let connector =
        helper.params().first().expect("`connector_name_helper` needs one argument").render();

    let connector_name = context
        .data()
        .get("connector_name_map")
        .expect("missing `connector_name_map`")
        .get(connector)
        .expect("missing id in connector_name_map");

    out.write(connector_name.as_str().unwrap())?;
}

#[throws(RenderError)]
pub fn localisation_helper(
    helper: &Helper<'_, '_>,
    _handlebars: &Handlebars,
    context: &Context,
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
        out.write(lang.get(&tag).ok_or_else(|| {
            RenderError::new(&format!("missing tanslation \"{}::{}\"", locale, tag))
        })?)?;
    }
}

pub fn setup_handlebars(handlebars: &mut handlebars::Handlebars<'static>) {
    handlebars.set_strict_mode(true);
    handlebars.source_map_enabled(true);
    handlebars.register_helper("locl", Box::new(localisation_helper));
    handlebars.register_helper("user_name", Box::new(user_name_helper));
    handlebars.register_helper("connector_name", Box::new(connector_name_helper));
}
