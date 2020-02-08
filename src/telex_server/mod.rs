#[macro_use]
pub mod errors;
pub mod background_tasks;
pub mod client;

const PEER_SEARCH_VERSION: u8 = 1;
const FULL_QUERY_VERSION: u8 = 1;
const LOGIN_VERSION: u8 = 1;
// 1/1/1900 =  1/1/1970 - 70 Years + 17 Days
// (for the 17 Leap Years in between these dates)
pub static ITELEX_EPOCH: Lazy<SystemTime> =
    Lazy::new(|| UNIX_EPOCH - Duration::from_secs(60 * 60 * 24 * (365 * 70 + 17)));

#[allow(clippy::cast_possible_truncation)]
pub fn get_current_itelex_timestamp() -> u32 {
    SystemTime::now().duration_since(*ITELEX_EPOCH).unwrap().as_secs() as u32
}

use super::*;
use background_tasks::start_background_tasks;

pub fn init(stop_server: oneshot::Receiver<()>) -> ResultJoinHandle {
    task::spawn(
        // #[allow(unreachable_code)] // TODO
        async move {
            // bail!(err_unimplemented!()); // TODO

            if config!(SERVER_PIN) == 0 {
                warn!(
                    "The server is running without a SERVER_PIN. Server interaction will be \
                     reduced to publicly available levels. DB sync will be disabled so that no \
                     private state is overwritten."
                );
            }

            let (background_task_handles, stop_background_tasks) = start_background_tasks();

            info!("starting acccept loop");

            if let Err(err) = listen_for_connections(stop_server).await {
                error!("{:?}", anyhow!(err).context("Failed to await accept loop"));
            }

            warn!("shutting down itelex server");

            warn!("stopping background tasks");
            for stop_background_task in stop_background_tasks {
                if stop_background_task.send(()).is_err() {
                    error!("Failed to stop a background task.");
                }
            }
            futures::future::join_all(background_task_handles).await;

            Ok(())
        },
    )
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

async fn listen_for_connections(stop_loop: oneshot::Receiver<()>) -> anyhow::Result<()> {
    use std::net::{Ipv4Addr, Ipv6Addr};

    let mut ipv4_listener =
        TcpListener::bind((Ipv4Addr::UNSPECIFIED, config!(SERVER_PORT))).await?;
    let ipv6_listener = TcpListener::bind((Ipv6Addr::UNSPECIFIED, config!(SERVER_PORT))).await.ok();

    let mut stop_loop = stop_loop.fuse();

    info!("listening for connections on port {}", config!(SERVER_PORT));

    if let Some(mut ipv6_listener) = ipv6_listener {
        loop {
            select! {
                res = ipv4_listener.accept().fuse() => register_client(res).await,
                res = ipv6_listener.accept().fuse() => register_client(res).await,

                _ = stop_loop => break,
            }
        }
    } else {
        loop {
            select! {
                res = ipv4_listener.accept().fuse() => register_client(res).await,

                _ = stop_loop => break,
            }
        }
    }

    info!("accept loop has ended");

    Ok(())
}

async fn handle_client_result(
    result: anyhow::Result<()>,
    client: &mut Client,
) -> anyhow::Result<()> {
    let addr = client.address;

    if let Err(error) = result.as_ref() {
        let message = format!("fail\r\n-\r\nerror: {}\r\n+++\r\n", error);

        if client.mode == Mode::Binary {
            let _ = client.send_package(Package::Error(Error { message })).await;
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
    trace!("starting to handle client");

    let task_id = {
        let mut task_id_counter = TASK_ID_COUNTER.lock().await;

        let mut task_id = *task_id_counter;
        while TASKS.contains_key(&task_id) {
            task_id = task_id.wrapping_add(1);
            info!("task id was already taken. Next id: {}", task_id);
        }

        *task_id_counter = task_id + 1;

        task_id
    };

    let task = task::spawn(async move {
        let res = handle_client_result(client.handle().await, &mut client).await;

        TASKS.remove(&task_id);
        info!("removed task {}", task_id);

        res
    });

    trace!("added task {}", task_id);
    TASKS.insert(task_id, task);

    task_id
}
