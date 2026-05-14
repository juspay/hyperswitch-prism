import { getConfig, type LlmConfig } from "./config.js";

export interface ToolDef {
  type: "function";
  function: {
    name: string;
    description: string;
    parameters: {
      type: "object";
      properties: Record<string, unknown>;
      required?: string[];
    };
  };
}

export type ToolHandler = (args: Record<string, unknown>) => Promise<string>;

export interface LlmCallOptions {
  system: string;
  user: string;
  maxTokens?: number;
  temperature?: number;
  label?: string;
  /** Per-call model override. Falls back to config.llm.model. */
  model?: string;
  /** OpenAI-style tool definitions. Triggers the tool loop when present. */
  tools?: ToolDef[];
  /** Map of tool.name → async handler. Required if `tools` is set. */
  toolHandlers?: Record<string, ToolHandler>;
  /** Hard cap on tool-loop iterations. Default 10. */
  maxToolSteps?: number;
}

export function modelForCheckpoint(checkpointId: string): string | undefined {
  return getConfig().llm.models?.[checkpointId];
}

interface ChatMessage {
  role: "system" | "user" | "assistant" | "tool";
  content: string | null;
  tool_calls?: ToolCall[];
  tool_call_id?: string;
}

interface ToolCall {
  id: string;
  type: "function";
  function: { name: string; arguments: string };
}

interface OpenAiResponse {
  choices?: Array<{
    message?: { content?: string | null; tool_calls?: ToolCall[] };
    text?: string;
  }>;
  error?: { message?: string; type?: string };
}

interface AnthropicResponse {
  content?: Array<{ type: string; text?: string }>;
  error?: { message?: string };
}

function dbg(...args: unknown[]) {
  // eslint-disable-next-line no-console
  console.error("\x1b[90m[llm]", ...args, "\x1b[0m");
}

function buildHeaders(cfg: LlmConfig): Record<string, string> {
  const headers: Record<string, string> = {
    "Content-Type": "application/json",
    Accept: "application/json",
    ...(cfg.extraHeaders ?? {}),
  };
  const authHeader = cfg.authHeader ?? "Authorization";
  const authScheme = cfg.authScheme ?? "Bearer";
  headers[authHeader] = authScheme
    ? `${authScheme} ${cfg.apiKey}`
    : cfg.apiKey;
  if (cfg.protocol === "anthropic" && !cfg.authHeader) {
    delete headers["Authorization"];
    headers["x-api-key"] = cfg.apiKey;
    headers["anthropic-version"] = "2023-06-01";
  }
  return headers;
}

function joinUrl(base: string, p: string): string {
  const b = base.replace(/\/+$/, "");
  const s = p.startsWith("/") ? p : `/${p}`;
  if (b.endsWith("/chat/completions") || b.endsWith("/messages")) return b;
  return b + s;
}

/**
 * One HTTP round-trip. Returns the assistant's content and any tool_calls
 * the model asked for (null if none). Caller decides whether to loop.
 */
