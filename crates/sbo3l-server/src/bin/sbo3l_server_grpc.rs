//! R14 P1 — combined HTTP + gRPC daemon entrypoint.
//!
//! Runs the existing axum HTTP router AND the tonic gRPC service in the
//! same Tokio runtime, on different ports. Both surfaces share a single
//! `AppState` (storage, signers, metrics) so a request landing on REST
//! and a request landing on gRPC see identical state.
//!
//! Bind ports:
//!   * `SBO3L_LISTEN`      — REST (default `127.0.0.1:8730`)
//!   * `SBO3L_GRPC_LISTEN` — gRPC (default `127.0.0.1:8731`)
//!
//! Like the HTTP-only `sbo3l-server` binary, this refuses non-loopback
//! binds unless `SBO3L_ALLOW_UNSAFE_PUBLIC_BIND=1`. The check is run
//! against BOTH listen addresses.

use std::net::SocketAddr;

use sbo3l_core::signers::signer_from_env;
use sbo3l_policy::Policy;
use sbo3l_server::grpc::GrpcService;
use sbo3l_server::{reference_policy, router, AppState, AuthConfig};
use sbo3l_storage::Storage;

const DEFAULT_HTTP_LISTEN: &str = "127.0.0.1:8730";
const DEFAULT_GRPC_LISTEN: &str = "127.0.0.1:8731";
const ENV_HTTP_LISTEN: &str = "SBO3L_LISTEN";
const ENV_GRPC_LISTEN: &str = "SBO3L_GRPC_LISTEN";
const ENV_ALLOW_UNSAFE_PUBLIC_BIND: &str = "SBO3L_ALLOW_UNSAFE_PUBLIC_BIND";
const ENV_POLICY: &str = "SBO3L_POLICY";
const ENV_SIGNER_BACKEND: &str = "SBO3L_SIGNER_BACKEND";
const UNSAFE_BIND_EXIT_CODE: i32 = 2;
const SIGNER_LOCKOUT_EXIT_CODE: i32 = 2;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // Parse + validate both bind addresses up-front so we fail fast
    // if either is misconfigured. We deliberately do NOT relax the
    // unsafe-bind check for one surface only — same posture as the
    // HTTP-only binary.
    let http_addr = std::env::var(ENV_HTTP_LISTEN).unwrap_or_else(|_| DEFAULT_HTTP_LISTEN.into());
    let grpc_addr = std::env::var(ENV_GRPC_LISTEN).unwrap_or_else(|_| DEFAULT_GRPC_LISTEN.into());

    let allow_unsafe = std::env::var(ENV_ALLOW_UNSAFE_PUBLIC_BIND)
        .map(|v| v == "1")
        .unwrap_or(false);

    for (name, addr) in [(ENV_HTTP_LISTEN, &http_addr), (ENV_GRPC_LISTEN, &grpc_addr)] {
        let resolved: Vec<SocketAddr> = tokio::net::lookup_host(addr.as_str())
            .await
            .map_err(|e| anyhow::anyhow!("failed to resolve {name}={addr}: {e}"))?
            .collect();
        if resolved.is_empty() {
            anyhow::bail!("{name}={addr} resolved to no socket addresses");
        }
        if !is_all_loopback(&resolved) && !allow_unsafe {
            eprintln!(
                "ERROR: refusing unsafe public bind on {name}={addr} (resolved: {resolved:?})."
            );
            eprintln!(
                "  set {ENV_ALLOW_UNSAFE_PUBLIC_BIND}=1 to override after reviewing SECURITY_NOTES.md."
            );
            std::process::exit(UNSAFE_BIND_EXIT_CODE);
        }
    }

    // F-5 signer factory gate (same posture as the HTTP-only binary).
    let configured_backend =
        std::env::var(ENV_SIGNER_BACKEND).unwrap_or_else(|_| "dev".to_string());
    if configured_backend != "dev" {
        eprintln!(
            "ERROR: {ENV_SIGNER_BACKEND}={configured_backend} is not yet wired into AppState."
        );
        std::process::exit(SIGNER_LOCKOUT_EXIT_CODE);
    }
    if let Err(e) = signer_from_env("audit") {
        eprintln!("ERROR: signer backend rejected: {e}");
        std::process::exit(SIGNER_LOCKOUT_EXIT_CODE);
    }
    if let Err(e) = signer_from_env("receipt") {
        eprintln!("ERROR: signer backend rejected: {e}");
        std::process::exit(SIGNER_LOCKOUT_EXIT_CODE);
    }

    let storage_path = std::env::var("SBO3L_DB").unwrap_or_else(|_| "sbo3l.db".to_string());
    let storage = if storage_path == ":memory:" {
        Storage::open_in_memory()?
    } else {
        Storage::open(storage_path.clone())?
    };

    let policy = match std::env::var(ENV_POLICY).ok() {
        None => reference_policy(),
        Some(path) => {
            let raw = std::fs::read_to_string(&path)
                .map_err(|e| anyhow::anyhow!("failed to read {ENV_POLICY}={path}: {e}"))?;
            Policy::parse_json(&raw)
                .map_err(|e| anyhow::anyhow!("failed to parse policy at {path}: {e}"))?
        }
    };

    let auth = AuthConfig::from_env();
    if auth.allow_unauthenticated {
        eprintln!("⚠ UNAUTHENTICATED MODE — DEV ONLY ⚠");
    }
    let state = AppState::with_auth_config(policy, storage, auth);
    let inner_for_grpc = state.0.clone();
    let app = router(state);

    // Spawn both servers; await whichever exits first. tonic exposes
    // a `.serve()` future on the `Router` returned from `add_service`;
    // axum exposes `axum::serve(listener, app)`. Both are graceful
    // long-running tasks — joining means a panic in one surface
    // tears down the other, which is the right behaviour for a
    // process supervisor (systemd / k8s) to observe.
    let http_listener = tokio::net::TcpListener::bind(&http_addr).await?;
    tracing::info!(addr = %http_listener.local_addr()?, "REST listening");
    let grpc_socket: SocketAddr = grpc_addr
        .parse()
        .map_err(|e| anyhow::anyhow!("{ENV_GRPC_LISTEN}={grpc_addr} not a SocketAddr: {e}"))?;
    tracing::info!(addr = %grpc_socket, "gRPC listening");

    let grpc_service = GrpcService::new(inner_for_grpc).into_server();

    let http_fut = async move {
        axum::serve(http_listener, app)
            .await
            .map_err(anyhow::Error::from)
    };
    let grpc_fut = async move {
        tonic::transport::Server::builder()
            .add_service(grpc_service)
            .serve(grpc_socket)
            .await
            .map_err(anyhow::Error::from)
    };

    tokio::select! {
        res = http_fut => {
            tracing::error!(?res, "HTTP server exited; tearing down");
            res
        }
        res = grpc_fut => {
            tracing::error!(?res, "gRPC server exited; tearing down");
            res
        }
    }
}

fn is_all_loopback(addrs: &[SocketAddr]) -> bool {
    !addrs.is_empty() && addrs.iter().all(|s| s.ip().is_loopback())
}
