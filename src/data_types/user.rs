#[derive(serde::Serialize, serde::Deserialize, Hash, PartialEq, Eq, Clone, Copy)]
pub struct UserId(uuid::Uuid);

impl std::fmt::Debug for UserId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "UserId({})", self.0)
    }
}

impl From<uuid::Uuid> for UserId {
    fn from(uuid: uuid::Uuid) -> Self {
        Self(uuid)
    }
}
impl AsRef<[u8]> for UserId {
    fn as_ref(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl From<sled::IVec> for UserId {
    fn from(id: sled::IVec) -> Self {
        uuid::Uuid::from_slice(id.as_ref()).expect("Key had too few bytes").into()
    }
}
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct User {
    pub id: UserId,
    pub name: String,
    pub email: Option<String>,
    pub city: Option<String>,
    pub coordinates: Option<(f64, f64)>, // lat, long
    pub timestamp: u64,

    pub password: String,
}

impl From<sled::IVec> for User {
    fn from(value: sled::IVec) -> Self {
        rmp_serde::from_read_ref(&value).unwrap()
    }
}

impl Into<sled::IVec> for &User {
    fn into(self) -> sled::IVec {
        rmp_serde::to_vec(self).unwrap().into()
    }
}

impl Into<sled::IVec> for User {
    fn into(self) -> sled::IVec {
        (&self).into()
    }
}
