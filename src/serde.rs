pub use crate::packages::*;
use nom::*;
use std::io::Write;

named!(
        _read_nul_terminated <&[u8], CString>,
        do_parse!(
            content: take_until!("\0") >>
            (CString::new(content).unwrap())
        )
    );

fn read_nul_terminated(input: &[u8]) -> Result<CString, nom::Err<&[u8]>> {
    _read_nul_terminated(input).map(|res| res.1)
}

named!(
        parse_type_1<&[u8], Package>,
        do_parse!(
            number: le_u32 >>
            pin: le_u16 >>
            port: le_u16 >>
            (Package::Type1(PackageData1 { number, pin, port }))
        )
    );

named!(
        parse_type_2<&[u8], Package>,
        do_parse!(
            ipaddress: be_u32 >>
            (Package::Type2(PackageData2 {ipaddress: Ipv4Addr::from(ipaddress)}))
        )
    );

named!(
        parse_type_3<&[u8], Package>,
        do_parse!(
            number: le_u32 >>
            version: le_u8 >>
            (Package::Type3(PackageData3 {number, version}))
        )
    );

named!(
        parse_type_5<&[u8], Package>,
        do_parse!(
            number: le_u32 >>
            name: take!(40) >>
            flags: le_u16 >>
            client_type: le_u8 >>
            hostname: take!(40) >>
            ipaddress: be_u32 >>
            port: le_u16 >>
            extension: le_u8 >>
            pin: le_u16 >>
            date: le_u32 >>
            (Package::Type5(PackageData5 {
                number,
                name: read_nul_terminated(name)?,
                flags,
                client_type,
                hostname: read_nul_terminated(hostname)?,
                ipaddress: Ipv4Addr::from(ipaddress),
                port,
                extension,
                pin,
                date
            }))
        )
    );

named!(
        parse_type_6<&[u8], Package>,
        do_parse!(
            version: le_u8 >>
            server_pin: le_u32 >>
            (Package::Type6(PackageData6 {
                version,
                server_pin,
            }))
        )
    );

named!(
        parse_type_7<&[u8], Package>,
        do_parse!(
            version: le_u8 >>
            server_pin: le_u32 >>
            (Package::Type7(PackageData7 {
                version,
                server_pin,
            }))
        )
    );

named!(
        parse_type_10<&[u8], Package>,
        do_parse!(
            version: le_u8 >>
            pattern: take!(40) >>
            (Package::Type10(PackageData10 {
                version,
                pattern: read_nul_terminated(pattern)?,
            }))
        )
    );

fn parse_type_255(input: &[u8]) -> Result<(&[u8], Package), nom::Err<&[u8]>> {
    Ok((
        input,
        Package::Type255(PackageData255 {
            message: read_nul_terminated(input)?,
        }),
    ))
}

pub fn deserialize(package_type: u8, input: &[u8]) -> Result<Package, String> {
    let data: Result<(&[u8], Package), nom::Err<&[u8]>> = match package_type {
        0x01 => parse_type_1(input),
        0x02 => parse_type_2(input),
        0x03 => parse_type_3(input),
        0x04 => Ok((input, Package::Type4(PackageData4 {}))),
        0x05 => parse_type_5(input),
        0x06 => parse_type_6(input),
        0x07 => parse_type_7(input),
        0x08 => Ok((input, Package::Type8(PackageData8 {}))),
        0x09 => Ok((input, Package::Type9(PackageData9 {}))),
        0x0A => parse_type_10(input),
        0xFF => parse_type_255(input),

        _ => return Err("unrecognized package type".into()),
    };

    let package = data
        .map_err(|err| {
            String::from(format!(
                "failed to parse package (type {}): {}",
                package_type, err
            ))
        })?
        .1;

    Ok(package)
}

// ! ################################################################################

extern crate byteorder;
use byteorder::{LittleEndian, WriteBytesExt};

