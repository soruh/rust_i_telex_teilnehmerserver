use std::{ffi::CString, mem::transmute, net::Ipv4Addr};


pub const LENGTH_TYPE_1: usize = 64 / 8;
pub const LENGTH_TYPE_2: usize = 32 / 8;
pub const LENGTH_TYPE_3: usize = 64 / 8;
pub const LENGTH_TYPE_4: usize = 0;
pub const LENGTH_TYPE_5: usize = 800 / 8;
pub const LENGTH_TYPE_6: usize = 64 / 8;
pub const LENGTH_TYPE_7: usize = 64 / 8;
pub const LENGTH_TYPE_8: usize = 0;
pub const LENGTH_TYPE_9: usize = 0;
pub const LENGTH_TYPE_10: usize = 328 / 8;


pub struct RawPackage1 {
    pub number: u32,
    pub pin: u16,
    pub port: u16,
}
pub struct RawPackage2 {
    pub ipaddress: [u8; 4],
}
pub struct RawPackage3 {
    pub number: u32,
    pub version: u8,
}
pub struct RawPackage4 {}
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
pub struct RawPackage6 {
    pub version: u8,
    pub server_pin: u32,
}
pub struct RawPackage7 {
    pub version: u8,
    pub server_pin: u32,
}
pub struct RawPackage8 {}
pub struct RawPackage9 {}
pub struct RawPackage10 {
    pub version: u8,
    pub pattern: [u8; 40],
}
pub struct RawPackage255 {
    pub message: Vec<u8>,
}

#[derive(Debug)]
pub struct ProcessedPackage1 {
    pub number: u32,
    pub pin: u16,
    pub port: u16,
}
#[derive(Debug)]
pub struct ProcessedPackage2 {
    pub ipaddress: Ipv4Addr,
}
#[derive(Debug)]
pub struct ProcessedPackage3 {
    pub number: u32,
    pub version: u8,
}
#[derive(Debug)]
pub struct ProcessedPackage4 {}
#[derive(Debug)]
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
#[derive(Debug)]
pub struct ProcessedPackage6 {
    pub version: u8,
    pub server_pin: u32,
}
#[derive(Debug)]
pub struct ProcessedPackage7 {
    pub version: u8,
    pub server_pin: u32,
}
#[derive(Debug)]
pub struct ProcessedPackage8 {}
#[derive(Debug)]
pub struct ProcessedPackage9 {}
#[derive(Debug)]
pub struct ProcessedPackage10 {
    pub version: u8,
    pub pattern: String,
}
#[derive(Debug)]
pub struct ProcessedPackage255 {
    pub message: String,
}

impl From<ProcessedPackage1> for RawPackage1 {
    fn from(pkg: ProcessedPackage1) -> Self {
        unsafe { transmute(pkg) }
    }
}
impl From<ProcessedPackage2> for RawPackage2 {
    fn from(pkg: ProcessedPackage2) -> Self {
        RawPackage2 {
            ipaddress: pkg.ipaddress.octets(),
        }
    }
}
impl From<ProcessedPackage3> for RawPackage3 {
    fn from(pkg: ProcessedPackage3) -> Self {
        unsafe { transmute(pkg) }
    }
}
impl From<ProcessedPackage4> for RawPackage4 {
    fn from(pkg: ProcessedPackage4) -> Self {
        unsafe { transmute(pkg) }
    }
}
impl From<ProcessedPackage5> for RawPackage5 {
    fn from(pkg: ProcessedPackage5) -> Self {
        RawPackage5 {
            number: pkg.number,
            name: array_from_string(pkg.name),
            client_type: pkg.client_type,
            hostname: array_from_string(pkg.hostname.unwrap_or("".into())),
            ipaddress: pkg
                .ipaddress
                .unwrap_or(Ipv4Addr::from([0, 0, 0, 0]))
                .octets(),
            port: pkg.port,
            extension: pkg.extension,
            pin: pkg.pin,
            flags: if pkg.disabled { 0x02 } else { 0x00 },
            timestamp: pkg.timestamp,
        }
    }
}
impl From<ProcessedPackage6> for RawPackage6 {
    fn from(pkg: ProcessedPackage6) -> Self {
        unsafe { transmute(pkg) }
    }
}
impl From<ProcessedPackage7> for RawPackage7 {
    fn from(pkg: ProcessedPackage7) -> Self {
        unsafe { transmute(pkg) }
    }
}
impl From<ProcessedPackage8> for RawPackage8 {
    fn from(pkg: ProcessedPackage8) -> Self {
        unsafe { transmute(pkg) }
    }
}
impl From<ProcessedPackage9> for RawPackage9 {
    fn from(pkg: ProcessedPackage9) -> Self {
        unsafe { transmute(pkg) }
    }
}
impl From<ProcessedPackage10> for RawPackage10 {
    fn from(pkg: ProcessedPackage10) -> Self {
        RawPackage10 {
            version: pkg.version,
            pattern: array_from_string(pkg.pattern),
        }
    }
}
impl From<ProcessedPackage255> for RawPackage255 {
    fn from(pkg: ProcessedPackage255) -> Self {
        RawPackage255 {
            message: pkg.message.bytes().collect(),
        }
    }
}





