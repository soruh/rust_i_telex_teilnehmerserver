use std::io::Read;
use std::net::{TcpListener, TcpStream};
use std::thread;

fn main() {
    let listener = TcpListener::bind("127.0.0.1:11811").unwrap();
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
}

impl Read for Client {
    fn read(&mut self, buf: &mut [u8]) -> std::result::Result<usize, std::io::Error> {
        self.socket.read(buf)
    }
}

#[derive(PartialEq, Debug)]
enum Control {
    Continue,
    Stop,
}

fn handle_connection(socket: TcpStream) -> Result<(), String> {
    let mut client = Client {
        socket: socket,
        mode: Mode::Unknown,
    };

    println!("new connection: {}", client.socket.peer_addr().unwrap());

    get_client_type(&mut client)?;
    assert_ne!(client.mode, Mode::Unknown);

    println!("client mode: {:?}", client.mode);

    loop {
        let control = handle_packages(&mut client)?;
        if control == Control::Stop {
            break;
        }
    }

    Ok(())
}

fn get_client_type(client: &mut Client) -> Result<(), String> {
    assert_eq!(client.mode, Mode::Unknown);

    let mut buf = [0; 1];
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

fn handle_packages(client: &mut Client) -> Result<Control, String> {
    assert_ne!(client.mode, Mode::Unknown);

    if client.mode == Mode::Binary {
        return handle_packages_binary(client);
    } else {
        return handle_packages_ascii(client);
    }
}

fn handle_packages_binary(client: &mut Client) -> Result<Control, String> {
    unimplemented!();

    Ok(Control::Continue)
}

fn handle_packages_ascii(client: &mut Client) -> Result<Control, String> {
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

    Ok(Control::Stop)
}
