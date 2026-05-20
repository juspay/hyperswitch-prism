import fs from "node:fs";
import path from "node:path";
import Mustache from "mustache";
import { runGrpcCommand, type GrpcCommandResult } from "./grpc-runner.js";

/**
 * Pluggable runner — same shape as `runGrpcCommand`. Tests inject a stub
 * that returns canned responses without spawning bash or hitting a
 * server. Production callers leave this unset and pick up the default.
 */
export type GrpcCommandRunner = (
  command: string,
  cwd: string,
  timeoutMs: number
) => Promise<GrpcCommandResult>;

/**
 * Phase B v2: structured gRPC test plan.
 *
 * Replaces the old "flat list of `grpcurl` bash lines" model. Claude now
 * returns a JSON plan keyed by step name with dependency edges, JSONPath
 * captures, and Mustache template substitution. The runner walks the plan
 * in dependency order, captures values from each response, substitutes
 * `{{ step.capture }}` references in later steps, and reports per-step
 * outcomes.
 */

const MAX_STEPS = 10;

// Disable HTML escaping on Mustache — substituted values often contain
// special chars (gRPC ids with hyphens, JSON-shaped fragments).
Mustache.escape = (s: string) => s;

export interface TestStep {
  name: string;
  method: string;
  depends_on?: string;
  headers?: Record<string, string>;
  body?: Record<string, unknown> | unknown;
  captures?: Record<string, string>;
  expect?: TestExpect;
}

export interface TestExpect {
  exit_code?: number;
  status_in?: string[];
  response_contains?: string;
}

export interface TestPlan {
  tests: TestStep[];
}

export interface TestStepResult {
  name: string;
  method: string;
  command: string;
  ok: boolean;
  skipped: boolean;
  skipReason?: string;
  exitCode: number | null;
  stdout: string;
  stderr: string;
  durationMs: number;
  timedOut: boolean;
  captures: Record<string, unknown>;
  expectMisses: string[];
  /** Rendered headers + body after Mustache substitution, for the UI. */
  renderedHeaders: Record<string, string>;
  renderedBody: unknown;
}

export interface ParsedPlan {
  ok: boolean;
  plan?: TestPlan;
  error?: string;
}

/**
 * Parse Claude's reply as a test plan. Accepts either a raw JSON object
 * or a single ```json fenced block. Anything else is a parse failure.
 */
export function parseTestPlan(reply: string): ParsedPlan {
  const trimmed = reply.trim();
  if (!trimmed) return { ok: false, error: "Empty Claude reply" };

  // Always scan for fenced blocks and use the largest one when present.
  // Falls through to the raw trimmed text if no fences exist (raw JSON
  // reply). This handles both single-block and multi-block replies with
  // the same code path.
  let jsonText = trimmed;
  const re = /```(?:json)?\s*\n([\s\S]+?)\n```/g;
  let best = "";
  let m: RegExpExecArray | null;
  while ((m = re.exec(trimmed)) !== null) {
    if ((m[1] ?? "").length > best.length) best = (m[1] ?? "").trim();
  }
  if (best) jsonText = best;

  let parsed: unknown;
  try {
    parsed = JSON.parse(jsonText);
  } catch (err) {
    return {
      ok: false,
      error: `Plan is not valid JSON: ${
        err instanceof Error ? err.message : String(err)
      }`,
    };
  }

  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
    return { ok: false, error: "Plan must be a JSON object with a `tests` array" };
  }
  const tests = (parsed as { tests?: unknown }).tests;
  if (!Array.isArray(tests)) {
    return { ok: false, error: "Plan.tests must be an array" };
  }
  if (tests.length === 0) {
    return { ok: false, error: "Plan.tests is empty" };
  }
  if (tests.length > MAX_STEPS) {
    return {
      ok: false,
      error: `Plan has ${tests.length} steps; cap is ${MAX_STEPS}`,
    };
  }
  const seen = new Set<string>();
  for (const raw of tests) {
    if (!raw || typeof raw !== "object") {
      return { ok: false, error: "Each step must be an object" };
    }
    const step = raw as TestStep;
    if (typeof step.name !== "string" || !step.name.trim()) {
      return { ok: false, error: "Step.name is required" };
    }
    if (seen.has(step.name)) {
      return { ok: false, error: `Duplicate step name '${step.name}'` };
    }
    seen.add(step.name);
    if (typeof step.method !== "string" || !step.method.trim()) {
      return {
        ok: false,
        error: `Step '${step.name}' is missing 'method'`,
      };
    }
  }
  return { ok: true, plan: { tests: tests as TestStep[] } };
}

/**
 * Read the connector's section out of creds.json. Returns `null` if the
 * file is missing or the connector key doesn't exist — caller decides
 * whether to fail the test step or proceed with empty creds.
 */
