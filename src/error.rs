use std::fmt;

#[derive(Debug, thiserror::Error)]
pub enum BzrError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Config error: {0}")]
    Config(String),

    #[error("Bugzilla API error: {message} (code {code})")]
    Api { code: i64, message: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("TOML parse error: {0}")]
    TomlParse(#[from] toml::de::Error),

    #[error("TOML serialize error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, BzrError>;

impl BzrError {
    pub fn config(msg: impl fmt::Display) -> Self {
        BzrError::Config(msg.to_string())
    }
}
