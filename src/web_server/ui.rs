use super::*;
use crate::data_types::*;
use rocket::http::Status;
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

#[get("/list/users")]
#[throws(Status)]
pub fn list_users(db: State<Database>) -> LocaleTemplate {
    #[derive(serde::Serialize)]
    struct UserListContext {
        page: Page,
        users: Vec<User>,
    }

    let users = db.users().err_to_status()?.iter().all_values().err_to_status()?;

    LocaleTemplate::render("list_users", UserListContext {
        page: Page::new("layout", "user_list"),
        users,
    })
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

#[get("/list/connectors")]
#[throws(Status)]
pub fn list_connectors(db: State<Database>) -> LocaleTemplate {
    let connectors = db.connectors().err_to_status()?.iter().all_values().err_to_status()?;

    let user_name_map =
        make_name_map(connectors.iter().map(|x| x.owner), db.users().err_to_status()?, |user| {
            user.name
        })?;

    LocaleTemplate::render("list_connectors", ConnectorListContext {
        page: Some(Page::new("layout", "connector_list")),
        connectors,
        user_name_map,
        show_owner: true,
    })
}

#[get("/list/machines")]
#[throws(Status)]
pub fn list_machines(db: State<Database>) -> LocaleTemplate {
    let machines = db.machines().err_to_status()?.iter().all_values().err_to_status()?;

    let connector_name_map = make_name_map(
        machines.iter().map(|x| x.connector),
        db.connectors().err_to_status()?,
        |connector| connector.name,
    )?;

    LocaleTemplate::render("list_machines", MachineListContext {
        page: Some(Page::new("layout", "machine_list")),
        machines,
        connector_name_map,
        show_connector: true,
    })
}

#[derive(serde::Serialize)]
struct ConnectorListContext {
    page: Option<Page>,
    connectors: Vec<Connector>,
    user_name_map: HashMap<UserId, String>,
    show_owner: bool,
}

#[derive(serde::Serialize)]
struct MachineListContext {
    page: Option<Page>,
    machines: Vec<Machine>,
    connector_name_map: HashMap<ConnectorId, String>,
    show_connector: bool,
}

#[get("/user/<id>")]
#[throws(Status)]
pub fn user(id: UserId, db: State<Database>) -> LocaleTemplate {
    #[derive(serde::Serialize)]
    struct UserContext {
        page: Page,
        user: User,
        connector_data: ConnectorListContext,
    }

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

    LocaleTemplate::render("user", UserContext {
        page: Page::new("layout", "user_detail"),
        user,
        connector_data: ConnectorListContext {
            page: None,
            connectors,
            show_owner: false,
            user_name_map: HashMap::new(),
        },
    })
}

#[get("/connector/<id>")]
#[throws(Status)]
pub fn connector(id: ConnectorId, db: State<Database>) -> LocaleTemplate {
    #[derive(serde::Serialize)]
    struct ConnectorContext {
        page: Page,
        connector: Connector,
        machine_data: MachineListContext,
    }

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

    LocaleTemplate::render("connector", ConnectorContext {
        page: Page::new("layout", "connector_detail"),
        connector,
        machine_data: MachineListContext {
            page: None,
            machines,
            show_connector: false,
            connector_name_map: HashMap::new(),
        },
    })
}

#[get("/machine/<id>")]
#[throws(Status)]
pub fn machine(id: MachineId, db: State<Database>) -> LocaleTemplate {
    let machine =
        db.machines().err_to_status()?.get(id).err_to_status()?.ok_or(Status::NotFound)?;

    let connector_name_map = make_name_map(
        std::iter::once(machine.connector),
        db.connectors().err_to_status()?,
        |connector| connector.name,
    )?;

    LocaleTemplate::render("machine", MachineListContext {
        page: Some(Page::new("layout", "machine_detail")),
        machines: vec![machine],
        connector_name_map,
        show_connector: true,
    })
}

#[macro_export]
macro_rules! ui_routes {
    () => {
        routes![index, list_users, user, list_connectors, connector, list_machines, machine]
    };
}
