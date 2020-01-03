#![feature(async_closure)]
#![warn(
    clippy::all,
    // clippy::pedantic,
    // clippy::cargo, // TODO
    clippy::nursery,
    clippy::unimplemented,
)]
#![allow(clippy::similar_names)]
#![feature(backtrace)]

// #[macro_use] extern crate diesel;
// use diesel::prelude::*;
// use diesel::sqlite::SqliteConnection;

#[macro_use]
extern crate anyhow;

#[macro_use]
extern crate lazy_static;

#[macro_use] pub mod errors;

pub mod client;
pub mod db;
pub mod packages;
pub mod serde;

use client::{Client, State};

pub use crate::packages::*;
pub use crate::errors::MyErrorKind;

#[macro_use]
extern crate log;
extern crate simple_logger;


use async_std::{
    net::{TcpListener, TcpStream},
    prelude::*,
    task,
};

use std::{
    cell::RefCell,
    net::SocketAddr,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use futures::{
    channel::{mpsc, oneshot},
    future::{select_all, FutureExt},
    select,
    sink::SinkExt,
    stream::StreamExt,
};

use db::*;

lazy_static! {
    pub static ref CLIENT_TIMEOUT: Duration = Duration::new(30, 0);
    pub static ref ITELEX_EPOCH: SystemTime = UNIX_EPOCH
        .checked_sub(Duration::from_secs(60 * 60 * 24 * 365 * 70))
        .unwrap();
    pub static ref SERVERS: Vec<SocketAddr> = vec![SocketAddr::new([0, 0, 0, 0].into(), 11814)];
}

#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn get_current_itelex_timestamp() -> u32 {
    SystemTime::now()
        .duration_since(*ITELEX_EPOCH)
        .unwrap()
        .as_secs() as u32
}

const LISTEN_ADDR: &str = "0.0.0.0:11814";
const SERVER_PIN: u32 = 42;
const SERVER_COOLDOWN: Duration = Duration::from_secs(30);
// const DB_FILE: &str = "./database";
//TODO: use config

#[async_std::main]
async fn main() {
    simple_logger::init().unwrap();

    let (background_task_handles, stop_background_tasks) = start_timers();
    info!("started background tasks");

    let (stop_accept_loop, stopped_accept_loop) = oneshot::channel::<()>();
    let stop_accept_loop = RefCell::new(Some(stop_accept_loop));
    ctrlc::set_handler(move || {
        let stop_accept_loop = stop_accept_loop.replace(None);

        if let Some(stop_accept_loop) = stop_accept_loop {
            info!("got first ctl-c: attempting to shut down gracefully");
            stop_accept_loop.send(()).unwrap();
        } else {
            warn!("got second ctl-c: aborting");
            std::process::abort();
        }
    })
    .expect("Failed to register Ctrl-C handler");

    let (client_watchdog, watchdog_sender) = start_client_watchdog();

    let mut stopped_accept_loop = stopped_accept_loop.fuse();

    let listener = TcpListener::bind(LISTEN_ADDR).await.unwrap();
    info!("listening for connections on {}", LISTEN_ADDR);
    loop {
        select! {
            res = listener.accept().fuse() => {
                let (socket, _) = res.expect("Failed to accept socket");
                let client = Client::new(socket);
                watchdog_sender
                    .clone()
                    .send(start_handling_client(client))
                    .await
                    .expect("Failed to register new client");
            },
            _ = stopped_accept_loop => break,
        }
    }
    drop(listener);
    info!("accept loop has ended; shutting down");

    info!("stopping background tasks");
    for stop_background_task in stop_background_tasks {
        if stop_background_task.send(()).is_err() {
            error!("Failed to stop a background task.");
        }
    }
    futures::future::join_all(background_task_handles).await;
    info!("done");

    info!("waiting for children to finish");
    drop(watchdog_sender);
    client_watchdog.await; // .expect("Failed to wait for children to die")
    info!("done");

    info!("syncing database to disk");
    sync_db_to_disk().await;
    info!("done");

    info!("exiting");
}

