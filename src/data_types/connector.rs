use super::*;

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct Connector {
    id: ConnectorId,
    address: String,
    port: u32,
    timestamp: u64,

    owner: UserId,

    pin: u32,
}

#[derive(serde::Serialize, serde::Deserialize, Hash, PartialEq, Eq, Clone, Copy)]
pub struct ConnectorId(uuid::Uuid);
