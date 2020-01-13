use anyhow::Context;
use std::time::Duration;

#[derive(Debug)]
pub struct Config {
    pub CLIENT_TIMEOUT: Duration,
    pub SERVER_COOLDOWN: Duration,
    pub CHANGED_SYNC_INTERVAL: Duration,
    pub DB_SYNC_INTERVAL: Duration,
    pub FULL_QUERY_INTERVAL: Duration,
    pub SERVER_PORT: u16,
    pub SERVER_PIN: u32,
    pub DB_PATH: String,
    pub DB_PATH_TEMP: String,
    pub SERVER_FILE_PATH: String,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        use std::env::var;
        Ok(Config {
            CLIENT_TIMEOUT: parse_duration_from_string(var("CLIENT_TIMEOUT")?)?,
            SERVER_COOLDOWN: parse_duration_from_string(var("SERVER_COOLDOWN")?)?,
            CHANGED_SYNC_INTERVAL: parse_duration_from_string(var("CHANGED_SYNC_INTERVAL")?)?,
            DB_SYNC_INTERVAL: parse_duration_from_string(var("DB_SYNC_INTERVAL")?)?,
            FULL_QUERY_INTERVAL: parse_duration_from_string(var("FULL_QUERY_INTERVAL")?)?,
            SERVER_PORT: var("SERVER_PORT")?.parse()?,
            SERVER_PIN: var("SERVER_PIN")?.parse()?,
            DB_PATH: var("DB_PATH")?,
            DB_PATH_TEMP: var("DB_PATH_TEMP")?,
            SERVER_FILE_PATH: var("SERVER_FILE_PATH")?,
        })
    }
}

fn parse_duration_from_string(input: String) -> anyhow::Result<Duration> {
    let mut parts = input.split(".");
    let number: u64 = parts.next().context("variable was empty")?.parse()?;

    if let Some(unit) = parts.next() {
        Ok(match unit {
            "s" => Duration::from_secs(number),
            "m" => Duration::from_secs(number * 60),
            "h" => Duration::from_secs(number * 60 * 60),
            "d" => Duration::from_secs(number * 24 * 60 * 60),

            _ => bail!("unknown unit"),
        })
    } else {
        Ok(Duration::from_secs(number))
    }
}
