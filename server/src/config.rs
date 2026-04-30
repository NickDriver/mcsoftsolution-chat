//! Configuration for the chat service.
//!
//! Two construction paths:
//! - [`ChatConfig::from_env`] — pulls from `MCSOFT_OPENROUTER_API_KEY`,
//!   `MCSOFT_CHAT_MODEL`, `MCSOFT_MCP_URL`, `MCSOFT_SITE_LABEL`. Returns
//!   `None` if the API key is unset.
//! - [`ChatConfig::builder`] — explicit, programmatic.

use std::env;

const DEFAULT_MODEL: &str = "google/gemma-4-26b-a4b-it";
const DEFAULT_MCP_URL: &str = "https://mcsoftsolution.com/mcp";

/// Runtime configuration. Cheap to clone (all fields are owned strings).
#[derive(Debug, Clone)]
pub struct ChatConfig {
    pub openrouter_api_key: String,
    pub model: String,
    pub mcp_url: String,
    /// Label used in the default system prompt to identify the surface the
    /// visitor is chatting from (e.g. `"franchise-match-ai"`).
    pub site_label: Option<String>,
    /// Full system prompt override. When `Some`, replaces the default
    /// entirely (the live tool list is still appended at the end).
    pub system_prompt_override: Option<String>,
}

impl ChatConfig {
    pub fn builder() -> ChatConfigBuilder {
        ChatConfigBuilder::default()
    }

    /// Read config from env. Returns `None` if `MCSOFT_OPENROUTER_API_KEY`
    /// isn't set (or is empty) — the widget will mount but report
    /// `enabled: false`.
    pub fn from_env() -> Option<Self> {
        let key = env::var("MCSOFT_OPENROUTER_API_KEY").ok().filter(|k| !k.is_empty())?;
        Some(Self {
            openrouter_api_key: key,
            model: env::var("MCSOFT_CHAT_MODEL")
                .ok()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| DEFAULT_MODEL.to_string()),
            mcp_url: env::var("MCSOFT_MCP_URL")
                .ok()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| DEFAULT_MCP_URL.to_string()),
            site_label: env::var("MCSOFT_SITE_LABEL").ok().filter(|s| !s.is_empty()),
            system_prompt_override: None,
        })
    }
}

#[derive(Debug, Default, Clone)]
pub struct ChatConfigBuilder {
    openrouter_api_key: Option<String>,
    model: Option<String>,
    mcp_url: Option<String>,
    site_label: Option<String>,
    system_prompt_override: Option<String>,
}

impl ChatConfigBuilder {
    pub fn openrouter_api_key(mut self, key: impl Into<String>) -> Self {
        self.openrouter_api_key = Some(key.into());
        self
    }
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }
    pub fn mcp_url(mut self, url: impl Into<String>) -> Self {
        self.mcp_url = Some(url.into());
        self
    }
    pub fn site_label(mut self, label: impl Into<String>) -> Self {
        self.site_label = Some(label.into());
        self
    }
    pub fn system_prompt_override(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt_override = Some(prompt.into());
        self
    }

    /// Build the config. Panics if no OpenRouter key was provided — call
    /// [`ChatConfig::from_env`] if you want a graceful "disabled" path.
    pub fn build(self) -> ChatConfig {
        ChatConfig {
            openrouter_api_key: self
                .openrouter_api_key
                .expect("ChatConfigBuilder: openrouter_api_key is required"),
            model: self.model.unwrap_or_else(|| DEFAULT_MODEL.to_string()),
            mcp_url: self.mcp_url.unwrap_or_else(|| DEFAULT_MCP_URL.to_string()),
            site_label: self.site_label,
            system_prompt_override: self.system_prompt_override,
        }
    }
}
