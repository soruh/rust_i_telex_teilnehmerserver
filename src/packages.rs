use std::{
    convert::{TryFrom, TryInto},
    ffi::CString,
    io::Write,
    net::Ipv4Addr,
};

// ! This is disgusting, but neccessary until we get const generics
pub struct ArrayImplWrapper<'a>(&'a [u8]);

impl<'a> TryInto<[u8; LENGTH_TYPE_5]> for ArrayImplWrapper<'a> {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<[u8; LENGTH_TYPE_5], Self::Error> {
        let mut res = [0_u8; LENGTH_TYPE_5];

        for (i, b) in self.0.into_iter().enumerate() {
            if i < LENGTH_TYPE_5 {
                res[i] = *b;
            } else {
                return Err(ItelexServerErrorKind::ParseFailure(5).into());
            }
        }

        Ok(res)
    }
}

impl<'a> TryInto<[u8; LENGTH_TYPE_10]> for ArrayImplWrapper<'a> {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<[u8; LENGTH_TYPE_10], Self::Error> {
        let mut res = [0_u8; LENGTH_TYPE_10];

        for (i, b) in self.0.into_iter().enumerate() {
            if i < LENGTH_TYPE_10 {
                res[i] = *b;
            } else {
                return Err(ItelexServerErrorKind::ParseFailure(10).into());
            }
        }

        Ok(res)
    }
}

