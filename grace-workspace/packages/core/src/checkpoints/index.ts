import type { Checkpoint } from "../types.js";
import { taskCheckpoint } from "./task.js";
import { preflightCheckpoint } from "./preflight.js";
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

// Grace 2.3_codegen.md workflow: task → preflight → L2_planning → L3_analysis → implementation → compiler_check → grpc_test
export const ALL_CHECKPOINTS: Checkpoint[] = [
  taskCheckpoint,
  preflightCheckpoint,
  l2PlanningCheckpoint,
  l2ReviewCheckpoint,
  l3AnalysisCheckpoint,
  l3ReviewCheckpoint,
  implementationCheckpoint,
  compilerCheckpoint,
  // REMOVED: designMatchCheckpoint,
  // REMOVED: cypressCheckpoint,
  // REMOVED: playwrightCheckpoint,
  // REMOVED: compilerCheckCheckpoint,
  grpcTestCheckpoint,
  prReviewCheckpoint,
  // 3_test.md — hardening via test-prism + positive-override fix loop.
  // Soft-fails (continueOnFailure) so it never blocks a PR that's already up.
  testSuiteCheckpoint,
  regressionCheckpoint,
];
