/**
 * Single injected stylesheet for the chat widget. Scoped under
 * `.mcsoft-chat-root` so it doesn't leak into the host app. Uses CSS vars
 * defined in `theme.css` (or your overrides) — every var has an inline
 * fallback so the widget still renders if the consumer forgot to import
 * `theme.css`.
 */

const STYLE_ID = "mcsoft-chat-widget-styles";

export function ensureStylesInjected(): void {
  if (typeof document === "undefined") return;
  if (document.getElementById(STYLE_ID)) return;

  const style = document.createElement("style");
  style.id = STYLE_ID;
  style.textContent = CSS;
  document.head.appendChild(style);
}

const CSS = `
.mcsoft-chat-root {
  --_brand: var(--mc-brand, #e32726);
  --_brand-hover: var(--mc-brand-hover, #b71d1c);
  --_brand-bright: var(--mc-brand-bright, #ff3b3a);
  --_ink: var(--mc-ink, #0a0a0a);
  --_ink-muted: var(--mc-ink-muted, #4a4a4a);
  --_paper: var(--mc-paper, #fafaf7);
  --_paper-muted: var(--mc-paper-muted, #ebebe6);
  --_radius: var(--mc-radius, 0.75rem);
  --_radius-sm: var(--mc-radius-sm, 0.375rem);
  --_radius-md: var(--mc-radius-md, 0.5rem);
  --_shadow: var(--mc-shadow, 0 1px 2px rgba(10,10,10,0.04), 0 6px 16px -8px rgba(10,10,10,0.08));
  --_font: var(--mc-font, "Inter", system-ui, -apple-system, "Segoe UI", sans-serif);

  position: fixed;
  inset: 0;
  z-index: 50;
  display: flex;
  align-items: flex-end;
  justify-content: flex-end;
  padding: 0.75rem;
  background: rgba(10,10,10,0.3);
  font-family: var(--_font);
  color: var(--_ink);
  box-sizing: border-box;
}
.mcsoft-chat-root *,
.mcsoft-chat-root *::before,
.mcsoft-chat-root *::after {
  box-sizing: border-box;
}
.mcsoft-chat-root button {
  font-family: inherit;
  cursor: pointer;
}
.mcsoft-chat-root button:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}

.mcsoft-chat-panel {
  display: flex;
  flex-direction: column;
  width: 100%;
  max-width: 28rem;
  height: 100%;
  max-height: 640px;
  overflow: hidden;
  border-radius: var(--_radius);
  border: 1px solid rgba(10,10,10,0.15);
  background: var(--_paper);
  box-shadow: var(--_shadow);
}

.mcsoft-chat-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 0.75rem;
  padding: 0.75rem 1rem;
  border-bottom: 1px solid rgba(10,10,10,0.1);
  background: var(--_ink);
  color: var(--_paper);
}
.mcsoft-chat-eyebrow {
  display: inline-flex;
  align-items: center;
  gap: 0.375rem;
  font-size: 10px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.18em;
  color: rgba(250,250,247,0.6);
  margin: 0;
}
.mcsoft-chat-title {
  margin: 0.125rem 0 0;
  font-size: 1rem;
  font-weight: 600;
  color: var(--_paper);
}
.mcsoft-chat-subtitle {
  margin: 0.125rem 0 0;
  font-size: 11px;
  color: rgba(250,250,247,0.55);
}
.mcsoft-chat-close {
  background: transparent;
  border: 0;
  padding: 0.25rem;
  border-radius: var(--_radius-sm);
  color: rgba(250,250,247,0.7);
  display: inline-flex;
  align-items: center;
  justify-content: center;
}
.mcsoft-chat-close:hover {
  background: rgba(250,250,247,0.1);
  color: var(--_paper);
}

.mcsoft-chat-body {
  flex: 1;
  overflow-y: auto;
  padding: 1rem;
  background: var(--_paper);
}

.mcsoft-chat-empty {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
  font-size: 13px;
}
.mcsoft-chat-empty-text {
  color: var(--_ink-muted);
  line-height: 1.55;
  margin: 0;
}
.mcsoft-chat-section-label {
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: rgba(74,74,74,0.8);
  margin: 0 0 0.375rem;
}
.mcsoft-chat-suggestion {
  display: block;
  width: 100%;
  text-align: left;
  border: 1px solid rgba(10,10,10,0.1);
  background: var(--_paper);
  color: var(--_ink-muted);
  border-radius: var(--_radius-sm);
  padding: 0.5rem 0.75rem;
  font-size: 13px;
  margin-bottom: 0.375rem;
  transition: border-color 120ms, color 120ms, background 120ms;
}
.mcsoft-chat-suggestion:hover:not(:disabled) {
  border-color: rgba(10,10,10,0.25);
  color: var(--_ink);
  background: rgba(235,235,230,0.3);
}

.mcsoft-chat-bubble-user {
  display: flex;
  justify-content: flex-end;
  margin-bottom: 0.75rem;
}
.mcsoft-chat-bubble-user > div {
  max-width: 85%;
  background: var(--_ink);
  color: var(--_paper);
  border-radius: 1rem 1rem 0.125rem 1rem;
  padding: 0.375rem 0.75rem;
  font-size: 13.5px;
}
.mcsoft-chat-bubble-assistant {
  margin-bottom: 1rem;
}
.mcsoft-chat-text {
  font-size: 13.5px;
  line-height: 1.55;
  color: var(--_ink);
  white-space: pre-wrap;
}
.mcsoft-chat-text a {
  color: var(--_brand);
  text-decoration: underline;
  text-underline-offset: 2px;
  word-break: break-all;
}
.mcsoft-chat-text a:hover {
  color: var(--_brand-hover);
}
.mcsoft-chat-cursor {
  display: inline-block;
  margin-left: 0.125rem;
  animation: mcsoft-pulse 1.2s infinite ease-in-out;
}
@keyframes mcsoft-pulse {
  0%, 100% { opacity: 0.3; }
  50% { opacity: 1; }
}

.mcsoft-chat-toolcall {
  margin: 0.25rem 0;
  border-radius: var(--_radius-sm);
  border: 1px solid rgba(10,10,10,0.1);
  background: rgba(235,235,230,0.4);
  padding: 0.25rem 0.625rem;
  font-size: 11.5px;
  color: var(--_ink-muted);
  display: flex;
  align-items: center;
  gap: 0.5rem;
}
.mcsoft-chat-toolcall.error {
  border-color: rgba(252,165,165,0.7);
  background: rgba(254,226,226,0.6);
  color: rgb(185,28,28);
}
.mcsoft-chat-toolcall code {
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  font-size: 11px;
  color: var(--_ink);
}
.mcsoft-chat-toolcall-summary {
  margin-left: auto;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.mcsoft-chat-toolcall-dot {
  width: 0.5rem;
  height: 0.5rem;
  border-radius: 9999px;
  background: var(--_brand);
}
.mcsoft-chat-toolcall-spinner {
  width: 0.75rem;
  height: 0.75rem;
  border: 1.5px solid var(--_brand);
  border-top-color: transparent;
  border-radius: 9999px;
  animation: mcsoft-spin 0.8s linear infinite;
}
@keyframes mcsoft-spin { to { transform: rotate(360deg); } }

.mcsoft-chat-followups {
  margin-top: 0.75rem;
}
.mcsoft-chat-followups-row {
  display: flex;
  flex-wrap: wrap;
  gap: 0.375rem;
}
.mcsoft-chat-followup-chip {
  border-radius: 9999px;
  border: 1px solid rgba(10,10,10,0.15);
  background: var(--_paper);
  color: var(--_ink-muted);
  padding: 0.25rem 0.625rem;
  font-size: 12px;
  transition: border-color 120ms, color 120ms;
}
.mcsoft-chat-followup-chip:hover:not(:disabled) {
  border-color: var(--_brand);
  color: var(--_brand);
}

.mcsoft-chat-error {
  margin-top: 0.25rem;
  border-radius: var(--_radius-sm);
  border: 1px solid rgba(252,165,165,0.8);
  background: rgba(254,226,226,0.7);
  padding: 0.375rem 0.5rem;
  font-size: 12px;
  color: rgb(185,28,28);
}

.mcsoft-chat-warning {
  border-radius: var(--_radius-sm);
  border: 1px solid rgba(252,211,77,0.7);
  background: rgba(254,243,199,0.7);
  padding: 0.75rem;
  font-size: 12px;
  color: rgb(146,64,14);
}
.mcsoft-chat-warning code {
  background: rgba(254,243,199,1);
  padding: 0 0.25rem;
  border-radius: 2px;
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
}

.mcsoft-chat-composer {
  display: flex;
  align-items: flex-end;
  gap: 0.5rem;
  padding: 0.75rem;
  border-top: 1px solid rgba(10,10,10,0.1);
  background: rgba(235,235,230,0.3);
}
.mcsoft-chat-textarea {
  flex: 1;
  resize: none;
  min-height: 36px;
  max-height: 120px;
  border-radius: var(--_radius-sm);
  border: 1px solid rgba(10,10,10,0.15);
  background: var(--_paper);
  color: var(--_ink);
  padding: 0.375rem 0.625rem;
  font-size: 14px;
  font-family: inherit;
  outline: none;
}
.mcsoft-chat-textarea::placeholder { color: rgba(74,74,74,0.7); }
.mcsoft-chat-textarea:focus {
  border-color: var(--_brand);
  box-shadow: 0 0 0 1px var(--_brand);
}
.mcsoft-chat-textarea:disabled { opacity: 0.6; }

.mcsoft-chat-send {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  height: 36px;
  padding: 0 0.75rem;
  border-radius: var(--_radius-sm);
  background: var(--_brand);
  color: var(--_paper);
  border: 0;
  font-size: 14px;
  font-weight: 500;
  transition: background 120ms;
}
.mcsoft-chat-send:hover:not(:disabled) {
  background: var(--_brand-hover);
}
.mcsoft-chat-send:disabled {
  opacity: 0.4;
}
`;
