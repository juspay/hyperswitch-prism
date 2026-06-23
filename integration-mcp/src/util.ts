/** Small shared helpers for building tool results and handling secrets. */

export interface ToolResult {
  [key: string]: unknown;
  content: { type: "text"; text: string }[];
  structuredContent: Record<string, unknown>;
  isError?: boolean;
}

/** Build a standard tool result with a human-readable text block + structured payload. */
export function result(text: string, structured: Record<string, unknown>): ToolResult {
  return { content: [{ type: "text", text }], structuredContent: structured };
}

/** Build an error tool result (isError true) so the agent sees it as a failure. */
export function errorResult(text: string, structured: Record<string, unknown> = {}): ToolResult {
  return {
    content: [{ type: "text", text }],
    structuredContent: { ok: false, ...structured },
    isError: true,
  };
}

export function unknownToMessage(err: unknown): string {
  if (err instanceof Error) return err.message;
  if (typeof err === "string") return err;
  try {
    return JSON.stringify(err);
  } catch {
    return String(err);
  }
}

/** Replace any of the given secret substrings in text with ***. */
export function scrubSecrets(text: string, secrets: string[]): string {
  let out = text;
  for (const s of secrets) {
    if (s && s.length >= 3) out = out.split(s).join("***");
  }
  return out;
}

/** Convert a value to a redacted display: secrets become ***. */
export function redactValue(value: unknown): string {
  if (value == null) return "";
  const s = String(value);
  if (s.length <= 4) return "***";
  return `${s.slice(0, 2)}***${s.slice(-2)}`;
}