export function loadConnectorCreds(
  credsPath: string,
  connector: string
): { creds: Record<string, unknown> | null; error?: string } {
  if (!fs.existsSync(credsPath)) {
    return {
      creds: null,
      error: `creds.json not found at ${credsPath} — set prResolver-related credsPath or place the file there`,
    };
  }
  let parsed: Record<string, unknown>;
  try {
    parsed = JSON.parse(fs.readFileSync(credsPath, "utf-8")) as Record<
      string,
      unknown
    >;
  } catch (err) {
    return {
      creds: null,
      error: `Failed to parse ${credsPath}: ${
        err instanceof Error ? err.message : String(err)
      }`,
    };
  }
  // Case-insensitive lookup so "fiservemea" matches "Fiservemea".
  const want = connector.toLowerCase();
  for (const [key, value] of Object.entries(parsed)) {
    if (key.toLowerCase() === want && value && typeof value === "object") {
      return { creds: value as Record<string, unknown> };
    }
  }
  return {
    creds: null,
    error: `creds.json has no entry for connector '${connector}' (looked for key match, case-insensitive)`,
  };
}

/**
 * Topologically sort steps by `depends_on`. Detects cycles and missing
 * references — both result in `ok: false` with a clear error.
 */
export function orderSteps(plan: TestPlan): {
  ok: boolean;
  order?: TestStep[];
  error?: string;
} {
  const byName = new Map<string, TestStep>();
  for (const s of plan.tests) byName.set(s.name, s);

  // Missing reference check
  for (const s of plan.tests) {
    if (s.depends_on && !byName.has(s.depends_on)) {
      return {
        ok: false,
        error: `Step '${s.name}' depends_on '${s.depends_on}' which doesn't exist`,
      };
    }
  }

  const order: TestStep[] = [];
  const visited = new Map<string, "visiting" | "done">();
  function visit(name: string, stack: string[]): string | null {
    const state = visited.get(name);
    if (state === "done") return null;
    if (state === "visiting") {
      return `Cycle detected: ${[...stack, name].join(" → ")}`;
    }
    visited.set(name, "visiting");
    const step = byName.get(name)!;
    if (step.depends_on) {
      const err = visit(step.depends_on, [...stack, name]);
      if (err) return err;
    }
    visited.set(name, "done");
    order.push(step);
    return null;
  }
  for (const s of plan.tests) {
    const err = visit(s.name, []);
    if (err) return { ok: false, error: err };
  }
  return { ok: true, order };
}

/** Subset-JSONPath: $.field, $.field.nested, $.field[0].sub. */
export function extractJsonPath(obj: unknown, jsonpath: string): unknown {
  const trimmed = jsonpath.replace(/^\$\.?/, "");
  if (!trimmed) return obj;
  const parts = trimmed.split(/\.|\[(\d+)\]/).filter(Boolean);
  let cursor: unknown = obj;
  for (const part of parts) {
    if (cursor === null || cursor === undefined) return undefined;
    if (/^\d+$/.test(part)) {
      if (!Array.isArray(cursor)) return undefined;
      cursor = cursor[Number(part)];
    } else if (typeof cursor === "object") {
      cursor = (cursor as Record<string, unknown>)[part];
    } else {
      return undefined;
    }
  }
  return cursor;
}

/**
 * Apply Mustache substitution to all string leaves in a JSON-like value.
 * The view is `{ stepName: { captureKey: value } }` for prior steps.
 */
export function renderTemplates<T>(
  value: T,
  view: Record<string, Record<string, unknown>>
): T {
  if (typeof value === "string") {
    return Mustache.render(value, view) as unknown as T;
  }
  if (Array.isArray(value)) {
    return value.map((v) => renderTemplates(v, view)) as unknown as T;
  }
  if (value && typeof value === "object") {
    const out: Record<string, unknown> = {};
    for (const [k, v] of Object.entries(value as Record<string, unknown>)) {
      out[k] = renderTemplates(v, view);
    }
    return out as unknown as T;
  }
  return value;
}

/**
 * Build a `grpcurl -plaintext -H ... -d ... localhost:port method` command
 * string for execution via `bash -c`. Single-quotes the JSON body and
 * single-quotes header values; embedded single quotes are bash-escaped.
 */
export function buildGrpcurlCommand(input: {
  port: number;
  method: string;
  headers: Record<string, string>;
  body: unknown;
}): string {
  const parts: string[] = ["grpcurl", "-plaintext"];
  for (const [k, v] of Object.entries(input.headers)) {
    parts.push("-H", bashSingleQuote(`${k}: ${v}`));
  }
  if (input.body !== undefined && input.body !== null) {
    const bodyStr =
      typeof input.body === "string" ? input.body : JSON.stringify(input.body);
    parts.push("-d", bashSingleQuote(bodyStr));
  }
  parts.push(`localhost:${input.port}`);
  parts.push(input.method);
  return parts.join(" ");
}