async function executeOnce(
  cfg: LlmConfig,
  opts: LlmCallOptions,
  messages: ChatMessage[],
  headers: Record<string, string>,
  signal: AbortSignal
): Promise<{ content: string; toolCalls: ToolCall[] | null }> {
  let url: string;
  let body: unknown;

  if (cfg.protocol === "anthropic") {
    if (opts.tools && opts.tools.length > 0) {
      throw new Error(
        "tool-calling is currently only implemented for protocol: openai"
      );
    }
    // Anthropic messages: system is a top-level field; only non-system messages go in `messages`.
    const systemMsg =
      messages.find((m) => m.role === "system")?.content ?? opts.system;
    url = joinUrl(cfg.baseUrl, "/v1/messages");
    body = {
      model: opts.model ?? cfg.model,
      max_tokens: opts.maxTokens ?? cfg.maxTokens,
      temperature: opts.temperature ?? cfg.temperature,
      system: systemMsg,
      messages: messages
        .filter((m) => m.role !== "system")
        .map((m) => ({ role: m.role, content: m.content ?? "" })),
    };
  } else {
    url = joinUrl(cfg.baseUrl, "/v1/chat/completions");
    const openaiBody: Record<string, unknown> = {
      model: opts.model ?? cfg.model,
      max_tokens: opts.maxTokens ?? cfg.maxTokens,
      temperature: opts.temperature ?? cfg.temperature,
      messages,
    };
    if (opts.tools && opts.tools.length > 0) {
      openaiBody.tools = opts.tools;
      openaiBody.tool_choice = "auto";
    }
    body = openaiBody;
  }

  dbg(`POST ${url}  model=${opts.model ?? cfg.model}  protocol=${cfg.protocol}`);

  let res: Response;
  try {
    res = await fetch(url, {
      method: "POST",
      headers,
      body: JSON.stringify(body),
      signal,
    });
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    dbg(`NETWORK ERROR: ${msg}`);
    throw new Error(`LLM network error hitting ${url}: ${msg}`);
  }

  const rawText = await res.text();

  if (!res.ok) {
    dbg(`HTTP ${res.status}  body: ${rawText.slice(0, 500)}`);
    throw new Error(
      `LLM HTTP ${res.status}: ${rawText.slice(0, 400) || "(empty body)"}`
    );
  }

  let data: (OpenAiResponse & AnthropicResponse) | null = null;
  try {
    data = JSON.parse(rawText) as OpenAiResponse & AnthropicResponse;
  } catch {
    dbg(`Non-JSON response body (first 500 chars): ${rawText.slice(0, 500)}`);
    throw new Error(
      `LLM returned non-JSON response: ${rawText.slice(0, 300)}`
    );
  }

  if (data.error) {
    dbg(`API error field: ${JSON.stringify(data.error)}`);
    throw new Error(`LLM API error: ${data.error.message ?? "unknown"}`);
  }

  if (cfg.protocol === "anthropic") {
    const first = data.content?.[0];
    const out = first?.type === "text" ? first.text ?? "" : "";
    if (!out) dbg(`Empty anthropic content. Full: ${rawText.slice(0, 400)}`);
    return { content: out, toolCalls: null };
  }

  const choice = data.choices?.[0];
  const msg = choice?.message;
  const content =
    (typeof msg?.content === "string" ? msg.content : undefined) ??
    choice?.text ??
    "";
  const toolCalls =
    Array.isArray(msg?.tool_calls) && msg!.tool_calls!.length > 0
      ? msg!.tool_calls!
      : null;
  if (!content && !toolCalls) {
    dbg(
      `Empty OpenAI response. choices=${JSON.stringify(data.choices)?.slice(0, 400)}`
    );
  }
  return { content, toolCalls };
}

async function runToolLoop(
  cfg: LlmConfig,
  opts: LlmCallOptions,
  headers: Record<string, string>,
  signal: AbortSignal,
  label: string,
  messages: ChatMessage[]
): Promise<string> {
  const maxSteps = opts.maxToolSteps ?? 10;
  for (let step = 0; step < maxSteps; step++) {
    const { content, toolCalls } = await executeOnce(
      cfg,
      opts,
      messages,
      headers,
      signal
    );

    if (!toolCalls) {
      // eslint-disable-next-line no-console
      console.log(
        `\x1b[36m[llm] · ${label} · loop ended at step ${step + 1}/${maxSteps}\x1b[0m`
      );
      return content;
    }

    // eslint-disable-next-line no-console
    console.log(
      `\x1b[36m[llm] · ${label} · step ${step + 1}/${maxSteps}: ${toolCalls.length} tool call(s)\x1b[0m`
    );

    // Keep the assistant message (with tool_calls) in history, as OpenAI
    // requires the tool_call_id to match a preceding assistant message.
    messages.push({
      role: "assistant",
      content: content || null,
      tool_calls: toolCalls,
    });

    for (const tc of toolCalls) {
      const name = tc.function?.name ?? "";
      const argsStr = tc.function?.arguments ?? "{}";
      let args: Record<string, unknown> = {};
      try {
        args = JSON.parse(argsStr) as Record<string, unknown>;
      } catch {
        /* malformed args — pass empty and let the handler error */
      }

      const handler = opts.toolHandlers?.[name];
      const toolStart = Date.now();
      let result: string;
      if (!handler) {
        result = `ERROR: unknown tool "${name}"`;
      } else {
        try {
          result = await handler(args);
        } catch (err) {
          result = `ERROR: ${err instanceof Error ? err.message : String(err)}`;
        }
      }
      const toolElapsed = Date.now() - toolStart;

      // Cap tool output so one runaway read can't blow the context window.
      const MAX_TOOL_OUTPUT = 50_000;
      const truncated = result.length > MAX_TOOL_OUTPUT;
      if (truncated) {
        result =
          result.slice(0, MAX_TOOL_OUTPUT) +
          `\n\n... [truncated from ${result.length} to ${MAX_TOOL_OUTPUT} chars]`;
      }

      const argPreview = Object.keys(args).slice(0, 3).join(",");
      // eslint-disable-next-line no-console
      console.log(
        `\x1b[36m[llm] ·   ${name}(${argPreview}) → ${result.length}ch${truncated ? " (truncated)" : ""} · ${toolElapsed}ms\x1b[0m`
      );

      messages.push({
        role: "tool",
        tool_call_id: tc.id,
        content: result,
      });
    }
  }
  throw new Error(
    `LLM tool loop exceeded ${maxSteps} steps without terminating (label: ${label})`
  );
}

