use std::io::IsTerminal;
use std::net::SocketAddr;

use sbo3l_policy::Policy;
use sbo3l_server::{reference_policy, router, AppState, AuthConfig};
use sbo3l_storage::Storage;

const DEFAULT_LISTEN: &str = "127.0.0.1:8730";
const ENV_LISTEN: &str = "SBO3L_LISTEN";
const ENV_ALLOW_UNSAFE_PUBLIC_BIND: &str = "SBO3L_ALLOW_UNSAFE_PUBLIC_BIND";
const ENV_POLICY: &str = "SBO3L_POLICY";
const UNSAFE_BIND_EXIT_CODE: i32 = 2;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let addr_str = std::env::var(ENV_LISTEN).unwrap_or_else(|_| DEFAULT_LISTEN.to_string());
    let resolved: Vec<SocketAddr> = tokio::net::lookup_host(addr_str.as_str())
        .await
        .map_err(|e| anyhow::anyhow!("failed to resolve {ENV_LISTEN}={addr_str}: {e}"))?
        .collect();

    if resolved.is_empty() {
        anyhow::bail!("{ENV_LISTEN}={addr_str} resolved to no socket addresses");
    }

    let allow_unsafe = std::env::var(ENV_ALLOW_UNSAFE_PUBLIC_BIND)
        .map(|v| v == "1")
        .unwrap_or(false);

    if !is_all_loopback(&resolved) {
        if !allow_unsafe {
            print_unsafe_bind_error(&addr_str, &resolved);
            std::process::exit(UNSAFE_BIND_EXIT_CODE);
        }
        print_unsafe_bind_warning(&addr_str, &resolved);
    }

    let storage_path = std::env::var("SBO3L_DB").unwrap_or_else(|_| "sbo3l.db".to_string());
    let storage = if storage_path == ":memory:" {
        Storage::open_in_memory()?
    } else {
        Storage::open(storage_path.clone())?
    };

    let auth = AuthConfig::from_env();
    if auth.allow_unauthenticated {
        // F-1 acceptance: a visible stderr banner when the dev bypass is
        // engaged. The exact substring "UNAUTHENTICATED MODE — DEV ONLY" is
        // grepped by the QA test plan; do not reword without updating that.
        eprintln!("⚠ UNAUTHENTICATED MODE — DEV ONLY ⚠");
        eprintln!(
            "  SBO3L_ALLOW_UNAUTHENTICATED=1 is set; \
             POST /v1/payment-requests will accept unauthenticated requests."
        );
    }

    let policy = match std::env::var(ENV_POLICY).ok() {
        None => reference_policy(),
        Some(path) => {
            let raw = std::fs::read_to_string(&path)
                .map_err(|e| anyhow::anyhow!("failed to read {ENV_POLICY}={path}: {e}"))?;
            Policy::parse_json(&raw)
                .map_err(|e| anyhow::anyhow!("failed to parse policy at {path}: {e}"))?
        }
    };
    let state = AppState::with_auth_config(policy, storage, auth);
    let app = router(state);

    let listener = tokio::net::TcpListener::bind(&addr_str).await?;
    tracing::info!(addr = %listener.local_addr()?, db = %storage_path, "sbo3l-server listening");
    axum::serve(listener, app).await?;
    Ok(())
}

fn is_all_loopback(addrs: &[SocketAddr]) -> bool {
    !addrs.is_empty() && addrs.iter().all(|s| s.ip().is_loopback())
}

fn print_unsafe_bind_error(addr: &str, resolved: &[SocketAddr]) {
    let (on, off) = stderr_red();
    eprintln!("{on}ERROR: refusing unsafe public bind on {addr} (resolved: {resolved:?}).{off}");
    eprintln!(
        "{on}set {ENV_ALLOW_UNSAFE_PUBLIC_BIND}=1 to override after reviewing SECURITY_NOTES.md.{off}"
    );
}

fn print_unsafe_bind_warning(addr: &str, resolved: &[SocketAddr]) {
    let (on, off) = stderr_red();
    eprintln!(
        "{on}UNSAFE PUBLIC BIND: sbo3l-server is listening on a non-loopback address {addr} (resolved: {resolved:?}). All hosts on this network can reach this daemon.{off}"
    );
}

fn stderr_red() -> (&'static str, &'static str) {
    if std::io::stderr().is_terminal() {
        ("\x1b[1;31m", "\x1b[0m")
    } else {
        ("", "")
    }
}

#[cfg(test)]
mod tests {
    use super::is_all_loopback;
    use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};

    fn v4(a: Ipv4Addr, port: u16) -> SocketAddr {
        SocketAddr::V4(SocketAddrV4::new(a, port))
    }

    fn v6(a: Ipv6Addr, port: u16) -> SocketAddr {
        SocketAddr::V6(SocketAddrV6::new(a, port, 0, 0))
    }

    #[test]
    fn loopback_v4_is_safe() {
        assert!(is_all_loopback(&[v4(Ipv4Addr::LOCALHOST, 8730)]));
        assert!(is_all_loopback(&[v4(Ipv4Addr::new(127, 0, 0, 5), 8730)]));
    }

    #[test]
    fn loopback_v6_is_safe() {
        assert!(is_all_loopback(&[v6(Ipv6Addr::LOCALHOST, 8730)]));
    }

    #[test]
    fn unspecified_v4_is_unsafe() {
        assert!(!is_all_loopback(&[v4(Ipv4Addr::UNSPECIFIED, 8730)]));
    }

    #[test]
    fn unspecified_v6_is_unsafe() {
        assert!(!is_all_loopback(&[v6(Ipv6Addr::UNSPECIFIED, 8730)]));
    }

    #[test]
    fn private_v4_is_unsafe() {
        assert!(!is_all_loopback(&[v4(Ipv4Addr::new(192, 168, 1, 5), 8730)]));
        assert!(!is_all_loopback(&[v4(Ipv4Addr::new(10, 0, 0, 1), 8730)]));
    }

    #[test]
    fn public_v4_is_unsafe() {
        assert!(!is_all_loopback(&[v4(Ipv4Addr::new(8, 8, 8, 8), 8730)]));
    }

    #[test]
    fn empty_is_unsafe() {
        assert!(!is_all_loopback(&[]));
    }

    #[test]
    fn mixed_loopback_and_public_is_unsafe() {
        assert!(!is_all_loopback(&[
            v4(Ipv4Addr::LOCALHOST, 8730),
            v4(Ipv4Addr::new(8, 8, 8, 8), 8730),
        ]));
    }

    #[test]
    fn dual_loopback_v4_v6_is_safe() {
        assert!(is_all_loopback(&[
            v4(Ipv4Addr::LOCALHOST, 8730),
            v6(Ipv6Addr::LOCALHOST, 8730),
        ]));
    }
}