function bashSingleQuote(s: string): string {
  // Wrap in single quotes; escape any embedded single quotes via the
  // standard `'\''` trick.
  return `'${s.replace(/'/g, `'\\''`)}'`;
}

export interface RunTestPlanInput {
  plan: TestPlan;
  worktreePath: string;
  port: number;
  timeoutMs: number;
  onStep?: (event:
    | { phase: "start"; name: string; depends_on?: string }
    | { phase: "done"; name: string; result: TestStepResult }
  ) => void;
  /**
   * Override the default `runGrpcCommand`. Tests pass a stub that returns
   * canned `GrpcCommandResult`s based on the input command, so the
   * runner's orchestration (substitution, captures, expectations,
   * dependency-failure cascade) can be exercised without bash or a real
   * server.
   */
  runner?: GrpcCommandRunner;
}

/**
 * Execute a plan. Returns per-step results in execution order.
 *
 * Failure semantics: if a step's dependency failed or was skipped, this
 * step is skipped with `skipReason`. Otherwise it runs even if a sibling
 * step failed (so the user sees the full picture, not just the first
 * failure).
 */
export async function runTestPlan(input: RunTestPlanInput): Promise<{
  ok: boolean;
  results: TestStepResult[];
}> {
  const ordered = orderSteps(input.plan);
  if (!ordered.ok || !ordered.order) {
    // Caller validated, but defend in depth.
    throw new Error(ordered.error ?? "orderSteps failed");
  }
  const results: TestStepResult[] = [];
  const captures: Record<string, Record<string, unknown>> = {};
  const okByName = new Map<string, boolean>();

  for (const step of ordered.order) {
    const event = (
      e:
        | { phase: "start"; name: string; depends_on?: string }
        | { phase: "done"; name: string; result: TestStepResult }
    ) => input.onStep?.(e);

    event({ phase: "start", name: step.name, depends_on: step.depends_on });

    // Dependency failed → skip
    if (step.depends_on && !okByName.get(step.depends_on)) {
      const placeholder: TestStepResult = {
        name: step.name,
        method: step.method,
        command: "",
        ok: false,
        skipped: true,
        skipReason: `Dependency '${step.depends_on}' failed or was skipped`,
        exitCode: null,
        stdout: "",
        stderr: "",
        durationMs: 0,
        timedOut: false,
        captures: {},
        expectMisses: [],
        renderedHeaders: {},
        renderedBody: undefined,
      };
      results.push(placeholder);
      okByName.set(step.name, false);
      event({ phase: "done", name: step.name, result: placeholder });
      continue;
    }

    // Render headers + body against prior captures
    const headers = renderTemplates(step.headers ?? {}, captures);
    const body = renderTemplates(step.body ?? {}, captures);
    const command = buildGrpcurlCommand({
      port: input.port,
      method: step.method,
      headers,
      body,
    });

    const cmdResult = await (input.runner ?? runGrpcCommand)(
      command,
      input.worktreePath,
      input.timeoutMs
    );

    // Parse stdout as JSON for capture extraction. If it isn't valid JSON,
    // captures fall through as undefined (we still record stdout/stderr).
    let parsedResponse: unknown;
    try {
      parsedResponse = JSON.parse(cmdResult.stdout);
    } catch {
      parsedResponse = undefined;
    }

    // JSONPath reads directly off the parsed gRPC response. Hyperswitch's
    // service typically wraps its data under a top-level `response` key, so
    // capture paths look like `$.response.connector_transaction_id` — we
    // don't add an extra wrapper here.
    const stepCaptures: Record<string, unknown> = {};
    const captureRoot: unknown = parsedResponse ?? {};
    for (const [k, jsonpath] of Object.entries(step.captures ?? {})) {
      stepCaptures[k] = extractJsonPath(captureRoot, jsonpath);
    }
    captures[step.name] = stepCaptures;

    // Apply expectations
    const expectMisses: string[] = [];
    const expectedExit = step.expect?.exit_code ?? 0;
    if (cmdResult.exitCode !== expectedExit) {
      expectMisses.push(
        `exit_code: expected ${expectedExit}, got ${cmdResult.exitCode}`
      );
    }
    if (step.expect?.status_in && step.expect.status_in.length > 0) {
      const got = stepCaptures.status;
      if (typeof got !== "string" || !step.expect.status_in.includes(got)) {
        expectMisses.push(
          `status_in: expected one of [${step.expect.status_in.join(", ")}], got ${JSON.stringify(got)}`
        );
      }
    }
    if (step.expect?.response_contains) {
      if (!cmdResult.stdout.includes(step.expect.response_contains)) {
        expectMisses.push(
          `response_contains: '${step.expect.response_contains}' not found in stdout`
        );
      }
    }

    const ok = cmdResult.ok && expectMisses.length === 0;
    const result: TestStepResult = {
      name: step.name,
      method: step.method,
      command,
      ok,
      skipped: false,
      exitCode: cmdResult.exitCode,
      stdout: cmdResult.stdout.slice(-4_000),
      stderr: cmdResult.stderr.slice(-4_000),
      durationMs: cmdResult.durationMs,
      timedOut: cmdResult.timedOut,
      captures: stepCaptures,
      expectMisses,
      renderedHeaders: headers,
      renderedBody: body,
    };
    results.push(result);
    okByName.set(step.name, ok);
    event({ phase: "done", name: step.name, result });
  }

  return {
    ok: results.every((r) => r.ok),
    results,
  };
}
