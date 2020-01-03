use crate::{
    db::*,
    errors::MyErrorKind,
    packages::*,
    serde::{deserialize, serialize},
    CLIENT_TIMEOUT, SERVER_PIN,
};

use futures::{future::FutureExt, lock::Mutex, select, stream::StreamExt};

use anyhow::Context;
use async_std::{io::BufReader, net::TcpStream, prelude::*, task};
use std::{
    convert::TryInto,
    net::{IpAddr, SocketAddr},
};

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
    pub mode: Mode,
    pub state: State,
    pub send_queue: Vec<Package5>,
}

impl Drop for Client {
    fn drop (&mut self) {
        if let Ok(addr) = self.socket.peer_addr() {
            debug!("dropping client at {}", addr);
        } else {
            debug!("dropping client");
        }
        let _ = self.shutdown();
    }
}

impl Client {
    pub fn new(socket: TcpStream) -> Self {
        Self {
            socket,
            mode: Mode::Unknown,
            state: State::Idle,
            send_queue: Vec::new(),
        }
    }

    pub async fn handle(&mut self) -> anyhow::Result<()> {
        info!("handling client at: {}", self.socket.peer_addr().unwrap());

        #[allow(clippy::mut_mut, clippy::unnecessary_mut_passed)]
        {
            select! {
                _ = task::sleep(*CLIENT_TIMEOUT).fuse() => {
                    bail!(MyErrorKind::Timeout);
                }
                res = self.peek_client_type().fuse() => {
                    res?;
                },
            }
        }

        debug_assert_ne!(self.mode, Mode::Unknown);

        debug!("client mode: {:?}", self.mode);

        while self.state != State::Shutdown {
            #[allow(clippy::mut_mut, clippy::unnecessary_mut_passed)]
            {
                {
                    select! {
                        _ = task::sleep(*CLIENT_TIMEOUT).fuse() => {
                            Err(MyErrorKind::Timeout)?;
                        }
                        res = self.consume_package().fuse() => {
                            res?;
                            continue;
                        },
                    }
                }
            }
        }

        self.shutdown()?;

        Ok(())
    }

    pub async fn push(&mut self, entry: Package5) {
        self.send_queue.push(entry);
    }

    pub async fn extend(&mut self, entries: Vec<Package5>) {
        self.send_queue.extend(entries);
    }

    pub async fn send_package(&mut self, package: Package) -> anyhow::Result<()> {
        debug!("sending package: {:#?}", package);

        let package_type = package.package_type();

        let body = serialize(package.try_into()?)?;

        let package_length = body.len() as u8;

        let header = [package_type, package_length];

        self.socket.write_all(&header).await.context(MyErrorKind::FailedToWrite)?;
        self.socket
            .write_all(body.as_slice())
            .await
            .context(MyErrorKind::FailedToWrite)?;

        Ok(())
    }

    pub fn shutdown(&mut self) -> std::result::Result<(), std::io::Error> {
        if self.state == State::Shutdown {
            debug!("tried to shut down client that was already shut down");
            return Ok(());
        }

        if let Ok(addr) = self.socket.peer_addr() {
            debug!("shutting down client at {}", addr);
        } else {
            debug!("shutting down client");
        }

        self.state = State::Shutdown;
        self.socket.shutdown(std::net::Shutdown::Both)?;

        Ok(())
    }

    pub async fn send_queue_entry(&mut self) -> anyhow::Result<()> {
        if self.state != State::Responding {
            bail!(MyErrorKind::InvalidState(State::Responding, self.state));
        }

        if let Some(package) = self.send_queue.pop() {
            if let Err(err) = self.send_package(Package::Type5(package.clone())).await {
                self.send_queue.push(package);

                return Err(err);
            }
        } else {
            self.send_package(Package::Type9(Package9 {})).await?;
        }

        Ok(())
    }

    pub async fn peek_client_type(self: &mut Client) -> anyhow::Result<()> {
        assert_eq!(self.mode, Mode::Unknown);

        let mut buf = [0_u8; 1];
        let len = self
            .socket
            .peek(&mut buf)
            .await
            .context(MyErrorKind::ConnectionCloseUnexpected)?; // read the first byte
        if len == 0 {
            bail!(MyErrorKind::ConnectionCloseUnexpected);
        }

        let [first_byte] = buf;

        debug!("first byte: {:#04x}", first_byte);

        self.mode = if first_byte >= 32 && first_byte <= 126 {
            Mode::Ascii
        } else {
            Mode::Binary
        };

        Ok(())
    }

    pub async fn consume_package(self: &mut Client) -> anyhow::Result<()> {
        assert_ne!(self.mode, Mode::Unknown);

        if self.mode == Mode::Binary {
            self.consume_package_binary().await
        } else {
            self.consume_package_ascii().await
        }
    }

