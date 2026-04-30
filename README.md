# mcsoftsolution-chat

Drop-in **"Chat with us"** widget for MC Soft Solution demo sites.

A small Rust crate + a small TypeScript package. Mount the crate's Axum router on your backend, render the widget in your React app, set one env var, done. The chat bot answers grounded in [MC Soft's public MCP server](https://mcsoftsolution.com/mcp) (services, blog posts, contact info, portfolio) — same MCP-first pattern we sell.

```
mcsoftsolution-chat/
├── server/    Rust library crate — Axum router + OpenRouter streaming + HTTP MCP client
└── web/       TypeScript package — React widget + SSE consumer
```

Used across MC Soft demos (franchise-match-ai, future ones). One repo to fix bugs in, one place to add features.

---

## What you get

- **Backend**: a single function `mcsoftsolution_chat::router(cfg)` returning an `axum::Router` you `.merge()` into your app. Adds `POST /api/mcsoft-chat` (SSE) and `GET /api/mcsoft-chat/status`.
- **Frontend**: a `<McSoftChatWidget open onClose />` React component you stick anywhere in your app. Self-contained styles via CSS variables — works without Tailwind.
- **Privacy**: no DB, no session storage. The browser holds the transcript and re-posts it on each turn. No keys ever reach the client.
- **MCP wedge**: tool calls are visible inline in the UI so visitors see the chat actually calling tools, not just hallucinating.
- **Graceful fallback**: if the API key is unset, the router still mounts and reports `enabled: false`, and the widget displays a friendly "not configured" panel instead of breaking.

---

## Backend — install

In the consuming Axum project's `Cargo.toml`:

```toml
[dependencies]
mcsoftsolution-chat = { path = "../mcsoftsolution-chat/server" }
# or as a git dep:
# mcsoftsolution-chat = { git = "https://github.com/mcsoftsolution/mcsoftsolution-chat", subdir = "server" }
```

In `main.rs`:

```rust
use axum::Router;
use mcsoftsolution_chat::{ChatConfig, router};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Reads MCSOFT_OPENROUTER_API_KEY / MCSOFT_CHAT_MODEL / MCSOFT_MCP_URL.
    // Returns None if the API key isn't set — chat is then a graceful no-op.
    let chat_cfg = ChatConfig::from_env();

    let app = Router::new()
        .merge(router(chat_cfg))
        // ... your other routes
        ;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080").await?;
    axum::serve(listener, app).await?;
    Ok(())
}
```

### Env vars

| Var | Required | Default | Notes |
|-----|----------|---------|-------|
| `MCSOFT_OPENROUTER_API_KEY` | yes | — | Server-side OpenRouter key. **Never exposed to the client.** Cap the spend on this key in OpenRouter — it's the budget for the chat. |
| `MCSOFT_CHAT_MODEL` | no | `google/gemma-4-26b-a4b-it` | Any OpenRouter-supported chat model with tool-use. |
| `MCSOFT_MCP_URL` | no | `https://mcsoftsolution.com/mcp` | Public MCP endpoint to ground answers in. Override to point at a different MCP server. |
| `MCSOFT_SITE_LABEL` | no | — | Inserted into the system prompt (e.g. `"franchise-match-ai demo"`) so the bot knows where the visitor is. |

### Programmatic config

For tighter control, skip `from_env` and use the builder:

```rust
let cfg = ChatConfig::builder()
    .openrouter_api_key("sk-or-v1-...")
    .model("anthropic/claude-haiku-4-5")
    .mcp_url("https://your-mcp.example.com/mcp")
    .site_label("acme-demo")
    .system_prompt_override("You are Acme's chat. Always answer in haiku.")
    .build();

let app = Router::new().merge(router(Some(cfg)));
```

### Standalone example

```sh
cd server
MCSOFT_OPENROUTER_API_KEY=sk-or-v1-... cargo run --example standalone
# then: curl http://127.0.0.1:8080/api/mcsoft-chat/status
```

---

## Frontend — install

In the consuming web project's `package.json`:

```json
{
  "dependencies": {
    "@mcsoftsolution/chat-widget": "file:../mcsoftsolution-chat/web",
    "lucide-react": "^0.469.0"
  }
}
```

(Or use `link:` / a git submodule / a private npm registry — whatever fits your workflow.)

In your root CSS file (e.g. `src/index.css`):

```css
@import "@mcsoftsolution/chat-widget/theme.css";
```

In your app:

```tsx
import { useState } from "react";
import { McSoftChatWidget } from "@mcsoftsolution/chat-widget";

export function App() {
  const [chatOpen, setChatOpen] = useState(false);

  return (
    <>
      <button onClick={() => setChatOpen(true)}>Chat with me</button>

      {/* …your app… */}

      <McSoftChatWidget
        open={chatOpen}
        onClose={() => setChatOpen(false)}
      />
    </>
  );
}
```

### Customizing the widget

All copy is overridable via props:

```tsx
<McSoftChatWidget
  open={open}
  onClose={() => setOpen(false)}
  apiBase=""                                    // set if backend is on a different origin
  eyebrow="Live · grounded in MC Soft's MCP"
  title="Chat with Mykolay"
  subtitle="MC Soft Solution. Ask anything."
  introText="..."
  starterQuestions={["...", "..."]}
  followupQuestions={["...", "..."]}
/>
```

### Styling

Themed via CSS custom properties. The defaults are MC Soft's red/black/cream palette. To override, declare the vars at any selector that contains the widget's root:

```css
.mcsoft-chat-root {
  --mc-brand: #1e40af;        /* indigo */
  --mc-brand-hover: #1e3a8a;
  --mc-brand-bright: #3b82f6;
  --mc-paper: #ffffff;
  --mc-ink: #0f172a;
  /* ...etc; see web/theme.css for the full list */
}
```

If you don't import `theme.css` at all, the widget falls back to the MC Soft palette via inline defaults — it'll always render.

---

## Architecture

```
Browser                              Your Axum server                    OpenRouter
─────────                            ─────────────────                   ──────────
McSoftChatWidget ───POST /api/mcsoft-chat──>  router()
   ▲                                          │
   │                                          ├── McpClient ──tools/call──> mcsoftsolution.com/mcp
   │                                          │                                      │
   │                                          │<─────────────────────────────────────┘
   │                                          │
   │                                          └── stream_chat ─────────────> OpenRouter
   │<─────SSE: text/tool_call_*/done──────────┘
```

- The **browser holds the transcript** — no DB, no session cookies, no PII.
- The **server holds the API key** — never sent to the client.
- The **MCP client speaks Streamable HTTP** with the spec-mandated `initialize` + `notifications/initialized` handshake; auto-reconnects once on `Session not found` (HTTP 401).

---

## What this is *not*

- Not a full chat platform. No persistence, no admin UI, no message history.
- Not multi-tenant. One config per process.
- Not a generic OpenAI proxy. The handler hard-codes the MCP-grounded agent loop; if you need a vanilla chat endpoint, you already have one.

---

## License

MIT. See [LICENSE](./LICENSE).
