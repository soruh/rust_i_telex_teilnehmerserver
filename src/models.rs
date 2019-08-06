use crate::packages::PackageData5;

pub struct DirectoryEntry {
    pub uid: u64,
    pub number: u32,
    pub name: String,
    pub connection_type: u8,
    pub hostname: Option<String>,
    pub ipaddress: Option<u32>,
    pub port: u16,
    pub extension: u8,
    pub pin: u16,
    pub disabled: bool,
    pub timestamp: u32,
    pub changed: bool,
}

pub struct DirectoryEntryChange {
    pub number: u32,
    pub name: String,
    pub connection_type: u8,
    pub hostname: Option<String>,
    pub ipaddress: Option<u32>,
    pub port: u16,
    pub extension: u8,
    pub pin: u16,
    pub disabled: bool,
    pub timestamp: u32,
    pub changed: bool,
}

impl From<PackageData5> for DirectoryEntryChange {
    fn from(entry: PackageData5) -> Self {
        let hostname = entry.hostname.to_str().unwrap().to_owned();

        let hostname = if hostname.is_empty() {
            None
        } else {
            Some(hostname)
        };

        let ipaddress = u32::from(entry.ipaddress);
        let ipaddress: Option<u32> = if ipaddress == 0 {
            None
        } else {
            Some(ipaddress)
        };

        DirectoryEntryChange {
            number: entry.number,
            name: entry.name.to_str().unwrap().to_owned(),
            connection_type: entry.client_type,
            hostname,
            ipaddress,
            port: entry.port,
            extension: entry.extension,
            pin: entry.pin,
            disabled: (entry.flags & 0x02) != 0,
            timestamp: entry.date,
            changed: true,
        }
    }
}

pub struct QueueEntry {
    pub uid: u64,
    pub server: u32,
    pub message: u32,
    pub timestamp: u32,
}

pub struct ServersEntry {
    pub uid: u64,
    pub address: String,
    pub version: u8,
    pub port: u16,
}