fn start_client_watchdog() -> (
    task::JoinHandle<()>,
    mpsc::Sender<task::JoinHandle<anyhow::Result<()>>>,
) {
    let (watchdog_sender, mut watchdog_receiver) =
        mpsc::channel::<task::JoinHandle<anyhow::Result<()>>>(1);

    let client_watchdog: task::JoinHandle<()> = task::spawn(async move {
        let mut clients = Vec::new();
        let mut shutdown = false;

        while !(shutdown && clients.is_empty()) {
            if shutdown {
                debug!("[watchdog] we're shutting down, but there are clients to wait for");
            } else if clients.is_empty() {
                debug!("[watchdog] we're running, but are not waiting for any clients");
            } else {
                debug!("[watchdog] we're running, and there are still clients left");
            }

            if clients.is_empty() {
                if let Some(client_handle) = watchdog_receiver.next().await {
                    debug!("[watchdog] Got a new client");
                    clients.push(client_handle);
                } else {
                    debug!("[watchdog] shutting down");

                    // shutdown = true;
                    break;
                }
            } else {
                debug!("[watchdog] waiting for {} clients", clients.len());
                let mut wait_for_clients = select_all(clients.drain(..)).fuse();

                'inner: loop {
                    let recv_client = async {
                        if shutdown {
                            futures::future::pending().await
                        } else {
                            watchdog_receiver.next().await
                        }
                    };

                    select! {
                        (res, _, mut rest) = wait_for_clients => {
                            debug!("[watchdog] a client we were waiting for finished: {:?}", res);

                            clients.append(&mut rest);
                            break 'inner;
                        },
                        res = recv_client.fuse() => {
                            if let Some(client_handle) = res {
                                debug!("[watchdog] Got a new client");
                                clients.push(client_handle);
                            } else {
                                debug!("[watchdog] shutting down");
                                shutdown = true;
                            }
                        },
                    }
                }
            }
        }

        debug!("[watchdog] successfully shut down");
    });

    (client_watchdog, watchdog_sender)
}

macro_rules! start_timer {
    ($name: literal, $function: ident, $interval: expr, $($arg:tt)*) => {{
        let (sender, receiver) = oneshot::channel();

        let join_handle = task::spawn(async move {
            info!("starting {:?} background task", $name);
            let mut exit = receiver.fuse();
            loop {
                debug!("running background task {:?}", $name);
                if let Err(err) = $function($($arg)*).await {
                    error!("failed to run background task {:?}: {:?}", $name, err);
                }
                // TODO: handle better

                #[allow(clippy::mut_mut, clippy::unnecessary_mut_passed)]
                {
                    select! {
                        _ = exit => break,
                        _ = task::sleep(Duration::new(60 * 60 * 24 * 7, 0)).fuse() => continue,
                    }
                }
            }
            info!("stopped {:?} background task", $name);
        });

        (join_handle, sender)
    }};
}

fn start_timers() -> (Vec<task::JoinHandle<()>>, Vec<oneshot::Sender<()>>) {
    let mut join_handles = Vec::new();
    let mut abort_senders = Vec::new();

    let (join_handle, abort_sender) = start_timer!(
        "prune old queue entries",
        prune_old_queue_entries,
        Duration::from_secs(60 * 60 * 24 * 7),
    );
    join_handles.push(join_handle);
    abort_senders.push(abort_sender);

    let (join_handle, abort_sender) =
        start_timer!("full query", full_query, Duration::from_secs(60 * 60 * 24),);
    join_handles.push(join_handle);
    abort_senders.push(abort_sender);


    let (server_join_handles, mut server_senders, server_abort_senders) = sync_other_servers(SERVERS.to_vec());
    abort_senders.extend(server_abort_senders);
    join_handles.extend(server_join_handles);

    let (join_handle, abort_sender) =
        start_timer!("sync changed", sync_changed, Duration::from_secs(30), &mut server_senders);
    join_handles.push(join_handle);
    abort_senders.push(abort_sender);

    (join_handles, abort_senders)
}

