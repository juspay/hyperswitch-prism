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

/** Canonical flow → CheckpointId mapping. Matches `.gracerules` flow names. */
export const FLOW_CHECKPOINT_IDS: Record<string, CheckpointId> = {
  Authorize: "impl_authorize",
  PSync: "impl_psync",
  Capture: "impl_capture",
  Refund: "impl_refund",
  RSync: "impl_rsync",
  Void: "impl_void",
  CreateAccessToken: "impl_create_access_token",
  CreateOrder: "impl_create_order",
  CreateConnectorCustomer: "impl_create_connector_customer",
  PaymentMethodToken: "impl_payment_method_token",
  CreateSessionToken: "impl_create_session_token",
};

/** Canonical core flow order from `.gracerules` PHASE 3. */
export const CORE_FLOW_ORDER: ReadonlyArray<string> = [
  "Authorize",
  "PSync",
  "Capture",
  "Refund",
  "RSync",
  "Void",
];

/** Canonical pre-auth flow order from `.gracerules` PHASE 3. */
export const PRE_AUTH_FLOW_ORDER: ReadonlyArray<string> = [
  "CreateAccessToken",
  "CreateOrder",
  "CreateConnectorCustomer",
  "PaymentMethodToken",
  "CreateSessionToken",
];

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

      const result = await runFlowImplementation({
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
