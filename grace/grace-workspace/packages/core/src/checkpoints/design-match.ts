import type { Checkpoint } from "../types.js";

export const designMatchCheckpoint: Checkpoint = {
  id: "design_match",
  name: "Design match",
  description: "Placeholder — visual diff is disabled in this build.",
  retryFrom: "implementation",
  async run(ctx) {
    if (!ctx.artifacts.designGate?.designRequired) {
      ctx.log("[design_match] Design not required by design_gate, skipping.", "info");
      return { passed: true };
    }
    ctx.log(
      "[design_match] Visual diff is disabled in this build — passing through.",
      "info"
    );
    return { passed: true };
  },
};
