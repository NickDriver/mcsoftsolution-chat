//! Configuration for the chat service.
//!
//! Two construction paths:
//! - [`ChatConfig::from_env`] — pulls from `MCSOFT_OPENROUTER_API_KEY`,
//!   `MCSOFT_CHAT_MODEL`, `MCSOFT_MCP_URL`, `MCSOFT_SITE_LABEL`. Returns
//!   `None` if the API key is unset.
//! - [`ChatConfig::builder`] — explicit, programmatic. Accepts either a
//!   static API key via [`ChatConfigBuilder::openrouter_api_key`] or a
//!   dynamic resolver via [`ChatConfigBuilder::key_provider`] (read fresh
//!   each request — useful when the key lives in a rotating secrets store).

use std::env;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

const DEFAULT_MODEL: &str = "google/gemma-4-26b-a4b-it";
const DEFAULT_MCP_URL: &str = "https://mcsoftsolution.com/mcp";

/// Async function returning the current OpenRouter API key, or `None` if
/// unset. Called once per `/api/mcsoft-chat` request — keep it cheap (a
/// cached DB read is fine; a network round-trip is not).
pub type KeyProvider = Arc<
    dyn Fn() -> Pin<Box<dyn Future<Output = Option<String>> + Send>>
        + Send
        + Sync,
>;

/// Async function returning the current full system-prompt override, or `None`
/// to fall back to the default body. Called once per `/api/mcsoft-chat`
/// request — keep it cheap (a cached DB read is fine; a network round-trip is
/// not). Lets the prompt live in an editable store and update without a restart.
pub type PromptProvider = Arc<
    dyn Fn() -> Pin<Box<dyn Future<Output = Option<String>> + Send>>
        + Send
        + Sync,
>;

/// Async function returning the widget's `(starters, followups)` suggestion
/// lists. Called once per `/api/mcsoft-chat/status` request — keep it cheap (a
/// cached DB read is fine). Empty lists mean the widget uses its own defaults.
pub type SuggestionsProvider = Arc<
    dyn Fn() -> Pin<Box<dyn Future<Output = (Vec<String>, Vec<String>)> + Send>>
        + Send
        + Sync,
>;

/// Runtime configuration. Cheap to clone (all owned fields).
#[derive(Clone)]
pub struct ChatConfig {
    /// Static API key. Used when `key_provider` is `None`.
    pub openrouter_api_key: String,
    /// Optional dynamic resolver. When set, takes precedence over
    /// `openrouter_api_key` and is awaited fresh on every request.
    pub key_provider: Option<KeyProvider>,
    pub model: String,
    pub mcp_url: String,
    /// Label used in the default system prompt to identify the surface the
    /// visitor is chatting from (e.g. `"franchise-match-ai"`).
    pub site_label: Option<String>,
    /// Full system prompt override. When `Some`, replaces the default
    /// entirely (the live tool list is still appended at the end). Used when
    /// `prompt_provider` is `None`.
    pub system_prompt_override: Option<String>,
    /// Optional dynamic resolver for the system-prompt override. When set,
    /// takes precedence over `system_prompt_override` and is awaited fresh on
    /// every request.
    pub prompt_provider: Option<PromptProvider>,
    /// Optional resolver for the widget's starter/follow-up suggestions,
    /// awaited fresh on each `/status` request.
    pub suggestions_provider: Option<SuggestionsProvider>,
}

