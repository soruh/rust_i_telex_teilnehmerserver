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
pub mod telex_server;
pub mod config;
pub mod db;
pub mod web_server;

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
use telex_server::*;

// types
pub type VoidJoinHandle = task::JoinHandle<()>;
pub type ResultJoinHandle = task::JoinHandle<anyhow::Result<()>>;
pub type TaskId = usize;
pub type Entry = packages::Package5;
pub type Entries = Vec<Entry>;

// global state
pub static CHANGED: Lazy<RwLock<HashMap<u32, ()>>> = Lazy::new(|| RwLock::new(HashMap::new()));
pub static DATABASE: Lazy<RwLock<HashMap<u32, Entry>>> = Lazy::new(|| RwLock::new(HashMap::new()));
pub static CONFIG: OnceCell<Config> = OnceCell::new();
pub static TASKS: Lazy<Mutex<HashMap<TaskId, ResultJoinHandle>>> = Lazy::new(|| Mutex::new(HashMap::new()));
pub static TASK_ID_COUNTER: Lazy<Mutex<TaskId>> = Lazy::new(|| Mutex::new(0));

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

    if let Err(err) = read_db_from_disk().await {
        let err = err.context("Failed to restore DB from disk");
        error!("{:?}", err);
        error!("repair or delete {:?}.", config!(DB_PATH));
        bail!(err);
    }

    let (stop_itelex_server, stopped_itelex_server) = oneshot::channel();
    let itelex_server = telex_server::init(stopped_itelex_server);

    let (stop_web_server, stopped_web_server) = oneshot::channel();
    let web_server = web_server::init(stopped_web_server);

    if let Err(err) = register_exit_handler().await {
        // swait until we should exit
        error!("{:?}", anyhow!(err).context("Failed to register exit handler"));
    }

    let _ = stop_web_server.send(());
    if let Err(err) = web_server.await {
        error!("{:?}", anyhow!(err).context("web server failed"));
    }

    let _ = stop_itelex_server.send(());
    if let Err(err) = itelex_server.await {
        error!("{:?}", anyhow!(err).context("itelex server failed"));
    }

    warn!("waiting for all tasks to finish");
    let tasks: Vec<ResultJoinHandle> = TASKS.lock().await.drain().map(|(_, value)| value).collect();
    if !tasks.is_empty() {
        let _ = select_all(tasks).await;
    } else {
        debug!("there were no tasks to wait for");
    }

    sync_db_to_disk().await.expect("Failed to sync DB");

    warn!("exiting");

    Ok(())
}

async fn wait_for_task(task_id: usize) -> anyhow::Result<()> {
    debug!("waiting for task {}", task_id);
    let task = TASKS.lock().await.remove(&task_id).expect("spawned task is not stored in TASKS");

    task.await
}

fn init_logger() -> anyhow::Result<()> {
    use simplelog::{CombinedLogger, Config, LevelFilter, SharedLogger, TermLogger, TerminalMode, WriteLogger};
    use std::fs::File;

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

fn register_exit_handler() -> oneshot::Receiver<()> {
    use simple_signal::{set_handler, Signal};

    let (exit_signal_sender, exit_signal_receiver) = oneshot::channel::<()>();
    let exit_signal_sender = RefCell::new(Some(exit_signal_sender));

    set_handler(&[Signal::Int, Signal::Term], move |signals| {
        if let Some(exit_signal_sender) = exit_signal_sender.replace(None) {
            warn!("got first exit signal {:?}: attempting to shut down gracefully", signals);

            exit_signal_sender.send(()).unwrap();
        } else {
            error!("got second exit signal {:?}: aborting", signals);

            std::process::abort();
        }
    });

    exit_signal_receiver
}
