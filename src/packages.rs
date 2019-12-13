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

impl Into<DirectoryEntry> for PackageData5 {
    fn into(self) -> DirectoryEntry {
        let hostname = self.hostname.to_str().unwrap().to_owned();

        let hostname = if hostname.is_empty() {
            None
        } else {
            Some(hostname)
        };

        let ipaddress = u32::from(self.ipaddress);
        let ipaddress: Option<u32> = if ipaddress == 0 {
            None
        } else {
            Some(ipaddress)
        };

        DirectoryEntry {
            number: self.number,
            name: self.name.to_str().unwrap().to_owned(),
            connection_type: self.client_type,
            hostname,
            ipaddress,
            port: self.port,
            extension: self.extension,
            pin: self.pin,
            disabled: (self.flags & 0x02) != 0,
            timestamp: self.timestamp,
            changed: true,
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
