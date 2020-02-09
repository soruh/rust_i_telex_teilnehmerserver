#![warn(clippy::all, clippy::nursery)]
#![allow(clippy::unnecessary_mut_passed)] // TODO: remove

#[macro_use] extern crate anyhow;
#[macro_use] extern crate log;
extern crate serde;

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
use client::{Client, Mode, State};
use config::Config;
use dashmap::DashMap;
use db::*;
use futures::{
    channel::{mpsc, oneshot},
    future::{select_all, FutureExt},
    select,
    sink::SinkExt,
    stream::StreamExt,
};
pub use itelex::server::{self as packages, *};
use once_cell::sync::{Lazy, OnceCell};
use std::{
    cell::RefCell,
    net::SocketAddr,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use telex_server::*;
use tokio::{
    net::{TcpListener, TcpStream},
    prelude::*,
    sync::Mutex,
    task,
};

// types
pub type VoidJoinHandle = task::JoinHandle<()>;
pub type ResultJoinHandle = task::JoinHandle<anyhow::Result<()>>;
pub type TaskId = usize;
pub type Entry = Box<PeerReply>;
pub type UnboxedEntry = PeerReply;
pub type Entries = Vec<PeerReply>;

// global state
pub static CHANGED: Lazy<DashMap<u32, ()>> = Lazy::new(|| DashMap::new());
pub static DATABASE: Lazy<DashMap<u32, UnboxedEntry>> = Lazy::new(|| DashMap::new());
pub static CONFIG: OnceCell<Config> = OnceCell::new();
pub static TASKS: Lazy<DashMap<TaskId, ResultJoinHandle>> = Lazy::new(|| DashMap::new());
pub static TASK_ID_COUNTER: Lazy<Mutex<TaskId>> = Lazy::new(|| Mutex::new(0));

#[tokio::main]
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
    let (stop_web_server, stopped_web_server) = oneshot::channel();

    let _ = futures::join! {
        async {
            if let Err(err) = web_server::init(stopped_web_server).await {
                error!("{:?}", anyhow!(err).context("web server failed"));
            }
        },
        async {
            if let Err(err) = telex_server::init(stopped_itelex_server).await {
                error!("{:?}", anyhow!(err).context("itelex server failed"));
            }
        },
        async {
            if let Err(err) = register_exit_handler().await {
                error!("{:?}", anyhow!(err).context("Failed to register exit handler"));
            }

            let _ = stop_web_server.send(());
            let _ = stop_itelex_server.send(());
        }
    };

    warn!("waiting for all tasks to finish");

    // TODO: convert to `drain` once dashmap supports it
    let mut tasks: Vec<ResultJoinHandle> = Vec::with_capacity(TASKS.len());
    let task_ids: Vec<TaskId> = TASKS.iter().map(|item| item.key().clone()).collect();
    for task_id in task_ids {
        if let Some(task) = TASKS.remove(&task_id) {
            tasks.push(task.1);
        }
    }
    TASKS.clear();

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
    let (_, task) = TASKS.remove(&task_id).expect("spawned task is not stored in TASKS");

    task.await?
}

fn init_logger() -> anyhow::Result<()> {
    use simplelog::{
        CombinedLogger, Config, LevelFilter, SharedLogger, TermLogger, TerminalMode, WriteLogger,
    };
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
