import type { Checkpoint, L2Plan } from "../types.js";
import { runHumanReview } from "./human-review.js";
import { maybeCreateGraceIssue } from "./grace-issue.js";

function renderL2(spec: L2Plan) {
  const line = "─".repeat(60);
  // eslint-disable-next-line no-console
  console.log(`
┌${line}┐
│  L2 SPECIFICATION REVIEW
├${line}┤
│  Summary       │ ${spec.summary}
│  Scope         │ ${spec.scope}
│  Out of scope  │ ${spec.outOfScope}
│  Constraints   │ ${spec.technicalConstraints.join(", ")}
│  Complexity    │ ${spec.estimatedComplexity}
└${line}┘`);
}

function validateL2(v: unknown): v is L2Plan {
  if (!v || typeof v !== "object") return false;
  const s = v as Record<string, unknown>;
  return (
    typeof s.summary === "string" &&
    typeof s.scope === "string" &&
    typeof s.outOfScope === "string" &&
    Array.isArray(s.technicalConstraints) &&
    ["low", "medium", "high"].includes(s.estimatedComplexity as string)
  );
}

export const l2ReviewCheckpoint: Checkpoint = {
  id: "l2_review",
  name: "Human review: L2 plan",
  description: "Pauses for human sign-off on the generated L2 plan.",
  retryFrom: "l2_planning",
  timeout: 24 * 60 * 60 * 1000,
  maxRetries: 5,
  async run(ctx) {
    const result = await runHumanReview<L2Plan>(ctx, {
      checkpointId: "l2_review",
      specType: "l2",
      loadSpec: (c) => c.artifacts.l2,
      render: renderL2,
      validate: validateL2,
      applySpec: (c, s) => {
        c.artifacts.l2 = s;
      },
      regenerateKey: "l2RegeneratePrompt",
      previousKey: "previousL2",
      sessionKey: "l2Review",
    });

    // Phase 13: on approve, file a tracking issue at juspay/grace. Warns and
    // returns the original result unchanged on failure — external system
    // side-effect is not a correctness gate.
    return maybeCreateGraceIssue(ctx, result);
  },
};
