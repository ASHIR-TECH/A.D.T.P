use std::fmt;

#[derive(Debug, serde::Serialize)]
pub enum CliError {
    Config { message: String },
    Api { status: u16, message: String },
    ChainTampered { first_failure_id: String },
    PermissionDenied { message: String },
    Http { message: String },
    Io { message: String },
    Parse { message: String },
    Interrupted,
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config { message } => write!(f, "Configuration error: {message}"),
            Self::Api { status, message } => {
                write!(f, "API error ({status}): {message}")
            }
            Self::ChainTampered { first_failure_id } => {
                write!(f, "Audit chain TAMPERED — first failure at {first_failure_id}")
            }
            Self::PermissionDenied { message } => write!(f, "Permission denied: {message}"),
            Self::Http { message } => write!(f, "Connection error: {message}"),
            Self::Io { message } => write!(f, "I/O error: {message}"),
            Self::Parse { message } => write!(f, "Parse error: {message}"),
            Self::Interrupted => write!(f, "Interrupted"),
        }
    }
}

impl From<anyhow::Error> for CliError {
    fn from(e: anyhow::Error) -> Self {
        Self::Http { message: e.to_string() }
    }
}

impl From<reqwest::Error> for CliError {
    fn from(e: reqwest::Error) -> Self {
        Self::Http { message: e.to_string() }
    }
}

impl From<toml::de::Error> for CliError {
    fn from(e: toml::de::Error) -> Self {
        Self::Parse { message: e.to_string() }
    }
}

impl From<toml::ser::Error> for CliError {
    fn from(e: toml::ser::Error) -> Self {
        Self::Parse { message: e.to_string() }
    }
}

impl From<std::io::Error> for CliError {
    fn from(e: std::io::Error) -> Self {
        Self::Io { message: e.to_string() }
    }
}

impl std::error::Error for CliError {}

pub fn exit_code(err: &CliError) -> i32 {
    match err {
        CliError::ChainTampered { .. } => 2,
        CliError::PermissionDenied { .. } => 3,
        CliError::Interrupted => 130,
        _ => 1,
    }
}

pub fn hint(err: &CliError) -> Option<&'static str> {
    match err {
        CliError::Config { message } if message.contains("TOKEN") => {
            Some("Set ADTP_API_TOKEN env var or run `adtp config set api-token`")
        }
        CliError::Http { message } if message.contains("Cannot connect") => {
            Some("Is adtp-api running? Try `systemctl start adtp-api` or `adtp status`")
        }
        CliError::Api { status, .. } => match status {
            401 => Some("Check your API token — set ADTP_API_TOKEN or run `adtp config set api-token`"),
            404 => Some("The resource was not found. Check the ID or path."),
            413 => Some("The file exceeds the maximum allowed size."),
            429 => Some("You are being rate-limited. Wait and retry."),
            503 => Some("The daemon is down. Start it with `systemctl start adtp-client`"),
            _ => Some("Check the API server status and your configuration."),
        },
        CliError::PermissionDenied { .. } => {
            Some("Use `adtp agent permissions` to grant access or update rules.")
        }
        CliError::ChainTampered { .. } => {
            Some("Audit chain integrity is compromised. Investigate immediately.")
        }
        CliError::Io { .. } => Some("Check file permissions and disk space."),
        _ => None,
    }
}
