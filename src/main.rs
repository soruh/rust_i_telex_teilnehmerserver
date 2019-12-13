#![feature(async_closure)]

// #[macro_use] extern crate diesel;
// use diesel::prelude::*;
// use diesel::sqlite::SqliteConnection;

#[macro_use]
extern crate anyhow;

#[macro_use]
extern crate lazy_static;

extern crate dotenv;
extern crate nom;

pub mod errors;
use errors::MyErrorKind;

pub mod client;
pub mod db;
pub mod db_backend;
pub mod models;
pub mod packages;
pub mod serde;

use crate::models::*;

use client::{Client, Mode, State};

use anyhow::Context;

pub use crate::packages::*;
use serde::deserialize;

use futures::future::join_all;

use tokio::{
    io::{AsyncBufReadExt, BufReader},
    net::{TcpListener, TcpStream},
    prelude::*,
    task,
};

use std::{
    net::{IpAddr, SocketAddr},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use futures::future::FutureExt;

use std::sync::{Arc, Mutex};

use db::*;
use db_backend::Database;

lazy_static! {
    pub static ref ITELEX_EPOCH: SystemTime = UNIX_EPOCH
        .checked_sub(Duration::from_secs(60 * 60 * 24 * 365 * 70))
        .unwrap();
    pub static ref DIRECTORY: Arc<Mutex<Database<DirectoryEntry>>> = Arc::new(Mutex::new(Database::new(16)));
    pub static ref SERVERS: Arc<Mutex<Database<ServersEntry>>> = Arc::new(Mutex::new(Database::new(16)));
    pub static ref QUEUE: Arc<Mutex<Database<QueueEntry>>> = Arc::new(Mutex::new(Database::new(16)));
}

pub fn get_current_itelex_timestamp() -> u32 {
    SystemTime::now()
        .duration_since(*ITELEX_EPOCH)
        .unwrap()
        .as_secs() as u32
}

const SERVER_PIN: u32 = 42;
const DB_FOLDER: &str = "./database";
//TODO: use env / .env file

#[tokio::main]
async fn main() {

    {
        start_background_tasks();
        println!("started background tasks");
    }

    let mut listener = {
        // TODO: use config
        let listen_addr = SocketAddr::from(([0, 0, 0, 0], 11814));

        let listener = TcpListener::bind(listen_addr).await.unwrap();
        println!("listening for connections on {}", listen_addr);

        listener
    };

    let (stop_accept_loop, stopped_accept_loop) = tokio::sync::oneshot::channel::<()>();

    let stop_accept_loop = std::cell::RefCell::new(Some(stop_accept_loop));

    ctrlc::set_handler(move || {
        stop_accept_loop
            .replace(None)
            .unwrap()
            .send(())
            .unwrap();
    }).expect("Error setting Ctrl-C handler");

    let mut stopped_accept_loop = stopped_accept_loop.fuse();
    loop {
        futures::select! {
            res = listener.accept().fuse() => {
                let (socket, _) = res.expect("Failed to accept socket");

                setup_socket(&socket);

                let client = Client::new(socket);

                start_handling_client(client);
            },
            _ = stopped_accept_loop => break,
        }

    }

    println!("cleaning up");

    let directory = DIRECTORY.lock().unwrap();
    let servers = SERVERS.lock().unwrap();
    let queue = QUEUE.lock().unwrap();

    futures::join! (directory.close(), servers.close(), queue.close());

    println!("change da world");
    println!("my final message: goodbye");
}

// TODO: rename
fn start_background_tasks() {
    task::spawn(async {
        loop {
            println!("calling `prune_old_queue_entries`");
            prune_old_queue_entries().expect("failed to prune old queue entries"); //.await;

            tokio::time::delay_for(Duration::new(60 * 60 * 24 * 7, 0)).await;
        }
    });

    task::spawn(async {
        loop {
            println!("calling `full_query`");
            full_query().await.expect("failed to perform full query");

            tokio::time::delay_for(Duration::new(60 * 60 * 24, 0)).await;
        }
    });

    task::spawn(async {
        loop {
            println!("calling `send_queue`");
            send_queue().await.expect("failed to send queue");

            tokio::time::delay_for(Duration::new(30, 0)).await;
        }
    });
}

fn setup_socket(socket: &TcpStream) {
    // TODO: fix read timeout
    // socket.set_read_timeout(Some(Duration::new(30, 0))).unwrap(); // TODO: check if is this correct
}

async fn connect_to_server(server_uid: u32) -> Client {
    let addr = get_server_address_for_uid(server_uid);

    let socket = TcpStream::connect(addr)
        .await
        .expect("Failed to connect to client"); // TODO: propagate error

    setup_socket(&socket);

    Client::new(socket)
}

fn start_handling_client(client: Client) {
    task::spawn(async {
        if let Err(error) = handle_connection(client).await {
            println!("error: {}", error);
        }

        println!("connection closed");
    });
}

async fn handle_connection(mut client: Client) -> anyhow::Result<()> {
    println!("new connection: {}", client.socket.peer_addr().unwrap());

    peek_client_type(&mut client).await?;
    debug_assert_ne!(client.mode, Mode::Unknown);

    println!("client mode: {:?}", client.mode);

    while client.state != State::Shutdown {
        consume_package(&mut client).await?;
    }

    Ok(())
}

async fn peek_client_type(client: &mut Client) -> anyhow::Result<()> {
    assert_eq!(client.mode, Mode::Unknown);

    let mut buf = [0u8; 1];
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
        return consume_package_binary(client).await;
    } else {
        return consume_package_ascii(client).await;
    }
}

