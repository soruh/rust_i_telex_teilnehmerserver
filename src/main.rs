extern crate crossbeam;
extern crate dotenv;
extern crate nom;

#[macro_use]
extern crate rusqlite;
use rusqlite::OpenFlags;

#[macro_use]
extern crate lazy_static;

pub mod models;

pub mod db;
pub mod packages;
pub mod serde;

use dotenv::dotenv;

pub use crate::packages::*;
use serde::{deserialize, serialize};

use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::time::{Duration, Instant};

use std::thread;

use db::*;

use crate::models::*;

const SERVER_PIN: u32 = 42;
const DB_PATH: &str = "./database.db";
//TODO: use config files

#[derive(Debug, PartialEq, Eq)]
enum Mode {
    Ascii,
    Binary,
    Unknown,
}

#[derive(Debug, PartialEq, Eq)]
enum State {
    Idle,
    Responding,
    Accepting,
    Shutdown,
}

struct Client {
    socket: TcpStream,

    mode: Mode,

    db_con: rusqlite::Connection,

    state: State,
    send_queue: Vec<(DirectoryEntry, Option<u32>)>,
}

impl Read for Client {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.socket.read(buf)
    }
}

impl Write for Client {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.socket.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.socket.flush()
    }
}

fn open_db_connection() -> rusqlite::Connection {
    let db_open_flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX;

    rusqlite::Connection::open_with_flags(DB_PATH, db_open_flags).unwrap()
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

    fn send_package(&mut self, package: Package) -> Result<(), String> {
        println!("sending package: {:#?}", package);
        self.write(serialize(package).as_slice())
            .map(|_res| ())
            .map_err(|_err| "failed to send Package".into())
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

    fn send_queue_entry(&mut self) -> Result<(), String> {
        if self.state != State::Responding {
            return Err(format!(
                "not in responding state. current state={:?}",
                self.state
            ));
        }

        let len = self.send_queue.len();

        println!(
            "entries left in queue: {} -> {}",
            len,
            if len == 0 { 0 } else { len - 1 }
        );

        if let Some(entry) = self.send_queue.pop() {
            let (package, queue_uid) = entry;

            self.send_package(Package::Type5(PackageData5::from(package)))?;

            if let Some(queue_uid) = queue_uid {
                remove_queue_entry(&self.db_con, queue_uid);
            }

            Ok(())
        } else {
            self.send_package(Package::Type9(PackageData9 {}))
        }
    }
}

fn main() {
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

    start_server_sync_thread();

    println!("started server sync thread");

    let addr = SocketAddr::from(([0, 0, 0, 0], 11814));

    let listener = TcpListener::bind(addr).unwrap();
    // TODO: use config
    println!("listening for connections on {}", addr);

    for socket in listener.incoming() {
        let socket = socket.unwrap();

        setup_socket(&socket);

        let db_conn = open_db_connection();
        let client = Client::new(socket, db_conn);

        start_handle_loop(client);
    }
}

fn start_server_sync_thread() {
    thread::spawn(move || {
        struct Syncronizer {
            name: String,
            last_sync: Instant,
            sync_interval: Duration,
            action: fn(&rusqlite::Connection) -> Result<(), String>,
        }

        impl Syncronizer {
            fn update(&mut self, conn: &rusqlite::Connection) -> Result<(), String> {
                let now = Instant::now();

                if now >= self.last_sync + self.sync_interval {
                    self.last_sync = now;

                    (self.action)(conn)
                } else {
                    Ok(())
                }
            }
        }

        let last_year = Instant::now() - Duration::new(60 * 60 * 24 * 365, 0);
        // ? assume last sync was one year ago

        let mut syncronizers = vec![
            Syncronizer {
                name: "prune_old_queue_entries".into(),
                action: prune_old_queue_entries,
                sync_interval: Duration::new(60 * 60 * 24 * 7, 0),
                last_sync: last_year,
            },
            Syncronizer {
                name: "full_query".into(),
                action: full_query,
                sync_interval: Duration::new(60 * 60 * 24, 0),
                last_sync: last_year,
            },
            Syncronizer {
                name: "send_queue".into(),
                action: send_queue,
                sync_interval: Duration::new(30, 0),
                last_sync: last_year,
            },
        ];

        let sleep_duration = Duration::new(60, 0);
        // ? check if sync is neccessary every _minute_

        let db_conn = open_db_connection();

        loop {
            for syncronizer in &mut syncronizers {
                if let Err(err) = syncronizer.update(&db_conn) {
                    println!(
                        "failed to run syncronizer {:?} error: {}",
                        syncronizer.name, err
                    );
                }
            }

            thread::sleep(sleep_duration);
        }
    });
}

fn setup_socket(socket: &TcpStream) {
    socket.set_read_timeout(Some(Duration::new(30, 0))).unwrap(); // TODO: check if is this correct
}

fn connect_to_server(server_uid: u32) -> Client {
    let db_conn = open_db_connection();

    let addr = get_server_address_for_uid(&db_conn, server_uid);

    let socket = TcpStream::connect(addr).expect("Failed to connect to client"); // TODO: propagate error

    setup_socket(&socket);

    Client::new(socket, db_conn)
}

fn start_handle_loop(client: Client) {
    thread::spawn(move || {
        if let Err(error) = handle_connection(client) {
            println!("error: {}", error);
        }

        println!("connection closed");
    });
}

fn handle_connection(mut client: Client) -> Result<(), String> {
    println!("new connection: {}", client.socket.peer_addr().unwrap());

    peek_client_type(&mut client)?;
    debug_assert_ne!(client.mode, Mode::Unknown);

    println!("client mode: {:?}", client.mode);

    while client.state != State::Shutdown {
        consume_package(&mut client)?;
    }

    Ok(())
}

fn peek_client_type(client: &mut Client) -> Result<(), String> {
    assert_eq!(client.mode, Mode::Unknown);

    let mut buf = [0u8; 1];
    let len = client.socket.peek(&mut buf).unwrap(); // read the first byte
    if len == 0 {
        // the connection was closed before any data could be read
        return Err("Connection closed by remote".into());
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

fn consume_package(client: &mut Client) -> Result<(), String> {
    assert_ne!(client.mode, Mode::Unknown);

    if client.mode == Mode::Binary {
        return consume_package_binary(client);
    } else {
        return consume_package_ascii(client);
    }
}

fn consume_package_ascii(client: &mut Client) -> Result<(), String> {
    let mut line = String::new();
    for byte in client.bytes() {
        let byte = byte.map_err(|_| String::from("failed to read byte"))? as char;

        if byte == '\n' {
            break;
        }

        line.push(byte);
    }

    let line = line.trim();

    println!("full line: {}", line);

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

        let number = if let Ok(number) = number.as_str().parse::<u32>() {
            number
        } else {
            return Err("failed to parse number".into());
        };

        println!("parsed number: '{}'", number);

        unimplemented!("send response");
    }

    client.shutdown().expect("failed to shutdown socket");

    Ok(())
}

fn consume_package_binary(client: &mut Client) -> Result<(), String> {
    let mut header = [0u8; 2];
    client
        .read_exact(&mut header)
        .map_err(|err| format!("failed to read package header. error: {:?}", err))?;

    println!("header: {:?}", header);

    let [package_type, package_length] = header;

    let mut body = vec![0u8; package_length as usize];
    client
        .read_exact(&mut body)
        .map_err(|_| String::from("failed to read package body"))?;

    println!(
        "got package of type: {} with length: {}",
        package_type, package_length
    );

    if body.len() > 0 {
        println!("body: {:?}", body);
    }

    let package = deserialize(package_type, &body)?;
    println!("parsed package: {:#?}", package);
    handle_package(client, package)?;

    Ok(())
}

fn handle_package(client: &mut Client, package: Package) -> Result<(), String> {
    println!("state: '{:?}'", client.state);
    match package {
        Package::Type1(package) => {
            if client.state != State::Idle {
                return Err(format!("invalid client state: {:?}", client.state));
            }

            let peer_addr = client.socket.peer_addr().unwrap();

            let ipaddress = if let IpAddr::V4(ipaddress) = peer_addr.ip() {
                Ok(ipaddress)
            } else {
                Err(String::from("client does not have an ipv4 address"))
            }?;

            let entry = get_entry_by_number(&client.db_con, package.number);

            if let Some(entry) = entry {
                if package.pin == entry.pin {
                    update_entry_address(
                        &client.db_con,
                        package.port,
                        u32::from(ipaddress),
                        package.number,
                    );
                } else {
                    return Err(String::from("wrong pin"));
                }
            } else {
                register_entry(
                    &client.db_con,
                    package.number,
                    package.pin,
                    package.port,
                    u32::from(ipaddress),
                );
            }

            client.send_package(Package::Type2(PackageData2 { ipaddress }))
        }
        // Package::Type2(package) => {}
        Package::Type3(package) => {
            if client.state != State::Idle {
                return Err(format!("invalid client state: {:?}", client.state));
            }

            let entry = get_entry_by_number(&client.db_con, package.number);

            if let Some(entry) = entry {
                client.send_package(Package::Type5(PackageData5::from(entry)))
            } else {
                client.send_package(Package::Type4(PackageData4 {}))
            }
        }
        // Package::Type4(_package) => {}
        Package::Type5(package) => {
            if client.state != State::Accepting {
                return Err(format!("invalid client state: {:?}", client.state));
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
            client.send_package(Package::Type8(PackageData8 {}))
        }
        Package::Type6(package) => {
            if package.version != 1 {
                return Err(format!("invalid package version: {}", package.version));
            }
            if package.server_pin != SERVER_PIN {
                return Err(String::from("invalid serverpin"));
            }
            if client.state != State::Idle {
                return Err(format!("invalid client state: {:?}", client.state));
            }

            client.state = State::Responding;

            client.push_entries_to_send_queue(get_all_entries(&client.db_con));

            client.send_queue_entry()
        }
        Package::Type7(package) => {
            if package.version != 1 {
                return Err(format!("invalid package version: {}", package.version));
            }
            if package.server_pin != SERVER_PIN {
                return Err(String::from("invalid serverpin"));
            }
            if client.state != State::Idle {
                return Err(format!("invalid client state: {:?}", client.state));
            }

            client.state = State::Accepting;

            client.send_package(Package::Type8(PackageData8 {}))
        }
        Package::Type8(_package) => {
            if client.state != State::Responding {
                return Err(format!("invalid client state: {:?}", client.state));
            }

            client.send_queue_entry()
        }
        Package::Type9(_package) => {
            if client.state != State::Accepting {
                return Err(format!("invalid client state: {:?}", client.state));
            }

            client
                .shutdown()
                .map_err(|err| format!("failed to shut down socket. {:?}", err))
        }
        Package::Type10(package) => {
            if package.version != 1 {
                return Err(format!("invalid package version: {}", package.version));
            }
            if client.state != State::Idle {
                return Err(format!("invalid client state: {:?}", client.state));
            }
            let entries = get_entries_by_pattern(&client.db_con, package.pattern.to_str().unwrap());

            client.state = State::Responding;

            client.push_entries_to_send_queue(entries);

            client.send_queue_entry()
        }
        Package::Type255(package) => Err(package.message.to_str().unwrap().to_owned()),

        _ => Err("recieved invalid package".into()),
    }
}

fn full_query_for_server(server_uid: u32) {
    let mut client = connect_to_server(server_uid);

    client.state = State::Accepting;

    client
        .send_package(Package::Type7(PackageData7 {
            version: 1,
            server_pin: SERVER_PIN,
        }))
        .unwrap();

    start_handle_loop(client);
}

fn send_queue_for_server(server_uid: u32) {
    let mut client = connect_to_server(server_uid);

    client.state = State::Responding;

    client.push_to_send_queue(get_queue_for_server(&client.db_con, server_uid));

    client
        .send_package(Package::Type6(PackageData6 {
            version: 1,
            server_pin: SERVER_PIN,
        }))
        .unwrap();

    start_handle_loop(client);
}

fn full_query(conn: &rusqlite::Connection) -> Result<(), String> {
    let servers = get_server_uids(conn);

    for server in servers {
        full_query_for_server(server);
    }

    Ok(()) //TODO
}
fn send_queue(conn: &rusqlite::Connection) -> Result<(), String> {
    update_queue(&conn)?;

    let servers = get_server_uids(conn);

    for server in servers {
        send_queue_for_server(server);
    }

    Ok(()) //TODO
}
