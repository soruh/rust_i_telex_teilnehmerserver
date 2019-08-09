extern crate crossbeam;
extern crate dotenv;
extern crate nom;

#[macro_use]
extern crate rusqlite;
use rusqlite::OpenFlags;

pub mod models;

pub mod db;
pub mod packages;
pub mod serde;

use dotenv::dotenv;

pub use crate::packages::*;
use serde::{deserialize, serialize};

use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::time::Duration;

use db::*;

use crate::models::*;

const SERVER_PIN: u32 = 42; //TODO: centralize

fn main() {
    dotenv().ok();

    let db_path = "./database.db";
    // std::env::var("DATABASE_PATH").expect("failed to read 'DATABASE_PATH' from environment");s

    {
        let initial_db_open_flags =
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE;
        let initial_db_con =
            rusqlite::Connection::open_with_flags(db_path, initial_db_open_flags).unwrap();
        create_tables(&initial_db_con);
        initial_db_con
            .close()
            .expect("failed to close initial database connection");
    }

    println!("database is initialized");

    let listener = TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], 11814))).unwrap();
    println!("listening started, ready to accept");

    for socket in listener.incoming() {
        let socket = socket.unwrap();
        socket.set_read_timeout(Some(Duration::new(30, 0))).unwrap(); // TODO: check if is this correct

        let db_open_flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let db_con = rusqlite::Connection::open_with_flags(db_path, db_open_flags).unwrap();

        std::thread::spawn(|| {
            if let Err(error) = handle_connection(socket, db_con) {
                println!("error: {}", error);
            }

            println!("connection closed");
        });
    }
}

/*
fn main() {
    let db_url = "test.sqlite3";
    let pool = Pool::builder()
        .build(ConnectionManager::<SqliteConnection>::new(db_url))
        .unwrap();

    crossbeam::scope(|scope| {
        let pool2 = pool.clone();
        scope.spawn(move |_| {
            let conn = pool2.get().unwrap();
            for i in 0..100 {
                let name = format!("John{}", i);
                diesel::delete(users::table)
                    .filter(users::name.eq(&name))
                    .execute(&conn)
                    .unwrap();
            }
        });

        let conn = pool.get().unwrap();
        for i in 0..100 {
            let name = format!("John{}", i);
            diesel::insert_into(users::table)
                .values(User { name })
                .execute(&conn)
                .unwrap();
        }
    })
    .unwrap();
}
*/

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
    parsing: bool,

    db_con: rusqlite::Connection,

    state: State,
    send_queue: Vec<DirectoryEntry>,
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

impl Client {
    fn send_package(&mut self, package: Package) -> Result<(), String> {
        self.write(serialize(package).as_slice())
            .map(|_res| ())
            .map_err(|_err| "failed to send Package".into())
    }

    fn shutdown(&mut self) -> std::result::Result<(), std::io::Error> {
        self.parsing = false;
        self.socket.shutdown(std::net::Shutdown::Both)
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
            self.send_package(Package::Type5(PackageData5::from(entry)))
        } else {
            self.send_package(Package::Type9(PackageData9 {}))
        }
    }
}

fn handle_connection(socket: TcpStream, db_con: rusqlite::Connection) -> Result<(), String> {
    let mut client = Client {
        socket: socket,

        mode: Mode::Unknown,
        parsing: true,

        db_con,

        state: State::Idle,
        send_queue: Vec::new(),
    };

    println!("new connection: {}", client.socket.peer_addr().unwrap());

    get_client_type(&mut client)?;
    assert_ne!(client.mode, Mode::Unknown);

    println!("client mode: {:?}", client.mode);

    while client.parsing {
        consume_package(&mut client)?;
    }

    Ok(())
}

fn get_client_type(client: &mut Client) -> Result<(), String> {
    assert_eq!(client.mode, Mode::Unknown);

    let mut buf = [0u8; 1];
    let len = client.socket.peek(&mut buf).unwrap(); // read the first byte
    if len == 0 {
        // the connection was closed before any data could be read
        return Err("Connection closed by remote".into());
    }

    let first_byte = buf[0];

    println!("first byte: {:#04x}", buf[0]);

    if first_byte >= 32 && first_byte <= 126 {
        client.mode = Mode::Ascii;
    } else {
        client.mode = Mode::Binary;
    }

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

    client.parsing = false;

    Ok(())
}

fn consume_package_binary(client: &mut Client) -> Result<(), String> {
    let mut header = [0u8; 2];
    client
        .read_exact(&mut header)
        .map_err(|err| format!("failed to read package header. error: {:?}", err))?;

    let package_type = header[0];
    let package_length = header[1];

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

    match deserialize(package_type, &body) {
        Err(err) => {
            println!("failed to parse package: {}", err);
        }
        Ok(package) => {
            println!("parsed package: {:#?}", package);
            handle_package(client, package)?;
        }
    }

    // client.parsing = true;
    Ok(())
}

fn handle_package(client: &mut Client, package: Package) -> Result<(), String> {
    println!("state: '{:?}'", client.state);
    match package {
        Package::Type1(package) => {
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
        // Package::Type3(package) => {}
        // Package::Type4(_package) => {}
        Package::Type5(package) => {
            if client.state != State::Accepting {
                return Err(format!("invalid client state: {:?}", client.state));
            }

            let entry = DirectoryEntry::from(package);

            upsert_entry(
                &client.db_con,
                entry.number,
                entry.name,
                entry.connection_type,
                entry.hostname,
                entry.ipaddress,
                entry.port,
                entry.extension,
                entry.pin,
                entry.disabled,
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

            client.send_queue.extend(get_all_entries(&client.db_con));

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

            client.state = State::Shutdown;
            client
                .shutdown()
                .map_err(|err| format!("failed to shut down socket. {:?}", err))
        }
        Package::Type10(package) => {
            if package.version != 1 {
                return Err(format!("invalid package version: {}", package.version));
            }
            get_entries_by_pattern(&client.db_con, package.pattern.to_str().unwrap().to_owned());
            Err("unimplented".into())
        }
        Package::Type255(package) => Err(package.message.to_str().unwrap().to_owned()),

        _ => Err("recieved invalid package".into()),
    }
}
