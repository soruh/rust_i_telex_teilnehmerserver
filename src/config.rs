use anyhow::Context;
use std::time::Duration;

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
    pub SERVERS: Vec<String>,
    pub LOG_FILE_PATH: Option<String>,
    pub LOG_LEVEL_FILE: Option<String>,
    pub LOG_LEVEL_TERM: Option<String>,

    pub WEBSERVER_PORT: u16,
    pub WEBSERVER_PASSWORD: String,
    pub WEBSERVER_SESSION_LIFETIME: Duration,
    pub WEBSERVER_REMOVE_SESSIONS_INTERVAL: Duration,
    pub WEBSERVER_SESSION_SECRET: Vec<u8>,

    pub MIN_PASSWORD_HASH_TIME: Duration,
}

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        #[derive(Debug)]
        struct Censored;

        f.debug_struct("Config")
            .field("client timeout", &self.CLIENT_TIMEOUT)
            .field("server cooldown", &self.SERVER_COOLDOWN)
            .field("changed sync interval", &self.CHANGED_SYNC_INTERVAL)
            .field("db sync interval", &self.DB_SYNC_INTERVAL)
            .field("full query interval", &self.FULL_QUERY_INTERVAL)
            .field("server port", &self.SERVER_PORT)
            .field("server pin", &self.SERVER_PIN)
            .field("db path", &self.DB_PATH)
            .field("db path temp", &self.DB_PATH_TEMP)
            .field("servers", &self.SERVERS)
            .field("log file path", &self.LOG_FILE_PATH)
            .field("log level file", &self.LOG_LEVEL_FILE)
            .field("log level term", &self.LOG_LEVEL_TERM)
            .field("webserver port", &self.WEBSERVER_PORT)
            .field("webserver password", &self.WEBSERVER_PASSWORD)
            .field("webserver session lifetime", &self.WEBSERVER_SESSION_LIFETIME)
            .field("webserver remove_sessions interval", &self.WEBSERVER_REMOVE_SESSIONS_INTERVAL)
            .field("minimum passwort hashing time", &self.MIN_PASSWORD_HASH_TIME)
            .field("webserver session secret", &Censored)
            .finish()
    }
}

macro_rules! get_variable {
    ($name:literal) => {
        var($name).context(format!("Failed to get config variable `{}`", $name))?
    };
}

macro_rules! parse_duration {
    ($name:literal) => {
        duration_from_string(get_variable!($name))
            .context(format!("Failed to parse config variable {} as {}", $name, "duration"))?
    };
}

macro_rules! parse_from_str {
    ($name:literal) => {
        get_variable!($name)
            .parse()
            .context(format!("Failed to parse config variable {} as {}", $name, "number"))?
    };
}

macro_rules! parse_bytes_from_base64_str {
    ($name:literal) => {
        base64::decode(get_variable!($name))
            .context(format!("Failed to parse config variable {} as {}", $name, "base64"))?
    };
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        use std::env::var;
        Ok(Self {
            CLIENT_TIMEOUT: parse_duration!("CLIENT_TIMEOUT"),
            SERVER_COOLDOWN: parse_duration!("SERVER_COOLDOWN"),
            CHANGED_SYNC_INTERVAL: parse_duration!("CHANGED_SYNC_INTERVAL"),
            DB_SYNC_INTERVAL: parse_duration!("DB_SYNC_INTERVAL"),
            FULL_QUERY_INTERVAL: parse_duration!("FULL_QUERY_INTERVAL"),
            SERVER_PORT: parse_from_str!("SERVER_PORT"),
            SERVER_PIN: parse_from_str!("SERVER_PIN"),
            DB_PATH: get_variable!("DB_PATH"),
            DB_PATH_TEMP: get_variable!("DB_PATH_TEMP"),
            LOG_FILE_PATH: var("LOG_FILE_PATH").ok(),
            LOG_LEVEL_FILE: var("LOG_LEVEL_FILE").ok(),
            LOG_LEVEL_TERM: var("LOG_LEVEL_TERM").ok(),
            WEBSERVER_PORT: parse_from_str!("WEBSERVER_PORT"),
            WEBSERVER_SESSION_SECRET: parse_bytes_from_base64_str!("WEBSERVER_SESSION_SECRET"),
            WEBSERVER_PASSWORD: get_variable!("WEBSERVER_PASSWORD"),
            WEBSERVER_SESSION_LIFETIME: parse_duration!("WEBSERVER_SESSION_LIFETIME"),
            WEBSERVER_REMOVE_SESSIONS_INTERVAL: parse_duration!(
                "WEBSERVER_REMOVE_SESSIONS_INTERVAL"
            ),
            MIN_PASSWORD_HASH_TIME: parse_duration!("MIN_PASSWORD_HASH_TIME"),
            SERVERS: parse_servers(get_variable!("SERVERS")),
        })
    }
}

fn parse_servers(input: String) -> Vec<String> {
    input.split(',').map(str::trim).filter(|x| !x.is_empty()).map(str::to_string).collect()
}

fn duration_from_string(input: String) -> anyhow::Result<Duration> {
    let mut parts = input.split('.');
    let number: u64 = parts.next().context("variable was empty")?.parse()?;

    if let Some(unit) = parts.next() {
        match unit {
            "ms" => Ok(Duration::from_millis(number)),
            "s" => Ok(Duration::from_secs(number)),
            "m" => Ok(Duration::from_secs(number * 60)),
            "h" => Ok(Duration::from_secs(number * 60 * 60)),
            "d" => Ok(Duration::from_secs(number * 24 * 60 * 60)),
            "w" => Ok(Duration::from_secs(number * 7 * 24 * 60 * 60)),

            _ => Err(anyhow!("unknown unit: `{}`", unit)),
        }
    } else {
        Ok(Duration::from_secs(number))
    }
    .context("Failed to parse duration")
}
