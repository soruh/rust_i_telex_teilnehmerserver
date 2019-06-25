use std::env;

use diesel::{insert_into, update};
use diesel::prelude::*;


use crate::models::*;

pub fn establish_connection() -> MysqlConnection {
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    MysqlConnection::establish(&database_url).expect(&format!(
        "Error connecting to database at: {}",
        database_url
    ))
}

pub fn get_entry(conn: &MysqlConnection, by_number: u32) -> Option<DirectoryEntry> {
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

    let affected_rows = update(directory)
        .set(entry)
        .execute(conn)
        .unwrap();

    affected_rows == 1
}
