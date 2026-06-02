//! SSE chat endpoint + status endpoint.

use std::convert::Infallible;
use std::sync::Arc;

use async_stream::stream;
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{
        IntoResponse,
        sse::{Event, KeepAlive, Sse},
    },
    routing::{get, post},
};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::config::ChatConfig;
use crate::mcp::{McpClient, McpTool};
use crate::openrouter::{
    self, AssistantToolCall, ChatRequest, Message, StreamEvent, ToolCallFunction,
};
use crate::prompt;

const MAX_TOOL_ITERATIONS: u32 = 6;
const MAX_RESPONSE_TOKENS: u32 = 800;
const TEMPERATURE: f32 = 0.5;
const MAX_HISTORY_MESSAGES: usize = 20;

/// Build a router exposing `POST /api/mcsoft-chat` (SSE) and
/// `GET /api/mcsoft-chat/status`. Pass `None` to mount a stub that always
/// reports `enabled: false`; pass `Some(cfg)` for an active service.
pub fn router(config: Option<ChatConfig>) -> Router {
    let state = match config {
        Some(cfg) => ServiceState {
            inner: Some(Arc::new(ServiceInner {
                config: cfg.clone(),
                mcp: McpClient::new(cfg.mcp_url.clone()),
            })),
        },
        None => ServiceState { inner: None },
    };

    Router::new()
        .route("/api/mcsoft-chat", post(chat_handler))
        .route("/api/mcsoft-chat/status", get(status_handler))
        .with_state(state)
}

#[derive(Clone)]
struct ServiceState {
    inner: Option<Arc<ServiceInner>>,
}

struct ServiceInner {
    config: ChatConfig,
    mcp: McpClient,
}

#[derive(Debug, Deserialize)]
struct ChatRequestBody {
    history: Vec<TranscriptEntry>,
}

#[derive(Debug, Deserialize)]
struct TranscriptEntry {
    role: String,
    text: String,
}

#[derive(Debug, Serialize)]
struct StatusBody {
    enabled: bool,
    /// Empty-state starter prompts; empty => widget uses its own defaults.
    starters: Vec<String>,
    /// Suggested follow-up chips; empty => widget uses its own defaults.
    followups: Vec<String>,
}

async fn status_handler(State(state): State<ServiceState>) -> Json<StatusBody> {
    match &state.inner {
        Some(svc) => {
            let enabled = svc.config.resolve_key().await.is_some();
            let (starters, followups) = svc.config.resolve_suggestions().await;
            Json(StatusBody { enabled, starters, followups })
        }
        None => Json(StatusBody { enabled: false, starters: Vec::new(), followups: Vec::new() }),
    }
}

#[derive(Debug, Serialize)]
struct ToolCallStartPayload {
    id: String,
    name: String,
    args: Value,
}

#[derive(Debug, Serialize)]
struct ToolCallResultPayload {
    id: String,
    ok: bool,
    summary: String,
}

#[derive(Debug, Serialize)]
struct ErrorPayload<'a> {
    message: &'a str,
}

async fn chat_handler(
    State(state): State<ServiceState>,
    Json(req): Json<ChatRequestBody>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let Some(svc) = state.inner else {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "Chat is not configured on this server.".to_string(),
        ));
    };

    // Resolve the API key per request so dynamic resolvers (e.g. backed by
    // an encrypted credentials store) can rotate without a restart.
    let Some(api_key) = svc.config.resolve_key().await else {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "Chat is not configured on this server.".to_string(),
        ));
    };

    if req.history.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "history is empty".to_string()));
    }
    if req.history.len() > MAX_HISTORY_MESSAGES {
        return Err((StatusCode::BAD_REQUEST, "history too long".to_string()));
    }
    let last = req.history.last().unwrap();
    if last.role != "user" {
        return Err((
            StatusCode::BAD_REQUEST,
            "last entry must be a user message".to_string(),
        ));
    }
    let trimmed_last = last.text.trim();
    if trimmed_last.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "user message is empty".to_string()));
    }
    if trimmed_last.len() > 2000 {
        return Err((StatusCode::BAD_REQUEST, "user message too long".to_string()));
    }

    let mcp_tools = match svc.mcp.list_tools().await {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!(?e, "mcp tools/list failed");
            Vec::new()
        }
    };

    let openai_tools: Vec<Value> = mcp_tools.iter().map(mcp_tool_to_openai).collect();

    // Resolve the prompt override per request so a DB-backed provider can pick
    // up published edits without a restart (mirrors the key-provider pattern).
    let prompt_override = svc.config.resolve_prompt_override().await;

    let mut messages: Vec<Message> = Vec::with_capacity(req.history.len() + 1);
    messages.push(Message::system(prompt::build(
        prompt_override.as_deref(),
        svc.config.site_label.as_deref(),
        &mcp_tools,
    )));
    for entry in req.history {
        match entry.role.as_str() {
            "user" => messages.push(Message::user(entry.text)),
            "assistant" => messages.push(Message::assistant_text(entry.text)),
            _ => {}
        }
    }

    let stream = run_agent_stream(svc, api_key, messages, openai_tools);
    Ok(Sse::new(stream).keep_alive(KeepAlive::default()).into_response())
}

