use super::*;

#[derive(serde::Serialize, serde::Deserialize, Hash, PartialEq, Eq, Clone, Copy)]
pub struct MachineId(uuid::Uuid);

impl std::fmt::Debug for MachineId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MachineId({})", self.0)
    }
}

impl From<uuid::Uuid> for MachineId {
    fn from(uuid: uuid::Uuid) -> Self {
        Self(uuid)
    }
}

impl From<sled::IVec> for MachineId {
    fn from(id: sled::IVec) -> Self {
        uuid::Uuid::from_slice(id.as_ref()).expect("Key had too few bytes").into()
    }
}

impl AsRef<[u8]> for MachineId {
    fn as_ref(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Machine {
    pub id: MachineId,
    pub number: u32,
    pub name: String,
    pub model: String,
    pub extension: Extension,
    pub compat_name_overwrite: Option<String>,

    pub connector: ConnectorId,
    pub timestamp: u64,
}

#[allow(clippy::fallible_impl_from)]
impl From<sled::IVec> for Machine {
    fn from(value: sled::IVec) -> Self {
        rmp_serde::from_read_ref(&value).expect("Failed to deserialize machine database")
    }
}

impl Into<sled::IVec> for &Machine {
    fn into(self) -> sled::IVec {
        rmp_serde::to_vec(self).expect("Failed to serialize machine database").into()
    }
}

impl Into<sled::IVec> for Machine {
    fn into(self) -> sled::IVec {
        (&self).into()
    }
}
