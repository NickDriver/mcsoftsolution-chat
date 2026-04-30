//! Minimal standalone example: bind on :8080, mount the chat router.
//!
//! Run with:
//!     MCSOFT_OPENROUTER_API_KEY=sk-or-v1-... cargo run --example standalone
//! Then POST to http://127.0.0.1:8080/api/mcsoft-chat/status to verify.

use axum::Router;
use mcsoftsolution_chat::{ChatConfig, router};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,mcsoftsolution_chat=debug")),
        )
        .init();

    let cfg = ChatConfig::from_env();
    if cfg.is_none() {
        tracing::warn!(
            "MCSOFT_OPENROUTER_API_KEY is unset — chat will mount but report enabled:false"
        );
    }

    let app = Router::new().merge(router(cfg));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080").await?;
    tracing::info!("listening on http://127.0.0.1:8080");
    axum::serve(listener, app).await?;
    Ok(())
}