fn run_agent_stream(
    svc: Arc<ServiceInner>,
    api_key: String,
    initial_messages: Vec<Message>,
    tool_defs: Vec<Value>,
) -> impl futures_util::Stream<Item = Result<Event, Infallible>> {
    stream! {
        let mut messages = initial_messages;

        for iteration in 0..MAX_TOOL_ITERATIONS {
            let req = ChatRequest {
                api_key: api_key.clone(),
                model: svc.config.model.clone(),
                messages: messages.clone(),
                tools: tool_defs.clone(),
                max_tokens: MAX_RESPONSE_TOKENS,
                temperature: Some(TEMPERATURE),
            };

            let upstream = match openrouter::stream_chat(req).await {
                Ok(s) => s,
                Err(e) => {
                    tracing::error!(?e, "openrouter request failed");
                    yield Ok(error_event(&format!("upstream error: {e}")));
                    yield Ok(done_event());
                    return;
                }
            };
            tokio::pin!(upstream);

            let mut current_text = String::new();
            let mut partials: Vec<PartialToolCall> = Vec::new();

            while let Some(event) = upstream.next().await {
                match event {
                    Ok(StreamEvent::TextDelta(delta)) => {
                        current_text.push_str(&delta);
                        yield Ok(text_event(&delta));
                    }
                    Ok(StreamEvent::ToolCallDelta(d)) => {
                        let idx = d.index as usize;
                        while partials.len() <= idx {
                            partials.push(PartialToolCall::default());
                        }
                        let tc = &mut partials[idx];
                        if let Some(id) = d.id {
                            tc.id = id;
                        }
                        if let Some(name) = d.name {
                            tc.name = name;
                        }
                        if let Some(delta) = d.args_delta {
                            tc.args.push_str(&delta);
                        }
                    }
                    Ok(StreamEvent::Finish { reason: _ }) => {}
                    Err(e) => {
                        tracing::error!(?e, "upstream stream error");
                        yield Ok(error_event(&format!("stream error: {e}")));
                        yield Ok(done_event());
                        return;
                    }
                }
            }

            let assistant_tool_calls: Vec<AssistantToolCall> = partials
                .iter()
                .filter(|p| !p.name.is_empty())
                .map(|p| AssistantToolCall {
                    id: if p.id.is_empty() {
                        format!("call_{iteration}")
                    } else {
                        p.id.clone()
                    },
                    kind: "function".to_string(),
                    function: ToolCallFunction {
                        name: p.name.clone(),
                        arguments: if p.args.is_empty() { "{}".to_string() } else { p.args.clone() },
                    },
                })
                .collect();

            let assistant_msg = if assistant_tool_calls.is_empty() {
                Message::assistant_text(current_text.clone())
            } else {
                Message::assistant_tool_calls(
                    if current_text.is_empty() { None } else { Some(current_text.clone()) },
                    assistant_tool_calls.clone(),
                )
            };
            messages.push(assistant_msg);

            if assistant_tool_calls.is_empty() {
                yield Ok(done_event());
                return;
            }

            for tc in &assistant_tool_calls {
                let parsed_args: Value =
                    serde_json::from_str(&tc.function.arguments).unwrap_or(json!({}));
                yield Ok(tool_call_start_event(&ToolCallStartPayload {
                    id: tc.id.clone(),
                    name: tc.function.name.clone(),
                    args: parsed_args.clone(),
                }));

                let outcome = svc.mcp.call_tool(&tc.function.name, parsed_args.clone()).await;
                let (ok, content, summary) = match outcome {
                    Ok(text) => {
                        let summary = format!("returned {} chars", text.len().min(9999));
                        (true, text, summary)
                    }
                    Err(e) => {
                        let msg = e.to_string();
                        (false, json!({"error": msg}).to_string(), msg)
                    }
                };
                yield Ok(tool_call_result_event(&ToolCallResultPayload {
                    id: tc.id.clone(),
                    ok,
                    summary,
                }));
                messages.push(Message::tool(tc.id.clone(), content));
            }

            if iteration + 1 == MAX_TOOL_ITERATIONS {
                yield Ok(error_event("conversation exceeded tool-call budget"));
                yield Ok(done_event());
                return;
            }
        }
    }
}

#[derive(Default)]
struct PartialToolCall {
    id: String,
    name: String,
    args: String,
}

fn mcp_tool_to_openai(tool: &McpTool) -> Value {
    let mut params = tool.input_schema.clone();
    if let Some(obj) = params.as_object_mut() {
        obj.remove("$schema");
        obj.remove("title");
        obj.entry("type".to_string()).or_insert(json!("object"));
        obj.entry("properties".to_string()).or_insert(json!({}));
    } else {
        params = json!({"type": "object", "properties": {}});
    }
    json!({
        "type": "function",
        "function": {
            "name": tool.name,
            "description": tool.description,
            "parameters": params,
        }
    })
}

fn text_event(delta: &str) -> Event {
    Event::default()
        .event("text")
        .json_data(json!({ "delta": delta }))
        .unwrap()
}

fn tool_call_start_event(p: &ToolCallStartPayload) -> Event {
    Event::default()
        .event("tool_call_start")
        .json_data(p)
        .unwrap()
}

fn tool_call_result_event(p: &ToolCallResultPayload) -> Event {
    Event::default()
        .event("tool_call_result")
        .json_data(p)
        .unwrap()
}

fn error_event(message: &str) -> Event {
    Event::default()
        .event("error")
        .json_data(ErrorPayload { message })
        .unwrap()
}

fn done_event() -> Event {
    Event::default().event("done").data("")
}
