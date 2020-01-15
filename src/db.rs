use crate::{
    errors::ItelexServerErrorKind, get_current_itelex_timestamp, packages::*, Entries, Entry, CHANGED, CONFIG, DATABASE,
};
use async_std::{fs, io::prelude::*, sync::Mutex};
use once_cell::sync::Lazy;
use std::{convert::TryInto, net::Ipv4Addr};

static FS_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

pub async fn sync_db_to_disk() -> anyhow::Result<()> {
    use fs::{copy, remove_file, File};

    if config!(SERVER_PIN) == 0 {
        warn!("Refused to sync DB to disk, so that no important data is overwritten.");

        return Ok(());
    }

    let fs_lock = FS_LOCK.lock().await;

    info!("Syncing DB to disk");

    let mut temp_file: File = File::create(&config!(DB_PATH_TEMP)).await?;

    for item in DATABASE.iter() {
        let entry = item.value();
        let buffer: Vec<u8> = entry.clone().try_into()?;

        temp_file.write_all(buffer.as_slice()).await?;
    }

    temp_file.sync_all().await?;

    drop(temp_file);

    debug!("replacing database with temp file");

    copy(&config!(DB_PATH_TEMP), &config!(DB_PATH)).await?;

    remove_file(&config!(DB_PATH_TEMP)).await?;

    // NOTE: we do not use rename here to make sure we only delete the temp file
    // only gets deleted if we successfully copied it to the final file
    // TODO: find out if this is really neccessary
    drop(fs_lock);

    info!("Synced Database");

    Ok(())
}

pub async fn read_db_from_disk() -> anyhow::Result<()> {
    use fs::File;
    use std::{convert::TryFrom, path::Path};

    info!("Reading entries from disk");

    let db_path = Path::new(&config!(DB_PATH));

    if !db_path.exists() {
        warn!("The database could not be found on disk. It will be created on the next sync.");

        return Ok(());
        // We can't read a DB, because there is none, which is okay
    }

    let mut db_file: File = File::open(db_path).await?;

    let file_size = db_file.metadata().await?.len();

    if file_size % 100 != 0 {
        bail!(anyhow!("DB file has length that is not a multiple of 100; It may be corrupted!"));
    }

    let mut packages = Vec::new();

    loop {
        let mut buffer = [0u8; 100];

        let res = db_file.read_exact(&mut buffer).await;

        if let Err(err) = res {
            if err.kind() == async_std::io::ErrorKind::UnexpectedEof {
                break; // read until there are no byes left in the file
            }
        }

        packages.push(Package5::try_from(&buffer as &[u8])?);
    }

    if config!(SERVER_PIN) == 0 {
        warn!(
            "Removing pins from read DB entries and removing private ones as to not leak them, since we are running \
             without a SERVER_PIN"
        );

        let private_packages: Entries = packages.drain(..).collect();

        for mut package in private_packages.into_iter() {
            if !package.disabled {
                package.pin = 0;

                packages.push(package);
            }
        }
    }

    info!("Writing {} read entries to in memory DB", packages.len());

    {
        for package in packages {
            DATABASE.insert(package.number, package);
        }
    }

    info!("Finished reading DB");

    Ok(())
}

pub async fn get_changed_entries() -> Entries {
    let mut changed_entries: Entries = Vec::new();

    // TODO: convert to `drain` once dashmap supports it
    for item in CHANGED.iter() {
        let number = item.key();
        if let Some(entry) = DATABASE.get(number) {
            changed_entries.push(entry.clone());
        }
    }

    CHANGED.clear();

    debug!("changed entries: {:#?}", changed_entries);

    changed_entries
}

pub async fn get_all_entries() -> Entries {
    DATABASE.iter().map(|item| item.value().clone()).collect()
}

pub async fn update_or_register_entry(package: Package1, ipaddress: Ipv4Addr) -> anyhow::Result<()> {
    // Confirm that ipaddress is not unspecified, since this could lead to entries
    // with neither an ip nor a hostname
    if ipaddress.is_unspecified() {
        bail!(ItelexServerErrorKind::UserInputError);
    }

    let number = package.number;

    let new_entry = Package5 {
        client_type: 5,
        disabled: true,
        extension: 0,
        hostname: None,
        ipaddress: Some(ipaddress),
        name: "?".into(),
        number,
        pin: package.pin,
        port: package.port,
        timestamp: get_current_itelex_timestamp(),
    };

    {
        if let Some(mut existing) = DATABASE.get_mut(&number) {
            if existing.client_type == 0 {
                DATABASE.insert(number, new_entry);
            } else if existing.client_type == 5 {
                if existing.pin == 0 {
                    // NOTE: overwrite 0 pins.
                    existing.pin = package.pin;

                    warn!("overwrote a 0 pin.");
                }

                if package.pin == existing.pin {
                    existing.ipaddress = Some(ipaddress);
                    existing.timestamp = get_current_itelex_timestamp();
                } else {
                    bail!(ItelexServerErrorKind::PasswordError);
                }
            } else {
                bail!(ItelexServerErrorKind::InvalidClientType(existing.client_type, 5));
            }
        } else {
            DATABASE.insert(number, new_entry);
        }
    }

    CHANGED.insert(number, ());

    Ok(())
}

pub async fn update_entry(entry: Entry) {
    CHANGED.insert(entry.number, ());

    DATABASE.insert(entry.number, entry);
}

pub async fn update_entry_if_newer(entry: Entry) {
    let do_update =
        if let Some(existing) = DATABASE.get(&entry.number) { entry.timestamp > existing.timestamp } else { true };

    if do_update {
        // NOTE: we duplicate the code from above almost exactly here
        // to keep the db locked so that no other task can
        // change the entry we just checked
        CHANGED.insert(entry.number, ());

        DATABASE.insert(entry.number, entry);
    }
}

fn pattern_matches(words: &[&str], name: &str) -> bool {
    for word in words {
        if !name.contains(word) {
            return false;
        }
    }

    true
}

pub async fn get_public_entries() -> Entries {
    DATABASE
        .iter()
        .filter(|item| !(item.value().disabled || item.value().client_type == 0))
        .map(|item| {
            let mut entry = item.value().clone();
            entry.pin = 0;
            entry
        })
        .collect()
}

pub async fn get_public_entries_by_pattern(pattern: &str) -> Entries {
    let words: Vec<&str> = pattern.split(' ').collect();
    get_public_entries().await.into_iter().filter(|e| pattern_matches(&words, &e.name)).collect()
}

pub async fn get_entry_by_number(number: u32) -> Option<Entry> {
    DATABASE.get(&number).map(|item| item.value().clone())
}

pub async fn get_public_entry_by_number(number: u32) -> Option<Entry> {
    DATABASE.get(&number).filter(|e| !(e.disabled || e.client_type == 0)).map(|e| {
        let mut entry = e.clone();

        entry.pin = 0;

        entry
    })
}
