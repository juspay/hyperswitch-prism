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
const CODE_BLOCK_RE = /```(?:bash|sh|shell)?\s*\n([\s\S]+?)\n```/g;

const MAX_COMMANDS = 10;

export function extractTestCommandsFromBody(body: string | undefined): string[] {
  if (!body) return [];
  const out: string[] = [];

  // Split by H2 headers. `split` keeps the part *after* the marker as the
  // section body, with the header text as the first line of the chunk.
  const sections = body.split(/^##\s+/m);
  for (const section of sections) {
    const firstLine = section.split("\n", 1)[0]?.trim() ?? "";
    if (!SECTION_HEADER_RE.test(firstLine)) continue;

    // Reset regex state — global flag preserves lastIndex across calls.
    CODE_BLOCK_RE.lastIndex = 0;
    let match: RegExpExecArray | null;
    while ((match = CODE_BLOCK_RE.exec(section)) !== null) {
      const block = match[1] ?? "";
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
  const re = /```(?:bash|sh|shell)?\s*\n([\s\S]+?)\n```/g;
  let match: RegExpExecArray | null;
  while ((match = re.exec(reply)) !== null) {
    const block = match[1] ?? "";
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
