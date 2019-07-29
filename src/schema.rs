table! {
    directory (uid) {
        uid -> Unsigned<Bigint>,
        number -> Unsigned<Integer>,
        name -> Varchar,
        connection_type -> Unsigned<Tinyint>,
        hostname -> Nullable<Varchar>,
        ipaddress -> Nullable<Unsigned<Integer>>,
        port -> Unsigned<Smallint>,
        extension -> Unsigned<Smallint>,
        pin -> Unsigned<Smallint>,
        disabled -> Bool,
        timestamp -> Unsigned<Integer>,
        changed -> Bool,
    }
}

table! {
    queue (uid) {
        uid -> Unsigned<Bigint>,
        server -> Unsigned<Integer>,
        message -> Unsigned<Integer>,
        timestamp -> Unsigned<Integer>,
    }
}

table! {
    servers (uid) {
        uid -> Unsigned<Bigint>,
        address -> Varchar,
        version -> Unsigned<Tinyint>,
        port -> Unsigned<Smallint>,
    }
}

allow_tables_to_appear_in_same_query!(
    directory,
    queue,
    servers,
);
