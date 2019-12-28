use std::{
    convert::{TryFrom, TryInto},
    ffi::CString,
    net::Ipv4Addr,
};

pub const LENGTH_TYPE_1: usize = 8;
pub const LENGTH_TYPE_2: usize = 4;
pub const LENGTH_TYPE_3: usize = 5;
pub const LENGTH_TYPE_4: usize = 0;
pub const LENGTH_TYPE_5: usize = 100;
pub const LENGTH_TYPE_6: usize = 5;
pub const LENGTH_TYPE_7: usize = 5;
pub const LENGTH_TYPE_8: usize = 0;
pub const LENGTH_TYPE_9: usize = 0;
pub const LENGTH_TYPE_10: usize = 41;

#[repr(C, packed)]
pub struct RawPackage1 {
    pub number: u32,
    pub pin: u16,
    pub port: u16,
}
#[repr(C, packed)]
pub struct RawPackage2 {
    pub ipaddress: [u8; 4],
}
#[repr(C, packed)]
pub struct RawPackage3 {
    pub number: u32,
    pub version: u8,
}
#[repr(C, packed)]
pub struct RawPackage4 {}
#[repr(C, packed)]
pub struct RawPackage5 {
    pub number: u32,
    pub name: [u8; 40],
    pub flags: u16,
    pub client_type: u8,
    pub hostname: [u8; 40],
    pub ipaddress: [u8; 4],
    pub port: u16,
    pub extension: u8,
    pub pin: u16,
    pub timestamp: u32,
}
#[repr(C, packed)]
pub struct RawPackage6 {
    pub version: u8,
    pub server_pin: u32,
}
#[repr(C, packed)]
pub struct RawPackage7 {
    pub version: u8,
    pub server_pin: u32,
}
#[repr(C, packed)]
pub struct RawPackage8 {}
#[repr(C, packed)]
pub struct RawPackage9 {}
#[repr(C, packed)]
pub struct RawPackage10 {
    pub version: u8,
    pub pattern: [u8; 40],
}
#[repr(C, packed)]
pub struct RawPackage255 {
    pub message: Vec<u8>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct ProcessedPackage1 {
    pub number: u32,
    pub pin: u16,
    pub port: u16,
}
#[derive(Debug, Eq, PartialEq)]
pub struct ProcessedPackage2 {
    pub ipaddress: Ipv4Addr,
}
#[derive(Debug, Eq, PartialEq)]
pub struct ProcessedPackage3 {
    pub number: u32,
    pub version: u8,
}
#[derive(Debug, Eq, PartialEq)]
pub struct ProcessedPackage4 {}
#[derive(Debug, Eq, PartialEq)]
pub struct ProcessedPackage5 {
    pub number: u32,
    pub name: String,
    pub disabled: bool,
    pub client_type: u8,
    pub hostname: Option<String>,
    pub ipaddress: Option<Ipv4Addr>,
    pub port: u16,
    pub extension: u8,
    pub pin: u16,
    pub timestamp: u32, // TODO
}
#[derive(Debug, Eq, PartialEq)]
pub struct ProcessedPackage6 {
    pub version: u8,
    pub server_pin: u32,
}
#[derive(Debug, Eq, PartialEq)]
pub struct ProcessedPackage7 {
    pub version: u8,
    pub server_pin: u32,
}
#[derive(Debug, Eq, PartialEq)]
pub struct ProcessedPackage8 {}
#[derive(Debug, Eq, PartialEq)]
pub struct ProcessedPackage9 {}
#[derive(Debug, Eq, PartialEq)]
pub struct ProcessedPackage10 {
    pub version: u8,
    pub pattern: String,
}
#[derive(Debug, Eq, PartialEq)]
pub struct ProcessedPackage255 {
    pub message: String,
}

impl TryFrom<ProcessedPackage1> for RawPackage1 {
    type Error = anyhow::Error;
    fn try_from(pkg: ProcessedPackage1) -> anyhow::Result<Self> {
        Ok(RawPackage1 {
            number: pkg.number,
            pin: pkg.pin,
            port: pkg.port,
        })
    }
}
impl TryFrom<ProcessedPackage2> for RawPackage2 {
    type Error = anyhow::Error;
    fn try_from(pkg: ProcessedPackage2) -> anyhow::Result<Self> {
        Ok(RawPackage2 {
            ipaddress: pkg.ipaddress.octets(),
        })
    }
}
impl TryFrom<ProcessedPackage3> for RawPackage3 {
    type Error = anyhow::Error;
    fn try_from(pkg: ProcessedPackage3) -> anyhow::Result<Self> {
        Ok(RawPackage3 {
            version: pkg.version,
            number: pkg.number,
        })
    }
}
impl TryFrom<ProcessedPackage4> for RawPackage4 {
    type Error = anyhow::Error;
    fn try_from(_pkg: ProcessedPackage4) -> anyhow::Result<Self> {
        Ok(RawPackage4 {})
    }
}
impl TryFrom<ProcessedPackage5> for RawPackage5 {
    type Error = anyhow::Error;
    fn try_from(pkg: ProcessedPackage5) -> anyhow::Result<Self> {
        Ok(RawPackage5 {
            number: pkg.number,
            name: array_from_string(pkg.name),
            client_type: pkg.client_type,
            hostname: array_from_string(pkg.hostname.unwrap_or("".into())),
            ipaddress: pkg
                .ipaddress
                .map(|addr| addr.octets())
                .unwrap_or([0, 0, 0, 0]),
            port: pkg.port,
            extension: pkg.extension,
            pin: pkg.pin,
            flags: if pkg.disabled { 0x02 } else { 0x00 },
            timestamp: pkg.timestamp,
        })
    }
}
impl TryFrom<ProcessedPackage6> for RawPackage6 {
    type Error = anyhow::Error;
    fn try_from(pkg: ProcessedPackage6) -> anyhow::Result<Self> {
        Ok(RawPackage6 {
            server_pin: pkg.server_pin,
            version: pkg.version,
        })
    }
}
impl TryFrom<ProcessedPackage7> for RawPackage7 {
    type Error = anyhow::Error;
    fn try_from(pkg: ProcessedPackage7) -> anyhow::Result<Self> {
        Ok(RawPackage7 {
            server_pin: pkg.server_pin,
            version: pkg.version,
        })
    }
}
impl TryFrom<ProcessedPackage8> for RawPackage8 {
    type Error = anyhow::Error;
    fn try_from(_pkg: ProcessedPackage8) -> anyhow::Result<Self> {
        Ok(RawPackage8 {})
    }
}
impl TryFrom<ProcessedPackage9> for RawPackage9 {
    type Error = anyhow::Error;
    fn try_from(_pkg: ProcessedPackage9) -> anyhow::Result<Self> {
        Ok(RawPackage9 {})
    }
}
impl TryFrom<ProcessedPackage10> for RawPackage10 {
    type Error = anyhow::Error;
    fn try_from(pkg: ProcessedPackage10) -> anyhow::Result<Self> {
        Ok(RawPackage10 {
            version: pkg.version,
            pattern: array_from_string(pkg.pattern),
        })
    }
}
impl TryFrom<ProcessedPackage255> for RawPackage255 {
    type Error = anyhow::Error;
    fn try_from(pkg: ProcessedPackage255) -> anyhow::Result<Self> {
        Ok(RawPackage255 {
            message: pkg.message.bytes().collect(),
        })
    }
}

impl TryFrom<RawPackage1> for ProcessedPackage1 {
    type Error = anyhow::Error;
    fn try_from(pkg: RawPackage1) -> anyhow::Result<Self> {
        Ok(ProcessedPackage1 {
            number: pkg.number,
            pin: pkg.pin,
            port: pkg.port,
        })
    }
}
impl TryFrom<RawPackage2> for ProcessedPackage2 {
    type Error = anyhow::Error;
    fn try_from(pkg: RawPackage2) -> anyhow::Result<Self> {
        Ok(ProcessedPackage2 {
            ipaddress: Ipv4Addr::from(pkg.ipaddress),
        })
    }
}
impl TryFrom<RawPackage3> for ProcessedPackage3 {
    type Error = anyhow::Error;
    fn try_from(pkg: RawPackage3) -> anyhow::Result<Self> {
        Ok(ProcessedPackage3 {
            number: pkg.number,
            version: pkg.version,
        })
    }
}
impl TryFrom<RawPackage4> for ProcessedPackage4 {
    type Error = anyhow::Error;
    fn try_from(_pkg: RawPackage4) -> anyhow::Result<Self> {
        Ok(ProcessedPackage4 {})
    }
}
impl TryFrom<RawPackage5> for ProcessedPackage5 {
    type Error = anyhow::Error;
    fn try_from(pkg: RawPackage5) -> anyhow::Result<Self> {
        let hostname = string_from_array(pkg.hostname)?;

        let hostname = if hostname.is_empty() {
            None
        } else {
            Some(hostname)
        };

        let ipaddress = Ipv4Addr::from(pkg.ipaddress);
        let ipaddress = if ipaddress == Ipv4Addr::from([0, 0, 0, 0]) {
            None
        } else {
            Some(ipaddress)
        };

        Ok(ProcessedPackage5 {
            number: pkg.number,
            name: string_from_array(pkg.name)?,
            client_type: pkg.client_type,
            hostname,
            ipaddress,
            port: pkg.port,
            extension: pkg.extension,
            pin: pkg.pin,
            disabled: (pkg.flags & 0x02) != 0,
            timestamp: pkg.timestamp,
        })
    }
}
impl TryFrom<RawPackage6> for ProcessedPackage6 {
    type Error = anyhow::Error;
    fn try_from(pkg: RawPackage6) -> anyhow::Result<Self> {
        Ok(ProcessedPackage6 {
            server_pin: pkg.server_pin,
            version: pkg.version,
        })
    }
}
impl TryFrom<RawPackage7> for ProcessedPackage7 {
    type Error = anyhow::Error;
    fn try_from(pkg: RawPackage7) -> anyhow::Result<Self> {
        Ok(ProcessedPackage7 {
            server_pin: pkg.server_pin,
            version: pkg.version,
        })
    }
}
impl TryFrom<RawPackage8> for ProcessedPackage8 {
    type Error = anyhow::Error;
    fn try_from(_pkg: RawPackage8) -> anyhow::Result<Self> {
        Ok(ProcessedPackage8 {})
    }
}
impl TryFrom<RawPackage9> for ProcessedPackage9 {
    type Error = anyhow::Error;
    fn try_from(_pkg: RawPackage9) -> anyhow::Result<Self> {
        Ok(ProcessedPackage9 {})
    }
}
impl TryFrom<RawPackage10> for ProcessedPackage10 {
    type Error = anyhow::Error;
    fn try_from(pkg: RawPackage10) -> anyhow::Result<Self> {
        Ok(ProcessedPackage10 {
            version: pkg.version,
            pattern: string_from_array(pkg.pattern)?,
        })
    }
}
impl TryFrom<RawPackage255> for ProcessedPackage255 {
    type Error = anyhow::Error;
    fn try_from(pkg: RawPackage255) -> anyhow::Result<Self> {
        let message = CString::new(
            pkg.message
                .into_iter()
                .take_while(|byte| *byte != 0)
                .collect::<Vec<u8>>(),
        )?
        .into_string()?;

        Ok(ProcessedPackage255 { message })
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum Package {
    Type1(ProcessedPackage1),
    Type2(ProcessedPackage2),
    Type3(ProcessedPackage3),
    Type4(ProcessedPackage4),
    Type5(ProcessedPackage5),
    Type6(ProcessedPackage6),
    Type7(ProcessedPackage7),
    Type8(ProcessedPackage8),
    Type9(ProcessedPackage9),
    Type10(ProcessedPackage10),
    Type255(ProcessedPackage255),
}

impl TryFrom<RawPackage> for Package {
    type Error = anyhow::Error;
    fn try_from(pkg: RawPackage) -> anyhow::Result<Package> {
        Ok(match pkg {
            RawPackage::Type1(pkg) => Package::Type1(pkg.try_into()?),
            RawPackage::Type2(pkg) => Package::Type2(pkg.try_into()?),
            RawPackage::Type3(pkg) => Package::Type3(pkg.try_into()?),
            RawPackage::Type4(pkg) => Package::Type4(pkg.try_into()?),
            RawPackage::Type5(pkg) => Package::Type5(pkg.try_into()?),
            RawPackage::Type6(pkg) => Package::Type6(pkg.try_into()?),
            RawPackage::Type7(pkg) => Package::Type7(pkg.try_into()?),
            RawPackage::Type8(pkg) => Package::Type8(pkg.try_into()?),
            RawPackage::Type9(pkg) => Package::Type9(pkg.try_into()?),
            RawPackage::Type10(pkg) => Package::Type10(pkg.try_into()?),
            RawPackage::Type255(pkg) => Package::Type255(pkg.try_into()?),
        })
    }
}

pub enum RawPackage {
    Type1(RawPackage1),
    Type2(RawPackage2),
    Type3(RawPackage3),
    Type4(RawPackage4),
    Type5(RawPackage5),
    Type6(RawPackage6),
    Type7(RawPackage7),
    Type8(RawPackage8),
    Type9(RawPackage9),
    Type10(RawPackage10),
    Type255(RawPackage255),
}

impl TryFrom<Package> for RawPackage {
    type Error = anyhow::Error;
    fn try_from(pkg: Package) -> anyhow::Result<RawPackage> {
        Ok(match pkg {
            Package::Type1(pkg) => RawPackage::Type1(pkg.try_into()?),
            Package::Type2(pkg) => RawPackage::Type2(pkg.try_into()?),
            Package::Type3(pkg) => RawPackage::Type3(pkg.try_into()?),
            Package::Type4(pkg) => RawPackage::Type4(pkg.try_into()?),
            Package::Type5(pkg) => RawPackage::Type5(pkg.try_into()?),
            Package::Type6(pkg) => RawPackage::Type6(pkg.try_into()?),
            Package::Type7(pkg) => RawPackage::Type7(pkg.try_into()?),
            Package::Type8(pkg) => RawPackage::Type8(pkg.try_into()?),
            Package::Type9(pkg) => RawPackage::Type9(pkg.try_into()?),
            Package::Type10(pkg) => RawPackage::Type10(pkg.try_into()?),
            Package::Type255(pkg) => RawPackage::Type255(pkg.try_into()?),
        })
    }
}

fn array_from_string(mut input: String) -> [u8; 40] {
    let mut buf: [u8; 40] = [0; 40];

    input.truncate(39);

    for (i, b) in input.into_bytes().into_iter().enumerate() {
        buf[i] = b;
    }

    buf
}

fn string_from_array(array: [u8; 40]) -> anyhow::Result<String> {
    let mut end_of_content = 40;
    for i in 0..40 {
        if array[i] == 0 {
            end_of_content = i;
            break;
        }
    }

    Ok(CString::new(&array[0..end_of_content])?.into_string()?)
}
