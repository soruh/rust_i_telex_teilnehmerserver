use super::*;
use crate::data_types::*;
use misc::Auth;
use rocket::{
    http::{uri::Uri, CookieJar, Status},
    request::Form,
};
use std::{borrow::Cow, collections::HashMap};
#[derive(serde::Serialize)]
struct Page {
    parent: Cow<'static, str>,
    title: Cow<'static, str>,
}

impl Page {
    fn new(parent: impl Into<Cow<'static, str>>, title: impl Into<Cow<'static, str>>) -> Self {
        Self { parent: parent.into(), title: title.into() }
    }
}

#[get("/")]
pub fn index() -> Redirect {
    Redirect::to("/list/users")
}

#[throws(Status)]
fn make_name_map<'value, K: std::fmt::Debug + Eq + std::hash::Hash + Copy, V>(
    ids: impl Iterator<Item = K>,
    tree: crate::database::Tree<K, V>,
    f: impl Fn(V) -> String,
) -> HashMap<K, String>
where
    K: AsRef<[u8]>,
    V: From<sled::IVec> + Into<sled::IVec>,
    &'value V: Into<sled::IVec> + 'value,
{
    let mut name_map = HashMap::new();
    for id in ids {
        name_map.entry(id).or_insert(f(tree.get(id).err_to_status()?.unwrap_or_else(|| {
            panic!(
                "Database is inconsistent. There is an a non-existant parent. Broken id: {:?}",
                id,
            )
        })));
    }
    name_map
}

macro_rules! redirect_to {
    ($source:expr) => {{
        use std::convert::TryInto;

        let uri: Uri =
            $source.and_then(|x| x.try_into().ok()).or_else(|| "/".try_into().ok()).unwrap();

        Redirect::permanent(uri)
    }};
}

#[post("/auth/log_out?<source>", rank = 1)]
pub fn log_out(source: Option<String>, auth: Auth, cookies: &CookieJar) -> Redirect {
    auth.deauthorize(cookies);

    redirect_to!(source)
}

#[post("/auth/log_out?<source>", rank = 2)]
pub fn log_out_unauthorized(source: Option<String>) -> Redirect {
    redirect_to!(source)
}

#[derive(rocket::FromForm)]
pub struct LogInData {
    username: String,
    password: String,
    remember: bool,
}

#[post("/auth/log_in?<source>", data = "<form>")]
#[throws(Status)]
pub fn auth_log_in(
    source: Option<String>,
    form: Form<LogInData>,
    cookies: &CookieJar,
    db: State<Database>,
) -> Redirect {
    let user = db
        .users()
        .err_to_status()?
        .iter()
        .values()
        .map(|x| x.unwrap())
        .find(|user| user.name == form.username)
        .ok_or_else(|| Status::NotFound)?;

    if !user.password.validate(&form.password) {
        fehler::throw!(Status::Unauthorized);
    }

    Auth::authorize(cookies, user.id, form.remember);

    redirect_to!(source)
}

#[get("/log_in?<source>")]
#[throws(Status)]
pub fn log_in(source: Option<String>) -> ContextTemplate {
    #[derive(serde::Serialize)]
    struct LogInContext {
        page: Page,
        source: Option<String>,
    }

    ContextTemplate::render("log_in", LogInContext { page: Page::new("layout", "log_in"), source })
}

#[get("/list/users")]
#[throws(Status)]
pub fn list_users(db: State<Database>) -> ContextTemplate {
    let users = db.users().err_to_status()?.iter().all_values().err_to_status()?;

    ContextTemplate::render("list_users", UserListContext {
        page: Some(Page::new("layout", "user_list")),
        users,
    })
}

#[get("/list/connectors")]
#[throws(Status)]
pub fn list_connectors(db: State<Database>) -> ContextTemplate {
    let connectors = db.connectors().err_to_status()?.iter().all_values().err_to_status()?;

    let user_name_map = Some(make_name_map(
        connectors.iter().map(|x| x.owner),
        db.users().err_to_status()?,
        |user| user.name,
    )?);

    ContextTemplate::render("list_connectors", ConnectorListContext {
        page: Some(Page::new("layout", "connector_list")),
        connectors,
        user_name_map,
    })
}

