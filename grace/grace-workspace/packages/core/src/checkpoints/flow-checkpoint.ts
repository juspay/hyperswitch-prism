import type { Checkpoint, CheckpointId } from "../types.js";
import {
  runFlowImplementation,
  defaultGracerulesPath,
  defaultSpecPath,
} from "../new-connector/flow-runner.js";

/**
 * Per-flow implementation checkpoint factory.
 *
 * `.gracerules` PHASE 3 prescribes ONE subagent per flow, executed
 * sequentially in canonical order:
 *
 *   Pre-Auth (conditional, per techspec):
 *     CreateAccessToken, CreateOrder, CreateConnectorCustomer,
 *     PaymentMethodToken, CreateSessionToken
 *   Core (always):
 *     Authorize, PSync, Capture, Refund, RSync, Void
 *
 * Each flow gets its own checkpoint row in the dashboard so the user can
 * see which subagent is running, what it's doing (via live runner logs
 * from Phase 3), and pass/fail status per flow.
 *
 * Phase 4 wires the run() body to actually spawn a Claude session with
 * the flow's subagent block from `.gracerules` lines 479–571. For Phase 1
 * the body is a placeholder that completes immediately so the rest of
 * the workflow-type-aware pipeline can be wired and verified end-to-end.
 */

/** Whether a flow runs in the pre-auth phase (before Authorize) or in the
 *  core phase (Authorize and after). All "core" flows implicitly depend on
 *  all present "pre-auth" flows. */
export type FlowKind = "pre-auth" | "core";

export interface FlowDef {
  readonly name: string;
  readonly checkpointId: CheckpointId;
  readonly kind: FlowKind;
  /**
   * Explicit dependencies on other flows by name. Intersected with the
   * connector's actual flow set at buildPipeline time — if CreateOrder
   * isn't in this connector's flows, Authorize won't actually wait on it.
   * "core" flows additionally implicitly depend on all present "pre-auth"
   * flows (no need to repeat them here).
   */
  readonly dependsOn: readonly string[];
}

/** Canonical flow registry from `.gracerules` PHASE 3.
 *
 *  Array order is the deterministic tiebreaker for `topologicalOrder` —
 *  flows ready to run at the same logical depth come out in the order they
 *  appear here.
 */
export const FLOWS: readonly FlowDef[] = [
  // Pre-auth: no inter-deps among themselves (all five are independent
  // setup steps; order among them is arbitrary but stable).
  { name: "CreateAccessToken",       checkpointId: "impl_create_access_token",       kind: "pre-auth", dependsOn: [] },
  { name: "CreateOrder",             checkpointId: "impl_create_order",              kind: "pre-auth", dependsOn: [] },
  { name: "CreateConnectorCustomer", checkpointId: "impl_create_connector_customer", kind: "pre-auth", dependsOn: [] },
  { name: "PaymentMethodToken",      checkpointId: "impl_payment_method_token",      kind: "pre-auth", dependsOn: [] },
  { name: "CreateSessionToken",      checkpointId: "impl_create_session_token",      kind: "pre-auth", dependsOn: [] },
  // Core: implicit dep on all present pre-auth flows + explicit chains.
  { name: "Authorize",               checkpointId: "impl_authorize",                 kind: "core",     dependsOn: [] },
  { name: "PSync",                   checkpointId: "impl_psync",                     kind: "core",     dependsOn: ["Authorize"] },
  { name: "Capture",                 checkpointId: "impl_capture",                   kind: "core",     dependsOn: ["Authorize"] },
  { name: "Refund",                  checkpointId: "impl_refund",                    kind: "core",     dependsOn: ["Capture"] },
  { name: "RSync",                   checkpointId: "impl_rsync",                     kind: "core",     dependsOn: ["Refund"] },
  { name: "Void",                    checkpointId: "impl_void",                      kind: "core",     dependsOn: ["Authorize"] },
];

/** Returns the flows this flow must wait for, given the connector's actual
 *  flow set. Used by the engine for build-time validation and by the UI
 *  for the "Runs after: X" hover tooltip on impl_* checkpoints. */
export function resolveFlowDeps(
  flow: FlowDef,
  presentFlows: ReadonlySet<string>,
): string[] {
  const deps = new Set<string>();
  for (const d of flow.dependsOn) if (presentFlows.has(d)) deps.add(d);
  if (flow.kind === "core") {
    for (const f of FLOWS) {
      if (f.kind === "pre-auth" && presentFlows.has(f.name)) deps.add(f.name);
    }
  }
  return Array.from(deps);
}

/** Topologically sort the present flows. Throws on a cycle. Preserves
 *  FLOWS-array order for deterministic tiebreaking. */
