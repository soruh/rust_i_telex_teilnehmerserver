use crate::errors::MyErrorKind;
use crate::packages::*;
use std::convert::TryInto;
use std::mem::transmute;

#[must_use]
pub fn serialize(package: RawPackage) -> Vec<u8> {
    unsafe {
        match package {
            RawPackage::Type1(package) => {
                transmute::<RawPackage1, [u8; LENGTH_TYPE_1]>(package).to_vec()
            }
            RawPackage::Type2(package) => {
                transmute::<RawPackage2, [u8; LENGTH_TYPE_2]>(package).to_vec()
            }
            RawPackage::Type3(package) => {
                transmute::<RawPackage3, [u8; LENGTH_TYPE_3]>(package).to_vec()
            }
            RawPackage::Type4(package) => {
                transmute::<RawPackage4, [u8; LENGTH_TYPE_4]>(package).to_vec()
            }
            RawPackage::Type5(package) => {
                transmute::<RawPackage5, [u8; LENGTH_TYPE_5]>(package).to_vec()
            }
            RawPackage::Type6(package) => {
                transmute::<RawPackage6, [u8; LENGTH_TYPE_6]>(package).to_vec()
            }
            RawPackage::Type7(package) => {
                transmute::<RawPackage7, [u8; LENGTH_TYPE_7]>(package).to_vec()
            }
            RawPackage::Type8(package) => {
                transmute::<RawPackage8, [u8; LENGTH_TYPE_8]>(package).to_vec()
            }
            RawPackage::Type9(package) => {
                transmute::<RawPackage9, [u8; LENGTH_TYPE_9]>(package).to_vec()
            }
            RawPackage::Type10(package) => {
                transmute::<RawPackage10, [u8; LENGTH_TYPE_10]>(package).to_vec()
            }
            RawPackage::Type255(mut package) => {
                package.message.push(0);
                package.message
            }
        }
    }
}

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
                return Err(MyErrorKind::ParseFailure(5).into());
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
                return Err(MyErrorKind::ParseFailure(10).into());
            }
        }

        Ok(res)
    }
}

pub fn deserialize(package_type: u8, slice: &[u8]) -> anyhow::Result<RawPackage> {
    Ok(match package_type {
        0x01 => RawPackage::Type1(unsafe {
            transmute::<[u8; LENGTH_TYPE_1], RawPackage1>(slice.try_into()?)
        }),
        0x02 => RawPackage::Type2(unsafe {
            transmute::<[u8; LENGTH_TYPE_2], RawPackage2>(slice.try_into()?)
        }),
        0x03 => RawPackage::Type3(unsafe {
            transmute::<[u8; LENGTH_TYPE_3], RawPackage3>(slice.try_into()?)
        }),
        0x04 => RawPackage::Type4(unsafe {
            transmute::<[u8; LENGTH_TYPE_4], RawPackage4>(slice.try_into()?)
        }),
        0x05 => RawPackage::Type5(unsafe {
            transmute::<[u8; LENGTH_TYPE_5], RawPackage5>(ArrayImplWrapper(slice).try_into()?)
        }),
        0x06 => RawPackage::Type6(unsafe {
            transmute::<[u8; LENGTH_TYPE_6], RawPackage6>(slice.try_into()?)
        }),
        0x07 => RawPackage::Type7(unsafe {
            transmute::<[u8; LENGTH_TYPE_7], RawPackage7>(slice.try_into()?)
        }),
        0x08 => RawPackage::Type8(unsafe {
            transmute::<[u8; LENGTH_TYPE_8], RawPackage8>(slice.try_into()?)
        }),
        0x09 => RawPackage::Type9(unsafe {
            transmute::<[u8; LENGTH_TYPE_9], RawPackage9>(slice.try_into()?)
        }),
        0x0A => RawPackage::Type10(unsafe {
            transmute::<[u8; LENGTH_TYPE_10], RawPackage10>(ArrayImplWrapper(slice).try_into()?)
        }),
        0xFF => RawPackage::Type255(RawPackage255 {
            message: Vec::from(slice),
        }),

        _ => bail!(MyErrorKind::ParseFailure(package_type)),
    })
}

#[cfg(test)]
mod tests {
    use super::{deserialize, serialize};
    use crate::packages::*;
    use std::convert::{TryFrom, TryInto};
    use std::net::Ipv4Addr;

    fn test_both(package_type: u8, package: Package, serialized: Vec<u8>) {
        assert_eq!(
            Package::try_from(
                deserialize(package_type, serialized.as_slice()).expect("deserialisation failed")
            )
            .expect("Failed to convert from RawPackage to Package"),
            package,
            "deserialize created unexpected result"
        );

        assert_eq!(
            serialize(
                package
                    .try_into()
                    .expect("Failed to convert from Package to RawPackage")
            ),
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

        let package = Package::Type1(ProcessedPackage1 {
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

        let package = Package::Type2(ProcessedPackage2 {
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

        let package = Package::Type3(ProcessedPackage3 {
            number: 0x11_22_33_44,
            version: 0xf7,
        });

        test_both(3, package, serialized);
    }

    #[test]
    fn type_4() {
        let serialized: Vec<u8> = vec![];

        let package = Package::Type4(ProcessedPackage4 {});

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

        let package = Package::Type5(ProcessedPackage5 {
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

        let package = Package::Type6(ProcessedPackage6 {
            server_pin: 0x44_33_22_11,
            version: 0x0f,
        });
        test_both(6, package, serialized);
    }

    #[test]
    fn type_7() {
        let serialized: Vec<u8> = vec![0x0f, 0x11, 0x22, 0x33, 0x44];

        let package = Package::Type7(ProcessedPackage7 {
            server_pin: 0x44_33_22_11,
            version: 0x0f,
        });
        test_both(7, package, serialized);
    }

    #[test]
    fn type_8() {
        let serialized: Vec<u8> = vec![];

        let package = Package::Type8(ProcessedPackage8 {});
        test_both(8, package, serialized);
    }

    #[test]
    fn type_9() {
        let serialized: Vec<u8> = vec![];

        let package = Package::Type9(ProcessedPackage9 {});
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

        let package = Package::Type10(ProcessedPackage10 {
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

        let package = Package::Type255(ProcessedPackage255 {
            message: String::from("An Error has occured!"),
        });
        test_both(255, package, serialized);
    }
}
