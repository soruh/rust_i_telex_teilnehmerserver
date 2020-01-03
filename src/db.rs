use crate::packages::*;
use crate::errors::MyErrorKind;
use async_std::sync::{Arc, RwLock};
use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};

lazy_static! {
    pub static ref DB: Arc<RwLock<HashMap<u32, Package5>>> = Arc::new(RwLock::new(HashMap::new()));
}

pub async fn sync_db_to_disk() {}

pub async fn get_changed_entries() -> Vec<&'static Package5> {
    let res = Vec::new();
    for entry in DB.write().await.iter_mut() {
        debug!("entry: {:?}", entry);
    }

    res
}

//

pub async fn get_all_entries() -> anyhow::Result<Vec<Package5>> {
    bail!(err_unimplemented!())
}

pub fn create_entry(_entry: &Package5) -> anyhow::Result<Result<(), ()>> {
    bail!(err_unimplemented!())
}

pub fn register_entry(
    _number: u32,
    _pin: u16,
    _port: u16,
    _ipaddress: u32,
    _overwrite: bool,
) -> anyhow::Result<Result<(), ()>> {
    bail!(err_unimplemented!())
}

pub fn update_entry_address(
    _port: u16,
    _ipaddress: u32,
    _number: u32,
) -> anyhow::Result<Result<(), ()>> {
    bail!(err_unimplemented!())
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
    bail!(err_unimplemented!())
}

#[must_use]
pub fn get_queue_for_server(_server_uid: u32) -> anyhow::Result<Vec<(Package5, Option<u32>)>> {
    bail!(err_unimplemented!())
}



pub async fn update_queue() -> anyhow::Result<()> {
    bail!(err_unimplemented!())
}

pub async fn prune_old_queue_entries() -> anyhow::Result<()> {
    bail!(err_unimplemented!())
}

#[must_use]
pub fn get_public_entries_by_pattern(_pattern: &str) -> anyhow::Result<Vec<Package5>> {
    bail!(err_unimplemented!())
}

#[must_use]
pub fn get_entry_by_number(_number: u32, _truncate_privates: bool) -> anyhow::Result<Option<Package5>> {
    bail!(err_unimplemented!())
}
