use crate::models::*;
use rusqlite::{Connection, NO_PARAMS};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub fn create_tables(conn: &Connection) {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS queue (
            uid BIGINT unsigned AUTO_INCREMENT PRIMARY KEY,
            server INTEGER unsigned NOT NULL,
            message INTEGER unsigned NOT NULL,
            timestamp INT unsigned NOT NULL
        );",
        NO_PARAMS,
    )
    .expect("failed to create 'queue' table");

    conn.execute(
        "CREATE TABLE IF NOT EXISTS servers (
            uid BIGINT unsigned AUTO_INCREMENT PRIMARY KEY,
            address VARCHAR(40) NOT NULL,
            version TINYINT unsigned NOT NULL,
            port SMALLINT unsigned NOT NULL
        );",
        NO_PARAMS,
    )
    .expect("failed to create 'servers' table");

    conn.execute(
        "CREATE TABLE IF NOT EXISTS directory (
            uid BIGINT unsigned AUTO_INCREMENT PRIMARY KEY,
            number int unsigned NOT NULL UNIQUE,
            name VARCHAR(40) NOT NULL,
            connection_type TINYINT unsigned NOT NULL,
            hostname VARCHAR(40),
            ipaddress INT unsigned,
            port SMALLINT unsigned NOT NULL,
            extension SMALLINT unsigned NOT NULL,
            pin SMALLINT unsigned NOT NULL,
            disabled BOOLEAN NOT NULL,
            timestamp INT unsigned NOT NULL,
            changed BOOLEAN NOT NULL
        );",
        NO_PARAMS,
    )
    .expect("failed to create 'directory' table");
}

pub fn get_entries<'a, P>(conn: &'a Connection, condition: &str, params: P) -> Vec<DirectoryEntry>
where
    P: IntoIterator,
    P::Item: rusqlite::ToSql,
{
    let mut query = String::from("SELECT uid, number, name, connection_type, hostname, ipaddress, port, extension, pin, disabled, timestamp, changed FROM directory ");
    query.push_str(condition);
    query.push_str(";");

    let mut stmt = conn.prepare(query.as_ref()).unwrap();

    let entry_iter = stmt
        .query_map(params, |row| -> rusqlite::Result<DirectoryEntry> {
            Ok(DirectoryEntry {
                uid: row.get_unwrap(0),
                number: row.get_unwrap(1),
                name: row.get_unwrap(2),
                connection_type: row.get_unwrap(3),
                hostname: row.get_unwrap(4),
                ipaddress: row.get_unwrap(5),
                port: row.get_unwrap(6),
                extension: row.get_unwrap(7),
                pin: row.get_unwrap(8),
                disabled: row.get_unwrap(9),
                timestamp: row.get_unwrap(10),
                changed: row.get_unwrap(11),
            })
        })
        .unwrap();

    let mut entries = Vec::new();

    for entry in entry_iter {
        entries.push(entry.unwrap());
    }

    println!("got entries from db: {:#?}", entries);

    entries
}

pub fn get_entry_by_number(conn: &Connection, number: u32) -> Option<DirectoryEntry> {
    get_entries(&conn, "WHERE number=?", params!(number)).pop()
}

pub fn get_all_entries(conn: &Connection) -> Vec<DirectoryEntry> {
    get_entries(&conn, "", NO_PARAMS)
}

pub fn create_entry(conn: &Connection, entry: &DirectoryEntry) -> bool {
    conn.execute(
        "INSERT INTO directory (
            uid,
            number,
            name,
            connection_type,
            hostname,
            ipaddress,
            port,
            extension,
            pin,
            disabled,
            timestamp,
            changed
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);",
        params![
            entry.uid,
            entry.number,
            entry.name,
            entry.connection_type,
            entry.hostname,
            entry.ipaddress,
            entry.port,
            entry.extension,
            entry.pin,
            entry.disabled,
            entry.timestamp,
            entry.changed,
        ],
    )
    .unwrap()
        > 0
}

