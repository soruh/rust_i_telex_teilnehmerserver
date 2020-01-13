use anyhow::Context;
use std::time::Duration;

#[derive(Debug)]
#[allow(non_snake_case)]
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

macro_rules! get_variable {
    ($name:literal) => {
        var($name).context(concat!("Failed to parse config variable `", $name, "`"))?
    };
}

macro_rules! parse_duration {
    ($name:literal) => {
        duration_from_string(get_variable!($name)).context(concat!(
            "Failed to parse config variable ",
            $name,
            " as duration"
        ))?
    };
}

macro_rules! parse_number {
    ($name:literal) => {
        get_variable!($name).parse().context(concat!("Failed to parse config variable ", $name, " as number"))?
    };
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        use std::env::var;
        Ok(Config {
            CLIENT_TIMEOUT: parse_duration!("CLIENT_TIMEOUT"),
            SERVER_COOLDOWN: parse_duration!("SERVER_COOLDOWN"),
            CHANGED_SYNC_INTERVAL: parse_duration!("CHANGED_SYNC_INTERVAL"),
            DB_SYNC_INTERVAL: parse_duration!("DB_SYNC_INTERVAL"),
            FULL_QUERY_INTERVAL: parse_duration!("FULL_QUERY_INTERVAL"),
            SERVER_PORT: parse_number!("SERVER_PORT"),
            SERVER_PIN: parse_number!("SERVER_PIN"),
            DB_PATH: get_variable!("DB_PATH"),
            DB_PATH_TEMP: get_variable!("DB_PATH_TEMP"),
            SERVER_FILE_PATH: get_variable!("SERVER_FILE_PATH"),
        })
    }
}

fn duration_from_string(input: String) -> anyhow::Result<Duration> {
    let mut parts = input.split(".");
    let number: u64 = parts.next().context("variable was empty")?.parse()?;

    if let Some(unit) = parts.next() {
        match unit {
            "s" => Ok(Duration::from_secs(number)),
            "m" => Ok(Duration::from_secs(number * 60)),
            "h" => Ok(Duration::from_secs(number * 60 * 60)),
            "d" => Ok(Duration::from_secs(number * 24 * 60 * 60)),

            _ => Err(anyhow!("unknown unit: `{}`", unit)),
        }
    } else {
        Ok(Duration::from_secs(number))
    }
    .context("Failed to parse duration")
}
