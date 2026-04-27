use mandate_server::{reference_policy, router, AppState};
use mandate_storage::Storage;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let storage_path = std::env::var("MANDATE_DB").unwrap_or_else(|_| "mandate.db".to_string());
    let storage = if storage_path == ":memory:" {
        Storage::open_in_memory()?
    } else {
        Storage::open(storage_path.clone())?
    };

    let policy = reference_policy();
    let state = AppState::new(policy, storage);
    let app = router(state);

    let addr = std::env::var("MANDATE_LISTEN").unwrap_or_else(|_| "127.0.0.1:8730".to_string());
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!(addr = %listener.local_addr()?, db = %storage_path, "mandate-server listening");
    axum::serve(listener, app).await?;
    Ok(())
}