    pub async fn consume_package_ascii(self: &mut Client) -> anyhow::Result<()> {
        let mut lines = BufReader::new(&self.socket).lines();

        let line = lines
            .next()
            .await
            .context(MyErrorKind::UserInputError)?
            .context(MyErrorKind::ConnectionCloseUnexpected)?;

        debug!("full line: {}", line);

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

            debug!("handling 'q' request");

            let number = number
                .as_str()
                .parse::<u32>()
                .context(MyErrorKind::UserInputError)?;

            debug!("parsed number: '{}'", number);

            let entry = get_entry_by_number(number, true)?;

            let message = if let Some(entry) = entry {
                let host_or_ip = if let Some(hostname) = entry.hostname {
                    hostname
                } else {
                    let ipaddress = entry.ipaddress.expect(
                        "database is incosistent: entry has neither hostname nor ipaddress",
                    );

                    format!("{}", ipaddress)
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

            self.socket
                .write_all(message.as_bytes())
                .await
                .context(MyErrorKind::FailedToWrite)?;
        } else {
            bail!(MyErrorKind::UserInputError);
        }

        self.shutdown()?;

        Ok(())
    }

    pub async fn consume_package_binary(self: &mut Client) -> anyhow::Result<()> {
        let mut header = [0_u8; 2];
        self.socket
            .read_exact(&mut header)
            .await
            .context(MyErrorKind::ConnectionCloseUnexpected)?;

        // debug!("header: {:?}", header);

        let [package_type, package_length] = header;

        debug!(
            "reading package of type: {} with length: {}",
            package_type, package_length
        );

        let mut body = vec![0_u8; package_length as usize];

        // TODO: remove!
        let mut read_total: usize = 0;
        while read_total < package_length as usize {
            read_total += self.socket
                .read(&mut body[read_total..])
                .await
                .context(MyErrorKind::ConnectionCloseUnexpected)?;

            debug!("read {}/{} bytes", read_total, package_length);
        }

        /*
        self.socket
            .read_exact(&mut body)
            .await
            .context(MyErrorKind::ConnectionCloseUnexpected)?;
        */

        // debug!("body: {:?}", body);

        let package = deserialize(package_type, body.as_slice())?.try_into()?;

        debug!("received package: {:#?}", package);

        self.handle_package(package).await?;

        Ok(())
    }

    pub async fn handle_package(self: &mut Client, package: Package) -> anyhow::Result<()> {
        debug!("state: '{:?}'", self.state);
        match package {
            Package::Type1(package) => {
                if self.state != State::Idle {
                    bail!(MyErrorKind::InvalidState(State::Idle, self.state));
                }

                let peer_addr = self.socket.peer_addr().unwrap();

                let ipaddress = if let IpAddr::V4(ipaddress) = peer_addr.ip() {
                    Ok(ipaddress)
                } else {
                    Err(MyErrorKind::UserInputError)
                }?;

                let entry = get_entry_by_number(package.number, false)?;

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

                self.send_package(Package::Type2(Package2 { ipaddress }))
                    .await?;

                Ok(())
            }
            // Package::Type2(package) => {}
            Package::Type3(package) => {
                if self.state != State::Idle {
                    bail!(MyErrorKind::InvalidState(State::Idle, self.state));
                }

                let entry = get_entry_by_number(package.number, true)?;

                if let Some(entry) = entry {
                    self.send_package(Package::Type5(entry)).await?;
                } else {
                    self.send_package(Package::Type4(Package4 {})).await?;
                }

                Ok(())
            }
            // Package::Type4(_package) => {}
            Package::Type5(package) => {
                if self.state != State::Accepting {
                    bail!(MyErrorKind::InvalidState(State::Accepting, self.state));
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
                self.send_package(Package::Type8(Package8 {})).await?;

                Ok(())
            }
            Package::Type6(package) => {
                if package.version != 1 {
                    bail!(MyErrorKind::UserInputError);
                }
                if package.server_pin != SERVER_PIN {
                    bail!(MyErrorKind::UserInputError);
                }
                if self.state != State::Idle {
                    bail!(MyErrorKind::InvalidState(State::Idle, self.state));
                }

                self.state = State::Responding;

                self.extend(get_all_entries().await?).await;

                self.send_queue_entry().await?;

                Ok(())
            }
            Package::Type7(package) => {
                if package.version != 1 {
                    bail!(MyErrorKind::UserInputError);
                }
                if package.server_pin != SERVER_PIN {
                    bail!(MyErrorKind::UserInputError);
                }
                if self.state != State::Idle {
                    bail!(MyErrorKind::InvalidState(State::Idle, self.state));
                }

                self.state = State::Accepting;

                self.send_package(Package::Type8(Package8 {})).await?;

                Ok(())
            }
            Package::Type8(_package) => {
                if self.state != State::Responding {
                    bail!(MyErrorKind::InvalidState(State::Responding, self.state));
                }

                self.send_queue_entry().await?;

                Ok(())
            }
            Package::Type9(_package) => {
                if self.state != State::Accepting {
                    bail!(MyErrorKind::InvalidState(State::Accepting, self.state));
                }

                self.shutdown()?;

                Ok(())
            }
            Package::Type10(package) => {
                if package.version != 1 {
                    bail!(MyErrorKind::UserInputError);
                }
                if self.state != State::Idle {
                    bail!(MyErrorKind::InvalidState(State::Idle, self.state));
                }

                let entries = get_public_entries_by_pattern(&package.pattern)?;

                self.state = State::Responding;

                self.extend(entries).await;

                self.send_queue_entry().await?;

                Ok(())
            }
            Package::Type255(package) => Err(anyhow!("remote error: {:?}", package.message)),

            _ => Err(MyErrorKind::UserInputError.into()),
        }
    }
}
