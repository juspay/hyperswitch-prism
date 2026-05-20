import { describe, expect, it } from "vitest";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import {
  buildGrpcurlCommand,
  extractJsonPath,
  loadConnectorCreds,
  orderSteps,
  parseTestPlan,
  renderTemplates,
  runTestPlan,
  type GrpcCommandRunner,
  type TestPlan,
} from "../test-plan.js";
import type { GrpcCommandResult } from "../grpc-runner.js";

// ─── parseTestPlan ───────────────────────────────────────────────────

describe("parseTestPlan", () => {
  it("accepts a raw JSON object", () => {
    const reply = JSON.stringify({
      tests: [{ name: "a", method: "x.S/M" }],
    });
    const r = parseTestPlan(reply);
    expect(r.ok).toBe(true);
    expect(r.plan?.tests).toHaveLength(1);
    expect(r.plan?.tests[0]?.name).toBe("a");
  });

  it("accepts a ```json fenced block with surrounding noise", () => {
    const reply = `
Here is your plan:

\`\`\`json
{ "tests": [ { "name": "auth", "method": "x.S/Authorize" } ] }
\`\`\`

That should cover it.`;
    const r = parseTestPlan(reply);
    expect(r.ok).toBe(true);
    expect(r.plan?.tests[0]?.name).toBe("auth");
  });

  it("rejects malformed JSON with a clear error", () => {
    const r = parseTestPlan("{ not json }");
    expect(r.ok).toBe(false);
    expect(r.error).toMatch(/not valid JSON/i);
  });

  it("rejects empty input", () => {
    expect(parseTestPlan("").ok).toBe(false);
    expect(parseTestPlan("   ").ok).toBe(false);
  });

  it("rejects a non-object root", () => {
    expect(parseTestPlan("[]").ok).toBe(false);
    expect(parseTestPlan('"hello"').ok).toBe(false);
    expect(parseTestPlan("42").ok).toBe(false);
  });

  it("rejects missing tests array", () => {
    const r = parseTestPlan(JSON.stringify({ foo: "bar" }));
    expect(r.ok).toBe(false);
    expect(r.error).toMatch(/tests must be an array/i);
  });

  it("rejects empty tests array", () => {
    const r = parseTestPlan(JSON.stringify({ tests: [] }));
    expect(r.ok).toBe(false);
    expect(r.error).toMatch(/empty/i);
  });

  it("rejects more than the step cap", () => {
    const tests = Array.from({ length: 11 }, (_, i) => ({
      name: `s${i}`,
      method: "x.S/M",
    }));
    const r = parseTestPlan(JSON.stringify({ tests }));
    expect(r.ok).toBe(false);
    expect(r.error).toMatch(/cap is 10/);
  });

  it("rejects duplicate step names", () => {
    const r = parseTestPlan(
      JSON.stringify({
        tests: [
          { name: "dup", method: "x.S/A" },
          { name: "dup", method: "x.S/B" },
        ],
      })
    );
    expect(r.ok).toBe(false);
    expect(r.error).toMatch(/Duplicate step name/);
  });

  it("rejects steps missing name or method", () => {
    expect(
      parseTestPlan(
        JSON.stringify({ tests: [{ method: "x.S/M" }] })
      ).ok
    ).toBe(false);
    expect(
      parseTestPlan(
        JSON.stringify({ tests: [{ name: "a" }] })
      ).ok
    ).toBe(false);
  });

  it("picks the largest fenced block when multiple are present", () => {
    const reply = `
\`\`\`json
{ "bogus": true }
\`\`\`

\`\`\`json
{ "tests": [ { "name": "real", "method": "x.S/M" } ] }
\`\`\`
`;
    const r = parseTestPlan(reply);
    expect(r.ok).toBe(true);
    expect(r.plan?.tests[0]?.name).toBe("real");
  });
});

// ─── orderSteps ──────────────────────────────────────────────────────

