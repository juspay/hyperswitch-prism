#![allow(clippy::result_large_err)]

use std::net::SocketAddr;
use tracing_subscriber::EnvFilter;

mod auth;
mod form;
mod handlers;
mod router;
mod scenarios;
mod services;
mod state;
mod types;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,mock=debug")),
        )
        .init();

    let bind: SocketAddr = std::env::var("DUMMY_BACKEND_BIND")
        .unwrap_or_else(|_| "127.0.0.1:8777".to_string())
        .parse()?;
    let require_token = std::env::var("MOCK_DUMMY_REQUIRE_TOKEN").ok();

    let state = state::AppState::new(require_token);
    state.spawn_ttl_sweeper();

    let app = router::build_router(state);
    let listener = tokio::net::TcpListener::bind(bind).await?;
    tracing::info!(%bind, "mock-dummy listening");
    axum::serve(listener, app).await?;
    Ok(())
}
