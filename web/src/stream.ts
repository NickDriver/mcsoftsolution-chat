/**
 * Streaming client for the chat widget.
 *
 * The endpoints are served by the companion `mcsoftsolution-chat` Rust crate
 * mounted at the same origin. By default we POST to `/api/mcsoft-chat`; pass
 * `apiBase` to retarget when the chat backend lives on a different origin
 * or path.
 */

export type McSoftRole = "user" | "assistant";

export type McSoftTranscriptEntry = {
  role: McSoftRole;
  text: string;
};

export type McSoftStreamEvent =
  | { type: "text"; delta: string }
  | { type: "tool_call_start"; id: string; name: string; args: unknown }
  | { type: "tool_call_result"; id: string; ok: boolean; summary: string }
  | { type: "done" }
  | { type: "error"; message: string };

type RawSseEvent = { event: string; data: string };

async function* parseSseStream(
  stream: ReadableStream<Uint8Array>,
): AsyncGenerator<RawSseEvent> {
  const reader = stream.getReader();
  const decoder = new TextDecoder();
  let buffer = "";
  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      buffer += decoder.decode(value, { stream: true });
      let boundary = buffer.indexOf("\n\n");
      while (boundary >= 0) {
        const raw = buffer.slice(0, boundary);
        buffer = buffer.slice(boundary + 2);
        boundary = buffer.indexOf("\n\n");
        if (!raw.trim()) continue;
        let event = "message";
        const dataLines: string[] = [];
        for (const line of raw.split("\n")) {
          const stripped = line.replace(/\r$/, "");
          if (stripped.startsWith(":")) continue;
          if (stripped.startsWith("event:")) {
            event = stripped.slice(6).trim();
          } else if (stripped.startsWith("data:")) {
            const rest = stripped.slice(5);
            dataLines.push(rest.startsWith(" ") ? rest.slice(1) : rest);
          }
        }
        yield { event, data: dataLines.join("\n") };
      }
    }
  } finally {
    reader.releaseLock();
  }
}

export interface StreamOptions {
  /** Base URL prefix; default `""` (same origin, relative paths). */
  apiBase?: string;
  signal?: AbortSignal;
}

export async function* streamMcSoftChat(
  history: McSoftTranscriptEntry[],
  options: StreamOptions = {},
): AsyncGenerator<McSoftStreamEvent> {
  const url = `${options.apiBase ?? ""}/api/mcsoft-chat`;
  let response: Response;
  try {
    response = await fetch(url, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Accept: "text/event-stream",
      },
      body: JSON.stringify({ history }),
      signal: options.signal,
    });
  } catch (err) {
    yield {
      type: "error",
      message: err instanceof Error ? err.message : "network error",
    };
    return;
  }

  if (!response.ok) {
    const text = await response.text().catch(() => "");
    yield {
      type: "error",
      message: text || `${response.status} ${response.statusText}`,
    };
    return;
  }
  if (!response.body) {
    yield { type: "error", message: "no response body" };
    return;
  }

  for await (const sse of parseSseStream(response.body)) {
    const eventName = sse.event || "message";
    let payload: Record<string, unknown> = {};
    if (sse.data) {
      try {
        payload = JSON.parse(sse.data);
      } catch {
        // tolerate non-JSON (e.g. empty `done` payload)
      }
    }
    yield {
      type: eventName,
      ...(payload as Record<string, unknown>),
    } as McSoftStreamEvent;
    if (eventName === "done" || eventName === "error") return;
  }
}

export async function isMcSoftChatEnabled(apiBase = ""): Promise<boolean> {
  try {
    const r = await fetch(`${apiBase}/api/mcsoft-chat/status`);
    if (!r.ok) return false;
    const j = (await r.json()) as { enabled?: boolean };
    return Boolean(j.enabled);
  } catch {
    return false;
  }
}