use crate::errors::ItelexServerErrorKind;
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

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Package1 {
    pub number: u32,
    pub pin: u16,
    pub port: u16,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Package2 {
    pub ipaddress: Ipv4Addr,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Package3 {
    pub number: u32,
    pub version: u8,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Package4 {}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Package5 {
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

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Package6 {
    pub version: u8,
    pub server_pin: u32,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Package7 {
    pub version: u8,
    pub server_pin: u32,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Package8 {}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Package9 {}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Package10 {
    pub version: u8,
    pub pattern: String,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Package255 {
    pub message: String,
}

impl TryFrom<&[u8]> for Package1 {
    type Error = anyhow::Error;

    fn try_from(slice: &[u8]) -> anyhow::Result<Self> {
        if slice.len() < LENGTH_TYPE_1 {
            bail!(ItelexServerErrorKind::ParseFailure(1))
        }

        Ok(Self {
            number: u32::from_le_bytes(slice[0..4].try_into()?),
            pin: u16::from_le_bytes(slice[4..6].try_into()?),
            port: u16::from_le_bytes(slice[6..8].try_into()?),
        })
    }
}

impl TryFrom<&[u8]> for Package2 {
    type Error = anyhow::Error;

    fn try_from(slice: &[u8]) -> anyhow::Result<Self> {
        if slice.len() < LENGTH_TYPE_2 {
            bail!(ItelexServerErrorKind::ParseFailure(2))
        }

        Ok(Self {
            ipaddress: {
                let array: [u8; 4] = slice[0..4].try_into()?;

                Ipv4Addr::from(array)
            },
        })
    }
}

impl TryFrom<&[u8]> for Package3 {
    type Error = anyhow::Error;

    fn try_from(slice: &[u8]) -> anyhow::Result<Self> {
        if slice.len() < LENGTH_TYPE_3 {
            bail!(ItelexServerErrorKind::ParseFailure(3))
        }

        Ok(Self {
            number: u32::from_le_bytes(slice[0..4].try_into()?),
            version: u8::from_le_bytes(slice[4..5].try_into()?),
        })
    }
}

impl TryFrom<&[u8]> for Package4 {
    type Error = anyhow::Error;

    fn try_from(slice: &[u8]) -> anyhow::Result<Self> {
        if slice.len() < LENGTH_TYPE_4 {
            bail!(ItelexServerErrorKind::ParseFailure(4))
        }

        Ok(Self {})
    }
}

impl TryFrom<&[u8]> for Package5 {
    type Error = anyhow::Error;

    fn try_from(slice: &[u8]) -> anyhow::Result<Self> {
        if slice.len() < LENGTH_TYPE_5 {
            bail!(ItelexServerErrorKind::ParseFailure(5))
        }

        Ok(Self {
            number: u32::from_le_bytes(slice[0..4].try_into()?),
            name: string_from_slice(&slice[4..44])?,
            disabled: {
                let flags = u16::from_le_bytes(slice[44..46].try_into()?);

                flags & 2 == 0x02
            },
            client_type: u8::from_le_bytes(slice[46..47].try_into()?),
            hostname: {
                let hostname = string_from_slice(&slice[47..87])?;

                if hostname.is_empty() { None } else { Some(hostname) }
            },
            ipaddress: {
                let octets: [u8; 4] = slice[87..91].try_into()?;

                let ipaddress = Ipv4Addr::from(octets);

                if ipaddress.is_unspecified() { None } else { Some(ipaddress) }
            },
            port: u16::from_le_bytes(slice[91..93].try_into()?),
            extension: u8::from_le_bytes(slice[93..94].try_into()?),
            pin: u16::from_le_bytes(slice[94..96].try_into()?),
            timestamp: u32::from_le_bytes(slice[96..100].try_into()?),
        })
    }
}

impl TryFrom<&[u8]> for Package6 {
    type Error = anyhow::Error;

    fn try_from(slice: &[u8]) -> anyhow::Result<Self> {
        if slice.len() < LENGTH_TYPE_6 {
            bail!(ItelexServerErrorKind::ParseFailure(6))
        }

        Ok(Self {
            version: u8::from_le_bytes(slice[0..1].try_into()?),
            server_pin: u32::from_le_bytes(slice[1..5].try_into()?),
        })
    }
}

impl TryFrom<&[u8]> for Package7 {
    type Error = anyhow::Error;

    fn try_from(slice: &[u8]) -> anyhow::Result<Self> {
        if slice.len() < LENGTH_TYPE_7 {
            bail!(ItelexServerErrorKind::ParseFailure(7))
        }

        Ok(Self {
            version: u8::from_le_bytes(slice[0..1].try_into()?),
            server_pin: u32::from_le_bytes(slice[1..5].try_into()?),
        })
    }
}

impl TryFrom<&[u8]> for Package8 {
    type Error = anyhow::Error;

    fn try_from(slice: &[u8]) -> anyhow::Result<Self> {
        if slice.len() < LENGTH_TYPE_8 {
            bail!(ItelexServerErrorKind::ParseFailure(8))
        }

        Ok(Self {})
    }
}

impl TryFrom<&[u8]> for Package9 {
    type Error = anyhow::Error;

    fn try_from(slice: &[u8]) -> anyhow::Result<Self> {
        if slice.len() < LENGTH_TYPE_9 {
            bail!(ItelexServerErrorKind::ParseFailure(9))
        }

        Ok(Self {})
    }
}

impl TryFrom<&[u8]> for Package10 {
    type Error = anyhow::Error;

    fn try_from(slice: &[u8]) -> anyhow::Result<Self> {
        if slice.len() < LENGTH_TYPE_10 {
            bail!(ItelexServerErrorKind::ParseFailure(10))
        }

        Ok(Self {
            version: u8::from_le_bytes(slice[0..1].try_into()?),
            pattern: string_from_slice(slice[1..41].try_into()?)?,
        })
    }
}

impl TryFrom<&[u8]> for Package255 {
    type Error = anyhow::Error;

    fn try_from(slice: &[u8]) -> anyhow::Result<Self> {
        Ok(Self { message: string_from_slice(&slice)? })
    }
}

impl TryInto<Vec<u8>> for Package1 {
    type Error = anyhow::Error;

    fn try_into(self: Package1) -> anyhow::Result<Vec<u8>> {
        let mut res: Vec<u8> = Vec::with_capacity(LENGTH_TYPE_1);

        res.write_all(&self.number.to_le_bytes())?;
        res.write_all(&self.pin.to_le_bytes())?;
        res.write_all(&self.port.to_le_bytes())?;

        Ok(res)
    }
}

impl TryInto<Vec<u8>> for Package2 {
    type Error = anyhow::Error;

    fn try_into(self: Package2) -> anyhow::Result<Vec<u8>> {
        let mut res: Vec<u8> = Vec::with_capacity(LENGTH_TYPE_2);

        res.write_all(&self.ipaddress.octets())?;

        Ok(res)
    }
}

impl TryInto<Vec<u8>> for Package3 {
    type Error = anyhow::Error;

    fn try_into(self: Package3) -> anyhow::Result<Vec<u8>> {
        let mut res: Vec<u8> = Vec::with_capacity(LENGTH_TYPE_3);

        res.write_all(&self.number.to_le_bytes())?;

        res.write_all(&self.version.to_le_bytes())?;

        Ok(res)
    }
}

impl TryInto<Vec<u8>> for Package4 {
    type Error = anyhow::Error;

    fn try_into(self: Package4) -> anyhow::Result<Vec<u8>> {
        Ok(Vec::new())
    }
}

impl TryInto<Vec<u8>> for Package5 {
    type Error = anyhow::Error;

    fn try_into(self: Package5) -> anyhow::Result<Vec<u8>> {
        let mut res: Vec<u8> = Vec::with_capacity(LENGTH_TYPE_5);

        let flags: u16 = if self.disabled { 0x02 } else { 0 };

        res.write_all(&self.number.to_le_bytes())?;
        res.write_all(&array_from_string(self.name))?;
        res.write_all(&flags.to_le_bytes())?;
        res.write_all(&self.client_type.to_le_bytes())?;
        res.write_all(&array_from_string(self.hostname.unwrap_or(String::new())))?;
        res.write_all(&self.ipaddress.map(|e| e.octets()).unwrap_or([0, 0, 0, 0]))?;
        res.write_all(&self.port.to_le_bytes())?;
        res.write_all(&self.extension.to_le_bytes())?;
        res.write_all(&self.pin.to_le_bytes())?;
        res.write_all(&self.timestamp.to_le_bytes())?;

        Ok(res)
    }
}

impl TryInto<Vec<u8>> for Package6 {
    type Error = anyhow::Error;

    fn try_into(self: Package6) -> anyhow::Result<Vec<u8>> {
        let mut res: Vec<u8> = Vec::with_capacity(LENGTH_TYPE_6);

        res.write_all(&self.version.to_le_bytes())?;

        res.write_all(&self.server_pin.to_le_bytes())?;

        Ok(res)
    }
}

impl TryInto<Vec<u8>> for Package7 {
    type Error = anyhow::Error;

    fn try_into(self: Package7) -> anyhow::Result<Vec<u8>> {
        let mut res: Vec<u8> = Vec::with_capacity(LENGTH_TYPE_7);

        res.write_all(&self.version.to_le_bytes())?;

        res.write_all(&self.server_pin.to_le_bytes())?;

        Ok(res)
    }
}

impl TryInto<Vec<u8>> for Package8 {
    type Error = anyhow::Error;

    fn try_into(self: Package8) -> anyhow::Result<Vec<u8>> {
        Ok(Vec::new())
    }
}

impl TryInto<Vec<u8>> for Package9 {
    type Error = anyhow::Error;

    fn try_into(self: Package9) -> anyhow::Result<Vec<u8>> {
        Ok(Vec::new())
    }
}

impl TryInto<Vec<u8>> for Package10 {
    type Error = anyhow::Error;

    fn try_into(self: Package10) -> anyhow::Result<Vec<u8>> {
        let mut res: Vec<u8> = Vec::with_capacity(LENGTH_TYPE_10);

        res.write_all(&self.version.to_le_bytes())?;

        res.write_all(&array_from_string(self.pattern))?;

        Ok(res)
    }
}

impl TryInto<Vec<u8>> for Package255 {
    type Error = anyhow::Error;

    fn try_into(self: Package255) -> anyhow::Result<Vec<u8>> {
        let mut res: Vec<u8> = CString::new(self.message)?.try_into()?;

        res.push(0);

        if res.len() > 0xff {
            bail!(ItelexServerErrorKind::SerializeFailure(255));
        }

        Ok(res)
    }
}

// TODO: Box some of the contents, so that not all instances
// TODO: of this enum are >= 101 Bytes
#[derive(Debug, Eq, PartialEq, Clone)]
pub enum Package {
    Type1(Package1),
    Type2(Package2),
    Type3(Package3),
    Type4(Package4),
    Type5(Package5),
    Type6(Package6),
    Type7(Package7),
    Type8(Package8),
    Type9(Package9),
    Type10(Package10),
    Type255(Package255),
}

impl Package {
    pub fn parse(package_type: u8, slice: &[u8]) -> anyhow::Result<Package> {
        Ok(match package_type {
            1 => Package::Type1(Package1::try_from(slice)?),
            2 => Package::Type2(Package2::try_from(slice)?),
            3 => Package::Type3(Package3::try_from(slice)?),
            4 => Package::Type4(Package4::try_from(slice)?),
            5 => Package::Type5(Package5::try_from(slice)?),
            6 => Package::Type6(Package6::try_from(slice)?),
            7 => Package::Type7(Package7::try_from(slice)?),
            8 => Package::Type8(Package8::try_from(slice)?),
            9 => Package::Type9(Package9::try_from(slice)?),
            10 => Package::Type10(Package10::try_from(slice)?),
            255 => Package::Type255(Package255::try_from(slice)?),

            _ => bail!(ItelexServerErrorKind::ParseFailure(package_type)),
        })
    }

    pub fn package_type(&self) -> u8 {
        match self {
            Package::Type1(_) => 1,
            Package::Type2(_) => 2,
            Package::Type3(_) => 3,
            Package::Type4(_) => 4,
            Package::Type5(_) => 5,
            Package::Type6(_) => 6,
            Package::Type7(_) => 7,
            Package::Type8(_) => 8,
            Package::Type9(_) => 9,
            Package::Type10(_) => 10,
            Package::Type255(_) => 255,
        }
    }
}

impl TryInto<Vec<u8>> for Package {
    type Error = anyhow::Error;

    fn try_into(self: Package) -> anyhow::Result<Vec<u8>> {
        match self {
            Package::Type1(pkg) => pkg.try_into(),
            Package::Type2(pkg) => pkg.try_into(),
            Package::Type3(pkg) => pkg.try_into(),
            Package::Type4(pkg) => pkg.try_into(),
            Package::Type5(pkg) => pkg.try_into(),
            Package::Type6(pkg) => pkg.try_into(),
            Package::Type7(pkg) => pkg.try_into(),
            Package::Type8(pkg) => pkg.try_into(),
            Package::Type9(pkg) => pkg.try_into(),
            Package::Type10(pkg) => pkg.try_into(),
            Package::Type255(pkg) => pkg.try_into(),
        }
    }
}

fn array_from_string(mut input: String) -> [u8; 40] {
    let mut buf: [u8; 40] = [0; 40];

    input.truncate(39); // ensure we don't write over capaciry and leave one 0 byte at the end

    for (i, b) in input.into_bytes().into_iter().enumerate() {
        buf[i] = b;
    }

    buf
}

fn string_from_slice(slice: &[u8]) -> anyhow::Result<String> {
    let mut end_of_content = slice.len();

    for i in 0..slice.len() {
        if slice[i] == 0 {
            end_of_content = i;

            break;
        }
    }

    Ok(CString::new(&slice[0..end_of_content])?.into_string()?)
}
