use std::fmt;

#[derive(Debug)]
pub enum CliError {
    Config { message: String },
    Api { status: u16, message: String },
    ChainTampered { first_failure_id: String },
    Http { message: String },
    Io { message: String },
    Parse { message: String },
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
            Self::Http { message } => write!(f, "Connection error: {message}"),
            Self::Io { message } => write!(f, "I/O error: {message}"),
            Self::Parse { message } => write!(f, "Parse error: {message}"),
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

impl From<std::io::Error> for CliError {
    fn from(e: std::io::Error) -> Self {
        Self::Io { message: e.to_string() }
    }
}

impl std::error::Error for CliError {}

pub fn exit_code(err: &CliError) -> i32 {
    match err {
        CliError::ChainTampered { .. } => 2,
        CliError::Io { .. } => 2,
        _ => 1,
    }
}
