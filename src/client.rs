use crate::{errors::MyErrorKind, models::*, packages::*, serde::serialize};
use anyhow::Context;
use std::io::Write;
use tokio::{net::TcpStream, prelude::*};

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
    send_queue: Vec<(DirectoryEntry, Option<u32>)>,
}

impl Client {
    pub fn new(socket: TcpStream) -> Self {
        Client {
            socket,
            mode: Mode::Unknown,
            state: State::Idle,
            send_queue: Vec::new(),
        }
    }

    pub async fn send_package(&mut self, package: Package) -> anyhow::Result<()> {
        println!("sending package: {:#?}", package);
        self.socket
            .write(serialize(package).as_slice())
            .await
            .context(MyErrorKind::FailedToWrite)?;

        Ok(())
    }

    pub fn shutdown(&mut self) -> std::result::Result<(), std::io::Error> {
        self.state = State::Shutdown;
        self.socket.shutdown(std::net::Shutdown::Both)
    }

    pub fn push_to_send_queue(&mut self, list: Vec<(DirectoryEntry, Option<u32>)>) {
        self.send_queue.extend(list);
    }

    pub fn push_entries_to_send_queue(&mut self, list: Vec<DirectoryEntry>) {
        self.send_queue.reserve(list.len());

        for entry in list {
            self.send_queue.push((entry, None));
        }
    }

    pub async fn send_queue_entry(&mut self) -> anyhow::Result<()> {
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

            self.send_package(Package::Type5(package.into())).await?;

            if let Some(queue_uid) = queue_uid {
                unimplemented!();
                // remove_queue_entry(&self.db_con, queue_uid);
            }
        } else {
            self.send_package(Package::Type9(PackageData9 {})).await?;
        }
        
        Ok(())
    }
}
