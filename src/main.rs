#![feature(async_closure)]
#![warn(
    clippy::all,
    clippy::pedantic,
    // clippy::cargo, // TODO
    clippy::nursery,
    clippy::unimplemented,
)]
#![allow(clippy::similar_names)]

// #[macro_use] extern crate diesel;
// use diesel::prelude::*;
// use diesel::sqlite::SqliteConnection;

#[macro_use]
extern crate anyhow;

#[macro_use]
extern crate lazy_static;


pub mod errors;
use errors::MyErrorKind;

pub mod client;
pub mod db;
pub mod packages;
pub mod serde;

use client::{Client, Mode, State};

use anyhow::Context;

pub use crate::packages::*;

/*
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    net::{TcpListener, TcpStream},
    prelude::*,
    task,
    sync::mpsc,
};
*/

use async_std::{
    io::BufReader,
    net::{Ipv4Addr, TcpListener, TcpStream},
    prelude::*,
    task,
};

use std::{
    cell::RefCell,
    mem,
    net::IpAddr,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use futures::{
    channel::{mpsc, oneshot},
    future::{join_all, select_all, FutureExt},
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
// const DB_FILE: &str = "./database";
//TODO: use config

#[async_std::main]
async fn main() {
    let (background_task_handles, stop_background_tasks) = start_background_tasks();
    println!("started background tasks");

    let (stop_accept_loop, stopped_accept_loop) = oneshot::channel::<()>();
    let stop_accept_loop = RefCell::new(Some(stop_accept_loop));
    ctrlc::set_handler(move || {
        stop_accept_loop.replace(None).unwrap().send(()).unwrap();
    })
    .expect("Error setting Ctrl-C handler");

    let (client_watchdog, watchdog_sender) = start_client_watchdog();

    let mut stopped_accept_loop = stopped_accept_loop.fuse();

    let listener = TcpListener::bind(LISTEN_ADDR).await.unwrap();
    println!("listening for connections on {}", LISTEN_ADDR);
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
    println!("accept loop has ended; shutting down");

    println!("stopping background tasks");
    for stop_background_task in stop_background_tasks {
        if stop_background_task.send(()).is_err() {
            println!("Failed to stop a background task. It probably paniced!");
        }
    }
    futures::future::join_all(background_task_handles).await;
    println!("done");

    println!("waiting for children to die");
    drop(watchdog_sender);
    client_watchdog.await; // .expect("Failed to wait for children to die")
    println!("done");

    println!("syncing database to disk");
    sync_db_to_disk().await;
    println!("done");

    println!("exiting");
}

fn start_client_watchdog() -> (task::JoinHandle<()>, mpsc::Sender<task::JoinHandle<()>>) {
    let (watchdog_sender, mut watchdog_receiver) = mpsc::channel::<task::JoinHandle<()>>(1);

    let client_watchdog: task::JoinHandle<()> = task::spawn(async move {
        let mut clients: Vec<task::JoinHandle<()>> = Vec::new();
        let mut done = false;

        while !(done && clients.is_empty()) {
            if done {
                println!("[watchdog] we're shutting down, but there are clients to wait for");
            } else if clients.is_empty() {
                println!("[watchdog] we're running, but are not waiting for any clients");
            } else {
                println!("[watchdog] we're running, and there are still clients left");
            }


            if clients.is_empty() {
                if let Some(client_handle) = watchdog_receiver.next().await {
                    println!("[watchdog] Got a new client");
                    clients.push(client_handle);
                } else {
                    println!("[watchdog] No new clients can be recieved");
                    println!("[watchdog] shutting down");
                    // done = true;
                    break;
                }
            } else {
                let mut wait_for_clients = select_all(clients.drain(..)).fuse();

                'inner: loop {
                    let recv_client = async {
                        if done {
                            futures::future::pending().await
                        } else {
                            watchdog_receiver.next().await
                        }
                    };

                    select! {
                        (res, _, mut rest) = wait_for_clients => {
                            println!("[watchdog] a client we were waiting for finished");
                            // println!("[watchdog] res: {:?}", res);

                            clients.append(&mut rest);
                            break 'inner;
                        },
                        res = recv_client.fuse() => {
                            if let Some(client_handle) = res {
                                println!("[watchdog] Got a new client");
                                clients.push(client_handle);
                            } else {
                                println!("[watchdog] No new clients can be recieved");
                                println!("[watchdog] shutting down");
                                done = true;
                            }
                        },
                    }
                }
            }
        }

        println!("[watchdog] done");
    });

    (client_watchdog, watchdog_sender)
}

// TODO: rename
fn start_background_tasks() -> (Vec<task::JoinHandle<()>>, Vec<oneshot::Sender<()>>) {
    let mut join_handles = Vec::new();
    let mut senders = Vec::new();

    let (sender, receiver) = oneshot::channel();
    senders.push(sender);
    join_handles.push(task::spawn(async move {
        println!("starting `prune_old_queue_entries` background task");
        let mut exit = receiver.fuse();
        loop {
            println!("[UNIMPLEMENTED]: prune_old_queue_entries");
            // TODO
            // prune_old_queue_entries().await.expect("failed to prune old queue entries");

            #[allow(clippy::mut_mut, clippy::unnecessary_mut_passed)]
            {
                select! {
                    _ = exit => break,
                    _ = task::sleep(Duration::new(60 * 60 * 24 * 7, 0)).fuse() => continue,
                }
            }
        }
        println!("stopped `prune_old_queue_entries` background task");
    }));

    let (sender, receiver) = oneshot::channel();
    senders.push(sender);
    join_handles.push(task::spawn(async move {
        println!("starting `full_query` background task");
        let mut exit = receiver.fuse();
        loop {
            println!("[UNIMPLEMENTED]: full_query");
            // TODO
            // full_query().await.expect("failed to perform full query");

            #[allow(clippy::mut_mut, clippy::unnecessary_mut_passed)]
            {
                select! {
                    _ = exit => break,
                    _ = task::sleep(Duration::new(60 * 60 * 24, 0)).fuse() => continue,
                }
            }
        }
        println!("stopped `full_query` background task");
    }));

    let (sender, receiver) = oneshot::channel();
    senders.push(sender);
    join_handles.push(task::spawn(async move {
        println!("starting `send_queue` background task");
        let mut exit = receiver.fuse();
        loop {
            println!("[UNIMPLEMENTED]: send_queue");
            // TODO
            // send_queue().await.expect("failed to send queue");

            #[allow(clippy::mut_mut, clippy::unnecessary_mut_passed)]
            {
                select! {
                    _ = exit => break,
                    _ = task::sleep(Duration::new(30, 0)).fuse() => continue,
                }
            }
        }
        println!("stopped `send_queue` background task");
    }));

    (join_handles, senders)
}

// fn setup_socket(socket: &TcpStream) {}

async fn connect_to_server(server_uid: Uid) -> Client {
    let addr = get_server_address_for_uid(server_uid);

    let socket = TcpStream::connect(addr)
        .await
        .expect("Failed to connect to client"); // TODO: propagate error

    // setup_socket(&socket);

    Client::new(socket)
}

fn start_handling_client(client: Client) -> task::JoinHandle<()> {
    task::spawn(async {
        if let Err(error) = handle_connection(client).await {
            println!("error: {}", error);
        }

        println!("connection closed");
    })
}

async fn handle_connection(mut client: Client) -> anyhow::Result<()> {
    println!("new connection: {}", client.socket.peer_addr().unwrap());

    #[allow(clippy::mut_mut, clippy::unnecessary_mut_passed)]
    {
        select! {
            _ = task::sleep(*CLIENT_TIMEOUT).fuse() => {
                Err(MyErrorKind::Timeout)?;
            }
            res = peek_client_type(&mut client).fuse() => {
                res?;
            },
        }
    }

    debug_assert_ne!(client.mode, Mode::Unknown);

    println!("client mode: {:?}", client.mode);

    while client.state != State::Shutdown {
        #[allow(clippy::mut_mut, clippy::unnecessary_mut_passed)]
        {
            {
                select! {
                    _ = task::sleep(*CLIENT_TIMEOUT).fuse() => {
                        Err(MyErrorKind::Timeout)?;
                    }
                    res = consume_package(&mut client).fuse() => {
                        res?;
                        continue;
                    },
                }
            }
        }
    }

    Ok(())
}

async fn peek_client_type(client: &mut Client) -> anyhow::Result<()> {
    assert_eq!(client.mode, Mode::Unknown);

    let mut buf = [0_u8; 1];
    let len = client
        .socket
        .peek(&mut buf)
        .await
        .context(MyErrorKind::ConnectionCloseUnexpected)?; // read the first byte
    if len == 0 {
        bail!(MyErrorKind::ConnectionCloseUnexpected);
    }

    let [first_byte] = buf;

    println!("first byte: {:#04x}", first_byte);

    client.mode = if first_byte >= 32 && first_byte <= 126 {
        Mode::Ascii
    } else {
        Mode::Binary
    };

    Ok(())
}

async fn consume_package(client: &mut Client) -> anyhow::Result<()> {
    assert_ne!(client.mode, Mode::Unknown);

    if client.mode == Mode::Binary {
        consume_package_binary(client).await
    } else {
        consume_package_ascii(client).await
    }
}

async fn consume_package_ascii(client: &mut Client) -> anyhow::Result<()> {
    let mut lines = BufReader::new(&mut client.socket).lines();

    let line = lines
        .next()
        .await
        .context(MyErrorKind::UserInputError)?
        .context(MyErrorKind::ConnectionCloseUnexpected)?;

    println!("full line: {}", line);

    if line.is_empty() {
        bail!(MyErrorKind::UserInputError);
    }

    if line.chars().nth(0).unwrap() == 'q' {
        let mut number = String::new();

        for character in line.chars().skip(1) {
            let char_code = character as u8;
            if char_code < 48 || char_code > 57 {
                break; // number is over
            }
            number.push(character);
        }

        println!("handling 'q' request");

        let number = number
            .as_str()
            .parse::<u32>()
            .context(MyErrorKind::UserInputError)?;

        println!("parsed number: '{}'", number);

        let entry = get_entry_by_number(number, true);

        let message = if let Some(entry) = entry {
            let host_or_ip = if let Some(hostname) = entry.hostname {
                hostname
            } else {
                let ipaddress = entry
                    .ipaddress
                    .expect("database is incosistent: entry has neither hostname nor ipaddress");

                format!("{}", Ipv4Addr::from(ipaddress))
            };

            format!(
                "ok\r\n{}\r\n{}\r\n{}\r\n{}\r\n{}\r\n{}\r\n+++\r\n",
                entry.number,
                entry.name,
                entry.client_type,
                host_or_ip,
                entry.port,
                entry.extension // TODO: use weird conversion?
            )
        } else {
            format!("fail\r\n{}\r\nunknown\r\n+++\r\n", number)
        };

        client
            .socket
            .write(message.as_bytes())
            .await
            .context(MyErrorKind::FailedToWrite)?;
    } else {
        bail!(MyErrorKind::UserInputError);
    }

    client.shutdown()?;

    Ok(())
}

async fn consume_package_binary(client: &mut Client) -> anyhow::Result<()> {
    let mut header = [0_u8; 2];
    client
        .socket
        .read_exact(&mut header)
        .await
        .context(MyErrorKind::ConnectionCloseUnexpected)?;

    println!("header: {:?}", header);

    let [package_type, package_length] = header;


    let package: Package = match package_type {
        0x01 => RawPackage::Type1(unsafe {
            if package_length != LENGTH_TYPE_1 as u8 {
                bail!(MyErrorKind::ParseFailure(package_type));
            }
            let mut content = [0_u8; LENGTH_TYPE_1];
            read_to_mut_slice(client, &mut content).await?;

            mem::transmute(content)
        }),
        0x02 => RawPackage::Type2(unsafe {
            if package_length != LENGTH_TYPE_2 as u8 {
                bail!(MyErrorKind::ParseFailure(package_type));
            }
            let mut content = [0_u8; LENGTH_TYPE_2];
            read_to_mut_slice(client, &mut content).await?;

            mem::transmute(content)
        }),
        0x03 => RawPackage::Type3(unsafe {
            if package_length != LENGTH_TYPE_3 as u8 {
                bail!(MyErrorKind::ParseFailure(package_type));
            }
            let mut content = [0_u8; LENGTH_TYPE_3];
            read_to_mut_slice(client, &mut content).await?;

            mem::transmute(content)
        }),
        0x04 => RawPackage::Type4(unsafe {
            if package_length != LENGTH_TYPE_4 as u8 {
                bail!(MyErrorKind::ParseFailure(package_type));
            }
            let mut content = [0_u8; LENGTH_TYPE_4];
            read_to_mut_slice(client, &mut content).await?;

            mem::transmute(content)
        }),
        0x05 => RawPackage::Type5(unsafe {
            if package_length != LENGTH_TYPE_5 as u8 {
                bail!(MyErrorKind::ParseFailure(package_type));
            }
            let mut content = [0_u8; LENGTH_TYPE_5];
            read_to_mut_slice(client, &mut content).await?;

            mem::transmute(content)
        }),
        0x06 => RawPackage::Type6(unsafe {
            if package_length != LENGTH_TYPE_6 as u8 {
                bail!(MyErrorKind::ParseFailure(package_type));
            }
            let mut content = [0_u8; LENGTH_TYPE_6];
            read_to_mut_slice(client, &mut content).await?;

            mem::transmute(content)
        }),
        0x07 => RawPackage::Type7(unsafe {
            if package_length != LENGTH_TYPE_7 as u8 {
                bail!(MyErrorKind::ParseFailure(package_type));
            }
            let mut content = [0_u8; LENGTH_TYPE_7];
            read_to_mut_slice(client, &mut content).await?;

            mem::transmute(content)
        }),
        0x08 => RawPackage::Type8(unsafe {
            if package_length != LENGTH_TYPE_8 as u8 {
                bail!(MyErrorKind::ParseFailure(package_type));
            }
            let mut content = [0_u8; LENGTH_TYPE_8];
            read_to_mut_slice(client, &mut content).await?;

            mem::transmute(content)
        }),
        0x09 => RawPackage::Type9(unsafe {
            if package_length != LENGTH_TYPE_9 as u8 {
                bail!(MyErrorKind::ParseFailure(package_type));
            }
            let mut content = [0_u8; LENGTH_TYPE_9];
            read_to_mut_slice(client, &mut content).await?;

            mem::transmute(content)
        }),
        0x0A => RawPackage::Type10(unsafe {
            if package_length != LENGTH_TYPE_10 as u8 {
                bail!(MyErrorKind::ParseFailure(package_type));
            }
            let mut content = [0_u8; LENGTH_TYPE_10];

            read_to_mut_slice(client, &mut content).await?;

            mem::transmute(content)
        }),
        0xFF => RawPackage::Type255({
            let mut content = Vec::with_capacity(package_length as usize);
            content.resize(package_length as usize, 0);

            read_to_mut_slice(client, &mut content).await?;

            println!("content: {:?}", content);

            RawPackage255 { message: content }
        }),

        _ => bail!(MyErrorKind::ParseFailure(package_type)),
    }.into();

    println!(
        "got package of type: {} with length: {}",
        package_type, package_length
    );

    println!("parsed package: {:#?}", package);
    handle_package(client, package).await?;

    Ok(())
}

async fn read_to_mut_slice(client: &mut Client, slice: &mut [u8]) -> anyhow::Result<()> {
    client
        .socket
        .read_exact(slice)
        .await
        .context(MyErrorKind::ConnectionCloseUnexpected)
}

async fn handle_package(client: &mut Client, package: Package) -> anyhow::Result<()> {
    println!("state: '{:?}'", client.state);
    match package {
        Package::Type1(package) => {
            if client.state != State::Idle {
                bail!(MyErrorKind::InvalidState(State::Idle, client.state));
            }

            let peer_addr = client.socket.peer_addr().unwrap();

            let ipaddress = if let IpAddr::V4(ipaddress) = peer_addr.ip() {
                Ok(ipaddress)
            } else {
                Err(MyErrorKind::UserInputError)
            }?;

            let entry = get_entry_by_number(package.number, false);

            if let Some(entry) = entry {
                if entry.client_type == 0 {
                    register_entry(
                        package.number,
                        package.pin,
                        package.port,
                        u32::from(ipaddress),
                        true,
                    )?
                    .expect("Failed to register entry"); // TODO: handle properly
                } else if package.pin == entry.pin {
                    update_entry_address(package.port, u32::from(ipaddress), package.number)?
                        .expect("Failed to update entry address"); // TODO: handle properly
                } else {
                    bail!(MyErrorKind::UserInputError);
                }
            } else {
                register_entry(
                    package.number,
                    package.pin,
                    package.port,
                    u32::from(ipaddress),
                    false,
                )?
                .expect("Failed to register entry"); // TODO: handle properly
            };

            client
                .send_package(Package::Type2(ProcessedPackage2 { ipaddress }))
                .await?;

            Ok(())
        }
        // Package::Type2(package) => {}
        Package::Type3(package) => {
            if client.state != State::Idle {
                bail!(MyErrorKind::InvalidState(State::Idle, client.state));
            }

            let entry = get_entry_by_number(package.number, true);

            if let Some(entry) = entry {
                client.send_package(Package::Type5(entry.into())).await?;
            } else {
                client
                    .send_package(Package::Type4(ProcessedPackage4 {}))
                    .await?;
            }

            Ok(())
        }
        // Package::Type4(_package) => {}
        Package::Type5(package) => {
            if client.state != State::Accepting {
                bail!(MyErrorKind::InvalidState(State::Accepting, client.state));
            }

            upsert_entry(
                package.number,
                package.name,
                package.client_type,
                package.hostname,
                package.ipaddress,
                package.port,
                package.extension,
                package.pin,
                package.disabled,
                package.timestamp,
            )?
            .expect("Failed to sync entry"); // TODO: handle properly
            client
                .send_package(Package::Type8(ProcessedPackage8 {}))
                .await?;

            Ok(())
        }
        Package::Type6(package) => {
            if package.version != 1 {
                bail!(MyErrorKind::UserInputError);
            }
            if package.server_pin != SERVER_PIN {
                bail!(MyErrorKind::UserInputError);
            }
            if client.state != State::Idle {
                bail!(MyErrorKind::InvalidState(State::Idle, client.state));
            }

            client.state = State::Responding;

            client.push_entries_to_send_queue(get_all_entries().await);

            client.send_queue_entry().await?;

            Ok(())
        }
        Package::Type7(package) => {
            if package.version != 1 {
                bail!(MyErrorKind::UserInputError);
            }
            if package.server_pin != SERVER_PIN {
                bail!(MyErrorKind::UserInputError);
            }
            if client.state != State::Idle {
                bail!(MyErrorKind::InvalidState(State::Idle, client.state));
            }

            client.state = State::Accepting;

            client
                .send_package(Package::Type8(ProcessedPackage8 {}))
                .await?;

            Ok(())
        }
        Package::Type8(_package) => {
            if client.state != State::Responding {
                bail!(MyErrorKind::InvalidState(State::Responding, client.state));
            }

            client.send_queue_entry().await?;

            Ok(())
        }
        Package::Type9(_package) => {
            if client.state != State::Accepting {
                bail!(MyErrorKind::InvalidState(State::Accepting, client.state));
            }

            client.shutdown()?;

            Ok(())
        }
        Package::Type10(package) => {
            if package.version != 1 {
                bail!(MyErrorKind::UserInputError);
            }
            if client.state != State::Idle {
                bail!(MyErrorKind::InvalidState(State::Idle, client.state));
            }

            let entries = get_public_entries_by_pattern(&package.pattern);

            client.state = State::Responding;

            client.push_entries_to_send_queue(entries);

            client.send_queue_entry().await?;

            Ok(())
        }
        Package::Type255(package) => Err(anyhow!("remote error: {:?}", package.message)),

        _ => Err(MyErrorKind::UserInputError.into()),
    }
}

async fn full_query_for_server(server_uid: Uid) {
    let mut client = connect_to_server(server_uid).await;

    client.state = State::Accepting;

    client
        .send_package(Package::Type7(ProcessedPackage7 {
            version: 1,
            server_pin: SERVER_PIN,
        }))
        .await
        .unwrap();

    start_handling_client(client);
}

async fn send_queue_for_server(server_uid: Uid) {
    let mut client = connect_to_server(server_uid).await;

    client.state = State::Responding;

    client.push_to_send_queue(get_queue_for_server(server_uid));

    client
        .send_package(Package::Type6(ProcessedPackage6 {
            version: 1,
            server_pin: SERVER_PIN,
        }))
        .await
        .unwrap();

    start_handling_client(client);
}

async fn full_query() -> anyhow::Result<()> {
    let servers = get_server_uids().await;

    let mut full_queries = Vec::new();
    for server in servers {
        full_queries.push(full_query_for_server(server));
    }

    futures::future::join_all(full_queries).await;

    Ok(()) //TODO
}

async fn send_queue() -> anyhow::Result<()> {
    update_queue().await?;

    let servers = get_server_uids().await;

    let server_interactions = servers.iter().map(|&server| send_queue_for_server(server));

    join_all(server_interactions).await;

    Ok(()) //TODO
}
