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

    #[error("XML-RPC error: {0}")]
    XmlRpc(String),

    #[error("{resource} not found: {id}")]
    NotFound { resource: &'static str, id: String },

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, BzrError>;

impl BzrError {
    pub fn config(msg: impl fmt::Display) -> Self {
        BzrError::Config(msg.to_string())
    }

    pub fn exit_code(&self) -> i32 {
        match self {
            BzrError::Config(_) | BzrError::TomlParse(_) | BzrError::TomlSerialize(_) => 3,
            BzrError::Api { .. } | BzrError::XmlRpc(_) => 4,
            BzrError::Http(_) => 5,
            BzrError::Io(_) => 6,
            BzrError::NotFound { .. } => 2,
            BzrError::Other(_) => 1,
        }
    }

    pub fn error_type(&self) -> &'static str {
        match self {
            BzrError::Config(_) | BzrError::TomlParse(_) | BzrError::TomlSerialize(_) => "config",
            BzrError::Api { .. } | BzrError::XmlRpc(_) => "api",
            BzrError::Http(_) => "http",
            BzrError::Io(_) => "io",
            BzrError::NotFound { .. } => "not_found",
            BzrError::Other(_) => "other",
        }
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn exit_code_config() {
        let err = BzrError::Config("bad config".into());
        assert_eq!(err.exit_code(), 3);
    }

    #[test]
    fn exit_code_api() {
        let err = BzrError::Api {
            code: 101,
            message: "Invalid Bug ID".into(),
        };
        assert_eq!(err.exit_code(), 4);
    }

    #[test]
    fn exit_code_io() {
        let err = BzrError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "file not found",
        ));
        assert_eq!(err.exit_code(), 6);
    }

    #[test]
    fn exit_code_other() {
        let err = BzrError::Other("something went wrong".into());
        assert_eq!(err.exit_code(), 1);
    }

    #[test]
    fn exit_code_toml_parse() {
        let toml_err: std::result::Result<toml::Value, _> = toml::from_str("{{bad");
        let err = BzrError::TomlParse(toml_err.unwrap_err());
        assert_eq!(err.exit_code(), 3);
    }

    #[test]
    fn error_type_config() {
        let err = BzrError::Config("x".into());
        assert_eq!(err.error_type(), "config");
    }

    #[test]
    fn error_type_api() {
        let err = BzrError::Api {
            code: 1,
            message: "x".into(),
        };
        assert_eq!(err.error_type(), "api");
    }

    #[test]
    fn error_type_io() {
        let err = BzrError::Io(std::io::Error::other("x"));
        assert_eq!(err.error_type(), "io");
    }

    #[test]
    fn error_type_other() {
        let err = BzrError::Other("x".into());
        assert_eq!(err.error_type(), "other");
    }

    #[test]
    fn exit_code_not_found() {
        let err = BzrError::NotFound {
            resource: "bug",
            id: "42".into(),
        };
        assert_eq!(err.exit_code(), 2);
        assert_eq!(err.error_type(), "not_found");
        assert_eq!(err.to_string(), "bug not found: 42");
    }

    #[test]
    fn error_type_toml_parse() {
        let toml_err: std::result::Result<toml::Value, _> = toml::from_str("{{bad");
        let err = BzrError::TomlParse(toml_err.unwrap_err());
        assert_eq!(err.error_type(), "config");
    }
}
