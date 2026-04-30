//! Drop-in "Chat with us" backend for Axum-based MC Soft Solution demos.
//!
//! Wires three pieces together:
//!
//! 1. **OpenRouter streaming client** — completes a conversation with a chosen
//!    model and yields text + tool-call deltas as a stream.
//! 2. **HTTP MCP client** — performs the spec-mandated `initialize` +
//!    `notifications/initialized` handshake against a public MCP endpoint
//!    (default: `https://mcsoftsolution.com/mcp`), then exposes its tools
//!    to the model.
//! 3. **SSE handler** — `POST /api/mcsoft-chat` runs the agent loop, routes
//!    tool calls through the MCP client, and streams text + tool-call events
//!    back to the browser. `GET /api/mcsoft-chat/status` lets the widget
//!    discover whether chat is configured.
//!
//! ## Quickstart
//!
//! ```no_run
//! use axum::Router;
//! use mcsoftsolution_chat::{ChatConfig, router};
//!
//! # async fn run() -> anyhow::Result<()> {
//! // Pulls MCSOFT_OPENROUTER_API_KEY / MCSOFT_CHAT_MODEL / MCSOFT_MCP_URL
//! // from env. Returns None when the key is unset; the router still mounts
//! // and reports `enabled: false` on /status.
//! let chat = ChatConfig::from_env();
//! let app = Router::new().merge(router(chat));
//! # let _ = app;
//! # Ok(())
//! # }
//! ```
//!
//! For tighter control:
//!
//! ```no_run
//! use mcsoftsolution_chat::{ChatConfig, router};
//!
//! let cfg = ChatConfig::builder()
//!     .openrouter_api_key("sk-or-v1-...")
//!     .site_label("acme-demo.example.com")  // shows up in the system prompt
//!     .build();
//! let app: axum::Router = axum::Router::new().merge(router(Some(cfg)));
//! ```

mod config;
mod handler;
mod mcp;
mod openrouter;
mod prompt;

pub use config::{ChatConfig, ChatConfigBuilder};
pub use handler::router;
