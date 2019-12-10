use crate::errors::MyErrorKind;
use crate::packages::*;
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
            timestamp: le_u32 >>
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
                timestamp
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

// TODO: should this take the whole buffer, including the header?
pub fn deserialize(package_type: u8, input: &[u8]) -> anyhow::Result<Package> {
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

        _ => Err(MyErrorKind::ParseFailure(package_type))?,
    };

    if let Ok(data) = data {
        Ok(data.1)
    } else {
        // TODO: do something with `error`?
        Err(MyErrorKind::ParseFailure(package_type))?
    }
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
            buf.write_u32::<LittleEndian>(package.timestamp).unwrap();

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

#[cfg(test)]
mod tests {
    use super::{deserialize, serialize};
    use crate::packages::*;

    fn test_both(package: Package, serialized: Vec<u8>) {
        let package_type = serialized[0];
        let package_length = serialized[1];
        let package_data: &[u8] = &serialized[2..2 + package_length as usize];

        assert_eq!(
            deserialize(package_type, package_data).expect("deserialisation failed"),
            package,
            "serialisation created unexpected result"
        );

        assert_eq!(
            serialize(package),
            serialized,
            "serialisation created unexpected result"
        );
    }

    #[test]
    fn type_1() {
        let serialized: Vec<u8> = vec![
            // header:
            0x01, 0x08, // number:
            0x0f, 0xf0, 0x00, 0xff, // pin:
            0x0f, 0xf0, // port:
            0xf0, 0x0f,
        ];

        let package = Package::Type1(PackageData1 {
            number: 0xff_00_f0_0f,
            pin: 0xf0_0f,
            port: 0x0f_f0,
        });

        test_both(package, serialized);
    }

    #[test]
    fn type_2() {
        let serialized: Vec<u8> = vec![
            // header:
            0x02, 0x04, // ipaddress
            0xff, 0x00, 0xf0, 0x0f,
        ];

        let package = Package::Type2(PackageData2 {
            ipaddress: Ipv4Addr::from([0xff, 0x00, 0xf0, 0x0f]),
        });

        test_both(package, serialized);
    }

    #[test]
    fn type_3() {
        let serialized: Vec<u8> = vec![
            // header:
            0x03, 0x05, // number:
            0x44, 0x33, 0x22, 0x11, // version:
            0xf7,
        ];

        let package = Package::Type3(PackageData3 {
            number: 0x11_22_33_44,
            version: 0xf7,
        });

        test_both(package, serialized);
    }

    #[test]
    fn type_4() {
        let serialized: Vec<u8> = vec![0x04, 0x00];

        let package = Package::Type4(PackageData4 {});

        test_both(package, serialized);
    }

    #[test]
    fn type_5() {
        let serialized: Vec<u8> = vec![
            // header:
            5, 100, // number:
            4, 3, 2, 1, // name:
            84, 101, 115, 116, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // flags:
            6, 5, // client_type:
            7, // hostname:
            104, 111, 115, 116, 46, 110, 97, 109, 101, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // ipaddress:
            8, 9, 0x0a, 0x0b, // port:
            0x0d, 0x0c, // extension:
            0x0e, // pin:
            0x10, 0x0f, //timestamp:
            0x14, 0x13, 0x12, 0x11,
        ];

        let package = Package::Type5(PackageData5 {
            number: 0x01_02_03_04,
            name: CString::new("Test").unwrap(),
            flags: 0x05_06,
            client_type: 0x07,
            hostname: CString::new("host.name").unwrap(),
            ipaddress: Ipv4Addr::from(0x08_09_0a_0b),
            port: 0x0c_0d,
            extension: 0x0e,
            pin: 0x0f_10,
            timestamp: 0x11_12_13_14,
        });

        test_both(package, serialized);
    }

    #[test]
    fn type_6() {
        let serialized: Vec<u8> = vec![0x06, 0x05, 0x0f, 0x11, 0x22, 0x33, 0x44];

        let package = Package::Type6(PackageData6 {
            server_pin: 0x44_33_22_11,
            version: 0x0f,
        });
        test_both(package, serialized);
    }

    #[test]
    fn type_7() {
        let serialized: Vec<u8> = vec![0x07, 0x05, 0x0f, 0x11, 0x22, 0x33, 0x44];

        let package = Package::Type7(PackageData7 {
            server_pin: 0x44_33_22_11,
            version: 0x0f,
        });
        test_both(package, serialized);
    }

    #[test]
    fn type_8() {
        let serialized: Vec<u8> = vec![0x08, 0x00];

        let package = Package::Type8(PackageData8 {});
        test_both(package, serialized);
    }

    #[test]
    fn type_9() {
        let serialized: Vec<u8> = vec![0x09, 0x00];

        let package = Package::Type9(PackageData9 {});
        test_both(package, serialized);
    }

    #[test]
    fn type_10() {
        let serialized: Vec<u8> = vec![
            // header:
            10, 41,  // / version:
            240, // pattern:
            80, 97, 116, 116, 101, 114, 110, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ];

        let package = Package::Type10(PackageData10 {
            pattern: CString::new("Pattern").unwrap(),
            version: 0xf0,
        });
        test_both(package, serialized);
    }

    #[test]
    fn type_255() {
        let serialized: Vec<u8> = vec![
            // header:
            255, 22, // message:
            65, 110, 32, 69, 114, 114, 111, 114, 32, 104, 97, 115, 32, 111, 99, 99, 117, 114, 101,
            100, 33, 0,
        ];

        let package = Package::Type255(PackageData255 {
            message: CString::new("An Error has occured!").unwrap(),
        });
        test_both(package, serialized);
    }
}
