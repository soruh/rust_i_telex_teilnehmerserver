#![warn(clippy::all, clippy::nursery)]
#![allow(clippy::unnecessary_mut_passed)] // TODO: remove

#[macro_use] extern crate anyhow;

#[macro_use] extern crate log;

macro_rules! config {
    ($key:ident) => {
        CONFIG.get().unwrap().$key
    };
}

#[macro_use]
pub mod errors;
pub mod client;
pub mod config;
pub mod db;
pub mod packages;
pub mod serde;

pub use errors::ItelexServerErrorKind;
pub use packages::*;

use anyhow::Context;
use async_std::{
    io::prelude::*,
    net::{TcpListener, TcpStream},
    sync::{Mutex, RwLock},
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

// types
pub type Packages = Vec<Package5>;
type VoidJoinHandle = task::JoinHandle<()>;
type ResultJoinHandle = task::JoinHandle<anyhow::Result<()>>;
type TaskId = usize;

// constants
const PEER_SEARCH_VERSION: u8 = 1;
const FULL_QUERY_VERSION: u8 = 1;
const LOGIN_VERSION: u8 = 1;
// pub static ITELEX_EPOCH: Lazy<SystemTime> = Lazy::new(|| UNIX_EPOCH -
// Duration::from_secs(60 * 60 * 24 * 365 * 70));
pub static ITELEX_EPOCH: Lazy<SystemTime> = Lazy::new(|| UNIX_EPOCH);

// global state
pub static CHANGED: Lazy<RwLock<HashMap<u32, ()>>> = Lazy::new(|| RwLock::new(HashMap::new()));
pub static DATABASE: Lazy<RwLock<HashMap<u32, Package5>>> = Lazy::new(|| RwLock::new(HashMap::new()));
pub static CONFIG: OnceCell<Config> = OnceCell::new();
pub static TASKS: Lazy<Mutex<HashMap<TaskId, ResultJoinHandle>>> = Lazy::new(|| Mutex::new(HashMap::new()));
pub static TASK_ID_COUNTER: Lazy<Mutex<TaskId>> = Lazy::new(|| Mutex::new(0));

fn init_logger() -> anyhow::Result<()> {
    let log_level_from_string = |level: &str| -> anyhow::Result<LevelFilter> {
        Ok(match level.to_lowercase().as_str() {
            "off" => LevelFilter::Off,
            "error" => LevelFilter::Error,
            "warn" => LevelFilter::Warn,
            "info" => LevelFilter::Info,

            // We don't compile the calls to these in release mode
            #[cfg(debug_assertions)]
            "debug" => LevelFilter::Debug,
            #[cfg(debug_assertions)]
            "trace" => LevelFilter::Trace,

            _ => bail!("invalid log level"),
        })
    };

    use simplelog::{CombinedLogger, Config, LevelFilter, SharedLogger, TermLogger, TerminalMode, WriteLogger};
    use std::fs::File;

    let mut loggers: Vec<Box<dyn SharedLogger>> = Vec::new();

    {
        let log_level = if let Some(log_level) = config!(LOG_LEVEL_TERM).as_ref() {
            log_level_from_string(log_level)?
        } else {
            #[cfg(debug_assertions)]
            let default_level = LevelFilter::Debug;

            #[cfg(not(debug_assertions))]
            let default_level = LevelFilter::Warn;

            default_level
        };

        loggers.push(
            TermLogger::new(log_level, Config::default(), TerminalMode::Mixed)
                .context("Failed to create terminal logger")?,
        );
    }

    if let Some(log_file_path) = config!(LOG_FILE_PATH).as_ref() {
        {
            let log_level = if let Some(log_level) = config!(LOG_LEVEL_FILE).as_ref() {
                log_level_from_string(log_level)?
            } else {
                LevelFilter::Info
            };

            loggers.push(WriteLogger::new(
                log_level,
                Config::default(),
                File::create(log_file_path).context("Failed to create file logger")?,
            ));
        }
    }

    CombinedLogger::init(loggers).context("Failed to initialize logger")?;

    Ok(())
}

#[async_std::main]
async fn main() -> anyhow::Result<()> {
    // simple_logger::init().expect("Failed to initialize logger");

    if let Err(err) = dotenv::dotenv() {
        if !err.not_found() {
            bail!(anyhow!(err).context("Failed to load configuration from `.env` file"));
        }
    }

    CONFIG.set(Config::from_env().await?).expect("Failed to set config");

    init_logger()?;

    debug!("using config: {:#?}", CONFIG.get().unwrap());

    if config!(SERVER_PIN) == 0 {
        warn!(
            "The server is running without a SERVER_PIN. Server interaction will be reduced to publicly available \
             levels. DB sync will be disabled so that no private state is overwritten."
        );
    }

    if let Err(err) = read_db_from_disk().await {
        let err = err.context("Failed to restore DB from disk");
        error!("{:?}", err);
        error!("repair or delete {:?}.", config!(DB_PATH));
        bail!(err);
    }

    let (background_task_handles, stop_background_tasks) = start_tasks();

    info!("starting acccept loop");

    match listen_for_connections(register_exit_handler()).await {
        Ok(accept_loop) => accept_loop.await, // Wait for accept loop to end
        Err(err) => error!("{:?}", anyhow!(err).context("Failed to start accept loop")),
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
    let tasks: Vec<ResultJoinHandle> = {
        let mut tasks = TASKS.lock().await;
        tasks.drain().map(|(_, value)| value).collect()
    };

    let _ = select_all(tasks).await;

    sync_db_to_disk().await.expect("Failed to sync DB");

    warn!("exiting");

    Ok(())
}

async fn register_client(listen_res: std::io::Result<(TcpStream, SocketAddr)>) {
    match listen_res {
        Ok((socket, addr)) => {
            debug!("new connection from {}", addr);

            start_handling_client(Client::new(socket, addr)).await;
        }
        Err(err) => error!("{:?}", anyhow!(err).context("Failed to accept a client")),
    }
}

async fn listen_for_connections(stop_the_loop: oneshot::Receiver<()>) -> anyhow::Result<VoidJoinHandle> {
    use std::net::{Ipv4Addr, Ipv6Addr};

    let ipv4_listener = TcpListener::bind((Ipv4Addr::UNSPECIFIED, config!(SERVER_PORT))).await?;
    let ipv6_listener = TcpListener::bind((Ipv6Addr::UNSPECIFIED, config!(SERVER_PORT))).await.ok();

    let mut stop_the_loop = stop_the_loop.fuse();

    Ok(task::spawn(async move {
        info!("listening for connections on port {}", config!(SERVER_PORT));

        if let Some(ipv6_listener) = ipv6_listener {
            loop {
                select! {
                    res = ipv4_listener.accept().fuse() => register_client(res).await,
                    res = ipv6_listener.accept().fuse() => register_client(res).await,

                    _ = stop_the_loop => break,
                }
            }
        } else {
            loop {
                select! {
                    res = ipv4_listener.accept().fuse() => register_client(res).await,

                    _ = stop_the_loop => break,
                }
            }
        }

        info!("accept loop has ended");
    }))
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

// TODO: refactor
fn start_tasks() -> (Vec<VoidJoinHandle>, Vec<oneshot::Sender<()>>) {
    info!("spawning background tasks");

    let mut join_handles = Vec::new();

    let mut abort_senders = Vec::new();

    let (server_join_handles, mut server_senders, server_abort_senders) =
        update_other_servers(config!(SERVERS).to_vec());

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
                error!("{:?}", anyhow!(err).context(format!("failed to run background task {}", name)));
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
                error!("{:?}", anyhow!(err).context(format!("failed to run background task {}", name)));
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
                error!("{:?}", anyhow!(err).context(format!("failed to run background task {}", name)));
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
        let message = format!("fail\r\n-\r\nerror: {}\r\n+++\r\n", error);

        if client.mode == Mode::Binary {
            let _ = client.send_package(Package::Type255(Package255 { message })).await;
        } else if client.mode == Mode::Ascii {
            let _ = client.socket.write_all(message.as_bytes()).await;
        }
    } else {
        info!("client at {} finished", addr);
    }

    if let Err(error) = client.shutdown() {
        debug!("{:?}", anyhow!(error).context(format!("Failed to shut down client at {}", addr)));
    }

    let result = result.context("client error");

    if let Err(err) = result.as_ref() {
        warn!("{:?}", err);
    }

    result
}

async fn start_handling_client(mut client: Client) -> TaskId {
    debug!("starting to handle client");
    let mut tasks = TASKS.lock().await;

    let task_id = {
        let mut task_id_counter = TASK_ID_COUNTER.lock().await;

        let mut task_id = *task_id_counter;
        while tasks.get(&task_id).is_some() {
            task_id = task_id.wrapping_add(1);
            debug!("next id: {}", task_id);
        }

        *task_id_counter = task_id + 1;

        task_id
    };

    let task = task::spawn(async move {
        let res = handle_client_result(client.handle().await, &mut client).await;

        TASKS.lock().await.remove(&task_id);
        info!("removed task {}", task_id);

        res
    });

    info!("added task {}", task_id);
    tasks.insert(task_id, task);

    task_id
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

    wait_for_task(start_handling_client(client).await).await?;

    warn!("finished full query for server {}", server);

    Ok(())
}

async fn full_query() -> anyhow::Result<()> {
    let mut full_queries = Vec::new();

    info!("starting full query");

    for server in config!(SERVERS).iter() {
        full_queries.push(full_query_for_server(*server));
    }

    for result in futures::future::join_all(full_queries).await {
        if let Err(err) = result {
            error!("{:?}", anyhow!(err).context("A full query failed"));
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

async fn update_server_with_packages(server: SocketAddr, packages: Packages) -> anyhow::Result<()> {
    if config!(SERVER_PIN) == 0 {
        bail!(anyhow!("Not updating other servers without a server pin"));
    }

    let mut client = connect_to(server).await?;

    client.send_queue.extend(packages.into_iter());

    client.state = State::Responding;

    client.send_package(Package::Type7(Package7 { server_pin: config!(SERVER_PIN), version: LOGIN_VERSION })).await?;

    let task_id = start_handling_client(client).await;

    wait_for_task(task_id).await
}

async fn wait_for_task(task_id: usize) -> anyhow::Result<()> {
    debug!("waiting for task {}", task_id);
    let task = TASKS.lock().await.remove(&task_id).expect("spawned task is not stored in TASKS");

    task.await
}

fn update_other_servers(
    servers: Vec<SocketAddr>,
) -> (Vec<VoidJoinHandle>, Vec<mpsc::UnboundedSender<Packages>>, Vec<oneshot::Sender<()>>) {
    let mut join_handles = Vec::new();

    let mut senders = Vec::new();

    let mut abort_senders = Vec::new();

    for server in servers {
        let (abort_sender, abort_receiver) = oneshot::channel::<()>();

        abort_senders.push(abort_sender);

        let (sender, mut receiver) = mpsc::unbounded::<Packages>();

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
                    error!("{:?}", anyhow!(err).context(format!("Failed to update server {}", server)));

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

async fn sync_changed(server_senders: &mut Vec<mpsc::UnboundedSender<Packages>>) -> anyhow::Result<()> {
    let changed = get_changed_entries().await;

    if changed.is_empty() {
        return Ok(());
    }

    for sender in server_senders {
        sender.send(changed.clone()).await?;
    }

    Ok(())
}
