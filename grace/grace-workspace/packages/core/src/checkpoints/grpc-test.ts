import path from "node:path";
import { promises as fs } from "node:fs";
import type { Checkpoint } from "../types.js";
import { runAI } from "../tools/runner-factory.js";
import {
  killStaleProcesses,
  startGrpcServer,
  waitForBuildComplete,
  waitForHealthy,
  tailLogFile,
  type ServerHandle,
} from "./grpc-server-lifecycle.js";
import {
  deriveClaudeSessionId,
  friendlySessionName,
} from "./session-id.js";

/**
 * gRPC Test Checkpoint — Phase 6-7 of 2.3_codegen.md, but with the server
 * lifecycle owned by the orchestrator (TypeScript) instead of the agent.
 *
 * Flow:
 *   1. Pre-flight: kill any stale process holding ports 8000/8080.
 *   2. Start grpc-server, redirecting stdout+stderr to a per-run log file.
 *   3. Wait for TCP healthy on localhost:8000 (≤45s).
 *   4. Hand the agent a slim prompt: "server is up at localhost:8000, run
 *      grpcurl, validate, return JSON."
 *   5. Always kill the server on exit (finally), regardless of agent outcome.
 *
 * Why: the prior version asked the agent to run `cargo run --bin grpc-server &`
 * inline. Background `&` from a remote agent's bash tool is unreliable; the
 * server often vanished between tool calls and the agent looped on grpcurl
 * until the 480s runner timeout fired with empty stderr — the exact failure
 * mode this rewrite eliminates.
 */

