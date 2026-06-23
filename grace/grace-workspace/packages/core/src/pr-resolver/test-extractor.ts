/**
 * Extract grpcurl test commands from a PR body. The convention we look for:
 *   ## Testing | Tests | How to test | Test plan | Verification
 *   ...
 *   ```bash
 *   grpcurl -plaintext -d '{ … }' localhost:8000 ucs.PaymentService/Authorize
 *   ```
 *
 * If the PR body doesn't follow this convention, the caller falls back to
 * generating commands via Claude (see `grpc-test-gen.md`).
 */

const SECTION_HEADER_RE =
  /^(testing|tests?|how to test|test plan|verification)/i;
const FENCE_OPEN_RE = /^```([A-Za-z0-9_+-]*)\s*$/;
const FENCE_CLOSE_RE = /^```\s*$/;

const MAX_COMMANDS = 10;

// Linear-time line-based fenced-block parser — replaces the regex
// /```…?\n([\s\S]+?)\n```/g flagged by CodeQL as polynomial-redos. We
// only care about bash/sh/shell blocks (or unlabelled).
function* iterCodeBlocks(input: string, langs: ReadonlySet<string>): Iterable<string> {
  const lines = input.split("\n");
  let i = 0;
  while (i < lines.length) {
    const open = FENCE_OPEN_RE.exec(lines[i] ?? "");
    if (!open) { i++; continue; }
    const lang = (open[1] ?? "").toLowerCase();
    if (lang && !langs.has(lang)) { i++; continue; }
    const start = i + 1;
    let end = start;
    while (end < lines.length && !FENCE_CLOSE_RE.test(lines[end] ?? "")) end++;
    if (end >= lines.length) return; // unterminated fence — stop scanning
    yield lines.slice(start, end).join("\n");
    i = end + 1;
  }
}

const BASH_LANGS: ReadonlySet<string> = new Set(["", "bash", "sh", "shell"]);

export function extractTestCommandsFromBody(body: string | undefined): string[] {
  if (!body) return [];
  const out: string[] = [];

  // Split by H2 headers. `split` keeps the part *after* the marker as the
  // section body, with the header text as the first line of the chunk.
  const sections = body.split(/^##\s+/m);
  for (const section of sections) {
    const firstLine = section.split("\n", 1)[0]?.trim() ?? "";
    if (!SECTION_HEADER_RE.test(firstLine)) continue;

    for (const block of iterCodeBlocks(section, BASH_LANGS)) {
      for (const raw of block.split("\n")) {
        const line = raw.trim();
        if (!line || line.startsWith("#")) continue;
        if (isGrpcurl(line)) {
          out.push(line);
          if (out.length >= MAX_COMMANDS) return out;
        }
      }
    }
  }

  return out;
}

function isGrpcurl(line: string): boolean {
  // Accept `grpcurl`, `./grpcurl`, or any path ending in /grpcurl
  return /(^|\/)grpcurl(\s|$)/.test(line);
}

/**
 * Pull grpcurl invocations out of a Claude reply. Tries fenced ```bash
 * blocks first; if that yields nothing, scans the whole reply for lines
 * starting with `grpcurl` — so a Claude reply that forgot the fence
 * still produces usable commands.
 */
export function extractCommandsFromClaudeReply(reply: string): string[] {
  const fenced: string[] = [];
  for (const block of iterCodeBlocks(reply, BASH_LANGS)) {
    for (const raw of block.split("\n")) {
      const line = raw.trim();
      if (!line || line.startsWith("#")) continue;
      if (isGrpcurl(line)) {
        fenced.push(line);
        if (fenced.length >= MAX_COMMANDS) return fenced;
      }
    }
  }
  if (fenced.length > 0) return fenced;

  // Fenceless fallback: scan the whole reply.
  const fallback: string[] = [];
  for (const raw of reply.split("\n")) {
    const line = raw.trim();
    if (!line || line.startsWith("#")) continue;
    if (isGrpcurl(line)) {
      fallback.push(line);
      if (fallback.length >= MAX_COMMANDS) return fallback;
    }
  }
  return fallback;
}
