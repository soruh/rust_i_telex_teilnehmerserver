use std::env;


use diesel::prelude::*;
use diesel::{insert_into, update};

use crate::models::*;

pub fn establish_connection() -> MysqlConnection {
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    MysqlConnection::establish(&database_url).expect(&format!(
        "Error connecting to database at: {}",
        database_url
    ))
}

pub fn get_entry_by_number(conn: &MysqlConnection, by_number: u32) -> Option<DirectoryEntry> {
    use crate::schema::directory::dsl::*;
    directory
        .filter(number.eq(by_number))
        .first::<DirectoryEntry>(conn)
        .optional()
        .unwrap()
}


pub fn create_entry(conn: &MysqlConnection, entry: &DirectoryEntry) -> bool {
    use crate::schema::directory::dsl::*;

    let affected_rows = insert_into(directory).values(entry).execute(conn).unwrap();

    affected_rows == 1
}


pub fn update_entry(conn: &MysqlConnection, entry: &DirectoryEntry) -> bool {
    use crate::schema::directory::dsl::*;

    let affected_rows = update(directory).set(entry).execute(conn).unwrap();

    affected_rows == 1
}

pub fn get_changed_entries(conn: &MysqlConnection) -> Vec<DirectoryEntry> {
    use crate::schema::directory::dsl::*;
    directory
        .filter(changed.eq(true))
        .get_results(conn)
        .unwrap()
}


pub fn update_queue(conn: &MysqlConnection) -> Vec<usize> {
    unimplemented!();
    use crate::schema::directory::dsl::*;
    // SELECT d.uid FROM directory AS d WHERE changed=1;
    // SELECT s.uid FROM servers AS s;
    // INSERT INTO queue (server, message, timestamp) VALUES (s.uid, d.uid, [timestamp]);
}
