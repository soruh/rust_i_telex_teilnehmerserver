use std::io::Read;
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
struct Client {
    socket: TcpStream,
    mode: Mode,
    parsing: bool,
}

impl Read for Client {
    fn read(&mut self, buf: &mut [u8]) -> std::result::Result<usize, std::io::Error> {
        self.socket.read(buf)
    }
}

fn handle_connection(socket: TcpStream) -> Result<(), String> {
    let mut client = Client {
        socket: socket,
        mode: Mode::Unknown,
        parsing: true,
    };

    println!("new connection: {}", client.socket.peer_addr().unwrap());

    get_client_type(&mut client)?;
    assert_ne!(client.mode, Mode::Unknown);

    println!("client mode: {:?}", client.mode);

    while client.parsing {
        handle_packages(&mut client)?;
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

fn handle_packages(client: &mut Client) -> Result<(), String> {
    assert_ne!(client.mode, Mode::Unknown);

    if client.mode == Mode::Binary {
        return handle_packages_binary(client);
    } else {
        return handle_packages_ascii(client);
    }
}

fn handle_packages_ascii(client: &mut Client) -> Result<(), String> {
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

fn handle_packages_binary(client: &mut Client) -> Result<(), String> {
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

    match deserialize(package_type, package_length, &body) {
        Ok(package) => {
            println!("package: {:#?}", package);
        }
        Err(err) => {
            println!("failed to parse package: {}", err);
        }
    }

    // client.parsing = true;
    Ok(())
}

// ################################################################

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
enum PackageData {
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

#[derive(Debug)]
struct Package {
    package_type: u8,
    package_length: u8,
    data: PackageData,
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
    parse_type_1<&[u8], PackageData>,
    do_parse!(
        number: le_u32 >>
        pin: le_u16 >>
        port: le_u16 >>
        (PackageData::Type1(PackageData1 { number, pin, port }))
    )
);

named!(
    parse_type_2<&[u8], PackageData>,
    do_parse!(
        ipaddress: be_u32 >>
        (PackageData::Type2(PackageData2 {ipaddress: Ipv4Addr::from(ipaddress)}))
    )
);

named!(
    parse_type_3<&[u8], PackageData>,
    do_parse!(
        number: le_u32 >>
        version: le_u8 >>
        (PackageData::Type3(PackageData3 {number, version}))
    )
);

named!(
    parse_type_5<&[u8], PackageData>,
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
        (PackageData::Type5(PackageData5 {
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
    parse_type_6<&[u8], PackageData>,
    do_parse!(
        version: le_u8 >>
        server_pin: le_u32 >>
        (PackageData::Type6(PackageData6 {
            version,
            server_pin,
        }))
    )
);

named!(
    parse_type_7<&[u8], PackageData>,
    do_parse!(
        version: le_u8 >>
        server_pin: le_u32 >>
        (PackageData::Type7(PackageData7 {
            version,
            server_pin,
        }))
    )
);

named!(
    parse_type_10<&[u8], PackageData>,
    do_parse!(
        version: le_u8 >>
        pattern: take!(40) >>
        (PackageData::Type10(PackageData10 {
            version,
            pattern: read_nul_terminated(pattern)?,
        }))
    )
);

fn parse_type_255(input: &[u8]) -> Result<(&[u8], PackageData), nom::Err<&[u8]>> {
    Ok((
        input,
        PackageData::Type255(PackageData255 {
            message: read_nul_terminated(input)?,
        }),
    ))
}

fn deserialize(package_type: u8, package_length: u8, input: &[u8]) -> Result<Package, String> {
    let data: Result<(&[u8], PackageData), nom::Err<&[u8]>> = match package_type {
        1 => parse_type_1(input),
        2 => parse_type_2(input),
        3 => parse_type_3(input),
        4 => Ok((input, PackageData::Type4(PackageData4 {}))),
        5 => parse_type_5(input),
        6 => parse_type_6(input),
        7 => parse_type_7(input),
        8 => Ok((input, PackageData::Type8(PackageData8 {}))),
        9 => Ok((input, PackageData::Type9(PackageData9 {}))),
        10 => parse_type_10(input),
        255 => parse_type_255(input),
        _ => return Err(String::from("unrecognized package type")),
    };

    let data = data
        .map_err(|err| {
            String::from(format!(
                "failed to parse package (type {}): {}",
                package_type, err
            ))
        })?
        .1;

    let package = Package {
        package_type,
        package_length,
        data,
    };

    Ok(package)
}

/*
fn serialize<'a>(package: Package) -> Result<&'a [u8], String> {
    Err(String::from("err"))
}
*/