async fn consume_package_ascii(client: &mut Client) -> anyhow::Result<()> {
    let mut lines = BufReader::new(&mut client.socket).lines();

    let line = lines
        .next_line()
        .await?
        .context(MyErrorKind::UserInputError)?;

    println!("full line: {}", line);

    if line.len() == 0 {
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
                entry.connection_type,
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
    let mut header = [0u8; 2];
    client
        .socket
        .read_exact(&mut header)
        .await
        .context(MyErrorKind::ConnectionCloseUnexpected)?;

    println!("header: {:?}", header);

    let [package_type, package_length] = header;

    let mut body = vec![0u8; package_length as usize];
    client
        .socket
        .read_exact(&mut body)
        .await
        .context(MyErrorKind::ConnectionCloseUnexpected)?;

    println!(
        "got package of type: {} with length: {}",
        package_type, package_length
    );

    if body.len() > 0 {
        println!("body: {:?}", body);
    }

    let package = deserialize(package_type, &body)?;
    println!("parsed package: {:#?}", package);
    handle_package(client, package).await?;

    Ok(())
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
                if entry.connection_type == 0 {
                    register_entry(
                        package.number,
                        package.pin,
                        package.port,
                        u32::from(ipaddress),
                        true,
                    );
                } else if package.pin == entry.pin {
                    update_entry_address(package.port, u32::from(ipaddress), package.number);
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
                );
            };

            client
                .send_package(Package::Type2(PackageData2 { ipaddress }))
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
                client.send_package(Package::Type4(PackageData4 {})).await?;
            }

            Ok(())
        }
        // Package::Type4(_package) => {}
        Package::Type5(package) => {
            if client.state != State::Accepting {
                bail!(MyErrorKind::InvalidState(State::Accepting, client.state));
            }

            let new_entry: DirectoryEntry = package.into();

            upsert_entry(
                new_entry.number,
                new_entry.name,
                new_entry.connection_type,
                new_entry.hostname,
                new_entry.ipaddress,
                new_entry.port,
                new_entry.extension,
                new_entry.pin,
                new_entry.disabled,
                new_entry.timestamp,
            );
            client.send_package(Package::Type8(PackageData8 {})).await?;

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

            client.push_entries_to_send_queue(get_all_entries());

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

            client.send_package(Package::Type8(PackageData8 {})).await?;

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

            let entries = get_public_entries_by_pattern(package.pattern.to_str().unwrap());

            client.state = State::Responding;

            client.push_entries_to_send_queue(entries);

            client.send_queue_entry().await?;

            Ok(())
        }
        Package::Type255(package) => Err(anyhow!("remote error: {:?}", package.message.to_str()?)),

        _ => Err(MyErrorKind::UserInputError)?,
    }
}

async fn full_query_for_server(server_uid: u32) {
    let mut client = connect_to_server(server_uid).await;

    client.state = State::Accepting;

    client
        .send_package(Package::Type7(PackageData7 {
            version: 1,
            server_pin: SERVER_PIN,
        }))
        .await
        .unwrap();

    start_handling_client(client);
}

async fn send_queue_for_server(server_uid: u32) {
    let mut client = connect_to_server(server_uid).await;

    client.state = State::Responding;

    client.push_to_send_queue(get_queue_for_server(server_uid));

    client
        .send_package(Package::Type6(PackageData6 {
            version: 1,
            server_pin: SERVER_PIN,
        }))
        .await
        .unwrap();

    start_handling_client(client);
}

async fn full_query() -> anyhow::Result<()> {
    let servers = get_server_uids();

    for server in servers {
        full_query_for_server(server).await;
    }

    Ok(()) //TODO
}

async fn send_queue() -> anyhow::Result<()> {
    update_queue()?;

    let servers = get_server_uids();

    let server_interactions = servers.iter().map(|&server| send_queue_for_server(server));

    join_all(server_interactions).await;

    Ok(()) //TODO
}