async fn handle_client_result(result: anyhow::Result<()>, client: &mut Client, addr: Option<SocketAddr>) -> anyhow::Result<()> {
    if let Err(error) = result.as_ref() {
        let addr = if let Some(addr) = addr.or_else(|| client.socket.peer_addr().ok()) {
            format!("{}", addr)
        } else {
            String::from("`unknown`")
        };

        warn!("client at {} had an error: {}", addr, error);


        let pkg = Package::Type255(Package255 {
            message: format!("The server encountered an error: {}", error),
        });

        if let Err(error) = client.send_package(pkg).await {
            debug!("Failed to notify client at {} of failure: {}", addr, error);
        } else {
            debug!("Notified client of failure");
        }

        if let Err(error) = client.shutdown() {
            debug!("Failed to shut down client at {}: {}", addr, error);
        }
    }

    info!("connection closed");

    result
}

fn start_handling_client(mut client: Client) -> task::JoinHandle<anyhow::Result<()>> {
    task::spawn(async move {
        handle_client_result(client.handle().await, &mut client, None).await
    })
}

async fn full_query_for_server(server: SocketAddr) -> anyhow::Result<()> {
    debug!("stating full query for server {}", server);

    let mut client = connect_to(server).await?;;

    client.state = State::Accepting;

    client
        .send_package(Package::Type6(Package6 {
            version: 1,
            server_pin: SERVER_PIN,
        }))
        .await?;

    start_handling_client(client).await?;

    debug!("finished full query for server {}", server);

    Ok(())
}

async fn send_queue_for_server(server: SocketAddr) -> anyhow::Result<()> {
    let mut client = connect_to(server).await?;;
    client.state = State::Responding;

    client
        .send_package(Package::Type6(Package6 {
            version: 1,
            server_pin: SERVER_PIN,
        }))
        .await
        .unwrap();

    start_handling_client(client).await
}

async fn full_query() -> anyhow::Result<()> {
    let mut full_queries = Vec::new();

    info!("stating full query");

    for server in SERVERS.iter() {
        full_queries.push(full_query_for_server(server.clone()));
    }

    futures::future::join_all(full_queries).await;

    info!("finished full query");

    Ok(()) //TODO
}

async fn send_queue() -> anyhow::Result<()> {
    let changed_entries = get_changed_entries().await;
    // TODO: handle res

    Ok(()) //TODO
}

async fn connect_to(addr: SocketAddr) -> anyhow::Result<Client> {
    Ok(Client::new(TcpStream::connect(addr).await?))
}


async fn send_packages_to_server(server: SocketAddr, packages: Vec<Package5>) -> anyhow::Result<()> {
    let mut client = connect_to(server).await?;

    client.send_queue.extend(packages.into_iter());

    client.state = State::Responding;

    client.send_package(Package::Type7(Package7 {
        server_pin: SERVER_PIN,
        version: 1,
    })).await?;


    start_handling_client(client);

    bail!(err_unimplemented!())
}

fn sync_other_servers(
    servers: Vec<SocketAddr>,
) -> (
    Vec<task::JoinHandle<()>>,
    Vec<mpsc::UnboundedSender<Vec<Package5>>>,
    Vec<oneshot::Sender<()>>,
) {
    let mut join_handles = Vec::new();
    let mut senders = Vec::new();
    let mut abort_senders = Vec::new();

    for server in servers {
        let (abort_sender, abort_receiver) = oneshot::channel::<()>();
        abort_senders.push(abort_sender);

        let (sender, receiver) = mpsc::unbounded::<Vec<Package5>>();
        senders.push(sender);

        join_handles.push(task::spawn(async move {
            info!("started syncing server: {}", server);

            let mut receiver = receiver.fuse();
            let mut abort_receiver = abort_receiver.fuse();
            'outer: while let Some(packages) = receiver.next().await {
                while let Err(err) = send_packages_to_server(server, packages.clone()).await {
                    warn!("Failed to sync server {}: {:?}", server, err);

                    select! {
                        res = abort_receiver => if res.is_ok() { break 'outer; },
                        _ = task::sleep(SERVER_COOLDOWN).fuse() => {},
                    }
                }
            }

            info!("stopped syncing server: {}", server);
        }));
    }

    (join_handles, senders, abort_senders)
}


async fn sync_changed(server_senders: &mut Vec<mpsc::UnboundedSender<Vec<Package5>>>) -> anyhow::Result<()> {
    // TODO
    for sender in server_senders {
        sender.send(Vec::new()).await?;
    }

    Ok(())
}