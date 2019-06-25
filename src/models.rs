use crate::schema::{directory, queue, servers};

#[derive(QueryableByName, Queryable, Insertable, AsChangeset)]
#[table_name = "directory"]
pub struct DirectoryEntry {
    uid: u64,
    number: u32,
    name: String,
    connection_type: u8,
    hostname: Option<String>,
    ipaddress: Option<u32>,
    port: u16,
    extension: u16,
    pin: u16,
    disabled: bool,
    timestamp: u32,
    changed: bool,
}


#[derive(QueryableByName, Queryable, Insertable, AsChangeset)]
#[table_name = "queue"]
pub struct QueueEntry {
    uid: u64,
    server: u32,
    message: u32,
    timestamp: u32,
}

#[derive(QueryableByName, Queryable, Insertable, AsChangeset)]
#[table_name = "servers"]
pub struct ServersEntry {
    uid: u64,
    address: String,
    version: u8,
    port: u16,
}
