use super::*;

#[derive(serde::Serialize, serde::Deserialize, Hash, PartialEq, Eq, Clone, Copy)]
pub struct ConnectorId(uuid::Uuid);

impl std::fmt::Debug for ConnectorId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ConnectorId({})", self.0)
    }
}

impl From<uuid::Uuid> for ConnectorId {
    fn from(uuid: uuid::Uuid) -> Self {
        Self(uuid)
    }
}
impl AsRef<[u8]> for ConnectorId {
    fn as_ref(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl From<sled::IVec> for ConnectorId {
    fn from(id: sled::IVec) -> Self {
        uuid::Uuid::from_slice(id.as_ref()).expect("Key had too few bytes").into()
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Connector {
    pub id: ConnectorId,
    pub address: String,
    pub port: u32,
    pub timestamp: u64,

    pub owner: UserId,

    pub pin: u32,
}

#[allow(clippy::fallible_impl_from)]
impl From<sled::IVec> for Connector {
    fn from(value: sled::IVec) -> Self {
        rmp_serde::from_read_ref(&value).expect("Failed to deserialize connector database")
    }
}

impl Into<sled::IVec> for &Connector {
    fn into(self) -> sled::IVec {
        rmp_serde::to_vec(self).expect("Failed to serialize connector database").into()
    }
}
impl Into<sled::IVec> for Connector {
    fn into(self) -> sled::IVec {
        (&self).into()
    }
}