pub fn serialize(package: Package) -> Vec<u8> {
    match package {
        Package::Type1(package) => {
            let package_type: u8 = 0x01;
            let package_length: u8 = 8;

            let mut buf: Vec<u8> = Vec::with_capacity(package_length as usize + 2);

            buf.write_u8(package_type).unwrap();
            buf.write_u8(package_length).unwrap();

            buf.write_u32::<LittleEndian>(package.number).unwrap();
            buf.write_u16::<LittleEndian>(package.pin).unwrap();
            buf.write_u16::<LittleEndian>(package.port).unwrap();

            buf
        }
        Package::Type2(package) => {
            let package_type: u8 = 0x02;
            let package_length: u8 = 4;

            let mut buf: Vec<u8> = Vec::with_capacity(package_length as usize + 2);

            buf.write_u8(package_type).unwrap();
            buf.write_u8(package_length).unwrap();

            buf.write(&package.ipaddress.octets()).unwrap();

            buf
        }
        Package::Type3(package) => {
            let package_type: u8 = 0x03;
            let package_length: u8 = 5;

            let mut buf: Vec<u8> = Vec::with_capacity(package_length as usize + 2);

            buf.write_u8(package_type).unwrap();
            buf.write_u8(package_length).unwrap();

            buf.write_u32::<LittleEndian>(package.number).unwrap();
            buf.write_u8(package.version).unwrap();

            buf
        }
        Package::Type4(_package) => {
            let package_type: u8 = 0x04;
            let package_length: u8 = 0;

            let mut buf: Vec<u8> = Vec::with_capacity(package_length as usize + 2);

            buf.write_u8(package_type).unwrap();
            buf.write_u8(package_length).unwrap();

            buf
        }
        Package::Type5(package) => {
            let package_type: u8 = 0x05;
            let package_length: u8 = 100;

            let mut buf: Vec<u8> = Vec::with_capacity(package_length as usize + 2);

            buf.write_u8(package_type).unwrap();
            buf.write_u8(package_length).unwrap();

            buf.write_u32::<LittleEndian>(package.number).unwrap();
            buf.write(string_to_n_bytes(package.name, 40).as_slice())
                .unwrap();
            buf.write_u16::<LittleEndian>(package.flags).unwrap();
            buf.write_u8(package.client_type).unwrap();
            buf.write(string_to_n_bytes(package.hostname, 40).as_slice())
                .unwrap();
            buf.write(&package.ipaddress.octets()).unwrap();
            buf.write_u16::<LittleEndian>(package.port).unwrap();
            buf.write_u8(package.extension).unwrap();
            buf.write_u16::<LittleEndian>(package.pin).unwrap();
            buf.write_u32::<LittleEndian>(package.date).unwrap();

            buf
        }
        Package::Type6(package) => {
            let package_type: u8 = 0x06;
            let package_length: u8 = 5;

            let mut buf: Vec<u8> = Vec::with_capacity(package_length as usize + 2);

            buf.write_u8(package_type).unwrap();
            buf.write_u8(package_length).unwrap();

            buf.write_u8(package.version).unwrap();
            buf.write_u32::<LittleEndian>(package.server_pin).unwrap();

            buf
        }
        Package::Type7(package) => {
            let package_type: u8 = 0x07;
            let package_length: u8 = 5;

            let mut buf: Vec<u8> = Vec::with_capacity(package_length as usize + 2);

            buf.write_u8(package_type).unwrap();
            buf.write_u8(package_length).unwrap();

            buf.write_u8(package.version).unwrap();
            buf.write_u32::<LittleEndian>(package.server_pin).unwrap();

            buf
        }
        Package::Type8(_package) => {
            let package_type: u8 = 0x08;
            let package_length: u8 = 0;

            let mut buf: Vec<u8> = Vec::with_capacity(package_length as usize + 2);

            buf.write_u8(package_type).unwrap();
            buf.write_u8(package_length).unwrap();

            buf
        }
        Package::Type9(_package) => {
            let package_type: u8 = 0x09;
            let package_length: u8 = 0;

            let mut buf: Vec<u8> = Vec::with_capacity(package_length as usize + 2);

            buf.write_u8(package_type).unwrap();
            buf.write_u8(package_length).unwrap();

            buf
        }
        Package::Type10(package) => {
            let package_type: u8 = 0x0A;
            let package_length: u8 = 41;

            let mut buf: Vec<u8> = Vec::with_capacity(package_length as usize + 2);

            buf.write_u8(package_type).unwrap();
            buf.write_u8(package_length).unwrap();

            buf.write_u8(package.version).unwrap();
            buf.write(string_to_n_bytes(package.pattern, 40).as_slice())
                .unwrap();

            buf
        }

        Package::Type255(package) => {
            let package_type: u8 = 0xFF;

            let message = package.message.into_bytes_with_nul();

            let package_length: u8 = message.capacity() as u8;

            let mut buf: Vec<u8> = Vec::with_capacity(package_length as usize + 2);

            buf.write_u8(package_type).unwrap();
            buf.write_u8(package_length).unwrap();

            buf.write(&message).unwrap();

            buf
        }
    }
}

fn string_to_n_bytes(input: CString, n: usize) -> Vec<u8> {
    let mut buf = input.into_bytes(); // data without nul

    buf.truncate(n - 1); // leave space for at least one nul

    buf.extend(vec![0u8; n - buf.len()]); // fill buffer up to `n` with 0u8

    buf
}
