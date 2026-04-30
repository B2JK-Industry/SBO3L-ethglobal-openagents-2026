use sbo3l_server::{reference_policy, router, AppState, AuthConfig};
use sbo3l_storage::Storage;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

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

    let policy = reference_policy();
    let state = AppState::with_auth_config(policy, storage, auth);
    let app = router(state);

    let addr = std::env::var("SBO3L_LISTEN").unwrap_or_else(|_| "127.0.0.1:8730".to_string());
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!(addr = %listener.local_addr()?, db = %storage_path, "sbo3l-server listening");
    axum::serve(listener, app).await?;
    Ok(())
}
