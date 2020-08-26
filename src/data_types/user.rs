#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct User {
    id: UserId,
    name: String,
    email: Option<String>,
    city: Option<String>,
    coordinates: Option<(f64, f64)>, // lat, long
    timestamp: u64,

    password: String,
}
#[derive(serde::Serialize, serde::Deserialize, Hash, PartialEq, Eq, Clone, Copy)]
pub struct UserId(uuid::Uuid);
