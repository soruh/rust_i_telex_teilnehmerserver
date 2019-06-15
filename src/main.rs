use std::io::{Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::thread;

fn main() {
    let listener = TcpListener::bind("127.0.0.1:11814").unwrap();
    println!("listening started, ready to accept");
    for socket in listener.incoming() {
        thread::spawn(|| {
            if let Err(error) = handle_connection(socket.unwrap()) {
                println!("error: {}", error);
            }

            println!("connection closed");
        });
    }
}

#[derive(PartialEq, Debug)]
enum Mode {
    Ascii,
    Binary,
    Unknown,
}

#[derive(Debug)]
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
            .map_err(|_err| String::from("failed to send Package"))
    }
}

fn handle_connection(socket: TcpStream) -> Result<(), String> {
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
        return Err(String::from("Connection closed by remote"));
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

        if let Ok(number) = number.as_str().parse::<u32>() {
            println!("parsed number: '{}'", number);
        } else {
            return Err(String::from("failed to parse number"));
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

// ! ################################################################

// #[macro_use]
extern crate nom;
use nom::*;
use std::ffi::CString; //NulError

#[derive(Debug)]
struct PackageData1 {
    number: u32,
    pin: u16,
    port: u16,
}
#[derive(Debug)]
struct PackageData2 {
    ipaddress: Ipv4Addr,
}
#[derive(Debug)]
struct PackageData3 {
    number: u32,
    version: u8,
}
#[derive(Debug)]
struct PackageData4 {}
#[derive(Debug)]
struct PackageData5 {
    number: u32,
    name: CString,
    flags: u16,
    client_type: u8,
    hostname: CString,
    ipaddress: Ipv4Addr,
    port: u16,
    extension: u8,
    pin: u16,
    date: u32,
}
#[derive(Debug)]
struct PackageData6 {
    version: u8,
    server_pin: u32,
}
#[derive(Debug)]
struct PackageData7 {
    version: u8,
    server_pin: u32,
}
#[derive(Debug)]
struct PackageData8 {}
#[derive(Debug)]
struct PackageData9 {}
#[derive(Debug)]
struct PackageData10 {
    version: u8,
    pattern: CString,
}
#[derive(Debug)]
struct PackageData255 {
    message: CString,
}

#[derive(Debug)]
enum Package {
    Type1(PackageData1),
    Type2(PackageData2),
    Type3(PackageData3),
    Type4(PackageData4),
    Type5(PackageData5),
    Type6(PackageData6),
    Type7(PackageData7),
    Type8(PackageData8),
    Type9(PackageData9),
    Type10(PackageData10),
    Type255(PackageData255),
}

named!(
    _read_nul_terminated <&[u8], CString>,
    do_parse!(
        content: take_until!("\0") >>
        (CString::new(content).unwrap())
    )
);

fn read_nul_terminated(input: &[u8]) -> Result<CString, nom::Err<&[u8]>> {
    _read_nul_terminated(input).map(|res| res.1)
}

named!(
    parse_type_1<&[u8], Package>,
    do_parse!(
        number: le_u32 >>
        pin: le_u16 >>
        port: le_u16 >>
        (Package::Type1(PackageData1 { number, pin, port }))
    )
);

named!(
    parse_type_2<&[u8], Package>,
    do_parse!(
        ipaddress: be_u32 >>
        (Package::Type2(PackageData2 {ipaddress: Ipv4Addr::from(ipaddress)}))
    )
);

named!(
    parse_type_3<&[u8], Package>,
    do_parse!(
        number: le_u32 >>
        version: le_u8 >>
        (Package::Type3(PackageData3 {number, version}))
    )
);

named!(
    parse_type_5<&[u8], Package>,
    do_parse!(
        number: le_u32 >>
        name: take!(40) >>
        flags: le_u16 >>
        client_type: le_u8 >>
        hostname: take!(40) >>
        ipaddress: be_u32 >>
        port: le_u16 >>
        extension: le_u8 >>
        pin: le_u16 >>
        date: le_u32 >>
        (Package::Type5(PackageData5 {
            number,
            name: read_nul_terminated(name)?,
            flags,
            client_type,
            hostname: read_nul_terminated(hostname)?,
            ipaddress: Ipv4Addr::from(ipaddress),
            port,
            extension,
            pin,
            date
        }))
    )
);

named!(
    parse_type_6<&[u8], Package>,
    do_parse!(
        version: le_u8 >>
        server_pin: le_u32 >>
        (Package::Type6(PackageData6 {
            version,
            server_pin,
        }))
    )
);

named!(
    parse_type_7<&[u8], Package>,
    do_parse!(
        version: le_u8 >>
        server_pin: le_u32 >>
        (Package::Type7(PackageData7 {
            version,
            server_pin,
        }))
    )
);

named!(
    parse_type_10<&[u8], Package>,
    do_parse!(
        version: le_u8 >>
        pattern: take!(40) >>
        (Package::Type10(PackageData10 {
            version,
            pattern: read_nul_terminated(pattern)?,
        }))
    )
);

fn parse_type_255(input: &[u8]) -> Result<(&[u8], Package), nom::Err<&[u8]>> {
    Ok((
        input,
        Package::Type255(PackageData255 {
            message: read_nul_terminated(input)?,
        }),
    ))
}

fn deserialize(package_type: u8, input: &[u8]) -> Result<Package, String> {
    let data: Result<(&[u8], Package), nom::Err<&[u8]>> = match package_type {
        0x01 => parse_type_1(input),
        0x02 => parse_type_2(input),
        0x03 => parse_type_3(input),
        0x04 => Ok((input, Package::Type4(PackageData4 {}))),
        0x05 => parse_type_5(input),
        0x06 => parse_type_6(input),
        0x07 => parse_type_7(input),
        0x08 => Ok((input, Package::Type8(PackageData8 {}))),
        0x09 => Ok((input, Package::Type9(PackageData9 {}))),
        0x0A => parse_type_10(input),
        0xFF => parse_type_255(input),

        _ => return Err(String::from("unrecognized package type")),
    };

    let package = data
        .map_err(|err| {
            String::from(format!(
                "failed to parse package (type {}): {}",
                package_type, err
            ))
        })?
        .1;

    Ok(package)
}

// ! ################################################################################
extern crate byteorder;
use byteorder::{LittleEndian, WriteBytesExt};

fn serialize(package: Package) -> Vec<u8> {
    match package {
        Package::Type1(package) => {
            let package_type: u8 = 0x01;
            let package_length: u8 = 8;

            let mut buf: Vec<u8> = vec![0u8; package_length as usize + 2];

            buf.write_u8(package_type).unwrap();
            buf.write_u8(package_length).unwrap();

            buf.write_u32::<LittleEndian>(package.number).unwrap();
            buf.write_u16::<LittleEndian>(package.pin).unwrap();
            buf.write_u16::<LittleEndian>(package.port).unwrap();

            buf
        }
        Package::Type2(package) => {
            let package_type: u8 = 0x02;
            let package_length: u8 = 4;

            let mut buf: Vec<u8> = vec![0u8; package_length as usize + 2];

            buf.write_u8(package_type).unwrap();
            buf.write_u8(package_length).unwrap();

            buf.write(&package.ipaddress.octets()).unwrap();

            buf
        }
        Package::Type3(package) => {
            let package_type: u8 = 0x03;
            let package_length: u8 = 5;

            let mut buf: Vec<u8> = vec![0u8; package_length as usize + 2];

            buf.write_u8(package_type).unwrap();
            buf.write_u8(package_length).unwrap();

            buf.write_u32::<LittleEndian>(package.number).unwrap();
            buf.write_u8(package.version).unwrap();

            buf
        }
        Package::Type4(_package) => {
            let package_type: u8 = 0x04;
            let package_length: u8 = 0;

            let mut buf: Vec<u8> = vec![0u8; package_length as usize + 2];

            buf.write_u8(package_type).unwrap();
            buf.write_u8(package_length).unwrap();

            buf
        }
        Package::Type5(package) => {
            let package_type: u8 = 0x05;
            let package_length: u8 = 100;

            let mut buf: Vec<u8> = vec![0u8; package_length as usize + 2];

            buf.write_u8(package_type).unwrap();
            buf.write_u8(package_length).unwrap();

            buf.write_u32::<LittleEndian>(package.number).unwrap();
            buf.write(string_to_40_bytes(package.name).as_slice())
                .unwrap();
            buf.write_u16::<LittleEndian>(package.flags).unwrap();
            buf.write_u8(package.client_type).unwrap();
            buf.write(string_to_40_bytes(package.hostname).as_slice())
                .unwrap();
            buf.write(&package.ipaddress.octets()).unwrap();
            buf.write_u16::<LittleEndian>(package.port).unwrap();
            buf.write_u8(package.extension).unwrap();
            buf.write_u16::<LittleEndian>(package.pin).unwrap();
            buf.write_u32::<LittleEndian>(package.date).unwrap();

            buf
        }
        Package::Type6(package) => {
            let package_type: u8 = 0x06;
            let package_length: u8 = 5;

            let mut buf: Vec<u8> = vec![0u8; package_length as usize + 2];

            buf.write_u8(package_type).unwrap();
            buf.write_u8(package_length).unwrap();

            buf.write_u8(package.version).unwrap();
            buf.write_u32::<LittleEndian>(package.server_pin).unwrap();

            buf
        }
        Package::Type7(package) => {
            let package_type: u8 = 0x07;
            let package_length: u8 = 5;

            let mut buf: Vec<u8> = vec![0u8; package_length as usize + 2];

            buf.write_u8(package_type).unwrap();
            buf.write_u8(package_length).unwrap();

            buf.write_u8(package.version).unwrap();
            buf.write_u32::<LittleEndian>(package.server_pin).unwrap();

            buf
        }
        Package::Type8(_package) => {
            let package_type: u8 = 0x08;
            let package_length: u8 = 0;

            let mut buf: Vec<u8> = vec![0u8; package_length as usize + 2];

            buf.write_u8(package_type).unwrap();
            buf.write_u8(package_length).unwrap();

            buf
        }
        Package::Type9(_package) => {
            let package_type: u8 = 0x09;
            let package_length: u8 = 0;

            let mut buf: Vec<u8> = vec![0u8; package_length as usize + 2];

            buf.write_u8(package_type).unwrap();
            buf.write_u8(package_length).unwrap();

            buf
        }
        Package::Type10(package) => {
            let package_type: u8 = 0x0A;
            let package_length: u8 = 41;

            let mut buf: Vec<u8> = vec![0u8; package_length as usize + 2];

            buf.write_u8(package_type).unwrap();
            buf.write_u8(package_length).unwrap();

            buf.write_u8(package.version).unwrap();
            buf.write(string_to_40_bytes(package.pattern).as_slice())
                .unwrap();

            buf
        }

        Package::Type255(package) => {
            let package_type: u8 = 0xFF;

            let message = package.message.into_bytes();

            let package_length: u8 = message.capacity() as u8;

            let mut buf: Vec<u8> = vec![0u8; package_length as usize + 2];

            buf.write_u8(package_type).unwrap();
            buf.write_u8(package_length).unwrap();

            buf.write(&message).unwrap();

            buf
        }
    }
}

fn string_to_40_bytes(input: CString) -> Vec<u8> {
    const STRING_LENGTH: usize = 40;

    let mut buf = vec![0u8; STRING_LENGTH];

    let mut input = input.into_bytes();
    input.truncate(STRING_LENGTH);

    buf.write(&input).unwrap();
    buf[STRING_LENGTH - 1] = 0;

    buf
}

// ! ################################################################################

fn handle_package(client: &mut Client, package: Package) -> Result<(), String> {
    match package {
        Package::Type1(package) => {
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

        _ => Err(String::from("recieved invalid package")),
    }
}
