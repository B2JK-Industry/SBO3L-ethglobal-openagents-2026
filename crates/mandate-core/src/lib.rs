//! Mandate core: protocol types, request hashing, error model, signed receipts.

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
