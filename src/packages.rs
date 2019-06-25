pub use std::ffi::CString;
pub use std::net::Ipv4Addr;

#[derive(Debug)]
pub struct PackageData1 {
    pub number: u32,
    pub pin: u16,
    pub port: u16,
}
#[derive(Debug)]
pub struct PackageData2 {
    pub ipaddress: Ipv4Addr,
}
#[derive(Debug)]
pub struct PackageData3 {
    pub number: u32,
    pub version: u8,
}
#[derive(Debug)]
pub struct PackageData4 {}
#[derive(Debug)]
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
    pub date: u32,
}
#[derive(Debug)]
pub struct PackageData6 {
    pub version: u8,
    pub server_pin: u32,
}
#[derive(Debug)]
pub struct PackageData7 {
    pub version: u8,
    pub server_pin: u32,
}
#[derive(Debug)]
pub struct PackageData8 {}
#[derive(Debug)]
pub struct PackageData9 {}
#[derive(Debug)]
pub struct PackageData10 {
    pub version: u8,
    pub pattern: CString,
}
#[derive(Debug)]
pub struct PackageData255 {
    pub message: CString,
}

#[derive(Debug)]
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
