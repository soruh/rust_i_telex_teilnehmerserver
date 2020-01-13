#![feature(async_closure)]
#![warn(clippy::all, clippy::nursery)]

#[macro_use] extern crate anyhow;

#[macro_use] extern crate log;

macro_rules! config {
    ($key:ident) => {
        CONFIG.get().unwrap().$key
    };
}

pub mod client;
pub mod config;
pub mod db;
pub mod errors;
pub mod packages;
pub mod serde;

pub use errors::ItelexServerErrorKind;
pub use packages::*;

use async_std::{
    io::prelude::*,
    net::{TcpListener, TcpStream},
    sync::RwLock,
    task,
};
use client::{Client, Mode, State};
use config::Config;
use db::*;
use futures::{
    channel::{mpsc, oneshot},
    future::{select_all, FutureExt},
    select,
    sink::SinkExt,
    stream::StreamExt,
};
use once_cell::sync::{Lazy, OnceCell};
use std::{
    cell::RefCell,
    collections::HashMap,
    net::SocketAddr,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[allow(clippy::cast_possible_truncation)]

pub fn get_current_itelex_timestamp() -> u32 {
    SystemTime::now().duration_since(*ITELEX_EPOCH).unwrap().as_secs() as u32
}

// Configuration
pub static CONFIG: OnceCell<Config> = OnceCell::new();

// Actual constants
const PEER_SEARCH_VERSION: u8 = 1;
const FULL_QUERY_VERSION: u8 = 1;
const LOGIN_VERSION: u8 = 1;
pub static ITELEX_EPOCH: Lazy<SystemTime> =
    Lazy::new(|| UNIX_EPOCH.checked_sub(Duration::from_secs(60 * 60 * 24 * 365 * 70)).unwrap());

// global state
pub static SERVERS: OnceCell<Vec<SocketAddr>> = OnceCell::new();
pub static CHANGED: Lazy<RwLock<HashMap<u32, ()>>> = Lazy::new(|| RwLock::new(HashMap::new()));
pub static DATABASE: Lazy<RwLock<HashMap<u32, Package5>>> = Lazy::new(|| RwLock::new(HashMap::new()));

#[async_std::main]
async fn main() -> anyhow::Result<()> {
    simple_logger::init().unwrap();

    if let Err(err) = dotenv::dotenv() {
        error!("Failed to load configuration from `.env` file: {}", err);
    }

    CONFIG.set(Config::from_env()?).expect("Failed to set config");

    if config!(SERVER_PIN) == 0 {
        warn!(
            "The server is running without a SERVER_PIN. Server interaction will be reduced to publicly available \
             levels. DB sync will be disabled so that no private state is overwritten."
        );
    }

    read_fs_data().await?;

    let (client_watchdog, watchdog_sender) = start_client_watchdog();

    let (background_task_handles, stop_background_tasks) = start_tasks();

    info!("starting acccept loop");

    match listen_for_connections(register_exit_handler(), watchdog_sender).await {
        Ok(accept_loop) => accept_loop.await, // Wait for accept loop to end
        Err(err) => error!("Failed to start accept loop: {}", err),
    }

    warn!("shutting down");

    warn!("stopping background tasks");
    for stop_background_task in stop_background_tasks {
        if stop_background_task.send(()).is_err() {
            error!("Failed to stop a background task.");
        }
    }
    futures::future::join_all(background_task_handles).await;

    warn!("waiting for children to finish");
    client_watchdog.await;

    sync_db_to_disk().await.expect("Failed to sync DB");

    warn!("exiting");

    Ok(())
}

async fn register_client(
    listen_res: std::io::Result<(TcpStream, SocketAddr)>,
    mut watchdog_sender: mpsc::Sender<task::JoinHandle<anyhow::Result<()>>>,
) {
    match listen_res {
        Ok((socket, addr)) => {
            if let Err(err) = watchdog_sender.send(start_handling_client(Client::new(socket, addr))).await {
                error!("Failed to register new client: {}", err);
            } else {
                info!("Info new connection from {}", addr);
            }
        }
        Err(err) => error!("Failed to accept a client: {}", err),
    }
}

async fn listen_for_connections(
    stop_the_loop: oneshot::Receiver<()>,
    watchdog_sender: mpsc::Sender<task::JoinHandle<anyhow::Result<()>>>,
) -> anyhow::Result<task::JoinHandle<()>> {
    use std::net::{Ipv4Addr, Ipv6Addr};

    let ipv4_listener = TcpListener::bind((Ipv4Addr::UNSPECIFIED, config!(SERVER_PORT))).await?;
    let ipv6_listener = TcpListener::bind((Ipv6Addr::UNSPECIFIED, config!(SERVER_PORT))).await.ok();

    let mut stop_the_loop = stop_the_loop.fuse();

    Ok(task::spawn(async move {
        info!("listening for connections on port {}", config!(SERVER_PORT));

        if let Some(ipv6_listener) = ipv6_listener {
            loop {
                select! {
                    res = ipv4_listener.accept().fuse() => register_client(res, watchdog_sender.clone()).await,
                    res = ipv6_listener.accept().fuse() => register_client(res, watchdog_sender.clone()).await,

                    _ = stop_the_loop => break,
                }
            }
        } else {
            loop {
                select! {
                    res = ipv4_listener.accept().fuse() => register_client(res, watchdog_sender.clone()).await,

                    _ = stop_the_loop => break,
                }
            }
        }

        info!("accept loop has ended");
    }))
}

async fn read_fs_data() -> anyhow::Result<()> {
    match read_servers().await {
        Ok(servers) => {
            if servers.is_empty() {
                warn!("No remote servers were set. This server will not syncronize with other servers.");
            }

            SERVERS.set(servers).expect("Failed to set server list");
        }
        Err(err) => {
            error!("Failed to read server list from {}: {}", config!(SERVER_FILE_PATH), err);

            bail!(err);
        }
    }

    if let Err(err) = read_db_from_disk().await {
        error!("Failed to restore DB from disk: {}", err);
        error!("repair or delete {:?}", config!(DB_PATH));
        // TODO: be smarter (try to restore from .temp etc.)
        // ? should we really be smarter or is that the responibility of the user ?
        bail!(err);
    }

    Ok(())
}

fn register_exit_handler() -> oneshot::Receiver<()> {
    use simple_signal::{set_handler, Signal};

    let (stop_accept_loop, stopped_accept_loop) = oneshot::channel::<()>();

    let stop_accept_loop = RefCell::new(Some(stop_accept_loop));

    set_handler(&[Signal::Int, Signal::Term], move |signals| {
        if let Some(stop_accept_loop) = stop_accept_loop.replace(None) {
            warn!("got first exit signal {:?}: attempting to shut down gracefully", signals);

            stop_accept_loop.send(()).unwrap();
        } else {
            error!("got second exit signal {:?}: aborting", signals);

            std::process::abort();
        }
    });

    stopped_accept_loop
}

async fn read_servers() -> anyhow::Result<Vec<SocketAddr>> {
    use async_std::{
        fs::File,
        io::{BufReader, ErrorKind},
        net::ToSocketAddrs,
    };

    let mut servers = Vec::new();

    match File::open(&config!(SERVER_FILE_PATH)).await {
        Ok(file) => {
            let mut lines = BufReader::new(file).lines();

            while let Some(line) = lines.next().await {
                let socket_addrs = line?.to_socket_addrs().await?;

                // only use the first result to prevent syncing a server twice
                // (e.g. if there is both an Ipv4 and an Ipv6 address for a server)
                // We prefer ipv4 addresses, since older servers only listen on those
                let ipv4 = socket_addrs.clone().find(|addr| addr.is_ipv4());

                if let Some(addr) = ipv4 {
                    servers.push(addr);
                } else {
                    servers.extend(socket_addrs.take(1));
                }
            }
        }
        Err(err) => {
            if err.kind() != ErrorKind::NotFound {
                bail!(anyhow!("Failed to open server list: {}", err));
            }

            if let Err(err) = File::create(&config!(SERVER_FILE_PATH)).await {
                bail!(anyhow!("Failed to create new server list: {}", err));
            } else {
                warn!("created new server list at {}", config!(SERVER_FILE_PATH));
            }
        }
    }

    warn!("Read {} servers from server list", servers.len());

    warn!("servers: {}", servers.iter().map(|e| format!("{}", e)).collect::<Vec<String>>().join(", "));

    Ok(servers)
}

fn start_client_watchdog() -> (task::JoinHandle<()>, mpsc::Sender<task::JoinHandle<anyhow::Result<()>>>) {
    let (watchdog_sender, mut watchdog_receiver) = mpsc::channel::<task::JoinHandle<anyhow::Result<()>>>(1);

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
                        if shutdown { futures::future::pending().await } else { watchdog_receiver.next().await }
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

// TODO: refactor
fn start_tasks() -> (Vec<task::JoinHandle<()>>, Vec<oneshot::Sender<()>>) {
    info!("spawning background tasks");

    let mut join_handles = Vec::new();

    let mut abort_senders = Vec::new();

    let (server_join_handles, mut server_senders, server_abort_senders) =
        update_other_servers(SERVERS.get().unwrap().to_vec());

    abort_senders.extend(server_abort_senders);

    join_handles.extend(server_join_handles);

    let name = "sync db";
    let (abort_sender, abort_receiver) = oneshot::channel();
    abort_senders.push(abort_sender);
    join_handles.push(task::spawn(async move {
        task::sleep(Duration::from_secs(1)).await;
        info!("starting {:?} background task", name);
        let mut exit = abort_receiver.fuse();
        loop {
            debug!("running background task {:?}", name);
            if let Err(err) = sync_db_to_disk().await {
                error!("failed to run background task {:?}: {:?}", name, err);
            }
            select! {
                _ = exit => break,
                _ = task::sleep(config!(DB_SYNC_INTERVAL)).fuse() => continue,
            }
        }
        info!("stopped {:?} background task", name);
    }));

    let name = "sync changed";
    let (abort_sender, abort_receiver) = oneshot::channel();
    abort_senders.push(abort_sender);
    join_handles.push(task::spawn(async move {
        task::sleep(Duration::from_secs(3)).await;
        info!("starting {:?} background task", name);
        let mut exit = abort_receiver.fuse();
        loop {
            debug!("running background task {:?}", name);
            if let Err(err) = sync_changed(&mut server_senders).await {
                error!("failed to run background task {:?}: {:?}", name, err);
            }
            select! {
                _ = exit => break,
                _ = task::sleep(config!(CHANGED_SYNC_INTERVAL)).fuse() => continue,
            }
        }
        info!("stopped {:?} background task", name);
    }));

    let name = "full query";
    let (abort_sender, abort_receiver) = oneshot::channel();
    abort_senders.push(abort_sender);
    join_handles.push(task::spawn(async move {
        task::sleep(Duration::from_secs(2)).await;
        info!("starting {:?} background task", name);
        let mut exit = abort_receiver.fuse();
        loop {
            debug!("running background task {:?}", name);
            if let Err(err) = full_query().await {
                error!("failed to run background task {:?}: {:?}", name, err);
            }
            select! {
                _ = exit => break,
                _ = task::sleep(config!(FULL_QUERY_INTERVAL)).fuse() => continue,
            }
        }
        info!("stopped {:?} background task", name);
    }));

    info!("spawned background tasks");

    (join_handles, abort_senders)
}

async fn handle_client_result(result: anyhow::Result<()>, client: &mut Client) -> anyhow::Result<()> {
    let addr = client.address;

    if let Err(error) = result.as_ref() {
        warn!("client at {} had an error: {}", addr, error);

        let message = format!("The server encountered an error: {}\r\n", error);

        if client.mode == Mode::Binary {
            let _ = client.send_package(Package::Type255(Package255 { message })).await;
        } else if client.mode == Mode::Ascii {
            let _ = client.socket.write_all(message.as_bytes()).await;
        }
    } else {
        info!("client at {} finished", addr);
    }

    if let Err(error) = client.shutdown() {
        debug!("Failed to shut down client at {}: {}", addr, error);
    }

    result
}

fn start_handling_client(mut client: Client) -> task::JoinHandle<anyhow::Result<()>> {
    task::spawn(async move { handle_client_result(client.handle().await, &mut client).await })
}

async fn full_query_for_server(server: SocketAddr) -> anyhow::Result<()> {
    debug!("starting full query for server {}", server);

    let mut client = connect_to(server).await?;

    client.state = State::Accepting;

    let pkg = if config!(SERVER_PIN) == 0 {
        warn!("Sending empty peer search instead of full query, because no server pin was specified");

        Package::Type10(Package10 { version: PEER_SEARCH_VERSION, pattern: String::from("") })
    } else {
        Package::Type6(Package6 { version: FULL_QUERY_VERSION, server_pin: config!(SERVER_PIN) })
    };

    client.send_package(pkg).await?;

    start_handling_client(client).await?;

    warn!("finished full query for server {}", server);

    Ok(())
}

async fn full_query() -> anyhow::Result<()> {
    let mut full_queries = Vec::new();

    info!("starting full query");

    for server in SERVERS.get().unwrap().iter() {
        full_queries.push(full_query_for_server(server.clone()));
    }

    for result in futures::future::join_all(full_queries).await {
        if let Err(err) = result {
            error!("A full query failed: {}", err);
        }
    }

    info!("finished full query");

    let n_changed = CHANGED.read().await.len();

    if n_changed > 0 {
        warn!("Server has {} changed entries", n_changed);
    }

    sync_db_to_disk().await?;

    Ok(()) //TODO
}

async fn connect_to(addr: SocketAddr) -> anyhow::Result<Client> {
    info!("connecting to server at {}", addr);

    Ok(Client::new(TcpStream::connect(addr).await?, addr))
}

async fn update_server_with_packages(server: SocketAddr, packages: Vec<Package5>) -> anyhow::Result<()> {
    if config!(SERVER_PIN) == 0 {
        bail!(anyhow!("Not updating other servers with an empty server pin"));
    }

    let mut client = connect_to(server).await?;

    client.send_queue.extend(packages.into_iter());

    client.state = State::Responding;

    client.send_package(Package::Type7(Package7 { server_pin: config!(SERVER_PIN), version: LOGIN_VERSION })).await?;

    start_handling_client(client).await
}

fn update_other_servers(
    servers: Vec<SocketAddr>,
) -> (Vec<task::JoinHandle<()>>, Vec<mpsc::UnboundedSender<Vec<Package5>>>, Vec<oneshot::Sender<()>>) {
    let mut join_handles = Vec::new();

    let mut senders = Vec::new();

    let mut abort_senders = Vec::new();

    for server in servers {
        let (abort_sender, abort_receiver) = oneshot::channel::<()>();

        abort_senders.push(abort_sender);

        let (sender, mut receiver) = mpsc::unbounded::<Vec<Package5>>();

        senders.push(sender);

        join_handles.push(task::spawn(async move {
            info!("started syncing server: {}", server);

            let mut abort_receiver = abort_receiver.fuse();

            // NOTE: receiver already implementes `FusedStream` and so does not need to be
            // `fuse`ed
            'outer: while let Some(mut packages) = receiver.next().await {
                debug!("Received {} initial packages", packages.len());

                task::sleep(Duration::from_millis(10)).await;

                // Wait a bit, in case there are more packages on the way, but not yet in the
                // channel TODO: should we really do this?
                while let Ok(additional) = receiver.try_next() {
                    if let Some(additional) = additional {
                        debug!("Extending queue for client by {} additional packages", additional.len());

                        packages.extend(additional);
                    } else {
                        break;
                    }
                }

                // This should never happen and be handled be handled by senders,
                // so that no task sync needs to take place:
                // TODO: remove?
                if packages.is_empty() {
                    debug!("There are no packages to sync");

                    continue;
                }

                while let Err(err) = update_server_with_packages(server, packages.clone()).await {
                    error!("Failed to update server {}: {:?}", server, err);

                    info!("retrying in: {:?}", config!(SERVER_COOLDOWN));

                    select! {
                        res = abort_receiver => if res.is_ok() { break 'outer; },
                        _ = task::sleep(config!(SERVER_COOLDOWN)).fuse() => {},
                    }
                }
            }

            info!("stopped syncing server: {}", server);
        }));
    }

    (join_handles, senders, abort_senders)
}

async fn sync_changed(server_senders: &mut Vec<mpsc::UnboundedSender<Vec<Package5>>>) -> anyhow::Result<()> {
    let changed = get_changed_entries().await;

    if changed.is_empty() {
        return Ok(());
    }

    for sender in server_senders {
        sender.send(changed.clone()).await?;
    }

    Ok(())
}
