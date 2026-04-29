//! Shared helpers used by `engine` and `budget`.

/// Returns `true` if two URLs refer to the same origin (host + scheme +
/// optional port). Trailing slashes are ignored, and either URL is accepted as
/// a prefix of the other (so `https://api.example.com` matches both
/// `https://api.example.com/v1/foo` and bare `https://api.example.com`).
///
/// This is intentionally permissive — it accepts `https://a.com` as the same
/// origin as `https://a.com/path` because SBO3L's `provider.url` config can
/// be either form. A stricter origin comparator would parse with the `url`
/// crate; that is overkill for the per-tx hot path.
pub(crate) fn same_origin(a: &str, b: &str) -> bool {
    let a = a.trim_end_matches('/');
    let b = b.trim_end_matches('/');
    a == b || b.starts_with(&format!("{a}/")) || a.starts_with(&format!("{b}/"))
}

#[cfg(test)]
mod tests {
    use super::same_origin;

    #[test]
    fn same_origin_exact_match() {
        assert!(same_origin(
            "https://api.example.com",
            "https://api.example.com"
        ));
    }

    #[test]
    fn same_origin_normalises_trailing_slash() {
        assert!(same_origin(
            "https://api.example.com/",
            "https://api.example.com"
        ));
    }

    #[test]
    fn same_origin_accepts_either_as_prefix() {
        assert!(same_origin(
            "https://api.example.com",
            "https://api.example.com/v1/inference"
        ));
        assert!(same_origin(
            "https://api.example.com/v1/inference",
            "https://api.example.com"
        ));
    }

    #[test]
    fn same_origin_rejects_different_hosts() {
        assert!(!same_origin(
            "https://api.example.com",
            "https://malicious.example"
        ));
        assert!(!same_origin(
            "https://api.example.com",
            "https://api.example.com.attacker.com"
        ));
    }
}
