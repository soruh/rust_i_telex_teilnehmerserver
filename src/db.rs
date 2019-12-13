use crate::{
    errors::{DbError, MyErrorKind},
    models::*,
};
use anyhow::Context;
use std::net::SocketAddr;

pub use crate::db_backend;

pub fn get_all_entries() -> Vec<DirectoryEntry> {
    unimplemented!()
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

pub fn get_queue_for_server(server_uid: u32) -> Vec<(DirectoryEntry, Option<u32>)> {
    unimplemented!()
}

pub fn get_server_address_for_uid(server_uid: u32) -> SocketAddr {
    unimplemented!()
}

pub fn remove_queue_entry(queue_uid: u32) {
    unimplemented!()
}

pub fn get_server_uids() -> Vec<u32> {
    unimplemented!()
}

pub fn get_changed_entry_uids() -> Vec<u32> {
    unimplemented!()
}

pub fn update_queue() -> anyhow::Result<()> {
    unimplemented!()
}

pub fn prune_old_queue_entries() -> anyhow::Result<()> {
    unimplemented!()
}

pub fn get_public_entries_by_pattern(pattern: &str) -> Vec<DirectoryEntry> {
    unimplemented!()
}

pub fn get_entry_by_number(number: u32, truncate_privates: bool) -> Option<DirectoryEntry> {
    unimplemented!()
}
