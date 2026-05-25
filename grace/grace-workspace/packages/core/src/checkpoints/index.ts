import type { Checkpoint, TaskDefinition } from "../types.js";
import { taskCheckpoint } from "./task.js";
import { preflightCheckpoint } from "./preflight.js";
import { scaffoldCheckpoint } from "./scaffold.js";
import { l2PlanningCheckpoint } from "./l2-planning.js";
import { l2ReviewCheckpoint } from "./l2-review.js";
import { l3AnalysisCheckpoint } from "./l3-analysis.js";
import { l3ReviewCheckpoint } from "./l3-review.js";
import { implementationCheckpoint } from "./implementation.js";
import { compilerCheckpoint } from "./compiler.js";
// REMOVED: import { designMatchCheckpoint } from "./design-match.js";
// REMOVED: import { cypressCheckpoint } from "./cypress.js";
// REMOVED: import { playwrightCheckpoint } from "./playwright.js";
// REMOVED: import { compilerCheckCheckpoint } from "./compiler-check.js";
import { grpcTestCheckpoint } from "./grpc-test.js";
import { prReviewCheckpoint } from "./pr-review.js";
import { testSuiteCheckpoint } from "./test-suite.js";
import { regressionCheckpoint } from "./regression.js";
import {
  makeFlowCheckpoint,
  CORE_FLOW_ORDER,
  PRE_AUTH_FLOW_ORDER,
} from "./flow-checkpoint.js";
import {
  detectFlowsFromSpec,
  orderedFlowList,
} from "../new-connector/flow-detector.js";
import {
  defaultSpecPath,
} from "../new-connector/flow-runner.js";

/**
 * Default checkpoint sequence — what add-payment-method, add-flow, and generic
 * tasks run. Also what `cli/status.ts` and the dashboard's initial state init
 * iterate, so adding new-connector-specific checkpoints (scaffold, impl_*)
 * here would visibly pollute every other workflow. They're only added by
 * `buildPipeline()` below when `workflowType === "new-connector"`.
 *
 * Grace 2.3_codegen.md workflow: task → preflight → L2 → L3 → implementation
 * → compiler → grpc_test → pr_review → regression.
 */
export const ALL_CHECKPOINTS: Checkpoint[] = [
  taskCheckpoint,
  preflightCheckpoint,
  l2PlanningCheckpoint,
  l2ReviewCheckpoint,
  l3AnalysisCheckpoint,
  l3ReviewCheckpoint,
  implementationCheckpoint,
  compilerCheckpoint,
  grpcTestCheckpoint,
  prReviewCheckpoint,
  // 3_test.md — hardening via test-prism + positive-override fix loop.
  // Soft-fails (continueOnFailure) so it never blocks a PR that's already up.
  testSuiteCheckpoint,
  regressionCheckpoint,
];

/**
 * Build the per-workflow checkpoint sequence the engine actually runs.
 *
 * - `new-connector`: task → preflight → scaffold → (per-flow subagents) →
 *   compiler → grpc_test → pr_review → regression. SKIPS L2 plan/review and
 *   L3 analysis/review (the wizard captured the techspec; `.gracerules` does
 *   its own per-flow analysis). SKIPS the monolithic implementation checkpoint
 *   in favour of one per-flow checkpoint each.
 * - `add-payment-method` / `add-flow` / `generic` / undefined: the full
 *   `ALL_CHECKPOINTS` sequence, unchanged. L2 and L3 human-review gates run
 *   as before — must not regress for add-payment-method.
 *
 * The scaffold checkpoint silently no-ops when `task.baseUrl` and
 * `task.sandboxUrl` are both unset, so it's safe to include unconditionally
 * for new-connector even if a future entry point forgets to set baseUrl.
 */
export function buildPipeline(task: TaskDefinition): Checkpoint[] {
  if (task.workflowType === "new-connector") {
    // Flow resolution priority:
    //   1. `task.flows` (wizard set them explicitly — trust the user)
    //   2. Auto-detect from the techspec at the canonical path (catches
    //      pre-auth flows the wizard didn't pre-flag)
    //   3. Fallback to the 6 canonical core flows from `.gracerules`
    let ordered: string[];
    if (task.flows && task.flows.length > 0) {
      const known = new Set([...PRE_AUTH_FLOW_ORDER, ...CORE_FLOW_ORDER]);
      const preAuth = PRE_AUTH_FLOW_ORDER.filter((f) => task.flows!.includes(f));
      const core = CORE_FLOW_ORDER.filter((f) => task.flows!.includes(f));
      const dropped = task.flows.filter((f) => !known.has(f));
      if (dropped.length > 0) {
        // eslint-disable-next-line no-console
        console.warn(
          `[buildPipeline] Dropped unknown flow names: ${dropped.join(", ")}`
        );
      }
      ordered = [...preAuth, ...core];
    } else {
      const connector = task.targetConnectors?.[0];
      if (connector) {
        const specPath = defaultSpecPath(task.projectRoot, connector);
        const detected = detectFlowsFromSpec(specPath);
        ordered = orderedFlowList(detected);
      } else {
        ordered = [...CORE_FLOW_ORDER];
      }
    }

    return [
      taskCheckpoint,
      preflightCheckpoint,
      scaffoldCheckpoint,
      ...ordered.map((f) => makeFlowCheckpoint(f)),
      compilerCheckpoint,
      grpcTestCheckpoint,
      prReviewCheckpoint,
      testSuiteCheckpoint,
      regressionCheckpoint,
    ];
  }

  // add-payment-method, add-flow, generic, or unset → full pipeline
  // unchanged. Scaffold is NOT included here: add-payment-method workflows
  // don't need add_connector.sh (the connector already exists), so we
  // keep the legacy behaviour identical to pre-Phase-2.
  return ALL_CHECKPOINTS;
}
