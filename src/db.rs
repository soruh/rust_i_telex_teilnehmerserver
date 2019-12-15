use crate::{
    errors::MyErrorKind,
    models::*,
};
use anyhow::Context;
use std::net::SocketAddr;

use crate::{DIRECTORY, QUEUE, SERVERS};
pub use crate::db_backend;

macro_rules! get {
    ($e: expr) => {
        $e.lock().unwrap().get()
    }
}

pub async fn get_all_entries() -> Vec<DirectoryEntry> {
    let mut directory = get!(DIRECTORY);

    directory.get_all().await
}

pub fn create_entry(entry: &DirectoryEntry) -> anyhow::Result<anyhow::Result<Result<(), ()>>> {
    unimplemented!()
}

pub fn register_entry(
    number: u32,
    pin: u16,
    port: u16,
    ipaddress: u32,
    overwrite: bool,
) -> anyhow::Result<Result<(), ()>> {
    unimplemented!()
}

pub fn update_entry_address(
    port: u16,
    ipaddress: u32,
    number: u32,
) -> anyhow::Result<Result<(), ()>> {
    unimplemented!()
}

pub fn upsert_entry(
    number: u32,
    name: String,
    connection_type: u8,
    hostname: Option<String>,
    ipaddress: Option<u32>,
    port: u16,
    extension: u8,
    pin: u16,
    disabled: bool,
    new_timestamp: u32,
) -> anyhow::Result<Result<(), ()>> {
    unimplemented!()
}

pub fn get_queue_for_server(server_uid: Uid) -> Vec<(DirectoryEntry, Option<u32>)> {
    unimplemented!()
}

pub fn get_server_address_for_uid(server_uid: Uid) -> SocketAddr {
    unimplemented!()
}

pub fn remove_queue_entry(queue_uid: Uid) {
    unimplemented!()
}

pub async fn get_server_uids() -> Vec<Uid> {
    let mut servers = get!(SERVERS);
    servers
        .get_all_with_uid()
        .await
        .into_iter()
        .map(|(uid, _)| uid)
        .collect()
}

pub fn get_changed_entry_uids() -> Vec<u32> {
    unimplemented!()
}

pub async fn update_queue() -> anyhow::Result<()> {
    let mut queue = get!(QUEUE);
    let mut directory = get!(DIRECTORY);

    let servers = get_server_uids().await;


    let changed_entry_uids: Vec<Uid> = directory
        .get_all_with_uid()
        .await
        .into_iter()
        .filter(|(_, entry)| entry.changed)
        .map(|(uid, _)| uid)
        .collect();



    for server in servers.iter() {
        for entry_uid in &changed_entry_uids {
            queue.push(QueueEntry {
                server: *server as u32,
                message: *entry_uid as u32,
                timestamp: get_unix_timestamp(),
            }).await;
        }
    }

    Ok(()) //TODO
}

fn get_unix_timestamp() -> u32 {
    (SystemTime::now())
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as u32
}

use std::time::{SystemTime, UNIX_EPOCH};
use db_backend::Uid;

pub async fn prune_old_queue_entries() -> anyhow::Result<()> {
    let mut queue: db_backend::Sender<QueueEntry> = get!(QUEUE);

    let timestamp_one_month_ago = get_unix_timestamp() - 31 * 24 * 60 * 60;

    let all_entries = queue.get_all_with_uid().await;

    let uids_to_delete: Vec<Uid> = all_entries
        .into_iter()
        .filter(|(_, entry)| entry.timestamp < timestamp_one_month_ago)
        .map(|(uid, _)| uid)
        .collect();

    for uid in uids_to_delete {
        queue.delete_uid(uid).await;
    }

    Ok(())
}

pub fn get_public_entries_by_pattern(pattern: &str) -> Vec<DirectoryEntry> {
    unimplemented!()
}

pub fn get_entry_by_number(number: u32, truncate_privates: bool) -> Option<DirectoryEntry> {
    unimplemented!()
}
