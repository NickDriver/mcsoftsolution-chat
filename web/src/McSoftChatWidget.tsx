import { useEffect, useRef, useState, type ReactNode } from "react";
import { Loader2, Send, Sparkles, X } from "lucide-react";

import {
  type McSoftTranscriptEntry,
  fetchMcSoftChatConfig,
  streamMcSoftChat,
} from "./stream";
import { ensureStylesInjected } from "./styles";

type ToolCall = {
  id: string;
  name: string;
  args: unknown;
  status: "running" | "ok" | "error";
  summary?: string;
};

type Bubble =
  | { id: string; role: "user"; text: string }
  | {
      id: string;
      role: "assistant";
      text: string;
      toolCalls: ToolCall[];
      error: string | null;
    };

export interface Props {
  /** Whether the panel is open. Controlled by the parent. */
  open: boolean;
  /** Called when the user closes the panel (Esc, X button, click-outside). */
  onClose: () => void;
  /** Backend prefix; default `""` (same origin). Set to e.g. `https://api.example.com` for cross-origin. */
  apiBase?: string;
  /** Header eyebrow text. Default: "Live · grounded in MC Soft's MCP". */
  eyebrow?: string;
  /** Header title. Default: "Chat with Mykolay". */
  title?: string;
  /** Header subtitle. Default: "MC Soft Solution. Ask anything." */
  subtitle?: string;
  /** Empty-state intro paragraph. */
  introText?: string;
  /** Empty-state starter prompts. */
  starterQuestions?: string[];
  /** Suggested follow-ups shown after each assistant turn. Rotates 3 at a time. */
  followupQuestions?: string[];
}

let nextId = 0;
const newId = () => `mc${++nextId}`;

// Last-resort fallbacks if the server returns no suggestions. The live lists
// come from the DB-versioned chat prompt (served via /api/mcsoft-chat/status).
const DEFAULT_STARTERS = [
  "What could custom software do for my business?",
  "How much would a booking system cost?",
  "Can you show me an example you've built?",
];

const DEFAULT_FOLLOWUPS = [
  "How much for my type of business?",
  "How long would it take?",
  "What would I actually own?",
  "Do I pay monthly fees?",
  "Can I see a real example?",
  "How do we get started?",
];

const DEFAULT_INTRO =
  "I'm an AI agent grounded in MC Soft Solution's public MCP server. " +
  "Ask about services, pricing, this demo, or how we'd build one for you. " +
  "Your messages aren't stored.";

function pickFollowups(turn: number, bank: string[]): string[] {
  if (bank.length === 0) return [];
  const n = Math.min(3, bank.length);
  const start = (Math.max(0, turn) * n) % bank.length;
  const out: string[] = [];
  for (let i = 0; i < n; i++) {
    out.push(bank[(start + i) % bank.length]);
  }
  return out;
}