impl std::fmt::Debug for ChatConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChatConfig")
            .field(
                "openrouter_api_key",
                &if self.openrouter_api_key.is_empty() { "<empty>" } else { "<redacted>" },
            )
            .field("key_provider", &self.key_provider.as_ref().map(|_| "<fn>"))
            .field("model", &self.model)
            .field("mcp_url", &self.mcp_url)
            .field("site_label", &self.site_label)
            .field(
                "system_prompt_override",
                &self.system_prompt_override.as_ref().map(|_| "<set>"),
            )
            .field("prompt_provider", &self.prompt_provider.as_ref().map(|_| "<fn>"))
            .field("suggestions_provider", &self.suggestions_provider.as_ref().map(|_| "<fn>"))
            .finish()
    }
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
            key_provider: None,
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
            prompt_provider: None,
            suggestions_provider: None,
        })
    }

    /// Resolve the system-prompt override for the current request. Awaits the
    /// provider when set; otherwise returns the static override (or `None`).
    pub async fn resolve_prompt_override(&self) -> Option<String> {
        if let Some(provider) = &self.prompt_provider {
            provider().await
        } else {
            self.system_prompt_override.clone()
        }
    }

    /// Resolve the widget's `(starters, followups)` for the current request.
    /// Returns empty lists when no provider is installed (the widget then falls
    /// back to its own built-in defaults).
    pub async fn resolve_suggestions(&self) -> (Vec<String>, Vec<String>) {
        if let Some(provider) = &self.suggestions_provider {
            provider().await
        } else {
            (Vec::new(), Vec::new())
        }
    }

    /// Resolve the API key for the current request. Awaits the provider
    /// when set; otherwise returns the static key (or `None` if empty).
    pub async fn resolve_key(&self) -> Option<String> {
        if let Some(provider) = &self.key_provider {
            provider().await
        } else if !self.openrouter_api_key.is_empty() {
            Some(self.openrouter_api_key.clone())
        } else {
            None
        }
    }
}

#[derive(Default, Clone)]
pub struct ChatConfigBuilder {
    openrouter_api_key: Option<String>,
    key_provider: Option<KeyProvider>,
    model: Option<String>,
    mcp_url: Option<String>,
    site_label: Option<String>,
    system_prompt_override: Option<String>,
    prompt_provider: Option<PromptProvider>,
    suggestions_provider: Option<SuggestionsProvider>,
}

impl ChatConfigBuilder {
    pub fn openrouter_api_key(mut self, key: impl Into<String>) -> Self {
        self.openrouter_api_key = Some(key.into());
        self
    }

    /// Install a dynamic key resolver. Takes precedence over the static
    /// key. Called fresh on every chat request — keep it fast.
    pub fn key_provider<F, Fut>(mut self, f: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Option<String>> + Send + 'static,
    {
        self.key_provider = Some(Arc::new(move || Box::pin(f())));
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

    /// Install a dynamic system-prompt resolver. Takes precedence over the
    /// static override. Called fresh on every chat request — keep it fast
    /// (a cached DB read is fine).
    pub fn prompt_provider<F, Fut>(mut self, f: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Option<String>> + Send + 'static,
    {
        self.prompt_provider = Some(Arc::new(move || Box::pin(f())));
        self
    }

    /// Install a dynamic resolver for the widget's starter/follow-up
    /// suggestions. Called fresh on each `/status` request — keep it fast.
    pub fn suggestions_provider<F, Fut>(mut self, f: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = (Vec<String>, Vec<String>)> + Send + 'static,
    {
        self.suggestions_provider = Some(Arc::new(move || Box::pin(f())));
        self
    }

    /// Build the config. Panics if neither a static `openrouter_api_key`
    /// nor a `key_provider` was supplied — there must be *some* path to
    /// an API key, even if it currently returns `None`.
    pub fn build(self) -> ChatConfig {
        if self.openrouter_api_key.is_none() && self.key_provider.is_none() {
            panic!("ChatConfigBuilder: openrouter_api_key or key_provider is required");
        }
        ChatConfig {
            openrouter_api_key: self.openrouter_api_key.unwrap_or_default(),
            key_provider: self.key_provider,
            model: self.model.unwrap_or_else(|| DEFAULT_MODEL.to_string()),
            mcp_url: self.mcp_url.unwrap_or_else(|| DEFAULT_MCP_URL.to_string()),
            site_label: self.site_label,
            system_prompt_override: self.system_prompt_override,
            prompt_provider: self.prompt_provider,
            suggestions_provider: self.suggestions_provider,
        }
    }
}
