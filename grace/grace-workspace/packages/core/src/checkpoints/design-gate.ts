import type { Checkpoint, DesignGateResult } from "../types.js";
import { ask, askYesNo } from "../prompts/cli-prompts.js";
import { autoDesignGate } from "../agents/auto-reviewer.js";

interface DesignGateResponse {
  designRequired: boolean;
  connectorDocUrls?: string[];
  skipReason?: string;
}

export const designGateCheckpoint: Checkpoint = {
  id: "design_gate",
  name: "Design gate",
  description: "Ask whether UI design is needed and capture connector reference document URLs if so.",
  retryFrom: "design_gate",
  timeout: 24 * 60 * 60 * 1000,
  async run(ctx) {
    // Auto mode: reviewer agent decides.
    if (ctx.options.autoMode) {
      try {
        const decision = await autoDesignGate(ctx);
        const gate: DesignGateResult = decision.designRequired
          ? {
              designRequired: true,
              docUrlsReady: !!(decision.connectorDocUrls && decision.connectorDocUrls.length > 0),
              connectorDocUrls: decision.connectorDocUrls,
            }
          : {
              designRequired: false,
              docUrlsReady: false,
              skipReason: decision.skipReason,
            };
        return { passed: true, artifacts: { designGate: gate } };
      } catch (err) {
        ctx.log(
          `auto-reviewer failed, falling back: ${err instanceof Error ? err.message : String(err)}`,
          "warn"
        );
      }
    }

    // CI mode: infer from the task definition and pass through.
    if (ctx.options.autoApproveReviews) {
      const urls = ctx.artifacts.task?.connectorDocUrls;
      const gate: DesignGateResult = urls && urls.length > 0
        ? { designRequired: true, docUrlsReady: true, connectorDocUrls: urls }
        : {
            designRequired: false,
            docUrlsReady: false,
            skipReason: "auto-approved — no connectorDocUrls in task",
          };
      return { passed: true, artifacts: { designGate: gate } };
    }

    // UI mode: wait for dashboard to answer.
    if (ctx.options.taskFromUi && ctx.bus) {
      ctx.log(
        "[design_gate] ⏳ Awaiting decision from the dashboard: does this task need UI design?",
        "warn"
      );
      ctx.bus.emitHumanWaiting("design_gate", {
        question: "Does this task require UI design?",
        currentConnectorDocUrls: ctx.artifacts.task?.connectorDocUrls,
      });

      while (true) {
        const response = await ctx.bus.waitFor<DesignGateResponse>(
          "human:design_gate"
        );
        if (typeof response.designRequired !== "boolean") {
          ctx.bus.emit("human:rejected", "design_gate", {
            reason: "designRequired must be a boolean",
          });
          continue;
        }
        if (response.designRequired) {
          const urls = response.connectorDocUrls?.filter(u => u.trim()).map(u => u.trim()) ?? [];
          if (urls.length === 0) {
            ctx.bus.emit("human:rejected", "design_gate", {
              reason: "At least one connector reference document URL is required when design is needed",
            });
            continue;
          }
          const gate: DesignGateResult = {
            designRequired: true,
            docUrlsReady: true,
            connectorDocUrls: urls,
          };
          ctx.log(`[design_gate] ✓ Design required (${urls.length} doc URLs)`, "success");
          ctx.bus.emit("human:resolved", "design_gate", {
            decision: "design_required",
          });
          return { passed: true, artifacts: { designGate: gate } };
        }
        const gate: DesignGateResult = {
          designRequired: false,
          docUrlsReady: false,
          skipReason: response.skipReason?.trim() || "Reviewer said no design needed",
        };
        ctx.log(
          `[design_gate] ✓ No design needed: ${gate.skipReason}`,
          "success"
        );
        ctx.bus.emit("human:resolved", "design_gate", {
          decision: "no_design",
        });
        return { passed: true, artifacts: { designGate: gate } };
      }
    }

    // CLI fallback
    const needsDesign = await askYesNo("Does this task require UI design?", true);
    if (!needsDesign) {
      const reason = await ask("Reason for skipping design gate: ");
      return {
        passed: true,
        artifacts: {
          designGate: {
            designRequired: false,
            docUrlsReady: false,
            skipReason: reason || "not required",
          } satisfies DesignGateResult,
        },
      };
    }
    const urlsFromTask = ctx.artifacts.task?.connectorDocUrls;
    let urls: string[];
    if (urlsFromTask && urlsFromTask.length > 0) {
      urls = urlsFromTask;
    } else {
      const raw = await ask("Connector reference document URLs (comma-separated): ");
      urls = raw.split(",").map((s) => s.trim()).filter(Boolean);
    }
    if (urls.length === 0) {
      return { passed: false, errors: ["At least one connector reference document URL is required"] };
    }
    return {
      passed: true,
      artifacts: {
        designGate: {
          designRequired: true,
          docUrlsReady: true,
          connectorDocUrls: urls,
        } satisfies DesignGateResult,
      },
    };
  },
};
