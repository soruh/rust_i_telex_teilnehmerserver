use crate::database::Database;
use fehler::throws;
use maplit::hashmap;
use rocket::{response::Redirect, *};
use rocket_contrib::templates::{handlebars, Template};
use tokio::task;

#[macro_use]
mod ui;
use ui::*;

#[macro_use]
mod api;
// use api::*;

#[macro_use]
mod misc;
use misc::*;

#[throws(anyhow::Error)]
pub async fn init(db: Database, stopped: futures::channel::oneshot::Receiver<()>) {
    let rocket = rocket::ignite()
        .manage(db)
        .mount("/", ui_routes!())
        .mount("/api", api_routes!())
        .register(misc_catchers!())
        .attach(misc_template_engine!());

    let shutdown_handle = rocket.shutdown();
    task::spawn(async {
        if stopped.await.is_ok() {
            shutdown_handle.shutdown();
        }
    });

    rocket.launch().await?;
}
