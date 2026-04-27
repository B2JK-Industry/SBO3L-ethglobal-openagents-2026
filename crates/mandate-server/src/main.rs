use axum::{routing::get, Router};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let app = Router::new().route("/v1/health", get(|| async { "ok" }));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8730").await?;
    tracing::info!(addr = %listener.local_addr()?, "mandate-server listening");
    axum::serve(listener, app).await?;
    Ok(())
}