pub fn get_changed_entries(conn: &Connection) -> Vec<DirectoryEntry> {
    get_entries(conn, "WHERE changed=1", NO_PARAMS)
}

pub fn update_queue(_conn: &Connection) -> Vec<usize> {
    unimplemented!("update_queue");
    // SELECT d.uid FROM directory AS d WHERE changed=1;
    // SELECT s.uid FROM servers AS s;
    // INSERT INTO queue (server, message, timestamp) VALUES (s.uid, d.uid, [timestamp]);
}

pub fn register_entry(conn: &Connection, number: u32, pin: u16, port: u16, ipaddress: u32) -> bool {
    conn.execute(
        "INSERT INTO directory (name, timestamp, changed, connection_type, extension, disabled, number, pin, port, ipaddress) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?);",
        params!["?", get_current_itelex_timestamp(), 1, 5, 0, 1, number, pin, port, ipaddress],
    )
    .unwrap()
        > 0
}

pub fn update_entry_address(conn: &Connection, port: u16, ipaddress: u32, number: u32) -> bool {
    conn.execute(
        "UPDATE directory SET
            port=?,
            ipaddress=?,
        WHERE
            number=?,
        ;",
        params![port, ipaddress, number],
    )
    .unwrap()
        > 0
}

lazy_static! {
    static ref ITELEX_EPOCH: SystemTime = UNIX_EPOCH
        .checked_sub(Duration::from_secs(60 * 60 * 24 * 365 * 70))
        .unwrap();
}

fn get_current_itelex_timestamp() -> u32 {
    SystemTime::now()
        .duration_since(*ITELEX_EPOCH)
        .unwrap()
        .as_secs() as u32
}

pub fn upsert_entry(
    conn: &Connection,
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
) -> bool {
    let timestamp: rusqlite::Result<u32> = conn.query_row(
        "SELECT timestamp FROM directory WHERE number=?;",
        params!(number),
        |row| row.get(0),
    );

    let params = params![
        number,
        name,
        connection_type,
        hostname,
        ipaddress,
        port,
        extension,
        pin,
        disabled,
        new_timestamp,
    ];

    println!(
        "old_timestamp={:?} current_timestamp={}",
        timestamp, new_timestamp
    );

    if let Ok(old_timestamp) = timestamp {
        if old_timestamp < new_timestamp {
            conn.execute(
                "UPDATE directory SET
                changed=1,
                number=?,
                name=?,
                connection_type=?,
                hostname=?,
                ipaddress=?,
                port=?,
                extension=?,
                pin=?,
                disabled=?,
                timestamp=?
            ;",
                params,
            )
            .unwrap()
                > 0
        } else {
            false
        }
    } else {
        conn.execute(
            "INSERT INTO directory (
                changed,
                number,
                name,
                connection_type,
                hostname,
                ipaddress,
                port,
                extension,
                pin,
                disabled,
                timestamp
            ) VALUES (1, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);",
            params,
        )
        .unwrap()
            > 0
    }
}

pub fn get_entries_by_pattern(conn: &Connection, pattern: String) -> Vec<DirectoryEntry> {
    let mut condition = String::from("");

    let mut params = Vec::new();
    for (i, word) in pattern.split_ascii_whitespace().enumerate() {
        println!("i: {}, word: {:?}", i, word);
        if i == 0 {
            condition.push_str("WHERE name LIKE ")
        } else {
            condition.push_str(" OR LIKE ")
        }
        condition.push_str("?");

        params.push(word);
    }

    println!("pattern: {:?}, params: {:?}", condition, params);

    get_entries(&conn, condition.as_ref(), &params)
}

pub fn get_queue(addr: std::net::SocketAddr) -> Vec<DirectoryEntry> {
    let identifier = format!("{}:{}", addr.ip(), addr.port());

    println!("identifier={:?}", identifier);

    unimplemented!();
}