const SLIM_PROMPT = `You are the gRPC Test Agent.

## CRITICAL RESTRICTION — TEST-ONLY MODE
The gRPC server is already running at SERVER_HOST. Do NOT start, kill, build,
or modify code. Do NOT run cargo. Do NOT touch source files. Your single
responsibility is: run grpcurl against the running server, validate the
response, return JSON.

## What to do

1. Read CREDS_PATH (a JSON file at the project root) to obtain the connector
   credentials. Pick the entry matching CONNECTOR.

2. Treat field_probe as a HINT, not authority. If HAS_FIELD_PROBE is true,
   read FIELD_PROBE_PATH. For the entry matching the current FLOW (or its
   payment-method name, e.g. \`Ach\` for BankDebit), IGNORE it if it has
   \`status: "not_supported"\` or \`status: "error"\` — those entries predate
   the current implementation and are stale. Otherwise the probe is a
   useful starting point but NOT authoritative; steps 3a and 3b override it
   on any conflict.

3. Resolve the actual request shape by reading these in order:

   a. **Proto file** at \`crates/types-traits/grpc-api-types/proto/payment_methods.proto\`.
      Find the \`PaymentMethod\` \`oneof payment_method\` block and pick the
      variant whose snake_case field name matches FLOW:
        - FLOW=BankDebit → direct-debit variants: \`ach\`, \`sepa\`, \`bacs\`,
          \`becs\`, \`sepa_guaranteed_debit\`, \`eft\`. Pick by region/spec
          (Cybersource ACH = \`ach\`, GoCardless EU = \`sepa\`, etc.).
        - FLOW=Wallet → \`apple_pay\`, \`google_pay\`, \`samsung_pay\`,
          \`paypal\`, etc.
        - FLOW=UPI → \`upi_collect\` or \`upi_intent\`.
        - FLOW=BankRedirect → \`ideal\`, \`giropay\`, \`sofort\`, etc.
        - FLOW=BankTransfer → \`ach_bank_transfer\`, \`sepa_bank_transfer\`, etc.
        - FLOW=Card → \`card\`.
      The variant's MESSAGE TYPE (e.g. \`Ach\`) defines the inner shape.
      Read its proto definition for required vs optional fields.

   b. **Just-generated transformer** at
      \`crates/integrations/connector-integration/src/connectors/<CONNECTOR>/transformers.rs\`.
      Grep for the FLOW match arm — typically
      \`PaymentMethodData::BankDebit(...)\`, \`PaymentMethodData::Wallet(...)\`,
      \`PaymentMethodData::Upi(...)\`, etc. Every field that arm reads with
      a \`?\` (no fallback) is REQUIRED in your payload. This includes
      top-level fields the transformer pulls off the request directly,
      such as \`request.get_email()\` (maps to top-level \`customer.email\`
      in the gRPC request), \`request.get_billing_address()\`, etc. — those
      are NOT inside \`payment_method\` and are easy to miss.

   c. field_probe (only if not flagged stale by step 2).

4. Discover the gRPC method to call against the running server:
     grpcurl -plaintext SERVER_HOST list
     grpcurl -plaintext SERVER_HOST describe <service>

5. Construct the grpcurl request:
   - Pass connector credentials via -H headers. Header names are
     connector-specific; populate them from CREDS_PATH. Do not invent
     headers that aren't in the credentials file.
   - For non-card flows: the payload uses the appropriate direct-debit /
     wallet / upi / bank_redirect variant of \`payment_method\`, NOT
     \`card\`. A card-shaped payload sent to a BankDebit/Wallet/UPI test
     will be rejected with INVALID_ARGUMENT every time. Re-confirm with
     step 3a before sending.
   - Include any required top-level fields the transformer reads
     (commonly \`customer.email\`, \`address.billing_address\`).
   - Capture the FULL command and FULL response.

6. Run the grpcurl call. Initial attempt = ATTEMPT 1.

7. **Corrective retry within this run.** On a non-success response, do
   NOT just report FAIL. Attempt up to 3 corrective retries (so up to 4
   grpcurl calls total per agent run) before giving up:

   a. Tail SERVER_LOG_PATH — find the most recent error line. Look for
      \`error_message\`, \`Missing required field: X\`,
      \`InvalidArgument\`, panics, or 4xx/5xx tower-http traces.
   b. Identify what was missing or wrong. Re-check step 3a (proto) and
      step 3b (transformer) to confirm the right field name and shape.
   c. Build a corrected payload, run grpcurl again, capture full command
      + response.

   Stop early on a 2xx-equivalent status (\`authorized\`, \`PENDING\`,
   \`charged\`, \`requires_capture\`, \`succeeded\`). If 4 attempts have all
   failed, stop and report FAIL with the final blocker explained.

8. Validate the FINAL response:
   - PASS if status is one of: authorized, PENDING, charged,
     requires_capture, succeeded.
   - FAIL otherwise — and \`response_summary\` must include the relevant
     SERVER_LOG_PATH excerpt explaining why.

## Output format

Return ONLY a single valid JSON object. No prose, no markdown fences. First
character must be \`{\`, last must be \`}\`.

{
  "status": "SUCCESS" | "FAILED",
  "grpcurl_result": "PASS" | "FAIL",
  "grpcurl_command": "the LAST grpcurl command (the one that produced the final response)",
  "copy_paste_command": "complete LAST command with JSON payload embedded inline",
  "request_payload": "the exact JSON payload of the LAST attempt",
  "grpcurl_output": "concatenation of ALL attempts, prefixed '=== ATTEMPT N ===' so the reviewer sees the iteration",
  "response_summary": "which attempt succeeded, or — if all failed — the final blocker plus the relevant SERVER_LOG_PATH excerpt"
}
`;

// Phase 12: bound the inner test-fix loop. Each iteration is: rebuild
// grpc-server + test via SLIM_PROMPT agent + (on fail) resume implementation
// to fix code. Keep this conservative — each iteration can take 10-20 min
// on a cold rebuild.
const MAX_GRPC_FIX_ITERATIONS = 3;