impl From<RawPackage1> for ProcessedPackage1 {
    fn from(pkg: RawPackage1) -> Self {
        unsafe { transmute(pkg) }
    }
}
impl From<RawPackage2> for ProcessedPackage2 {
    fn from(pkg: RawPackage2) -> Self {
        ProcessedPackage2 {
            ipaddress: Ipv4Addr::from(pkg.ipaddress),
        }
    }
}
impl From<RawPackage3> for ProcessedPackage3 {
    fn from(pkg: RawPackage3) -> Self {
        unsafe { transmute(pkg) }
    }
}
impl From<RawPackage4> for ProcessedPackage4 {
    fn from(pkg: RawPackage4) -> Self {
        unsafe { transmute(pkg) }
    }
}
impl From<RawPackage5> for ProcessedPackage5 {
    fn from(pkg: RawPackage5) -> Self {
        let hostname = string_from_array(pkg.hostname).unwrap(); // TODO

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

        ProcessedPackage5 {
            number: pkg.number,
            name: string_from_array(pkg.name).unwrap(), // TODO
            client_type: pkg.client_type,
            hostname,
            ipaddress,
            port: pkg.port,
            extension: pkg.extension,
            pin: pkg.pin,
            disabled: (pkg.flags & 0x02) != 0,
            timestamp: pkg.timestamp,
        }
    }
}
impl From<RawPackage6> for ProcessedPackage6 {
    fn from(pkg: RawPackage6) -> Self {
        unsafe { transmute(pkg) }
    }
}
impl From<RawPackage7> for ProcessedPackage7 {
    fn from(pkg: RawPackage7) -> Self {
        unsafe { transmute(pkg) }
    }
}
impl From<RawPackage8> for ProcessedPackage8 {
    fn from(pkg: RawPackage8) -> Self {
        unsafe { transmute(pkg) }
    }
}
impl From<RawPackage9> for ProcessedPackage9 {
    fn from(pkg: RawPackage9) -> Self {
        unsafe { transmute(pkg) }
    }
}
impl From<RawPackage10> for ProcessedPackage10 {
    fn from(pkg: RawPackage10) -> Self {
        ProcessedPackage10 {
            version: pkg.version,
            pattern: string_from_array(pkg.pattern).unwrap(), //TODO
        }
    }
}
impl From<RawPackage255> for ProcessedPackage255 {
    fn from(pkg: RawPackage255) -> Self {
        let message = CString::new(
            pkg.message
                .into_iter()
                .take_while(|byte| *byte != 0)
                .collect::<Vec<u8>>(),
        )
        .unwrap() // TODO
        .into_string()
        .unwrap(); // TODO

        ProcessedPackage255 { message }
    }
}




#[derive(Debug)]
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

impl From<RawPackage> for Package {
    fn from(pkg: RawPackage) -> Package {
        match pkg {
            RawPackage::Type1(pkg) => Package::Type1(pkg.into()),
            RawPackage::Type2(pkg) => Package::Type2(pkg.into()),
            RawPackage::Type3(pkg) => Package::Type3(pkg.into()),
            RawPackage::Type4(pkg) => Package::Type4(pkg.into()),
            RawPackage::Type5(pkg) => Package::Type5(pkg.into()),
            RawPackage::Type6(pkg) => Package::Type6(pkg.into()),
            RawPackage::Type7(pkg) => Package::Type7(pkg.into()),
            RawPackage::Type8(pkg) => Package::Type8(pkg.into()),
            RawPackage::Type9(pkg) => Package::Type9(pkg.into()),
            RawPackage::Type10(pkg) => Package::Type10(pkg.into()),
            RawPackage::Type255(pkg) => Package::Type255(pkg.into()),
        }
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

impl From<Package> for RawPackage {
    fn from(pkg: Package) -> RawPackage {
        match pkg {
            Package::Type1(pkg) => RawPackage::Type1(pkg.into()),
            Package::Type2(pkg) => RawPackage::Type2(pkg.into()),
            Package::Type3(pkg) => RawPackage::Type3(pkg.into()),
            Package::Type4(pkg) => RawPackage::Type4(pkg.into()),
            Package::Type5(pkg) => RawPackage::Type5(pkg.into()),
            Package::Type6(pkg) => RawPackage::Type6(pkg.into()),
            Package::Type7(pkg) => RawPackage::Type7(pkg.into()),
            Package::Type8(pkg) => RawPackage::Type8(pkg.into()),
            Package::Type9(pkg) => RawPackage::Type9(pkg.into()),
            Package::Type10(pkg) => RawPackage::Type10(pkg.into()),
            Package::Type255(pkg) => RawPackage::Type255(pkg.into()),
        }
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
