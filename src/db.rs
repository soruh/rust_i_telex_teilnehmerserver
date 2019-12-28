use super::packages::*;
use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, RwLock};

pub type Uid = u32;

lazy_static! {
    pub static ref DB: Arc<RwLock<HashMap<u32, ProcessedPackage5>>> =
        Arc::new(RwLock::new(HashMap::new()));
}

pub async fn sync_db_to_disk() {}

//

pub async fn get_all_entries() -> Vec<ProcessedPackage5> {
    unimplemented!()
}

pub fn create_entry(_entry: &ProcessedPackage5) -> anyhow::Result<anyhow::Result<Result<(), ()>>> {
    unimplemented!()
}

pub fn register_entry(
    _number: u32,
    _pin: u16,
    _port: u16,
    _ipaddress: u32,
    _overwrite: bool,
) -> anyhow::Result<Result<(), ()>> {
    unimplemented!()
}

pub fn update_entry_address(
    _port: u16,
    _ipaddress: u32,
    _number: u32,
) -> anyhow::Result<Result<(), ()>> {
    unimplemented!()
}

#[allow(clippy::needless_pass_by_value, clippy::too_many_arguments)]
pub fn upsert_entry(
    _number: u32,
    _name: String,
    _connection_type: u8,
    _hostname: Option<String>,
    _ipaddress: Option<Ipv4Addr>,
    _port: u16,
    _extension: u8,
    _pin: u16,
    _disabled: bool,
    _new_timestamp: u32,
) -> anyhow::Result<Result<(), ()>> {
    unimplemented!()
}

#[must_use]
pub fn get_queue_for_server(_server_uid: Uid) -> Vec<(ProcessedPackage5, Option<u32>)> {
    unimplemented!()
}

#[must_use]
pub fn get_server_address_for_uid(_server_uid: Uid) -> SocketAddr {
    unimplemented!()
}

pub fn remove_queue_entry(_queue_uid: Uid) {
    unimplemented!()
}

pub async fn get_server_uids() -> Vec<Uid> {
    unimplemented!()
}

#[must_use]
pub fn get_changed_entry_uids() -> Vec<u32> {
    unimplemented!()
}

pub async fn update_queue() -> anyhow::Result<()> {
    unimplemented!()
}

pub async fn prune_old_queue_entries() -> anyhow::Result<()> {
    unimplemented!()
}

#[must_use]
pub fn get_public_entries_by_pattern(_pattern: &str) -> Vec<ProcessedPackage5> {
    unimplemented!()
}

#[must_use]
pub fn get_entry_by_number(_number: u32, _truncate_privates: bool) -> Option<ProcessedPackage5> {
    unimplemented!()
}
