use crate::packages::*;
use std::convert::TryInto;

#[must_use]
pub fn serialize(package: Package) -> anyhow::Result<Vec<u8>> {
    Ok(package.try_into()?)
}

pub fn deserialize(package_type: u8, slice: &[u8]) -> anyhow::Result<Package> {
    Package::parse(package_type, slice)
}

#[cfg(test)]
mod tests {
    use super::{deserialize, serialize};
    use crate::packages::*;
    use std::convert::{TryFrom, TryInto};
    use std::net::Ipv4Addr;

    fn test_both(package_type: u8, package: Package, serialized: Vec<u8>) {
        assert_eq!(
            deserialize(package_type, serialized.as_slice())
                .expect("Failed to convert from slice to Package"),
            package,
            "deserialize created unexpected result"
        );

        assert_eq!(
            serialize(package).expect("Failed to convert from Package to slice"),
            serialized,
            "serialisation created unexpected result"
        );
    }

    #[test]
    fn type_1() {
        let serialized: Vec<u8> = vec![
            // number:
            0x0f, 0xf0, 0x00, 0xff, // pin:
            0x0f, 0xf0, // port:
            0xf0, 0x0f,
        ];

        let package = Package::Type1(Package1 {
            number: 0xff_00_f0_0f,
            pin: 0xf0_0f,
            port: 0x0f_f0,
        });

        test_both(1, package, serialized);
    }

    #[test]
    fn type_2() {
        let serialized: Vec<u8> = vec![
            // ipaddress
            0xff, 0x00, 0xf0, 0x0f,
        ];

        let package = Package::Type2(Package2 {
            ipaddress: Ipv4Addr::from([0xff, 0x00, 0xf0, 0x0f]),
        });

        test_both(2, package, serialized);
    }

    #[test]
    fn type_3() {
        let serialized: Vec<u8> = vec![
            // number:
            0x44, 0x33, 0x22, 0x11, // version:
            0xf7,
        ];

        let package = Package::Type3(Package3 {
            number: 0x11_22_33_44,
            version: 0xf7,
        });

        test_both(3, package, serialized);
    }

    #[test]
    fn type_4() {
        let serialized: Vec<u8> = vec![];

        let package = Package::Type4(Package4 {});

        test_both(4, package, serialized);
    }

    #[test]
    fn type_5() {
        let serialized: Vec<u8> = vec![
            // number:
            4, 3, 2, 1, // name:
            84, 101, 115, 116, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // flags:
            2, 0, // client_type:
            7, // hostname:
            104, 111, 115, 116, 46, 110, 97, 109, 101, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // ipaddress:
            8, 9, 0x0a, 0x0b, // port:
            0x0d, 0x0c, // extension:
            0x0e, // pin:
            0x10, 0x0f, //timestamp:
            0x14, 0x13, 0x12, 0x11,
        ];

        let package = Package::Type5(Package5 {
            number: 0x01_02_03_04,
            name: String::from("Test"),
            disabled: true,
            client_type: 0x07,
            hostname: Some(String::from("host.name")),
            ipaddress: Some(Ipv4Addr::from(0x08_09_0a_0b)),
            port: 0x0c_0d,
            extension: 0x0e,
            pin: 0x0f_10,
            timestamp: 0x11_12_13_14,
        });

        test_both(5, package, serialized);
    }

    #[test]
    fn type_6() {
        let serialized: Vec<u8> = vec![0x0f, 0x11, 0x22, 0x33, 0x44];

        let package = Package::Type6(Package6 {
            server_pin: 0x44_33_22_11,
            version: 0x0f,
        });
        test_both(6, package, serialized);
    }

    #[test]
    fn type_7() {
        let serialized: Vec<u8> = vec![0x0f, 0x11, 0x22, 0x33, 0x44];

        let package = Package::Type7(Package7 {
            server_pin: 0x44_33_22_11,
            version: 0x0f,
        });
        test_both(7, package, serialized);
    }

    #[test]
    fn type_8() {
        let serialized: Vec<u8> = vec![];

        let package = Package::Type8(Package8 {});
        test_both(8, package, serialized);
    }

    #[test]
    fn type_9() {
        let serialized: Vec<u8> = vec![];

        let package = Package::Type9(Package9 {});
        test_both(9, package, serialized);
    }

    #[test]
    fn type_10() {
        let serialized: Vec<u8> = vec![
            // version:
            240, // pattern:
            80, 97, 116, 116, 101, 114, 110, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ];

        let package = Package::Type10(Package10 {
            pattern: String::from("Pattern"),
            version: 0xf0,
        });
        test_both(10, package, serialized);
    }

    #[test]
    fn type_255() {
        let serialized: Vec<u8> = vec![
            // message:
            65, 110, 32, 69, 114, 114, 111, 114, 32, 104, 97, 115, 32, 111, 99, 99, 117, 114, 101,
            100, 33, 0,
        ];

        let package = Package::Type255(Package255 {
            message: String::from("An Error has occured!"),
        });
        test_both(255, package, serialized);
    }
}
