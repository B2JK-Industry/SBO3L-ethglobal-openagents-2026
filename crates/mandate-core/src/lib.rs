//! Mandate core: protocol types, request hashing, error model, signed receipts.

pub mod aprp;
pub mod audit;
pub mod audit_bundle;
pub mod decision_token;
pub mod error;
pub mod hashing;
pub mod mock_kms;
pub mod receipt;
pub mod schema;
pub mod signer;

pub use error::{CoreError, Result, SchemaError};

pub const SCHEMA_VERSION: u32 = 1;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_set() {
        assert!(!version().is_empty());
    }
}
