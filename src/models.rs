use crate::packages::PackageData5;

#[derive(Debug, Clone)]
pub struct DirectoryEntry {
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

pub use std::ffi::{CStr, CString};

impl Into<PackageData5> for DirectoryEntry {
    fn into(self) -> PackageData5 {
        let hostname = if let Some(hostname) = self.hostname {
            CString::new(hostname).unwrap()
        } else {
            CString::new("").unwrap()
        };

        let ipaddress = if let Some(ipaddress) = self.ipaddress {
            std::net::Ipv4Addr::from(ipaddress)
        } else {
            std::net::Ipv4Addr::from(0)
        };

        let mut flags = 0u16;
        if self.disabled {
            flags |= 0x02;
        }

        PackageData5 {
            number: self.number,
            name: CString::new(self.name).unwrap(),
            flags,
            client_type: self.connection_type,
            hostname,
            ipaddress,
            port: self.port,
            extension: self.extension,
            pin: self.pin,
            timestamp: self.timestamp,
        }
    }
}
#[derive(Debug, Clone)]
pub struct QueueEntry {
    pub uid: u64,
    pub server: u32,
    pub message: u32,
    pub timestamp: u32,
}

#[derive(Debug, Clone)]
pub struct ServersEntry {
    pub uid: u64,
    pub ip_address: u32,
    pub version: u8,
    pub port: u16,
}
