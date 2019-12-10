#![feature(async_closure)]

extern crate dotenv;
extern crate nom;

#[macro_use]
extern crate rusqlite;

#[macro_use]
extern crate anyhow;

use rusqlite::OpenFlags;

#[macro_use]
extern crate lazy_static;

pub mod errors;
use errors::*;

pub mod models;

pub mod db;
pub mod packages;
pub mod serde;

use anyhow::Context;
use dotenv::dotenv;

pub use crate::packages::*;
use serde::{deserialize, serialize};

use std::time::Duration;

use futures::future::join_all;

use async_std::{
    net::{IpAddr, SocketAddr, TcpListener, TcpStream},
    prelude::*,
    task,
};

use db::*;

use crate::models::*;

const SERVER_PIN: u32 = 42;
const DB_PATH: &str = "./database.db";
//TODO: use env / .env file

#[derive(Debug, PartialEq, Eq)]
pub enum Mode {
    Ascii,
    Binary,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Idle,
    Responding,
    Accepting,
    Shutdown,
}

pub struct Client {
    pub socket: TcpStream,

    mode: Mode,

    db_con: rusqlite::Connection,

    state: State,
    send_queue: Vec<(DirectoryEntry, Option<u32>)>,
}

fn open_db_connection() -> rusqlite::Connection {
    let db_open_flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX;

    rusqlite::Connection::open_with_flags(DB_PATH, db_open_flags).expect("failed to open database")
}

impl Client {
    fn new(socket: TcpStream, db_con: rusqlite::Connection) -> Self {
        Client {
            socket,

            mode: Mode::Unknown,

            db_con,

            state: State::Idle,
            send_queue: Vec::new(),
        }
    }

    async fn send_package(&mut self, package: Package) -> anyhow::Result<()> {
        println!("sending package: {:#?}", package);
        self.socket
            .write(serialize(package).as_slice())
            .await
            .context(MyErrorKind::FailedToWrite)?;

        Ok(())
    }

    fn shutdown(&mut self) -> std::result::Result<(), std::io::Error> {
        self.state = State::Shutdown;
        self.socket.shutdown(std::net::Shutdown::Both)
    }

    fn push_to_send_queue(&mut self, list: Vec<(DirectoryEntry, Option<u32>)>) {
        self.send_queue.extend(list);
    }

    fn push_entries_to_send_queue(&mut self, list: Vec<DirectoryEntry>) {
        self.send_queue.reserve(list.len());

        for entry in list {
            self.send_queue.push((entry, None));
        }
    }

    async fn send_queue_entry(&mut self) -> anyhow::Result<()> {
        if self.state != State::Responding {
            bail!(MyErrorKind::InvalidState(State::Responding, self.state));
        }

        let len = self.send_queue.len();

        println!(
            "entries left in queue: {} -> {}",
            len,
            if len == 0 { 0 } else { len - 1 }
        );

        if let Some(entry) = self.send_queue.pop() {
            let (package, queue_uid) = entry;

            self.send_package(Package::Type5(PackageData5::from(package)))
                .await?;

            if let Some(queue_uid) = queue_uid {
                remove_queue_entry(&self.db_con, queue_uid);
            }

            Ok(())
        } else {
            self.send_package(Package::Type9(PackageData9 {})).await?;

            Ok(())
        }
    }
}

fn main() {
    task::block_on(async_main());
}
async fn async_main() {
    dotenv().ok();

    // std::env::var("DATABASE_PATH").expect("failed to read 'DATABASE_PATH' from environment");s

    {
        let initial_db_open_flags =
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE;

        let initial_db_con =
            rusqlite::Connection::open_with_flags(DB_PATH, initial_db_open_flags).unwrap();

        create_tables(&initial_db_con);

        initial_db_con
            .close()
            .expect("failed to close initial database connection");
    }

    println!("database is initialized");

    start_server_sync();

    println!("started server sync thread");

    let addr = SocketAddr::from(([0, 0, 0, 0], 11814));

    let listener = TcpListener::bind(addr).await.unwrap();
    // TODO: use config
    println!("listening for connections on {}", addr);

    let mut incoming = listener.incoming();

    while let Some(socket) = incoming.next().await {
        let socket = socket.unwrap();

        setup_socket(&socket);

        let db_conn = open_db_connection();
        let client = Client::new(socket, db_conn);

        start_handle_loop(client);
    }
}

