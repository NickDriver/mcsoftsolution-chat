//! Tiny HTTP MCP client (Streamable HTTP transport).
//!
//! Performs the spec-mandated handshake:
//! 1. POST `initialize` — capture `Mcp-Session-Id` from response headers.
//! 2. POST `notifications/initialized` (under that session) — confirm the
//!    session. Without this, real MCP servers (mcsoftsolution.com/mcp
//!    included) reject subsequent requests with HTTP 401 "Session not found".
//!
//! Caches the tool list for `TOOL_LIST_TTL`; auto-reconnects once if the
//! server drops the session.

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::RwLock;

const PROTOCOL_VERSION: &str = "2025-06-18";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
const TOOL_LIST_TTL: Duration = Duration::from_secs(15 * 60);

#[derive(Clone)]
pub struct McpClient {
    pub mcp_url: Arc<String>,
    client: reqwest::Client,
    session_id: Arc<RwLock<Option<String>>>,
    tools_cache: Arc<RwLock<Option<ToolsCache>>>,
}

#[derive(Clone)]
struct ToolsCache {
    fetched_at: Instant,
    tools: Vec<McpTool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema", default = "empty_object_schema")]
    pub input_schema: Value,
}

fn empty_object_schema() -> Value {
    json!({"type": "object", "properties": {}})
}

impl McpClient {
    pub fn new(mcp_url: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .expect("reqwest client builds");
        Self {
            mcp_url: Arc::new(mcp_url),
            client,
            session_id: Arc::new(RwLock::new(None)),
            tools_cache: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn list_tools(&self) -> anyhow::Result<Vec<McpTool>> {
        if let Some(cache) = self.tools_cache.read().await.as_ref() {
            if cache.fetched_at.elapsed() < TOOL_LIST_TTL {
                return Ok(cache.tools.clone());
            }
        }

        if self.session_id.read().await.is_none() {
            self.initialize_handshake()
                .await
                .context("MCP initialize handshake")?;
        }

        let resp = self
            .rpc("tools/list", json!({}))
            .await
            .context("calling tools/list")?;

        let tools_value = resp
            .get("tools")
            .ok_or_else(|| anyhow!("tools/list response missing 'tools' field"))?;
        let tools: Vec<McpTool> = serde_json::from_value(tools_value.clone())
            .context("deserializing tools/list response")?;

        *self.tools_cache.write().await = Some(ToolsCache {
            fetched_at: Instant::now(),
            tools: tools.clone(),
        });
        Ok(tools)
    }

    pub async fn call_tool(&self, name: &str, arguments: Value) -> anyhow::Result<String> {
        let params = json!({
            "name": name,
            "arguments": arguments,
        });
        let resp = self
            .rpc("tools/call", params)
            .await
            .with_context(|| format!("calling tool '{name}'"))?;

        let is_error = resp
            .get("isError")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let content = resp
            .get("content")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let mut text = String::new();
        for block in content {
            if let Some(t) = block.get("text").and_then(|t| t.as_str()) {
                if !text.is_empty() {
                    text.push_str("\n\n");
                }
                text.push_str(t);
            }
        }
        if text.is_empty() {
            text = serde_json::to_string(&resp)
                .unwrap_or_else(|_| "(empty response)".to_string());
        }

        if is_error {
            return Err(anyhow!("tool error: {text}"));
        }
        Ok(text)
    }

    async fn initialize_handshake(&self) -> anyhow::Result<()> {
        let params = json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": {
                "name": "mcsoftsolution-chat",
                "version": env!("CARGO_PKG_VERSION"),
            }
        });
        let req_body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": params,
        });
        let resp = self
            .send(&req_body)
            .await
            .context("posting initialize request")?;
        if let Some(sid) = resp
            .headers()
            .get("Mcp-Session-Id")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
        {
            *self.session_id.write().await = Some(sid);
        }
        let _ = decode_response(resp).await?;

        let init_done = json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });
        let resp = self
            .send(&init_done)
            .await
            .context("posting notifications/initialized")?;
        let _ = resp.text().await;
        Ok(())
    }

    async fn rpc(&self, method: &str, params: Value) -> anyhow::Result<Value> {
        match self.rpc_once(method, params.clone()).await {
            Ok(v) => Ok(v),
            Err(e) => {
                let msg = format!("{e:#}");
                if msg.contains("Session not found") || msg.contains("HTTP 401") {
                    tracing::debug!(method, "MCP session lost; re-handshaking");
                    *self.session_id.write().await = None;
                    self.initialize_handshake()
                        .await
                        .context("MCP re-handshake after 401")?;
                    self.rpc_once(method, params).await
                } else {
                    Err(e)
                }
            }
        }
    }

    async fn rpc_once(&self, method: &str, params: Value) -> anyhow::Result<Value> {
        let req_body = json!({
            "jsonrpc": "2.0",
            "id": next_id(),
            "method": method,
            "params": params,
        });
        let resp = self
            .send(&req_body)
            .await
            .with_context(|| format!("posting {method} request"))?;
        let value = decode_response(resp).await?;

        if let Some(err) = value.get("error") {
            let msg = err
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown error");
            return Err(anyhow!("mcp error from {method}: {msg}"));
        }
        Ok(value.get("result").cloned().unwrap_or(Value::Null))
    }

    async fn send(&self, body: &Value) -> anyhow::Result<reqwest::Response> {
        let mut builder = self
            .client
            .post(self.mcp_url.as_str())
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream")
            .header("MCP-Protocol-Version", PROTOCOL_VERSION)
            .json(body);
        if let Some(sid) = self.session_id.read().await.as_ref() {
            builder = builder.header("Mcp-Session-Id", sid);
        }
        let resp = builder.send().await.context("posting to MCP server")?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp
                .text()
                .await
                .unwrap_or_else(|_| "<unreadable error body>".to_string());
            return Err(anyhow!("MCP HTTP {status}: {text}"));
        }
        Ok(resp)
    }
}

/// Decode an MCP HTTP response — handles both `application/json` and
/// `text/event-stream` (Streamable HTTP). For SSE, returns the first parseable
/// JSON-RPC payload.
async fn decode_response(resp: reqwest::Response) -> anyhow::Result<Value> {
    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_lowercase();

    let body = resp.text().await.context("reading MCP response body")?;
    if body.is_empty() {
        return Err(anyhow!("empty MCP response body"));
    }

    if content_type.starts_with("text/event-stream") {
        for event in body.split("\n\n") {
            let mut data = String::new();
            for line in event.lines() {
                let line = line.trim_end_matches('\r');
                if let Some(rest) = line.strip_prefix("data:") {
                    let rest = rest.strip_prefix(' ').unwrap_or(rest);
                    if !data.is_empty() {
                        data.push('\n');
                    }
                    data.push_str(rest);
                }
            }
            if data.is_empty() || data == "[DONE]" {
                continue;
            }
            if let Ok(v) = serde_json::from_str::<Value>(&data) {
                return Ok(v);
            }
        }
        Err(anyhow!("MCP SSE response had no parseable JSON-RPC payload"))
    } else {
        serde_json::from_str(&body)
            .with_context(|| format!("parsing MCP JSON response: {}", truncate(&body, 256)))
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max])
    }
}

fn next_id() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(2);
    NEXT.fetch_add(1, Ordering::Relaxed)
}
