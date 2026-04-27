//! Error model. Mirrors §3 of `docs/spec/17_interface_contracts.md`.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum SchemaError {
    #[error("schema.unknown_field at {path}")]
    UnknownField { path: String },
    #[error("schema.missing_field {field}")]
    MissingField { field: String },
    #[error("schema.wrong_type at {path}: {detail}")]
    WrongType { path: String, detail: String },
    #[error("schema.value_out_of_range at {path}: {detail}")]
    ValueOutOfRange { path: String, detail: String },
    #[error("schema.invalid_root: {detail}")]
    InvalidRoot { detail: String },
}

impl SchemaError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::UnknownField { .. } => "schema.unknown_field",
            Self::MissingField { .. } => "schema.missing_field",
            Self::WrongType { .. } => "schema.wrong_type",
            Self::ValueOutOfRange { .. } => "schema.value_out_of_range",
            Self::InvalidRoot { .. } => "schema.invalid_root",
        }
    }

    pub fn http_status(&self) -> u16 {
        400
    }
}

#[derive(Debug, Error)]
pub enum CoreError {
    #[error(transparent)]
    Schema(#[from] SchemaError),
    #[error("canonicalization: {0}")]
    Canonicalization(String),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, CoreError>;