describe("orderSteps", () => {
  const step = (name: string, depends_on?: string) => ({
    name,
    method: "x.S/M",
    depends_on,
  });

  it("orders a linear chain", () => {
    const plan: TestPlan = {
      tests: [step("c", "b"), step("a"), step("b", "a")],
    };
    const r = orderSteps(plan);
    expect(r.ok).toBe(true);
    expect(r.order?.map((s) => s.name)).toEqual(["a", "b", "c"]);
  });

  it("orders independent steps in declaration order", () => {
    const plan: TestPlan = { tests: [step("a"), step("b"), step("c")] };
    const r = orderSteps(plan);
    expect(r.order?.map((s) => s.name)).toEqual(["a", "b", "c"]);
  });

  it("rejects a missing dependency", () => {
    const plan: TestPlan = { tests: [step("a", "ghost")] };
    const r = orderSteps(plan);
    expect(r.ok).toBe(false);
    expect(r.error).toMatch(/depends_on 'ghost' which doesn't exist/);
  });

  it("rejects a 2-node cycle", () => {
    const plan: TestPlan = {
      tests: [step("a", "b"), step("b", "a")],
    };
    const r = orderSteps(plan);
    expect(r.ok).toBe(false);
    expect(r.error).toMatch(/Cycle detected/);
  });

  it("rejects a 3-node cycle", () => {
    const plan: TestPlan = {
      tests: [step("a", "c"), step("b", "a"), step("c", "b")],
    };
    const r = orderSteps(plan);
    expect(r.ok).toBe(false);
    expect(r.error).toMatch(/Cycle detected/);
  });

  it("orders a diamond (a → b, a → c)", () => {
    const plan: TestPlan = {
      tests: [step("b", "a"), step("c", "a"), step("a")],
    };
    const r = orderSteps(plan);
    expect(r.ok).toBe(true);
    expect(r.order?.[0]?.name).toBe("a");
    expect(r.order?.slice(1).map((s) => s.name).sort()).toEqual(["b", "c"]);
  });
});

// ─── extractJsonPath ─────────────────────────────────────────────────

describe("extractJsonPath", () => {
  const obj = {
    response: {
      transaction_id: "abc123",
      nested: { deep: "yes" },
      list: [{ tag: "first" }, { tag: "second" }],
      status: "Authorized",
    },
  };

  it("returns the root for $", () => {
    expect(extractJsonPath(obj, "$")).toBe(obj);
  });

  it("walks $.field.nested", () => {
    expect(extractJsonPath(obj, "$.response.transaction_id")).toBe("abc123");
    expect(extractJsonPath(obj, "$.response.nested.deep")).toBe("yes");
  });

  it("indexes arrays with $.field[N]", () => {
    expect(extractJsonPath(obj, "$.response.list[1].tag")).toBe("second");
  });

  it("returns undefined for missing paths", () => {
    expect(extractJsonPath(obj, "$.response.missing")).toBeUndefined();
    expect(extractJsonPath(obj, "$.bogus.path")).toBeUndefined();
  });

  it("returns undefined when crossing a non-object", () => {
    expect(
      extractJsonPath(obj, "$.response.status.cannot_descend")
    ).toBeUndefined();
  });

  it("tolerates a missing leading $", () => {
    expect(extractJsonPath(obj, "response.transaction_id")).toBe("abc123");
  });
});

// ─── renderTemplates ─────────────────────────────────────────────────

describe("renderTemplates", () => {
  it("substitutes string leaves only", () => {
    const view = { step1: { id: "xyz" } };
    expect(renderTemplates("plain string", view)).toBe("plain string");
    expect(renderTemplates("ref={{ step1.id }}", view)).toBe("ref=xyz");
  });

  it("recurses into nested objects", () => {
    const view = { auth: { tx: "T-1" } };
    const out = renderTemplates(
      {
        body: { tx_id: "{{ auth.tx }}", static: "no" },
        nested: { deeper: { ref: "[{{ auth.tx }}]" } },
      },
      view
    );
    expect(out).toEqual({
      body: { tx_id: "T-1", static: "no" },
      nested: { deeper: { ref: "[T-1]" } },
    });
  });

  it("recurses into arrays", () => {
    const view = { a: { x: "1", y: "2" } };
    expect(renderTemplates(["{{ a.x }}", "{{ a.y }}", "static"], view)).toEqual(
      ["1", "2", "static"]
    );
  });

  it("leaves non-strings (numbers, booleans, null) untouched", () => {
    const view = { a: { id: "X" } };
    expect(renderTemplates(42, view)).toBe(42);
    expect(renderTemplates(true, view)).toBe(true);
    expect(renderTemplates(null, view)).toBe(null);
  });

  it("renders empty string when a template key is missing (Mustache default)", () => {
    const view = { existing: { x: "1" } };
    expect(renderTemplates("{{ missing.y }}", view)).toBe("");
  });
});

// ─── buildGrpcurlCommand ─────────────────────────────────────────────

describe("buildGrpcurlCommand", () => {
  it("produces a command with headers and a JSON body", () => {
    const cmd = buildGrpcurlCommand({
      port: 8000,
      method: "types.PaymentService/Authorize",
      headers: { "x-connector": "stripe", "x-api-key": "secret" },
      body: { amount: 1000, currency: "USD" },
    });
    // grpcurl -plaintext -H 'x-connector: stripe' -H 'x-api-key: secret' \
    //   -d '{"amount":1000,"currency":"USD"}' localhost:8000 types.PaymentService/Authorize
    expect(cmd).toContain("grpcurl -plaintext");
    expect(cmd).toContain(`-H 'x-connector: stripe'`);
    expect(cmd).toContain(`-H 'x-api-key: secret'`);
    expect(cmd).toContain(`-d '{"amount":1000,"currency":"USD"}'`);
    expect(cmd).toContain("localhost:8000");
    expect(cmd).toContain("types.PaymentService/Authorize");
  });

  it("omits the -d flag when body is undefined", () => {
    const cmd = buildGrpcurlCommand({
      port: 8000,
      method: "types.X/List",
      headers: {},
      body: undefined,
    });
    expect(cmd).not.toContain("-d");
  });

  it("escapes single quotes in JSON bodies via the bash-safe trick", () => {
    const cmd = buildGrpcurlCommand({
      port: 8000,
      method: "x.S/M",
      headers: {},
      body: { note: "user's request" },
    });
    // The standard bash single-quote escape sequence '\'' breaks out, escapes,
    // and reopens — so the embedded "'" survives intact.
    expect(cmd).toContain(`'\\''`);
  });

  it("accepts a pre-stringified body", () => {
    const cmd = buildGrpcurlCommand({
      port: 8000,
      method: "x.S/M",
      headers: {},
      body: '{"raw":"json"}',
    });
    expect(cmd).toContain(`-d '{"raw":"json"}'`);
  });
});

// ─── loadConnectorCreds ──────────────────────────────────────────────

describe("loadConnectorCreds", () => {
  it("returns an error when the file is missing", () => {
    const r = loadConnectorCreds("/no/such/path.json", "adyen");
    expect(r.creds).toBeNull();
    expect(r.error).toMatch(/not found/);
  });

  it("returns an error on malformed JSON", () => {
    const tmp = path.join(
      fs.mkdtempSync(path.join(os.tmpdir(), "pr-resolver-creds-")),
      "creds.json"
    );
    fs.writeFileSync(tmp, "{ not json", "utf-8");
    const r = loadConnectorCreds(tmp, "adyen");
    expect(r.creds).toBeNull();
    expect(r.error).toMatch(/Failed to parse/);
    fs.unlinkSync(tmp);
  });

  it("returns an error when the connector key is missing", () => {
    const tmp = path.join(
      fs.mkdtempSync(path.join(os.tmpdir(), "pr-resolver-creds-")),
      "creds.json"
    );
    fs.writeFileSync(
      tmp,
      JSON.stringify({ stripe: { connector_account_details: { api_key: "x" } } }),
      "utf-8"
    );
    const r = loadConnectorCreds(tmp, "adyen");
    expect(r.creds).toBeNull();
    expect(r.error).toMatch(/no entry for connector 'adyen'/);
    fs.unlinkSync(tmp);
  });

  it("returns the connector block on a case-insensitive match", () => {
    const tmp = path.join(
      fs.mkdtempSync(path.join(os.tmpdir(), "pr-resolver-creds-")),
      "creds.json"
    );
    const block = {
      connector_account_details: {
        api_key: "real-key",
        key1: "M-12345",
        auth_type: "header-key",
      },
    };
    fs.writeFileSync(
      tmp,
      JSON.stringify({ Fiservemea: block }),
      "utf-8"
    );
    const r = loadConnectorCreds(tmp, "fiservemea");
    expect(r.creds).toEqual(block);
    expect(r.error).toBeUndefined();
    fs.unlinkSync(tmp);
  });
});

// ─── runTestPlan integration with stub runner ────────────────────────

/**
 * Build a stub runner that pretends to be `grpcurl` for a known set of
 * methods. Records each call so tests can assert on the rendered command
 * (which is what proves substitution + capture flow worked).
 */
function makeStubRunner(
  fixtures: Record<string, GrpcCommandResult | ((command: string) => GrpcCommandResult)>
): {
  runner: GrpcCommandRunner;
  calls: Array<{ command: string; cwd: string; timeoutMs: number }>;
} {
  const calls: Array<{ command: string; cwd: string; timeoutMs: number }> = [];
  const runner: GrpcCommandRunner = async (command, cwd, timeoutMs) => {
    calls.push({ command, cwd, timeoutMs });
    // Match by method (last whitespace-separated token in the command).
    const method = command.trim().split(/\s+/).pop() ?? "";
    const fixture = fixtures[method] ?? {
      command,
      ok: false,
      exitCode: 1,
      stdout: "",
      stderr: `no fixture for method '${method}'`,
      durationMs: 1,
      timedOut: false,
    };
    return typeof fixture === "function" ? fixture(command) : fixture;
  };
  return { runner, calls };
}

const okResult = (stdout: string): GrpcCommandResult => ({
  command: "",
  ok: true,
  exitCode: 0,
  stdout,
  stderr: "",
  durationMs: 12,
  timedOut: false,
});

describe("runTestPlan", () => {
  it("flows captures from one step into the next via Mustache templates", async () => {
    const plan: TestPlan = {
      tests: [
        {
          name: "authorize",
          method: "types.PaymentService/Authorize",
          headers: { "x-connector": "stripe" },
          body: { amount: 1000 },
          captures: { connector_transaction_id: "$.response.connector_transaction_id" },
        },
        {
          name: "capture",
          method: "types.PaymentService/Capture",
          depends_on: "authorize",
          headers: { "x-connector": "stripe" },
          body: {
            connector_transaction_id:
              "{{ authorize.connector_transaction_id }}",
          },
        },
      ],
    };
    const { runner, calls } = makeStubRunner({
      "types.PaymentService/Authorize": okResult(
        JSON.stringify({ response: { connector_transaction_id: "tx_abc_123" } })
      ),
      "types.PaymentService/Capture": okResult(JSON.stringify({ response: {} })),
    });

    const { ok, results } = await runTestPlan({
      plan,
      worktreePath: "/tmp",
      port: 8000,
      timeoutMs: 30_000,
      runner,
    });

    expect(ok).toBe(true);
    expect(results).toHaveLength(2);
    expect(results[0]?.ok).toBe(true);
    expect(results[0]?.captures.connector_transaction_id).toBe("tx_abc_123");
    // The Capture command should contain the actual captured id, not the template.
    expect(calls[1]?.command).toContain(`"connector_transaction_id":"tx_abc_123"`);
    expect(calls[1]?.command).not.toContain("{{");
  });

  it("skips dependents when a dependency fails", async () => {
    const plan: TestPlan = {
      tests: [
        { name: "a", method: "x.S/A" },
        { name: "b", method: "x.S/B", depends_on: "a" },
        { name: "c", method: "x.S/C", depends_on: "b" },
      ],
    };
    const { runner } = makeStubRunner({
      "x.S/A": { ...okResult("{}"), ok: false, exitCode: 7 },
    });
    const { ok, results } = await runTestPlan({
      plan,
      worktreePath: "/tmp",
      port: 8000,
      timeoutMs: 30_000,
      runner,
    });
    expect(ok).toBe(false);
    expect(results[0]?.ok).toBe(false);
    expect(results[0]?.skipped).toBe(false);
    expect(results[1]?.skipped).toBe(true);
    expect(results[1]?.skipReason).toMatch(/'a' failed/);
    expect(results[2]?.skipped).toBe(true);
    expect(results[2]?.skipReason).toMatch(/'b' failed/);
  });

  it("runs independent steps even when a sibling failed", async () => {
    const plan: TestPlan = {
      tests: [
        { name: "a", method: "x.S/A" },
        { name: "b", method: "x.S/B" }, // no dep on a
      ],
    };
    const { runner } = makeStubRunner({
      "x.S/A": { ...okResult("{}"), ok: false, exitCode: 7 },
      "x.S/B": okResult("{}"),
    });
    const { ok, results } = await runTestPlan({
      plan,
      worktreePath: "/tmp",
      port: 8000,
      timeoutMs: 30_000,
      runner,
    });
    expect(ok).toBe(false);
    expect(results[0]?.ok).toBe(false);
    expect(results[1]?.ok).toBe(true);
    expect(results[1]?.skipped).toBe(false);
  });

  it("enforces status_in expectation", async () => {
    const plan: TestPlan = {
      tests: [
        {
          name: "a",
          method: "x.S/A",
          captures: { status: "$.response.status" },
          expect: { status_in: ["Authorized", "Pending"] },
        },
      ],
    };
    const { runner } = makeStubRunner({
      "x.S/A": okResult(JSON.stringify({ response: { status: "Failed" } })),
    });
    const { ok, results } = await runTestPlan({
      plan,
      worktreePath: "/tmp",
      port: 8000,
      timeoutMs: 30_000,
      runner,
    });
    expect(ok).toBe(false);
    expect(results[0]?.ok).toBe(false);
    expect(results[0]?.expectMisses[0]).toMatch(/status_in.*expected/);
  });

  it("enforces response_contains expectation", async () => {
    const plan: TestPlan = {
      tests: [
        {
          name: "a",
          method: "x.S/A",
          expect: { response_contains: "magic_token" },
        },
      ],
    };
    const { runner } = makeStubRunner({
      "x.S/A": okResult(`{"response":{"status":"Authorized"}}`),
    });
    const { ok, results } = await runTestPlan({
      plan,
      worktreePath: "/tmp",
      port: 8000,
      timeoutMs: 30_000,
      runner,
    });
    expect(ok).toBe(false);
    expect(results[0]?.expectMisses[0]).toMatch(/response_contains/);
  });

  it("treats a non-zero exit as failure even when stdout looks fine", async () => {
    const plan: TestPlan = {
      tests: [{ name: "a", method: "x.S/A" }],
    };
    const { runner } = makeStubRunner({
      "x.S/A": {
        command: "",
        ok: false,
        exitCode: 14,
        stdout: `{"response":{"status":"Authorized"}}`,
        stderr: "Code: Unavailable\n",
        durationMs: 1,
        timedOut: false,
      },
    });
    const { ok, results } = await runTestPlan({
      plan,
      worktreePath: "/tmp",
      port: 8000,
      timeoutMs: 30_000,
      runner,
    });
    expect(ok).toBe(false);
    expect(results[0]?.expectMisses[0]).toMatch(/exit_code/);
  });

  it("emits start + done events with the right phases", async () => {
    const plan: TestPlan = {
      tests: [
        { name: "a", method: "x.S/A" },
        { name: "b", method: "x.S/B", depends_on: "a" },
      ],
    };
    const { runner } = makeStubRunner({
      "x.S/A": okResult("{}"),
      "x.S/B": okResult("{}"),
    });
    const events: Array<
      | { phase: "start"; name: string }
      | { phase: "done"; name: string; ok: boolean }
    > = [];
    await runTestPlan({
      plan,
      worktreePath: "/tmp",
      port: 8000,
      timeoutMs: 30_000,
      runner,
      onStep: (e) => {
        if (e.phase === "start") {
          events.push({ phase: "start", name: e.name });
        } else {
          events.push({ phase: "done", name: e.name, ok: e.result.ok });
        }
      },
    });
    expect(events.map((e) => `${e.phase}:${e.name}`)).toEqual([
      "start:a",
      "done:a",
      "start:b",
      "done:b",
    ]);
  });

  it("rendered command for step 2 substitutes step 1's capture verbatim", async () => {
    // Reverse takes a connector_transaction_id from capture; verify it lands.
    const plan: TestPlan = {
      tests: [
        {
          name: "authorize",
          method: "types.PaymentService/Authorize",
          headers: {},
          body: { amount: 100 },
          captures: { tx: "$.response.connector_transaction_id" },
        },
        {
          name: "capture",
          method: "types.PaymentService/Capture",
          depends_on: "authorize",
          headers: { "x-tx": "{{ authorize.tx }}" },
          body: { tx: "{{ authorize.tx }}" },
          captures: { cap_id: "$.response.capture_id" },
        },
        {
          name: "reverse",
          method: "types.PaymentService/Reverse",
          depends_on: "capture",
          headers: {},
          body: { id: "{{ capture.cap_id }}" },
        },
      ],
    };
    const { runner, calls } = makeStubRunner({
      "types.PaymentService/Authorize": okResult(
        JSON.stringify({ response: { connector_transaction_id: "AUTHORIZE_TX" } })
      ),
      "types.PaymentService/Capture": okResult(
        JSON.stringify({ response: { capture_id: "CAPTURE_ID_42" } })
      ),
      "types.PaymentService/Reverse": okResult("{}"),
    });
    const { ok } = await runTestPlan({
      plan,
      worktreePath: "/tmp",
      port: 8000,
      timeoutMs: 30_000,
      runner,
    });
    expect(ok).toBe(true);
    expect(calls[1]?.command).toContain("AUTHORIZE_TX");
    expect(calls[1]?.command).toContain(`-H 'x-tx: AUTHORIZE_TX'`);
    expect(calls[2]?.command).toContain(`"id":"CAPTURE_ID_42"`);
  });
});
