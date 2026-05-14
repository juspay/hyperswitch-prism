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
 * Pull the first ```bash fenced block out of a Claude reply. Used when
 * test commands are generated rather than parsed from the PR body.
 */
export function extractCommandsFromClaudeReply(reply: string): string[] {
  const out: string[] = [];
  const re = /```(?:bash|sh|shell)?\s*\n([\s\S]+?)\n```/g;
  let match: RegExpExecArray | null;
  while ((match = re.exec(reply)) !== null) {
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
  return out;
}