export function McSoftChatWidget({
  open,
  onClose,
  apiBase = "",
  eyebrow = "Live · grounded in MC Soft's MCP",
  title = "Chat with Mykolay",
  subtitle = "MC Soft Solution. Ask anything.",
  introText = DEFAULT_INTRO,
  starterQuestions = DEFAULT_STARTERS,
  followupQuestions = DEFAULT_FOLLOWUPS,
}: Props) {
  const [enabled, setEnabled] = useState<boolean | null>(null);
  const [bubbles, setBubbles] = useState<Bubble[]>([]);
  const [input, setInput] = useState("");
  const [streaming, setStreaming] = useState(false);
  // Server-driven suggestion lists (DB-versioned). `null` until fetched; an
  // empty server list leaves these null so the prop/built-in defaults win.
  const [remoteStarters, setRemoteStarters] = useState<string[] | null>(null);
  const [remoteFollowups, setRemoteFollowups] = useState<string[] | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    ensureStylesInjected();
  }, []);

  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    void fetchMcSoftChatConfig(apiBase).then((cfg) => {
      if (cancelled) return;
      setEnabled(cfg.enabled);
      if (cfg.starters.length > 0) setRemoteStarters(cfg.starters);
      if (cfg.followups.length > 0) setRemoteFollowups(cfg.followups);
    });
    return () => {
      cancelled = true;
    };
  }, [open, apiBase]);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  useEffect(() => {
    scrollRef.current?.scrollTo({
      top: scrollRef.current.scrollHeight,
      behavior: "smooth",
    });
  }, [bubbles]);

  if (!open) return null;

  const sendMessage = async (text: string) => {
    if (streaming || !text.trim()) return;
    const trimmed = text.trim();

    const userBubble: Bubble = { id: newId(), role: "user", text: trimmed };
    const assistantBubble: Bubble = {
      id: newId(),
      role: "assistant",
      text: "",
      toolCalls: [],
      error: null,
    };

    setBubbles((b) => [...b, userBubble, assistantBubble]);
    setStreaming(true);

    const transcript: McSoftTranscriptEntry[] = [
      ...bubbles.map<McSoftTranscriptEntry>((b) => ({
        role: b.role,
        text: b.text,
      })),
      { role: "user", text: trimmed },
    ];

    const updateAssistant = (mut: (b: Bubble) => Bubble) =>
      setBubbles((current) => {
        const next = [...current];
        const idx = next.findIndex((m) => m.id === assistantBubble.id);
        if (idx === -1) return current;
        next[idx] = mut(next[idx]);
        return next;
      });

    try {
      for await (const evt of streamMcSoftChat(transcript, { apiBase })) {
        switch (evt.type) {
          case "text":
            updateAssistant((b) =>
              b.role === "assistant" ? { ...b, text: b.text + evt.delta } : b,
            );
            break;
          case "tool_call_start":
            updateAssistant((b) =>
              b.role === "assistant"
                ? {
                    ...b,
                    toolCalls: [
                      ...b.toolCalls,
                      { id: evt.id, name: evt.name, args: evt.args, status: "running" },
                    ],
                  }
                : b,
            );
            break;
          case "tool_call_result":
            updateAssistant((b) =>
              b.role === "assistant"
                ? {
                    ...b,
                    toolCalls: b.toolCalls.map((tc) =>
                      tc.id === evt.id
                        ? { ...tc, status: evt.ok ? "ok" : "error", summary: evt.summary }
                        : tc,
                    ),
                  }
                : b,
            );
            break;
          case "error":
            updateAssistant((b) =>
              b.role === "assistant" ? { ...b, error: evt.message } : b,
            );
            break;
          case "done":
            break;
        }
      }
    } finally {
      setStreaming(false);
    }
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    const text = input;
    setInput("");
    void sendMessage(text);
  };

  // Precedence: server (DB-versioned) > caller prop > built-in default.
  const starters = remoteStarters ?? starterQuestions;
  const followups = remoteFollowups ?? followupQuestions;

  return (
    <div
      className="mcsoft-chat-root"
      role="dialog"
      aria-modal="true"
      aria-label="Chat with MC Soft Solution"
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="mcsoft-chat-panel">
        <header className="mcsoft-chat-header">
          <div style={{ minWidth: 0 }}>
            <p className="mcsoft-chat-eyebrow">
              <Sparkles size={12} strokeWidth={2.5} style={{ color: "var(--_brand-bright)" }} />
              {eyebrow}
            </p>
            <h2 className="mcsoft-chat-title">{title}</h2>
            <p className="mcsoft-chat-subtitle">{subtitle}</p>
          </div>
          <button
            type="button"
            onClick={onClose}
            aria-label="Close"
            className="mcsoft-chat-close"
          >
            <X size={16} strokeWidth={2} />
          </button>
        </header>

        <div ref={scrollRef} className="mcsoft-chat-body">
          {enabled === false ? (
            <div className="mcsoft-chat-warning">
              The chat backend isn't configured.{" "}
              <code>MCSOFT_OPENROUTER_API_KEY</code> isn't set on the server.
            </div>
          ) : bubbles.length === 0 ? (
            <div className="mcsoft-chat-empty">
              <p className="mcsoft-chat-empty-text">{introText}</p>
              {starters.length > 0 && (
                <div>
                  <p className="mcsoft-chat-section-label">Try one of these</p>
                  {starters.map((q) => (
                    <button
                      key={q}
                      type="button"
                      disabled={enabled !== true}
                      onClick={() => void sendMessage(q)}
                      className="mcsoft-chat-suggestion"
                    >
                      {q}
                    </button>
                  ))}
                </div>
              )}
            </div>
          ) : (
            bubbles.map((b, i) => {
              if (b.role === "user") {
                return (
                  <div key={b.id} className="mcsoft-chat-bubble-user">
                    <div>{b.text}</div>
                  </div>
                );
              }
              const isLast = i === bubbles.length - 1;
              const assistantTurn = bubbles
                .slice(0, i + 1)
                .filter((x) => x.role === "assistant").length;
              return (
                <AssistantBubble
                  key={b.id}
                  bubble={b}
                  streaming={streaming}
                  showFollowups={isLast && !streaming}
                  followupTurn={assistantTurn - 1}
                  followupBank={followups}
                  onFollowupPick={(q) => void sendMessage(q)}
                />
              );
            })
          )}
        </div>

        <form onSubmit={handleSubmit} className="mcsoft-chat-composer">
          <textarea
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                handleSubmit(e);
              }
            }}
            rows={1}
            disabled={streaming || enabled === false}
            placeholder={
              enabled === false
                ? "Chat unavailable"
                : streaming
                  ? "Thinking…"
                  : "Ask about MC Soft, this demo, pricing…"
            }
            className="mcsoft-chat-textarea"
          />
          <button
            type="submit"
            disabled={!input.trim() || streaming || enabled === false}
            className="mcsoft-chat-send"
            aria-label="Send"
          >
            {streaming ? (
              <Loader2 size={16} strokeWidth={2} style={{ animation: "mcsoft-spin 0.8s linear infinite" }} />
            ) : (
              <Send size={16} strokeWidth={2} />
            )}
          </button>
        </form>
      </div>
    </div>
  );
}

