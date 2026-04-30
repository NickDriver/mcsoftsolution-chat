//! Default system prompt + tool-list appendix.

use crate::mcp::McpTool;

/// Build the system prompt fed to OpenRouter for each turn. If
/// `override_text` is set, that text replaces the default body; the live
/// tool list is always appended at the end so the model is grounded in
/// what's actually callable.
pub fn build(override_text: Option<&str>, site_label: Option<&str>, tools: &[McpTool]) -> String {
    let mut out = match override_text {
        Some(text) => text.to_string(),
        None => default_body(site_label),
    };
    if !tools.is_empty() {
        out.push_str("\n\nAvailable tools (live, via MC Soft's public MCP server):\n");
        for t in tools {
            out.push_str(&format!("- {}: {}\n", t.name, t.description));
        }
    }
    out
}

fn default_body(site_label: Option<&str>) -> String {
    let surface = site_label
        .map(|l| format!(" on the {l}"))
        .unwrap_or_default();

    format!(
        "You are a friendly assistant{surface}, representing Mykolay Chelabchi (MC Soft Solution). \
Visitors clicked \"Chat with me\" to ask about MC Soft's services, this site, pricing, or to start \
a project.\n\n\
Tone: warm, concise, plain-spoken. No hype. Short paragraphs.\n\n\
Rules:\n\
- For any factual question about MC Soft (services, pricing, contact, blog, portfolio), CALL THE \
TOOLS below. Do not invent details. Quote tool results faithfully.\n\
- Refer to MC Soft as \"we\"/\"I\" (Mykolay is the sole engineer).\n\
- For \"how do I hire you / start a project\", point to https://mcsoftsolution.com/contact and the \
three engagement tiers (free demo, free private pilot, $5–8K starter sprint).\n\
- Don't promise timelines unless the visitor gives you scope.\n\
- Refuse off-topic asks politely (\"I'm here to talk about MC Soft\").\n",
    )
}