export async function callLlm(opts: LlmCallOptions): Promise<string> {
  const cfg = getConfig().llm;
  if (!cfg.baseUrl) {
    throw new Error(
      "llm.baseUrl is not set. Edit config.yml and fill in llm.baseUrl and llm.apiKey."
    );
  }
  if (!cfg.apiKey) {
    throw new Error(
      "llm.apiKey is not set. Edit config.yml (or export TENXGRACE_LLM_API_KEY)."
    );
  }
  if (opts.tools && opts.tools.length > 0 && !opts.toolHandlers) {
    throw new Error(
      "callLlm: `tools` was provided but `toolHandlers` is missing — nothing can execute the tool calls."
    );
  }

  const label = opts.label ?? "unlabelled";
  const effectiveModel = opts.model ?? cfg.model;
  const started = Date.now();
  const systemChars = opts.system.length;
  const userChars = opts.user.length;
  const approxTokens = Math.round((systemChars + userChars) / 4);
  const toolsInfo =
    opts.tools && opts.tools.length > 0
      ? ` · tools=${opts.tools.map((t) => t.function.name).join(",")}`
      : "";
  // eslint-disable-next-line no-console
  console.log(
    `\x1b[36m[llm] → ${effectiveModel} (${cfg.protocol}) · ${label} · sys=${systemChars}ch user=${userChars}ch (~${approxTokens} tok)${toolsInfo}\x1b[0m`
  );

  const heartbeat = setInterval(() => {
    const elapsed = Math.round((Date.now() - started) / 1000);
    // eslint-disable-next-line no-console
    console.log(
      `\x1b[90m[llm] … still waiting · ${label} · ${elapsed}s elapsed (timeout ${Math.round(cfg.timeoutMs / 1000)}s)\x1b[0m`
    );
  }, 15_000);

  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), cfg.timeoutMs);

  try {
    const headers = buildHeaders(cfg);
    const messages: ChatMessage[] = [
      { role: "system", content: opts.system },
      { role: "user", content: opts.user },
    ];

    // Tool-enabled path: run the loop until the model stops calling tools.
    if (opts.tools && opts.tools.length > 0) {
      return await runToolLoop(
        cfg,
        opts,
        headers,
        controller.signal,
        label,
        messages
      );
    }

    // Single-shot path: unchanged semantics.
    const { content } = await executeOnce(
      cfg,
      opts,
      messages,
      headers,
      controller.signal
    );
    if (!content) dbg("Empty content from single-shot call");
    return content;
  } finally {
    clearTimeout(timer);
    clearInterval(heartbeat);
    const elapsed = Date.now() - started;
    // eslint-disable-next-line no-console
    console.log(
      `\x1b[36m[llm] ← ${effectiveModel} · ${label} · ${elapsed}ms\x1b[0m`
    );
  }
}