#[get("/list/machines")]
#[throws(Status)]
pub fn list_machines(db: State<Database>) -> ContextTemplate {
    let machines = db.machines().err_to_status()?.iter().all_values().err_to_status()?;

    let connector_name_map = Some(make_name_map(
        machines.iter().map(|x| x.connector),
        db.connectors().err_to_status()?,
        |connector| connector.name,
    )?);

    ContextTemplate::render("list_machines", MachineListContext {
        page: Some(Page::new("layout", "machine_list")),
        machines,
        connector_name_map,
    })
}

#[derive(serde::Serialize)]
struct UserListContext {
    page: Option<Page>,
    users: Vec<User>,
}

#[derive(serde::Serialize)]
struct ConnectorListContext {
    page: Option<Page>,
    connectors: Vec<Connector>,
    user_name_map: Option<HashMap<UserId, String>>,
}

#[derive(serde::Serialize)]
struct MachineListContext {
    page: Option<Page>,
    machines: Vec<Machine>,
    connector_name_map: Option<HashMap<ConnectorId, String>>,
}

#[get("/user/<id>")]
#[throws(Status)]
pub fn user(id: UserId, db: State<Database>) -> ContextTemplate {
    let user = db.users().err_to_status()?.get(id).err_to_status()?.ok_or(Status::NotFound)?;
    let connectors = db
        .connectors()
        .err_to_status()?
        .iter()
        .all_values()
        .err_to_status()?
        .into_iter()
        .filter(|x| x.owner == user.id)
        .collect();

    #[derive(serde::Serialize)]
    struct UserContext {
        page: Page,
        user_data: UserListContext,
        connector_data: ConnectorListContext,
    }

    ContextTemplate::render("user", UserContext {
        page: Page::new("layout", "user_detail"),
        user_data: UserListContext { page: None, users: vec![user] },
        connector_data: ConnectorListContext { page: None, connectors, user_name_map: None },
    })
}

#[get("/connector/<id>")]
#[throws(Status)]
pub fn connector(id: ConnectorId, db: State<Database>) -> ContextTemplate {
    let connector =
        db.connectors().err_to_status()?.get(id).err_to_status()?.ok_or(Status::NotFound)?;

    let machines = db
        .machines()
        .err_to_status()?
        .iter()
        .all_values()
        .err_to_status()?
        .into_iter()
        .filter(|x| x.connector == connector.id)
        .collect();

    #[derive(serde::Serialize)]
    struct ConnectorContext {
        page: Page,
        connector_data: ConnectorListContext,
        machine_data: MachineListContext,
    }

    ContextTemplate::render("connector", ConnectorContext {
        page: Page::new("layout", "connector_detail"),
        connector_data: ConnectorListContext {
            page: None,
            connectors: vec![connector],
            user_name_map: None,
        },
        machine_data: MachineListContext { page: None, machines, connector_name_map: None },
    })
}

#[get("/machine/<id>")]
#[throws(Status)]
pub fn machine(id: MachineId, db: State<Database>) -> ContextTemplate {
    let machine =
        db.machines().err_to_status()?.get(id).err_to_status()?.ok_or(Status::NotFound)?;

    let connector_name_map = Some(make_name_map(
        std::iter::once(machine.connector),
        db.connectors().err_to_status()?,
        |connector| connector.name,
    )?);

    #[derive(serde::Serialize)]
    struct MachineContext {
        page: Page,
        machine_data: MachineListContext,
    }

    ContextTemplate::render("machine", MachineContext {
        page: Page::new("layout", "machine_detail"),
        machine_data: MachineListContext {
            page: None,
            machines: vec![machine],
            connector_name_map,
        },
    })
}

#[macro_export]
macro_rules! ui_routes {
    () => {
        routes![
            index,
            list_users,
            user,
            list_connectors,
            connector,
            list_machines,
            machine,
            log_out,
            log_out_unauthorized,
            auth_log_in,
            log_in
        ]
    };
}
