//! Minimal OpenRouter streaming client.
//!
//! Only the slice we need: chat/completions with `stream: true` and tools,
//! parsed into a typed event stream that surfaces text deltas, tool-call
//! deltas, and the terminal finish event separately.

use std::time::Duration;

use anyhow::{Context, anyhow};
use async_stream::try_stream;
use bytes::BytesMut;
use futures_util::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1/chat/completions";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<AssistantToolCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".into(),
            content: Some(content.into()),
            tool_calls: vec![],
            tool_call_id: None,
        }
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            content: Some(content.into()),
            tool_calls: vec![],
            tool_call_id: None,
        }
    }
    pub fn assistant_text(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".into(),
            content: Some(content.into()),
            tool_calls: vec![],
            tool_call_id: None,
        }
    }
    pub fn assistant_tool_calls(text: Option<String>, tool_calls: Vec<AssistantToolCall>) -> Self {
        Self {
            role: "assistant".into(),
            content: text,
            tool_calls,
            tool_call_id: None,
        }
    }
    pub fn tool(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: "tool".into(),
            content: Some(content.into()),
            tool_calls: vec![],
            tool_call_id: Some(tool_call_id.into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub function: ToolCallFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone)]
pub struct ChatRequest {
    pub api_key: String,
    pub model: String,
    pub messages: Vec<Message>,
    pub tools: Vec<Value>,
    pub max_tokens: u32,
    pub temperature: Option<f32>,
}

#[derive(Debug, Clone)]
pub enum StreamEvent {
    TextDelta(String),
    ToolCallDelta(ToolCallDelta),
    Finish {
        /// Server-side `finish_reason`; surfaced for parity with OpenAI's
        /// streaming format. The default handler ignores it.
        #[allow(dead_code)]
        reason: String,
    },
}

#[derive(Debug, Clone, Default)]
pub struct ToolCallDelta {
    pub index: u32,
    pub id: Option<String>,
    pub name: Option<String>,
    pub args_delta: Option<String>,
}

pub async fn stream_chat(
    req: ChatRequest,
) -> anyhow::Result<impl Stream<Item = anyhow::Result<StreamEvent>>> {
    let client = reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .build()
        .context("building reqwest client")?;

    let mut body = json!({
        "model": req.model,
        "messages": req.messages,
        "stream": true,
        "max_tokens": req.max_tokens,
    });
    if !req.tools.is_empty() {
        body["tools"] = Value::Array(req.tools.clone());
    }
    if let Some(t) = req.temperature {
        body["temperature"] = json!(t);
    }

    let response = client
        .post(OPENROUTER_URL)
        .bearer_auth(&req.api_key)
        .header("HTTP-Referer", "https://mcsoftsolution.com")
        .header("X-Title", "MC Soft Solution chat widget")
        .header("Accept", "text/event-stream")
        .json(&body)
        .send()
        .await
        .context("posting to openrouter")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "<unreadable error body>".to_string());
        return Err(anyhow!("openrouter error {status}: {body}"));
    }

    let mut byte_stream = response.bytes_stream();
    let stream = try_stream! {
        let mut buffer = BytesMut::with_capacity(8192);

        while let Some(chunk) = byte_stream.next().await {
            let chunk = chunk.context("reading openrouter stream chunk")?;
            buffer.extend_from_slice(&chunk);

            while let Some(event_end) = find_event_boundary(&buffer) {
                let raw = buffer.split_to(event_end);
                let raw = std::str::from_utf8(&raw).context("non-utf8 SSE event")?;
                if let Some(event) = parse_sse_event(raw)? {
                    match event {
                        ParsedEvent::Done => return,
                        ParsedEvent::Chunk(chunk_value) => {
                            for sse in extract_events(&chunk_value)? {
                                yield sse;
                            }
                        }
                    }
                }
            }
        }
    };

    Ok(stream)
}

#[derive(Debug)]
enum ParsedEvent {
    Chunk(Value),
    Done,
}

fn find_event_boundary(buf: &[u8]) -> Option<usize> {
    buf.windows(2).position(|w| w == b"\n\n").map(|i| i + 2)
}

fn parse_sse_event(raw: &str) -> anyhow::Result<Option<ParsedEvent>> {
    let mut data = String::new();
    for line in raw.lines() {
        let line = line.trim_end_matches('\r');
        if line.is_empty() || line.starts_with(':') {
            continue;
        }
        if let Some(rest) = line.strip_prefix("data:") {
            let rest = rest.strip_prefix(' ').unwrap_or(rest);
            if !data.is_empty() {
                data.push('\n');
            }
            data.push_str(rest);
        }
    }

    if data.is_empty() {
        return Ok(None);
    }
    if data == "[DONE]" {
        return Ok(Some(ParsedEvent::Done));
    }

    let value: Value =
        serde_json::from_str(&data).with_context(|| format!("parsing SSE data line: {data}"))?;
    Ok(Some(ParsedEvent::Chunk(value)))
}

fn extract_events(chunk: &Value) -> anyhow::Result<Vec<StreamEvent>> {
    let mut out = Vec::new();
    let Some(choices) = chunk.get("choices").and_then(|c| c.as_array()) else {
        return Ok(out);
    };
    for choice in choices {
        let delta = choice.get("delta");
        if let Some(delta) = delta {
            if let Some(content) = delta.get("content").and_then(|c| c.as_str()) {
                if !content.is_empty() {
                    out.push(StreamEvent::TextDelta(content.to_string()));
                }
            }
            if let Some(tool_calls) = delta.get("tool_calls").and_then(|t| t.as_array()) {
                for tc in tool_calls {
                    let index = tc
                        .get("index")
                        .and_then(|i| i.as_u64())
                        .map(|i| i as u32)
                        .unwrap_or(0);
                    let id = tc
                        .get("id")
                        .and_then(|s| s.as_str())
                        .map(|s| s.to_string());
                    let function = tc.get("function");
                    let name = function
                        .and_then(|f| f.get("name"))
                        .and_then(|s| s.as_str())
                        .map(|s| s.to_string());
                    let args_delta = function
                        .and_then(|f| f.get("arguments"))
                        .and_then(|s| s.as_str())
                        .map(|s| s.to_string());
                    out.push(StreamEvent::ToolCallDelta(ToolCallDelta {
                        index,
                        id,
                        name,
                        args_delta,
                    }));
                }
            }
        }

        if let Some(reason) = choice.get("finish_reason").and_then(|r| r.as_str()) {
            out.push(StreamEvent::Finish {
                reason: reason.to_string(),
            });
        }
    }
    Ok(out)
}
