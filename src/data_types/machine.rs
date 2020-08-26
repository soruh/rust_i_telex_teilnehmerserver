use super::*;

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct Machine {
    id: MachineId,
    number: u32,
    name: String,
    model: String,
    extension: Extension,
    compat_name: String,

    connector: ConnectorId,
    timestamp: u64,
}

#[derive(serde::Serialize, serde::Deserialize, Hash, PartialEq, Eq, Clone, Copy)]
pub struct MachineId(uuid::Uuid);