export function topologicalOrder(
  presentFlowNames: readonly string[],
): string[] {
  const present = new Set(presentFlowNames);
  const presentFlows = FLOWS.filter((f) => present.has(f.name));
  const indeg = new Map<string, number>(presentFlows.map((f) => [f.name, 0]));
  const adj = new Map<string, string[]>(presentFlows.map((f) => [f.name, []]));
  for (const f of presentFlows) {
    for (const d of resolveFlowDeps(f, present)) {
      adj.get(d)!.push(f.name);
      indeg.set(f.name, (indeg.get(f.name) ?? 0) + 1);
    }
  }
  const out: string[] = [];
  // Initial ready queue follows FLOWS-array order so ties between two
  // pre-auth flows (which all have indeg 0) resolve deterministically.
  const ready: string[] = presentFlows
    .filter((f) => indeg.get(f.name) === 0)
    .map((f) => f.name);
  while (ready.length) {
    const n = ready.shift()!;
    out.push(n);
    // Re-sort newly-ready children by FLOWS-array order before pushing.
    const newlyReady: string[] = [];
    for (const m of adj.get(n) ?? []) {
      indeg.set(m, (indeg.get(m) ?? 0) - 1);
      if (indeg.get(m) === 0) newlyReady.push(m);
    }
    newlyReady.sort((a, b) => {
      const ia = FLOWS.findIndex((f) => f.name === a);
      const ib = FLOWS.findIndex((f) => f.name === b);
      return ia - ib;
    });
    ready.push(...newlyReady);
  }
  if (out.length !== presentFlows.length) {
    throw new Error(
      `Cycle detected in flow deps; got ${out.length}/${presentFlows.length} after Kahn's. Check FLOWS dependsOn definitions.`,
    );
  }
  return out;
}

// ── Backward-compat derived exports ─────────────────────────────────────────
// Keep older array-based exports working for any external callers. All
// derived from the FLOWS registry above — single source of truth.

/** Canonical flow → CheckpointId mapping. Derived from FLOWS. */
export const FLOW_CHECKPOINT_IDS: Record<string, CheckpointId> =
  Object.fromEntries(FLOWS.map((f) => [f.name, f.checkpointId]));

/** Canonical core flow order from `.gracerules` PHASE 3. Derived from FLOWS. */
export const CORE_FLOW_ORDER: ReadonlyArray<string> = FLOWS
  .filter((f) => f.kind === "core")
  .map((f) => f.name);

/** Canonical pre-auth flow order from `.gracerules` PHASE 3. Derived from FLOWS. */
export const PRE_AUTH_FLOW_ORDER: ReadonlyArray<string> = FLOWS
  .filter((f) => f.kind === "pre-auth")
  .map((f) => f.name);

export function makeFlowCheckpoint(flow: string): Checkpoint {
  const id = FLOW_CHECKPOINT_IDS[flow];
  if (!id) {
    throw new Error(
      `Unknown flow "${flow}" — no CheckpointId mapping. ` +
        `Update FLOW_CHECKPOINT_IDS in flow-checkpoint.ts and CheckpointId in types.ts.`
    );
  }
  return {
    id,
    name: `Implement ${flow}`,
    description: `Subagent implementation for ${flow} flow (per .gracerules PHASE 3)`,
    retryFrom: id,
    timeout: 30 * 60 * 1000, // 30 min per flow
    async run(ctx) {
      const task = ctx.artifacts.task;
      if (!task) {
        return { passed: false, errors: ["Missing task artifact"] };
      }
      const connector = task.targetConnectors?.[0];
      if (!connector || connector === "unknown") {
        return {
          passed: false,
          errors: [
            `Per-flow subagent for "${flow}" needs a target connector — task.targetConnectors[0] was empty.`,
          ],
        };
      }
      const projectRoot = task.projectRoot;
      const gracerulesPath = defaultGracerulesPath(projectRoot);
      const specPath = defaultSpecPath(projectRoot, connector);

      ctx.log(
        `[${id}] Invoking ${flow} subagent for ${connector} (per .gracerules PHASE 3)`,
        "info"
      );

      // Resume support: previous attempt may have stashed a session id so
      // the same Claude conversation continues, preserving its prompt
      // cache and context across the engine's retry boundary.
      const sessionKey = `flowSessionId_${id}`;
      const priorSessionId = (ctx.artifacts as Record<string, unknown>)[
        sessionKey
      ] as string | undefined;

      let result;
      try {
        result = await runFlowImplementation({
          connector,
          flow,
          projectRoot,
          specPath,
          gracerulesPath,
          priorSessionId,
          claudeModel: task.runnerModel,
          // Live-stream the subagent's stdout to the dashboard so the
          // centre spinner shows each tool use as it happens.
          onStdoutLine: (line) => ctx.log(line, "info"),
        });
      } catch (err) {
        // On a Claude timeout-kill, the session's lock file lingers for
        // several minutes. The engine's checkpoint-retry layer reads
        // `flowSessionId_${id}` and re-passes it; Claude then refuses with
        // "Session ID … is already in use". Clear the stored session so the
        // next retry mints a fresh UUID. Other failure modes preserve the
        // session (resume keeps prompt cache + context).
        const msg = err instanceof Error ? err.message : String(err);
        if (/timed out after \d+ms/.test(msg)) {
          ctx.log(
            `[${id}] timeout detected — clearing stored session ${priorSessionId?.slice(0, 8)}… so the next retry starts fresh`,
            "warn",
          );
          delete (ctx.artifacts as Record<string, unknown>)[sessionKey];
        }
        throw err;
      }

      if (result.claudeSessionId) {
        (ctx.artifacts as Record<string, unknown>)[sessionKey] =
          result.claudeSessionId;
      }

      if (result.status === "passed") {
        ctx.log(
          `[${id}] ✓ ${flow} flow done in ${Math.round(
            result.durationMs / 1000
          )}s`,
          "success"
        );
        return {
          passed: true,
          output:
            result.summary ?? `${flow} subagent completed in ${result.durationMs}ms`,
          artifacts: { [`flowResult_${flow}`]: result },
        };
      }

      return {
        passed: false,
        errors: [
          result.lastError ?? `${flow} subagent failed without an error message`,
        ],
        output: result.summary,
      };
    },
  };
}
