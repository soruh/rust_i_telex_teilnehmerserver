use std::env;



use crate::models::*;
use std::time::{SystemTime, UNIX_EPOCH};
use std::path::Path;






fn loop_until_available<F, T>(mut action: F) -> sqlite::Result<T>
where
    F: FnMut() -> sqlite::Result<T>,
{
    loop {
        let res = action();
        match res {
            Ok(value) => return Ok(value),
            Err(err) => {
                if err.message != Some("database is locked".into()) {
                    return Err(err);
                }
            }
        }

        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

fn execute<T: AsRef<str>>(conn: &sqlite::Connection, statement: T) -> sqlite::Result<()> {
    loop_until_available(|| conn.execute(&statement))
}

fn prepare<T: AsRef<str>>(
    conn: &sqlite::Connection,
    statement: T,
) -> sqlite::Result<sqlite::Statement> {
    loop_until_available(|| conn.prepare(&statement))
}

fn next(statement: &mut sqlite::Statement) -> sqlite::Result<sqlite::State> {
    loop_until_available(|| statement.next())
}


pub fn connect<T: AsRef<Path>>(path: T) -> sqlite::Connection {
    let open_flags = sqlite::OpenFlags::new()
        .set_create()
        .set_read_write()
        .set_no_mutex();
    let conn = sqlite::Connection::open_with_flags(path, open_flags).unwrap();
    conn
}


pub fn create_tables(conn: &sqlite::Connection) -> sqlite::Result<()> {
    execute(conn, "
    CREATE TABLE IF NOT EXISTS queue (
        uid BIGINT unsigned AUTO_INCREMENT PRIMARY KEY,
        server INTEGER unsigned NOT NULL,
        message INTEGER unsigned NOT NULL,
        timestamp INT unsigned NOT NULL
    );
    CREATE TABLE IF NOT EXISTS servers (
        uid BIGINT unsigned AUTO_INCREMENT PRIMARY KEY,
        address VARCHAR(40) NOT NULL,
        version TINYINT unsigned NOT NULL,
        port SMALLINT unsigned NOT NULL
    );
    CREATE TABLE IF NOT EXISTS directory (
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
    );
    ")
}



pub fn get_entry_by_number(conn: &sqlite::Connection, by_number: u32) -> Option<DirectoryEntry> {
    let statement = prepare(conn, "SELECT * FROM directory WHERE number=?;").unwrap();
    statement.bind(1, by_number);
}

pub fn get_all_entries(conn: &sqlite::Connection) -> Vec<DirectoryEntry> {
    directory.get_results(conn).unwrap()
}

pub fn create_entry(conn: &sqlite::Connection, entry: &DirectoryEntry) -> bool {

    let affected_rows = insert_into(directory).values(entry).execute(conn).unwrap();

    affected_rows == 1
}

pub fn update_entry(conn: &sqlite::Connection, entry: &DirectoryEntryChange) -> bool {

    let affected_rows = update(
        directory
            .filter(number.eq(entry.number))
            .filter(timestamp.lt(entry.timestamp)),
    )
    .set(entry)
    .execute(conn)
    .unwrap();

    affected_rows == 1
}

pub fn get_changed_entries(conn: &sqlite::Connection) -> Vec<DirectoryEntry> {
    directory
        .filter(changed.eq(true))
        .get_results(conn)
        .unwrap()
}

pub fn update_queue(conn: &sqlite::Connection) -> Vec<usize> {
    unimplemented!("update_queue");
    // SELECT d.uid FROM directory AS d WHERE changed=1;
    // SELECT s.uid FROM servers AS s;
    // INSERT INTO queue (server, message, timestamp) VALUES (s.uid, d.uid, [timestamp]);
}

fn get_unix_timestamp() -> u32 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as u32
}

pub fn register_entry(
    conn: &sqlite::Connection,
    arg_number: u32,
    arg_pin: u16,
    arg_port: u16,
    arg_ipaddress: u32,
) {
    insert_into(directory)
        .values((
            number.eq(arg_number),
            pin.eq(arg_pin),
            port.eq(arg_port),
            name.eq(String::from("?")),
            connection_type.eq(5),
            ipaddress.eq(arg_ipaddress),
            changed.eq(true),
            timestamp.eq(get_unix_timestamp()),
            disabled.eq(true),
            extension.eq(0),
        ))
        .execute(conn)
        .unwrap();
}

pub fn update_entry_public(
    conn: &sqlite::Connection,
    arg_number: u32,
    arg_port: u16,
    arg_ipaddress: u32,
) {
    update(directory.filter(number.eq(arg_number)))
        .set((port.eq(arg_port), ipaddress.eq(arg_ipaddress)))
        .execute(conn)
        .unwrap();
}
pub fn get_entries_by_pattern( conn: &sqlite::Connection, pattern: String) {
    unimplemented!();
    // select()
}
