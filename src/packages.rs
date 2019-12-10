use crate::models::DirectoryEntry;
pub use std::ffi::{CStr, CString};
pub use std::net::Ipv4Addr;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PackageData1 {
    pub number: u32,
    pub pin: u16,
    pub port: u16,
}
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PackageData2 {
    pub ipaddress: Ipv4Addr,
}
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PackageData3 {
    pub number: u32,
    pub version: u8,
}
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PackageData4 {}
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PackageData5 {
    pub number: u32,
    pub name: CString,
    pub flags: u16,
    pub client_type: u8,
    pub hostname: CString,
    pub ipaddress: Ipv4Addr,
    pub port: u16,
    pub extension: u8,
    pub pin: u16,
    pub timestamp: u32,
}

impl From<DirectoryEntry> for PackageData5 {
    fn from(entry: DirectoryEntry) -> Self {
        let hostname = if let Some(hostname) = entry.hostname {
            CString::new(hostname).unwrap()
        } else {
            CString::new("").unwrap()
        };

        let ipaddress = if let Some(ipaddress) = entry.ipaddress {
            std::net::Ipv4Addr::from(ipaddress)
        } else {
            std::net::Ipv4Addr::from(0)
        };

        let mut flags = 0u16;
        if entry.disabled {
            flags |= 0x02;
        }

        PackageData5 {
            number: entry.number,
            name: CString::new(entry.name).unwrap(),
            flags,
            client_type: entry.connection_type,
            hostname,
            ipaddress,
            port: entry.port,
            extension: entry.extension,
            pin: entry.pin,
            timestamp: entry.timestamp,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PackageData6 {
    pub version: u8,
    pub server_pin: u32,
}
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PackageData7 {
    pub version: u8,
    pub server_pin: u32,
}
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PackageData8 {}
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PackageData9 {}
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PackageData10 {
    pub version: u8,
    pub pattern: CString,
}
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PackageData255 {
    pub message: CString,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Package {
    Type1(PackageData1),
    Type2(PackageData2),
    Type3(PackageData3),
    Type4(PackageData4),
    Type5(PackageData5),
    Type6(PackageData6),
    Type7(PackageData7),
    Type8(PackageData8),
    Type9(PackageData9),
    Type10(PackageData10),
    Type255(PackageData255),
}
