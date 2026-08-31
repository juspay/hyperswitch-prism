/** Small shared helpers for building tool results. */

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
