use crate::database::Database;
use fehler::throws;
use maplit::hashmap;
use rocket::{response::Redirect, *};
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

#[throws(anyhow::Error)]
pub async fn init(db: Database, stopped: futures::channel::oneshot::Receiver<()>) {
    let rocket = rocket::ignite()
        .manage(db)
        .mount("/", ui_routes!())
        .mount("/api", api_routes!())
        .mount("/static", StaticFiles::from("./static"))
        .register(catchers![not_found])
        .attach(template_engine!());

    let shutdown_handle = rocket.shutdown();
    task::spawn(async {
        if stopped.await.is_ok() {
            shutdown_handle.shutdown();
        }
    });

    rocket.launch().await?;
}
