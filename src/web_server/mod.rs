use crate::database::Database;
use fehler::throws;
use maplit::hashmap;
use rocket::{http::Status, response::Redirect, *};
use rocket_contrib::{
    serve::StaticFiles,
    templates::{handlebars, Template},
};

use tokio::task;

#[macro_use]
mod ui;
#[macro_use]
mod api;
#[macro_use]
mod misc;

use ui::*;
// use api::*;
use misc::*;

trait ErrToStatus<T> {
    fn err_to_status(self) -> Result<T, Status>
    where
        Self: Sized;
}

impl<T> ErrToStatus<T> for anyhow::Result<T> {
    fn err_to_status(self) -> Result<T, Status>
    where
        Self: Sized,
    {
        // TODO: return error mesage in debug env
        self.map_err(|err| {
            error!("client had error: {}", err);
            Status::new(500, "Internal Server Error")
        })
    }
}

#[throws(anyhow::Error)]
pub async fn init(db: Database, stopped: futures::channel::oneshot::Receiver<()>) {
    let rocket = rocket::ignite()
        .manage(db)
        .mount("/", ui_routes!())
        .mount("/api", api_routes!())
        .mount("/static", StaticFiles::from("./static"))
        .register(catchers![not_found])
        .attach(Template::custom(|engines| {
            misc::setup_handlebars(&mut engines.handlebars);
        }));

    let shutdown_handle = rocket.shutdown();
    task::spawn(async {
        if stopped.await.is_ok() {
            shutdown_handle.shutdown();
        }
    });

    rocket.launch().await?;
}