export const grpcTestCheckpoint: Checkpoint = {
  id: "grpc_test",
  name: "gRPC Test",
  description:
    "Test connector via gRPC calls using grpcurl. Phase 12: owns its own test-fix loop — on grpcurl failure, resumes implementation's Claude session to fix the code, rebuilds the server, and retries up to MAX_GRPC_FIX_ITERATIONS times.",
  // Phase 12: self-loop. Failures fix code via implementation's session
  // directly, rather than rolling back to the implementation checkpoint.
  retryFrom: "grpc_test",
  // Phase 14: effectively no upper bound on the outer wrapper. A cold cargo
  // rebuild + multiple fix iterations can legitimately push past 80 min on
  // slower machines, and the engine's `withTimeout` (utils.ts:11-28) was
  // killing live runs. The four inner timeouts inside `run()` still bound
  // each individual operation — waitForBuildComplete (20m), waitForHealthy
  // (45s), runAI test agent (10m), runAI fix agent (15m) — so a wedged
  // subprocess won't sit forever. 24h is a soft cap matching the human-
  // review checkpoints' idiom; using Infinity / MAX_SAFE_INTEGER would be
  // silently clamped to 1ms by Node's setTimeout (signed-32-bit limit).
  timeout: 24 * 60 * 60 * 1000,
  continueOnFailure: true,

  async run(ctx) {
    const task = ctx.artifacts.task;
    const connector = task?.targetConnectors?.[0];
    const flow = task?.paymentMethod || "Unknown";
    const projectRoot = task?.projectRoot;

    if (!projectRoot) {
      return { passed: false, errors: ["Missing project root"] };
    }

    ctx.log(
      "[grpc_test] ╔═══════════════════════════════════════════════════════════╗",
      "info"
    );
    ctx.log(
      "[grpc_test] ║  gRPC Test (orchestrator-managed server)               ║",
      "info"
    );
    ctx.log(
      "[grpc_test] ╚═══════════════════════════════════════════════════════════╝",
      "info"
    );
    ctx.log(`[grpc_test] Connector: ${connector}`, "info");
    ctx.log(`[grpc_test] Flow: ${flow}`, "info");

    // Phase 10: resolve per-session ports set by run.ts at engine boot.
    // Default session uses unshifted 8000/8080; new sessions get
    // 8000+portSlot / 8080+portSlot so parallel runs don't collide.
    const grpcPort = task.grpcPort ?? 8000;
    const dummyConnectorPort = task.dummyConnectorPort ?? 8080;
    const serverHost = `localhost:${grpcPort}`;
    const credsPath = path.join(projectRoot, "creds.json");
    const logFile = path.join(
      projectRoot,
      ".grace",
      `grpc-server-${ctx.runId}.log`
    );

    // Field probe is optional — the agent uses it as authoritative payload
    // structure when present, otherwise builds from spec.
    let fieldProbePath: string | null = null;
    let hasFieldProbe = false;
    try {
      const probePath = path.join(
        projectRoot,
        "data",
        "field_probe",
        `${connector?.toLowerCase() || "unknown"}.json`
      );
      await fs.access(probePath);
      fieldProbePath = probePath;
      hasFieldProbe = true;
      ctx.log(`[grpc_test] Field probe: ${probePath}`, "info");
    } catch {
      ctx.log("[grpc_test] No field probe data available", "warn");
    }

    type GrpcTestResult = {
      status?: string;
      grpcurl_result?: string;
      grpcurl_command?: string;
      copy_paste_command?: string;
      request_payload?: string;
      grpcurl_output?: string;
      response_summary?: string;
      output?: string;
      reason?: string;
    };

    let result: GrpcTestResult | undefined;
    let server: ServerHandle | undefined;

    const tsLog = (msg: string, level: "info" | "warn" | "error" = "info") =>
      ctx.log(`[grpc_test] ${msg}`, level);

    // Helper: (re)start the grpc-server and wait for it to be healthy. Used
    // both for the initial startup and after each fix iteration, since a
    // code fix invalidates the running binary.
    const bringServerUp = async (label: string): Promise<void> => {
      tsLog(`${label}: killing stale processes on :${grpcPort}/:${dummyConnectorPort}`);
      await killStaleProcesses(tsLog, grpcPort, dummyConnectorPort);
      tsLog(`${label}: starting grpc-server (gRPC :${grpcPort}, dummy :${dummyConnectorPort}), log → ${logFile}`);
      server = await startGrpcServer(
        { projectRoot, logFile, grpcPort, dummyConnectorPort },
        tsLog
      );
      // Cold cargo builds can take 10-20 min on this tree (diesel + macros);
      // post-fix rebuilds are usually <1 min because cargo only rebuilds the
      // changed crate. waitForBuildComplete watches the log for the
      // "Running `target/.../grpc-server`" marker before falling through to
      // the TCP health probe.
      tsLog(`${label}: waiting for cargo build to finish (≤20min worst-case)`);
      await waitForBuildComplete(
        { logFile, timeoutMs: 20 * 60 * 1000 },
        tsLog
      );
      tsLog(`${label}: waiting for ${serverHost} to become healthy (≤45s)`);
      await waitForHealthy(
        { host: "localhost", port: grpcPort, timeoutMs: 45_000 },
        tsLog
      );
    };

    try {
      // Initial server startup (iteration 0).
      await bringServerUp("preflight");

      const implSessionId = ctx.artifacts.implementationSessionId;
      let grpcSessionId = ctx.artifacts.grpcTestSessionId as
        | string
        | undefined;
      let grpcCommand = "";
      let grpcOutput = "";
      let responseSummary = "";
      let uiOutput = "";

      // Phase 12: inner test-fix loop. Each iteration: run grpcurl via the
      // test agent → if pass, return success; if fail, resume implementation
      // to fix code, restart server, re-test. Bounded by
      // MAX_GRPC_FIX_ITERATIONS.
      for (let iter = 0; iter <= MAX_GRPC_FIX_ITERATIONS; iter++) {
        tsLog(
          iter === 0
            ? "server healthy — invoking test agent (iter 0)"
            : `re-testing after fix iteration ${iter}…`,
          "info"
        );

        // Phase 15: deterministic session id from (connector, flow, phase).
        const grpcFriendly = friendlySessionName(connector, flow, "grpctest");
        const grpcDerived = deriveClaudeSessionId(connector, flow, "grpctest");

        const testCall = grpcSessionId
          ? {
              claudeSessionId: grpcSessionId,
              incremental: true,
              userPayload:
                `Iteration ${iter}: the implementation Claude has updated the code and the grpc-server has been rebuilt and restarted at ${serverHost}. Re-run your grpcurl test against this fresh server (same connector ${connector}, same flow ${flow}). Reply with the same JSON shape as your first reply.`,
              skillBody: "",
            }
          : {
              skillBody: SLIM_PROMPT,
              userPayload: {
                CONNECTOR: connector,
                FLOW: flow,
                SERVER_HOST: serverHost,
                CREDS_PATH: credsPath,
                SERVER_LOG_PATH: logFile,
                HAS_FIELD_PROBE: hasFieldProbe,
                FIELD_PROBE_PATH: fieldProbePath || "",
              },
              preferredSessionId: grpcDerived,
            };

        const { result: r, sessionId: nextGrpcSessionId } =
          await runAI<GrpcTestResult>({
            ...testCall,
            cwd: projectRoot,
            label: grpcSessionId ? `grpc:test:retry-${iter}` : "grpc:test",
            timeoutMs: 10 * 60 * 1000,
            sessionLabel: grpcFriendly,
          });
        result = r;
        grpcSessionId = nextGrpcSessionId;
        ctx.artifacts.grpcTestSessionId = nextGrpcSessionId;

        const testPassed =
          r.status === "SUCCESS" || r.grpcurl_result === "PASS";

        grpcCommand = r.grpcurl_command || "";
        grpcOutput = r.grpcurl_output || r.output || "";
        responseSummary = r.response_summary || "";

        uiOutput = `gRPC Test Results (iteration ${iter}/${MAX_GRPC_FIX_ITERATIONS})
═══════════════════════════════════════════════════════════════

COMMAND USED:
${grpcCommand || "Not captured"}

RESPONSE RECEIVED:
${responseSummary || grpcOutput.slice(0, 2000)}

FULL OUTPUT:
${grpcOutput.slice(0, 3000)}
`;

        if (testPassed) {
          tsLog(`✓ gRPC tests passed at iteration ${iter}`, "info");
          return {
            passed: true,
            output: uiOutput,
            artifacts: {
              grpcTest: r,
              grpcurlOutput: grpcOutput,
              grpcurlCommand: grpcCommand,
              grpcResponse: responseSummary,
              grpcTestIterations: iter,
              // Phase 15: belt-and-suspenders echo. The session id is mutated
              // inside the fix loop (line ~325) but checkpoint runs can span
              // many iterations and only state.save at checkpoint boundary
              // persists. Echoing here makes the persistence contract explicit
              // for the success path.
              grpcTestSessionId: ctx.artifacts.grpcTestSessionId,
            },
          };
        }

        tsLog(`⚠ gRPC test failed at iteration ${iter}`, "warn");

        // Out of fix iterations — return failure.
        if (iter === MAX_GRPC_FIX_ITERATIONS) {
          break;
        }

        // No implementation session to drive fixes — also bail out.
        if (!implSessionId) {
          tsLog(
            "No implementationSessionId on artifacts — cannot auto-fix. Returning failure.",
            "warn"
          );
          break;
        }

        // Resume implementation's session with the grpcurl errors and ask
        // it to fix the code. Then rebuild + restart the server before the
        // next iteration's test.
        tsLog(
          `asking implementation Claude (session ${implSessionId.slice(0, 8)}…) to fix grpcurl errors before iteration ${iter + 1}`,
          "warn"
        );
        const fixMsg = [
          `grpcurl test failed against your last code update. Details:`,
          ``,
          `Connector: ${connector} · Flow: ${flow} · Endpoint: ${serverHost}`,
          ``,
          `Test agent response:`,
          `  ${r.reason || "(no reason given)"}`,
          ``,
          `Response summary:`,
          responseSummary || "(empty)",
          ``,
          `Grpcurl output (last 2000 chars):`,
          "```",
          grpcOutput.slice(-2000),
          "```",
          ``,
          `Fix the connector code (request struct, response parsing, transformers, status mapping — whatever the error points at). Do NOT touch unrelated files. The grpc-server will be rebuilt and restarted after your edits. Reply briefly when you're done.`,
        ].join("\n");

        try {
          await runAI({
            claudeSessionId: implSessionId,
            incremental: true,
            userPayload: fixMsg,
            skillBody: "",
            rawText: true,
            cwd: projectRoot,
            label: `grpc_test:fix-iter-${iter + 1}`,
            timeoutMs: 15 * 60 * 1000,
            allowWrite: true,
          });
        } catch (fixErr) {
          const msg = fixErr instanceof Error ? fixErr.message : String(fixErr);
          tsLog(`fix iteration ${iter + 1} threw: ${msg}`, "error");
          break;
        }

        // Restart server to pick up the fix.
        try {
          if (server) {
            await server.kill();
            server = undefined;
          }
          await bringServerUp(`fix-iter-${iter + 1}`);
        } catch (restartErr) {
          const msg = restartErr instanceof Error ? restartErr.message : String(restartErr);
          tsLog(`server restart after fix failed: ${msg}`, "error");
          break;
        }
      }

      // Fell out of the loop without success — return the last failure state.
      const errorMsg = result?.reason || "gRPC test failed after all fix iterations";
      ctx.artifacts.grpcTestErrors = [errorMsg, grpcOutput].filter(Boolean);
      ctx.artifacts.grpcurlOutput = grpcOutput;

      return {
        passed: false,
        output: uiOutput,
        errors: [errorMsg],
        artifacts: {
          grpcTest: result,
          grpcurlOutput: grpcOutput,
          grpcurlCommand: grpcCommand,
          grpcResponse: responseSummary,
          grpcTestFailed: true,
          grpcTestError: errorMsg,
          // Phase 15: belt-and-suspenders echo on failure too (this is the
          // path the audit flagged as highest-risk for losing the in-loop
          // mutation).
          grpcTestSessionId: ctx.artifacts.grpcTestSessionId,
        },
      };
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      // Tail the server log so the dashboard shows real diagnostics instead of
      // "No output captured" — this is the failure mode that motivated the
      // whole refactor.
      const logTail = await tailLogFile(logFile, 2048);
      const errorOutput = `gRPC Test Error: ${msg}

═══════════════════════════════════════════════════════════════
COMMAND ATTEMPTED:
═══════════════════════════════════════════════════════════════
${result?.grpcurl_command || "Not captured - test timed out before execution"}

═══════════════════════════════════════════════════════════════
RESPONSE/OUTPUT:
═══════════════════════════════════════════════════════════════
${result?.grpcurl_output || result?.output || "No output captured"}

═══════════════════════════════════════════════════════════════
SERVER LOG TAIL (${logFile}):
═══════════════════════════════════════════════════════════════
${logTail || "(empty — server produced no output before timeout)"}`;

      tsLog(`gRPC test failed: ${msg}`, "error");

      const errorResult: GrpcTestResult = {
        status: "FAILED",
        grpcurl_result: "FAIL",
        reason: `gRPC test failed: ${msg}`,
        grpcurl_command: result?.grpcurl_command || "",
        grpcurl_output: errorOutput,
        output: errorOutput,
      };

      return {
        passed: false,
        output: errorOutput,
        errors: [`gRPC test failed: ${msg}`],
        artifacts: {
          grpcTest: errorResult,
          grpcurlOutput: errorOutput,
          grpcTestFailed: true,
        },
      };
    } finally {
      if (server) {
        try {
          await server.kill();
          tsLog(`server pid=${server.pid} stopped`);
        } catch (e) {
          tsLog(
            `warning: server cleanup failed: ${e instanceof Error ? e.message : String(e)}`,
            "warn"
          );
        }
      }
    }
  },
};