// TODO: rename
fn start_server_sync() {
    task::spawn((async || {
        // let db_conn = open_db_connection();
        loop {
            println!("calling `prune_old_queue_entries`");
            // prune_old_queue_entries(&db_conn); //.await;
            task::sleep(Duration::new(60 * 60 * 24 * 7, 0)).await;
        }
    })());

    task::spawn((async || {
        // let db_conn = open_db_connection();
        loop {
            println!("calling `full_query`");
            // full_query(&db_conn).await;
            task::sleep(Duration::new(60 * 60 * 24, 0)).await;
        }
    })());

    task::spawn((async || {
        // let db_conn = open_db_connection();
        loop {
            println!("calling `send_queue`");
            // send_queue(&db_conn).await;
            task::sleep(Duration::new(30, 0)).await;
        }
    })());
}

fn setup_socket(socket: &TcpStream) {
    // TODO: fix read timeout
    // socket.set_read_timeout(Some(Duration::new(30, 0))).unwrap(); // TODO: check if is this correct
}

async fn connect_to_server(server_uid: u32) -> Client {
    let db_conn = open_db_connection();

    let addr = get_server_address_for_uid(&db_conn, server_uid);

    let socket = TcpStream::connect(addr)
        .await
        .expect("Failed to connect to client"); // TODO: propagate error

    setup_socket(&socket);

    Client::new(socket, db_conn)
}

fn start_handle_loop(client: Client) {
    task::spawn((async move || {
        if let Err(error) = handle_connection(client).await {
            println!("error: {}", error);
        }

        println!("connection closed");
    })());
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
    let mut line = String::new();

    let mut bytes = (&mut client.socket).bytes();
    while let Some(byte) = bytes.next().await {
        let byte = byte? as char;

        if byte == '\n' {
            break;
        }

        line.push(byte);
    }

    let line = line.trim();

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

        let entry = get_entry_by_number(&client.db_con, number, true);

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

            let entry = get_entry_by_number(&client.db_con, package.number, false);

            if let Some(entry) = entry {
                if entry.connection_type == 0 {
                    register_entry(
                        &client.db_con,
                        package.number,
                        package.pin,
                        package.port,
                        u32::from(ipaddress),
                        true,
                    );
                } else if package.pin == entry.pin {
                    update_entry_address(
                        &client.db_con,
                        package.port,
                        u32::from(ipaddress),
                        package.number,
                    );
                } else {
                    bail!(MyErrorKind::UserInputError);
                }
            } else {
                register_entry(
                    &client.db_con,
                    package.number,
                    package.pin,
                    package.port,
                    u32::from(ipaddress),
                    false,
                );
            };

            client.send_package(Package::Type2(PackageData2 { ipaddress })).await?;

            Ok(())
        }
        // Package::Type2(package) => {}
        Package::Type3(package) => {
            if client.state != State::Idle {
                bail!(MyErrorKind::InvalidState(State::Idle, client.state));
            }

            let entry = get_entry_by_number(&client.db_con, package.number, true);

            if let Some(entry) = entry {
                client.send_package(Package::Type5(PackageData5::from(entry))).await?;
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

            let new_entry = DirectoryEntry::from(package);

            upsert_entry(
                &client.db_con,
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

            client.push_entries_to_send_queue(get_all_entries(&client.db_con));

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

            let entries =
                get_public_entries_by_pattern(&client.db_con, package.pattern.to_str().unwrap());

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

    start_handle_loop(client);
}

async fn send_queue_for_server(server_uid: u32) {
    let mut client = connect_to_server(server_uid).await;

    client.state = State::Responding;

    client.push_to_send_queue(get_queue_for_server(&client.db_con, server_uid));

    client
        .send_package(Package::Type6(PackageData6 {
            version: 1,
            server_pin: SERVER_PIN,
        }))
        .await
        .unwrap();

    start_handle_loop(client);
}

async fn full_query(conn: &rusqlite::Connection) -> anyhow::Result<()> {
    let servers = get_server_uids(conn);

    for server in servers {
        full_query_for_server(server).await;
    }

    Ok(()) //TODO
}

async fn send_queue(conn: &rusqlite::Connection) -> anyhow::Result<()> {
    update_queue(&conn)?;

    let servers = get_server_uids(conn);

    let server_interactions = servers.iter().map(|&server| send_queue_for_server(server));

    join_all(server_interactions).await;

    Ok(()) //TODO
}