function AssistantBubble({
  bubble,
  streaming,
  showFollowups,
  followupTurn,
  followupBank,
  onFollowupPick,
}: {
  bubble: Extract<Bubble, { role: "assistant" }>;
  streaming: boolean;
  showFollowups: boolean;
  followupTurn: number;
  followupBank: string[];
  onFollowupPick: (q: string) => void;
}) {
  const showCursor =
    streaming &&
    !bubble.error &&
    bubble.toolCalls.every((tc) => tc.status !== "running");

  const renderFollowups =
    showFollowups && !bubble.error && bubble.text.trim().length > 0;

  return (
    <div className="mcsoft-chat-bubble-assistant">
      {bubble.toolCalls.map((tc) => (
        <div
          key={tc.id}
          className={`mcsoft-chat-toolcall${tc.status === "error" ? " error" : ""}`}
        >
          {tc.status === "running" ? (
            <span className="mcsoft-chat-toolcall-spinner" aria-hidden />
          ) : (
            <span className="mcsoft-chat-toolcall-dot" aria-hidden />
          )}
          <code>{tc.name}</code>
          <span className="mcsoft-chat-toolcall-summary">
            {tc.status === "running" ? "calling MCP…" : tc.summary || "ok"}
          </span>
        </div>
      ))}

      {bubble.text && (
        <div className="mcsoft-chat-text">
          {renderRich(bubble.text)}
          {showCursor && <span className="mcsoft-chat-cursor">▍</span>}
        </div>
      )}

      {bubble.error && <div className="mcsoft-chat-error">{bubble.error}</div>}

      {renderFollowups && (
        <div className="mcsoft-chat-followups">
          <p className="mcsoft-chat-section-label">Suggested follow-ups</p>
          <div className="mcsoft-chat-followups-row">
            {pickFollowups(followupTurn, followupBank).map((q) => (
              <button
                key={q}
                type="button"
                onClick={() => onFollowupPick(q)}
                className="mcsoft-chat-followup-chip"
              >
                {q}
              </button>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

/**
 * Render assistant text as React nodes: `**bold**` becomes <strong>, and
 * link-ish spans become anchors — markdown links `[label](href)` (relative or
 * absolute), bracketed bare paths like `[ /contact ]`, bare http(s) URLs, and
 * bare site paths (`/contact`, `/pricing`, `/case-studies`, `/blog`). Newlines
 * are preserved by the caller's `white-space` CSS.
 */
function renderRich(text: string): ReactNode[] {
  const out: ReactNode[] = [];
  const boldRe = /\*\*([^*]+)\*\*/g;
  let last = 0;
  let m: RegExpExecArray | null;
  let i = 0;
  while ((m = boldRe.exec(text)) !== null) {
    if (m.index > last) out.push(...linkify(text.slice(last, m.index), `t${i}`));
    out.push(<strong key={`b${i}`}>{linkify(m[1], `s${i}`)}</strong>);
    i++;
    last = m.index + m[0].length;
  }
  if (last < text.length) out.push(...linkify(text.slice(last), `t${i}`));
  return out;
}

/**
 * Turn link-ish spans within a plain-text run into anchors. Relative hrefs
 * (same-site, e.g. `/contact`) open in place; absolute URLs open in a new tab.
 * Trailing punctuation on bare URLs stays as text so the link doesn't swallow a
 * sentence's period.
 */
function linkify(text: string, keyPrefix: string): ReactNode[] {
  const pattern =
    /\[([^\]]+)\]\(\s*([^\s)]+)\s*\)|\[\s*(\/[A-Za-z0-9][\w/-]*)\s*\]|(https?:\/\/[^\s<>"]+)|(\/(?:contact|pricing|case-studies|blog)\b)/g;
  const out: ReactNode[] = [];
  let last = 0;
  let m: RegExpExecArray | null;
  let i = 0;
  const anchor = (href: string, label: string) =>
    /^https?:\/\//.test(href) ? (
      <a key={`${keyPrefix}-${i++}`} href={href} target="_blank" rel="noopener noreferrer">
        {label}
      </a>
    ) : (
      <a key={`${keyPrefix}-${i++}`} href={href}>
        {label}
      </a>
    );
  while ((m = pattern.exec(text)) !== null) {
    if (m.index > last) out.push(text.slice(last, m.index));
    if (m[1] !== undefined && m[2] !== undefined) {
      out.push(anchor(m[2], m[1])); // [label](href)
    } else if (m[3] !== undefined) {
      out.push(anchor(m[3], m[3])); // [ /path ] — drop the brackets
    } else if (m[4] !== undefined) {
      let url = m[4];
      let trail = "";
      while (url.length > 0 && /[.,!?;:)\]]/.test(url[url.length - 1])) {
        trail = url[url.length - 1] + trail;
        url = url.slice(0, -1);
      }
      out.push(anchor(url, url)); // bare absolute URL
      if (trail) out.push(trail);
    } else if (m[5] !== undefined) {
      out.push(anchor(m[5], m[5])); // bare site path
    }
    last = m.index + m[0].length;
  }
  if (last < text.length) out.push(text.slice(last));
  return out;
}
