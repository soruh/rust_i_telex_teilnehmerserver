#[derive(serde::Deserialize, Debug)]
pub struct LoginRequest {
    pub password: String,
}

#[derive(serde::Serialize, Debug)]
pub struct LoggedInResponse(pub bool); // TODO: remove?

#[derive(Debug)]
pub struct ApiError(pub String);

#[cfg(debug_assertions)]
impl serde::Serialize for ApiError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

#[cfg(not(debug_assertions))]
impl serde::Serialize for ApiError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str("Internal Server Error")
    }
}
