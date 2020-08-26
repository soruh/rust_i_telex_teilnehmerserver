use super::{FULL_QUERY_VERSION, LOGIN_VERSION, PEER_SEARCH_VERSION};
use crate::{db::*, errors::ItelexServerErrorKind, Entries, CONFIG};
use anyhow::Context;
use futures::{future::FutureExt, select, stream::StreamExt};
use itelex::{server::*, Package};
use std::net::{IpAddr, SocketAddr};
use tokio::{io::BufReader, net::TcpStream, prelude::*};

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
    pub send_queue: Entries,
}

impl Drop for Client {
    fn drop(&mut self) {
        debug!("dropping client at {}", self.address);

        let _ = self.shutdown();
    }
}

impl Client {
    pub const fn new(socket: TcpStream, address: SocketAddr) -> Self {
        Self { socket, address, mode: Mode::Unknown, state: State::Idle, send_queue: Vec::new() }
    }

    pub async fn handle(&mut self) -> anyhow::Result<()> {
        info!("handling client at: {}", self.address);

        #[allow(clippy::mut_mut, clippy::unnecessary_mut_passed)]
        {
            select! {
                _ = tokio::time::delay_for(config!(CLIENT_TIMEOUT)).fuse() => {
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
                        _ = tokio::time::delay_for(config!(CLIENT_TIMEOUT)).fuse() => {
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

    pub async fn send_package<P>(&mut self, package: P) -> anyhow::Result<()>
    where
        P: Into<Package<Server>> + Send,
    {
        let package: Package<Server> = package.into();

        debug!("sending package: {:#?}", package);

        let mut package_buffer = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut package_buffer);

        package.serialize(&mut cursor)?;

        debug!("sending package buffer: {:?}", package_buffer);

        self.socket
            .write_all(package_buffer.as_slice())
            .await
            .context(ItelexServerErrorKind::FailedToWrite)?;

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
            if let Err(err) = self.send_package(package.clone()).await {
                self.send_queue.push(package);

                return Err(err);
            }
        } else {
            self.send_package(EndOfList {}).await?;

            self.shutdown()?; // TODO: check if this is correct (it should be)
        }

        Ok(())
    }

    pub async fn peek_client_type(&mut self) -> anyhow::Result<()> {
        assert_eq!(self.mode, Mode::Unknown);

        let mut buf = [0_u8; 1];

        let len = self
            .socket
            .peek(&mut buf)
            .await
            .context(ItelexServerErrorKind::ConnectionCloseUnexpected)?; // read the first byte
        if len == 0 {
            bail!(ItelexServerErrorKind::ConnectionCloseUnexpected);
        }

        let [first_byte] = buf;

        debug!("first byte: {:#04x}", first_byte);

        self.mode = if first_byte >= 32 && first_byte <= 126 { Mode::Ascii } else { Mode::Binary };

        Ok(())
    }

    pub async fn consume_package(&mut self) -> anyhow::Result<()> {
        assert_ne!(self.mode, Mode::Unknown);

        if self.mode == Mode::Binary {
            self.consume_package_binary().await
        } else {
            self.consume_package_ascii().await
        }
    }

    pub async fn consume_package_ascii(&mut self) -> anyhow::Result<()> {
        let mut lines = BufReader::new(&mut self.socket).lines();

        let line = lines
            .next()
            .await
            .context(ItelexServerErrorKind::UserInputError)?
            .context(ItelexServerErrorKind::ConnectionCloseUnexpected)?;

        debug!("full line: {}", line);

        if line.is_empty() {
            bail!(ItelexServerErrorKind::UserInputError);
        }

        if line.chars().next().context(ItelexServerErrorKind::UserInputError)? == 'q' {
            let mut number = String::new();

            for character in line.chars().skip(1) {
                if !character.is_digit(10) {
                    break; // number is over
                }

                number.push(character);
            }

            debug!("handling 'q' request");

            let number =
                number.as_str().parse::<u32>().context(ItelexServerErrorKind::UserInputError)?;

            debug!("parsed number: '{}'", number);

            let message = if let Some(entry) = get_public_entry_by_number(number) {
                let address = if let Some(hostname) = entry.hostname() {
                    String::from(hostname)
                } else {
                    let ipaddress = entry.ipaddress().context(
                        "database is incosistent: entry has neither hostname nor ipaddress",
                    )?;

                    format!("{}", ipaddress)
                };

                format!(
                    "ok\r\n{}\r\n{}\r\n{}\r\n{}\r\n{}\r\n{}\r\n+++\r\n",
                    entry.number,
                    entry.name.0,
                    entry.client_type,
                    address,
                    entry.port,
                    entry
                        .extension_as_str()
                        .map_err(|ext| anyhow!(format!("ivalid extension: {}", ext)))?,
                )
            } else {
                format!("fail\r\n{}\r\nunknown\r\n+++\r\n", number)
            };

            self.socket
                .write_all(message.as_bytes())
                .await
                .context(ItelexServerErrorKind::FailedToWrite)?
        } else {
            bail!(ItelexServerErrorKind::UserInputError);
        }

        self.shutdown()?;

        Ok(())
    }

    pub async fn consume_package_binary(&mut self) -> anyhow::Result<()> {
        let mut header = [0_u8; 2];

        self.socket
            .peek(&mut header)
            .await
            .context(ItelexServerErrorKind::ConnectionCloseUnexpected)?;

        // debug!("header: {:?}", header);
        let [package_type, package_length] = header;

        debug!("reading package of type: {} with length: {}", package_type, package_length);

        let mut buffer = vec![0_u8; package_length as usize + 2];

        self.socket
            .read_exact(&mut buffer)
            .await
            .context(ItelexServerErrorKind::ConnectionCloseUnexpected)?;

        let package = Package::<Server>::deserialize(&mut std::io::Cursor::new(buffer))?;

        debug!("received package: {:#?}", package);

        self.handle_package(package).await?;

        Ok(())
    }

    pub async fn handle_package(&mut self, package: Package<Server>) -> anyhow::Result<()> {
        debug!("state: '{:?}'", self.state);

        match package.package_type() {
            Server::ClientUpdate => {
                let package = package.downcast::<ClientUpdate>().unwrap();
                if self.state != State::Idle {
                    bail!(ItelexServerErrorKind::InvalidState(State::Idle, self.state));
                }

                let ipaddress = match self.address.ip() {
                    IpAddr::V4(ipaddress) => ipaddress,

                    // Note: Ipv6 addresses can't be handled by the itelex system
                    _ => bail!(ItelexServerErrorKind::Ipv6Address),
                };

                update_or_register_entry(*package, ipaddress)?;
                self.send_package(AddressConfirm { ipaddress }).await?;

                Ok(())
            }
            // Server::AddressConfirm => { let package =
            // package.downcast::<AddressConfirm>().unwrap(); }
            Server::PeerQuery => {
                let package = package.downcast::<PeerQuery>().unwrap();

                if self.state != State::Idle {
                    bail!(ItelexServerErrorKind::InvalidState(State::Idle, self.state));
                }

                if let Some(entry) = get_public_entry_by_number(package.number) {
                    self.send_package(entry).await?;
                } else {
                    self.send_package(PeerNotFound {}).await?;
                }

                Ok(())
            }
            // Server::PeerNotFound => { let package = package.downcast::<PeerNotFound>().unwrap();
            // }
            Server::PeerReply => {
                let package = package.downcast::<PeerReply>().unwrap();
                if self.state != State::Accepting {
                    bail!(ItelexServerErrorKind::InvalidState(State::Accepting, self.state));
                }

                update_entry_if_newer(package);

                self.send_package(Acknowledge {}).await?;

                Ok(())
            }
            Server::FullQuery => {
                let package = package.downcast::<FullQuery>().unwrap();
                if package.version != FULL_QUERY_VERSION {
                    bail!(ItelexServerErrorKind::UserInputError);
                }

                if package.server_pin != config!(SERVER_PIN) {
                    bail!(ItelexServerErrorKind::PasswordError);
                }

                if self.state != State::Idle {
                    bail!(ItelexServerErrorKind::InvalidState(State::Idle, self.state));
                }

                self.state = State::Responding;

                self.send_queue.extend(get_all_entries());

                self.send_queue_entry().await?;

                Ok(())
            }
            Server::Login => {
                let package = package.downcast::<Login>().unwrap();
                if package.version != LOGIN_VERSION {
                    bail!(ItelexServerErrorKind::UserInputError);
                }

                if package.server_pin != config!(SERVER_PIN) {
                    bail!(ItelexServerErrorKind::PasswordError);
                }

                if self.state != State::Idle {
                    bail!(ItelexServerErrorKind::InvalidState(State::Idle, self.state));
                }

                warn!("receiving update from server {}", self.address);

                self.state = State::Accepting;

                self.send_package(Acknowledge {}).await?;

                Ok(())
            }
            Server::Acknowledge => {
                // let package = package.downcast::<Acknowledge>().unwrap();
                if self.state != State::Responding {
                    bail!(ItelexServerErrorKind::InvalidState(State::Responding, self.state));
                }

                self.send_queue_entry().await?;

                Ok(())
            }
            Server::EndOfList => {
                // let package = package.downcast::<EndOfList>().unwrap();
                if self.state != State::Accepting {
                    bail!(ItelexServerErrorKind::InvalidState(State::Accepting, self.state));
                }

                self.shutdown()?;

                Ok(())
            }
            Server::PeerSearch => {
                let package = package.downcast::<PeerSearch>().unwrap();
                if package.version != PEER_SEARCH_VERSION {
                    bail!(ItelexServerErrorKind::UserInputError);
                }

                if self.state != State::Idle {
                    bail!(ItelexServerErrorKind::InvalidState(State::Idle, self.state));
                }

                let entries = get_public_entries_by_pattern(&package.pattern);

                self.state = State::Responding;

                self.send_queue.extend(entries);

                self.send_queue_entry().await?;

                Ok(())
            }
            Server::Error => {
                let package = package.downcast::<Error>().unwrap();
                Err(anyhow!("remote error: {:?}", package.message))
            }

            _ => Err(ItelexServerErrorKind::UserInputError.into()),
        }
    }
}
