use crate::{
    db::*,
    errors::ItelexServerErrorKind,
    packages::*,
    serde::{deserialize, serialize},
    CLIENT_TIMEOUT, FULL_QUERY_VERSION, LOGIN_VERSION, PEER_SEARCH_VERSION, SERVER_PIN,
};
use anyhow::Context;
use async_std::{io::BufReader, net::TcpStream, prelude::*, task};
use futures::{future::FutureExt, select, stream::StreamExt};
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
    pub address: SocketAddr,
    pub mode: Mode,
    pub state: State,
    pub send_queue: Vec<Package5>,
}

impl Drop for Client {
    fn drop(&mut self) {
        debug!("dropping client at {}", self.address);

        let _ = self.shutdown();
    }
}

impl Client {
    pub fn new(socket: TcpStream, address: SocketAddr) -> Self {
        Self { socket, address, mode: Mode::Unknown, state: State::Idle, send_queue: Vec::new() }
    }

    pub async fn handle(&mut self) -> anyhow::Result<()> {
        info!("handling client at: {}", self.address);

        #[allow(clippy::mut_mut, clippy::unnecessary_mut_passed)]
        {
            select! {
                _ = task::sleep(CLIENT_TIMEOUT).fuse() => {
                    bail!(ItelexServerErrorKind::Timeout);
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
                        _ = task::sleep(CLIENT_TIMEOUT).fuse() => {
                            Err(ItelexServerErrorKind::Timeout)?;
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

    pub async fn send_package(&mut self, package: Package) -> anyhow::Result<()> {
        debug!("sending package: {:#?}", package);

        let package_type = package.package_type();

        let body = serialize(package.try_into()?)?;

        let package_length = body.len() as u8;

        let header = [package_type, package_length];

        self.socket.write_all(&header).await.context(ItelexServerErrorKind::FailedToWrite)?;

        self.socket.write_all(body.as_slice()).await.context(ItelexServerErrorKind::FailedToWrite)?;

        Ok(())
    }

    pub fn shutdown(&mut self) -> std::result::Result<(), std::io::Error> {
        if self.state == State::Shutdown {
            debug!("tried to shut down client that was already shut down");

            return Ok(());
        }

        debug!("shutting down client at {}", self.address);

        self.state = State::Shutdown;

        self.socket.shutdown(std::net::Shutdown::Both)?;

        Ok(())
    }

    pub async fn send_queue_entry(&mut self) -> anyhow::Result<()> {
        if self.state != State::Responding {
            bail!(ItelexServerErrorKind::InvalidState(State::Responding, self.state));
        }

        if let Some(package) = self.send_queue.pop() {
            if let Err(err) = self.send_package(Package::Type5(package.clone())).await {
                self.send_queue.push(package);

                return Err(err);
            }
        } else {
            self.send_package(Package::Type9(Package9 {})).await?;

            self.shutdown()?; // TODO: check if this is correct (it should be)
        }

        Ok(())
    }

    pub async fn peek_client_type(self: &mut Client) -> anyhow::Result<()> {
        assert_eq!(self.mode, Mode::Unknown);

        let mut buf = [0_u8; 1];

        let len = self.socket.peek(&mut buf).await.context(ItelexServerErrorKind::ConnectionCloseUnexpected)?; // read the first byte
        if len == 0 {
            bail!(ItelexServerErrorKind::ConnectionCloseUnexpected);
        }

        let [first_byte] = buf;

        debug!("first byte: {:#04x}", first_byte);

        self.mode = if first_byte >= 32 && first_byte <= 126 { Mode::Ascii } else { Mode::Binary };

        Ok(())
    }

    pub async fn consume_package(self: &mut Client) -> anyhow::Result<()> {
        assert_ne!(self.mode, Mode::Unknown);

        if self.mode == Mode::Binary { self.consume_package_binary().await } else { self.consume_package_ascii().await }
    }

    pub async fn consume_package_ascii(self: &mut Client) -> anyhow::Result<()> {
        let mut lines = BufReader::new(&self.socket).lines();

        let line = lines
            .next()
            .await
            .context(ItelexServerErrorKind::UserInputError)?
            .context(ItelexServerErrorKind::ConnectionCloseUnexpected)?;

        debug!("full line: {}", line);

        if line.is_empty() {
            bail!(ItelexServerErrorKind::UserInputError);
        }

        if line.chars().nth(0).context(ItelexServerErrorKind::UserInputError)? == 'q' {
            let mut number = String::new();

            for character in line.chars().skip(1) {
                let char_code = character as u8;

                if char_code < 48 || char_code > 57 {
                    break; // number is over
                }

                number.push(character);
            }

            debug!("handling 'q' request");

            let number = number.as_str().parse::<u32>().context(ItelexServerErrorKind::UserInputError)?;

            debug!("parsed number: '{}'", number);

            let message = if let Some(entry) = get_public_entry_by_number(number).await {
                let host_or_ip = if let Some(hostname) = entry.hostname {
                    hostname
                } else {
                    let ipaddress =
                        entry.ipaddress.expect("database is incosistent: entry has neither hostname nor ipaddress");

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

            self.socket.write_all(message.as_bytes()).await.context(ItelexServerErrorKind::FailedToWrite)?
        } else {
            bail!(ItelexServerErrorKind::UserInputError);
        }

        self.shutdown()?;

        Ok(())
    }

    pub async fn consume_package_binary(self: &mut Client) -> anyhow::Result<()> {
        let mut header = [0_u8; 2];

        self.socket.read_exact(&mut header).await.context(ItelexServerErrorKind::ConnectionCloseUnexpected)?;

        // debug!("header: {:?}", header);
        let [package_type, package_length] = header;

        debug!("reading package of type: {} with length: {}", package_type, package_length);

        let mut body = vec![0_u8; package_length as usize];

        self.socket.read_exact(&mut body).await.context(ItelexServerErrorKind::ConnectionCloseUnexpected)?;

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
                    bail!(ItelexServerErrorKind::InvalidState(State::Idle, self.state));
                }

                let ipaddress = match self.address.ip() {
                    IpAddr::V4(ipaddress) => ipaddress,

                    // Note: Ipv6 addresses can't be handled by the itelex system
                    _ => bail!(ItelexServerErrorKind::Ipv6Address),
                };

                update_or_register_entry(package, ipaddress).await?;

                self.send_package(Package::Type2(Package2 { ipaddress })).await?;

                Ok(())
            }
            // Package::Type2(package) => {}
            Package::Type3(package) => {
                if self.state != State::Idle {
                    bail!(ItelexServerErrorKind::InvalidState(State::Idle, self.state));
                }

                if let Some(entry) = get_public_entry_by_number(package.number).await {
                    self.send_package(Package::Type5(entry)).await?;
                } else {
                    self.send_package(Package::Type4(Package4 {})).await?;
                }

                Ok(())
            }
            // Package::Type4(_package) => {}
            Package::Type5(package) => {
                if self.state != State::Accepting {
                    bail!(ItelexServerErrorKind::InvalidState(State::Accepting, self.state));
                }

                update_entry_if_newer(package).await;

                self.send_package(Package::Type8(Package8 {})).await?;

                Ok(())
            }
            Package::Type6(package) => {
                if package.version != FULL_QUERY_VERSION {
                    bail!(ItelexServerErrorKind::UserInputError);
                }

                if package.server_pin != SERVER_PIN {
                    bail!(ItelexServerErrorKind::PasswordError);
                }

                if self.state != State::Idle {
                    bail!(ItelexServerErrorKind::InvalidState(State::Idle, self.state));
                }

                self.state = State::Responding;

                self.send_queue.extend(get_all_entries().await);

                self.send_queue_entry().await?;

                Ok(())
            }
            Package::Type7(package) => {
                if package.version != LOGIN_VERSION {
                    bail!(ItelexServerErrorKind::UserInputError);
                }

                if package.server_pin != SERVER_PIN {
                    bail!(ItelexServerErrorKind::PasswordError);
                }

                if self.state != State::Idle {
                    bail!(ItelexServerErrorKind::InvalidState(State::Idle, self.state));
                }

                warn!("receiving update from server {}", self.address);

                self.state = State::Accepting;

                self.send_package(Package::Type8(Package8 {})).await?;

                Ok(())
            }
            Package::Type8(_package) => {
                if self.state != State::Responding {
                    bail!(ItelexServerErrorKind::InvalidState(State::Responding, self.state));
                }

                self.send_queue_entry().await?;

                Ok(())
            }
            Package::Type9(_package) => {
                if self.state != State::Accepting {
                    bail!(ItelexServerErrorKind::InvalidState(State::Accepting, self.state));
                }

                self.shutdown()?;

                Ok(())
            }
            Package::Type10(package) => {
                if package.version != PEER_SEARCH_VERSION {
                    bail!(ItelexServerErrorKind::UserInputError);
                }

                if self.state != State::Idle {
                    bail!(ItelexServerErrorKind::InvalidState(State::Idle, self.state));
                }

                let entries = get_public_entries_by_pattern(&package.pattern).await;

                self.state = State::Responding;

                self.send_queue.extend(entries);

                self.send_queue_entry().await?;

                Ok(())
            }
            Package::Type255(package) => Err(anyhow!("remote error: {:?}", package.message)),

            _ => Err(ItelexServerErrorKind::UserInputError.into()),
        }
    }
}
