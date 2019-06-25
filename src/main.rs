#[macro_use]
extern crate diesel;
extern crate dotenv;
extern crate nom;

pub mod models;
pub mod schema;

pub mod db;
pub mod packages;
pub mod serde;

use diesel::mysql::MysqlConnection;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};

use dotenv::dotenv;

pub use crate::packages::*;
use serde::{deserialize, serialize};

use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

fn main() {
    dotenv().ok();

    let manager = ConnectionManager::<MysqlConnection>::new("postgres://localhost/");
    let db_pool = Pool::builder()
        .build(manager)
        .expect("Failed to create pool.");

    println!("connected to database");

    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 11814))).unwrap();
    println!("listening started, ready to accept");
    for socket in listener.incoming() {
        let socket = socket.unwrap();
        socket.set_read_timeout(Some(Duration::new(30, 0))).unwrap(); // TODO: check if is this correct

        let db_con = db_pool.get().expect("failed to get connection from pool");

        thread::spawn(|| {
            if let Err(error) = handle_connection(socket, db_con) {
                println!("error: {}", error);
            }

            println!("connection closed");
        });
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Mode {
    Ascii,
    Binary,
    Unknown,
}

#[derive(Debug, PartialEq, Eq)]
enum Status {
    Idle,
    Responding,
    Accepting,
}

#[derive(Debug)]
struct Client {
    socket: TcpStream,

    mode: Mode,
    parsing: bool,

    status: Status,
    send_queue: Vec<u8>,
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
}

fn handle_connection(
    socket: TcpStream,
    db_pool: PooledConnection<ConnectionManager<MysqlConnection>>,
) -> Result<(), String> {
    let mut client = Client {
        socket: socket,

        mode: Mode::Unknown,
        parsing: true,

        status: Status::Idle,
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

    println!("first byte: {}", buf[0]);

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
        let byte = byte.map_err(|_| String::from(String::from("failed to read byte")))? as char;

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

        if let Ok(number) = number.as_str().parse::<u32>() {
            println!("parsed number: '{}'", number);
        } else {
            return Err("failed to parse number".into());
        }
    }

    client.parsing = false;
    Ok(())
}

fn consume_package_binary(client: &mut Client) -> Result<(), String> {
    let mut header = [0u8; 2];
    client
        .read_exact(&mut header)
        .map_err(|_| String::from("failed to read package header"))?;

    let package_type = header[0];
    let package_length = header[1];

    let mut body = vec![0u8; package_length as usize];
    client
        .read_exact(&mut body)
        .map_err(|_| String::from("failed to read package body"))?;

    println!(
        "got package of type: {} with length: {}\nbody: {:?}",
        package_type, package_length, body
    );

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
    match package {
        Package::Type1(package) => {
            let peer_addr = client.socket.peer_addr().unwrap();
            client.send_package(Package::Type2(PackageData2 {
                ipaddress: Ipv4Addr::new(127, 0, 0, 1),
                //TODO: replace with correct logic
            }))
        }
        // Package::Type2(package) => {}
        // Package::Type3(package) => {}
        // Package::Type4(_package) => {}
        Package::Type5(package) => Ok(()),
        Package::Type6(package) => Ok(()),
        Package::Type7(package) => Ok(()),
        Package::Type8(_package) => Ok(()),
        Package::Type9(_package) => Ok(()),
        Package::Type10(package) => Ok(()),
        Package::Type255(package) => Ok(()),

        _ => Err("recieved invalid package".into()),
    }
}
