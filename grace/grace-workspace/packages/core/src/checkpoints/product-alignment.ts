import { promises as fs } from "node:fs";
import path from "node:path";
import type { Checkpoint, ProductAlignmentDoc } from "../types.js";
import { callLlm } from "../llm.js";
import { artifactsDir, ensureDir, safeParseJson } from "../utils.js";
import { autoAnswerClarifyingQuestions } from "../agents/auto-reviewer.js";

interface Attachment {
  name: string;
  dataUrl: string;
}

async function saveAttachments(
  ctx: { runId: string; task: { projectRoot: string } },
  attachments: Record<string, Attachment[]>
): Promise<Record<string, string[]>> {
  const dir = path.join(artifactsDir(ctx.task.projectRoot), "clarifications", ctx.runId);
  await ensureDir(dir);
  const result: Record<string, string[]> = {};
  for (const [question, files] of Object.entries(attachments)) {
    result[question] = [];
    for (const [i, file] of files.entries()) {
      const match = file.dataUrl.match(/^data:(image\/\w+);base64,(.*)$/);
      if (!match) continue;
      const ext = (match[1] ?? "image/png").split("/")[1] ?? "png";
      const safeName = file.name.replace(/[^a-zA-Z0-9._-]/g, "_").slice(0, 40);
      const filename = `${Date.now()}-${i}-${safeName}.${ext}`;
      const abs = path.join(dir, filename);
      await fs.writeFile(abs, Buffer.from(match[2] ?? "", "base64"));
      result[question]!.push(abs);
    }
  }
  return result;
}

interface PmReviewResult {
  approved: boolean;
  notes: string;
  adjustedCriteria?: string[];
  clarifyingQuestions?: string[];
  /** References from Requirements Discovery */
  references?: {
    connectors?: string[];
    similarPaymentMethods?: string[];
    patternFiles?: string[];
  };
  /** GRACE: Implementation plan */
  implementationPlan?: {
    approach: string;
    considerations?: string[];
    phases?: Array<{
      name: string;
      description: string;
      dependsOn?: string[];
    }>;
    perConnectorPlan?: Record<
      string,
      {
        files: string[];
        patternToFollow: string;
        estimatedEffort?: string;
      }
    >;
  };
  /** GRACE: Confirmed complexity */
  confirmedComplexity?: "low" | "medium" | "high";
  /** GRACE: Ready for implementation */
  readyForImplementation?: boolean;
}

const SYSTEM = `You are a product manager reviewing payment method implementation requirements for the hyperswitch-prism connector system.

## CONTEXT PROVIDED
You have access to:
- Task definition with {PAYMENT_METHOD} and {TARGET_CONNECTORS}
- Requirements Discovery results showing:
  - Connector files analyzed
  - Current payment methods supported
  - Files that need modification
  - Implementation patterns identified
  - Per-connector scores (0-10)

## YOUR RESPONSIBILITIES

### Phase 1: Review Requirements
1. Verify all target connectors were analyzed
2. Check that implementation patterns are clear
3. Confirm similar payment methods exist as reference

### Phase 2: Assess Feasibility
For the {PAYMENT_METHOD} implementation:
- Are the identified files correct?
- Is the transformer pattern clear?
- Are there any blockers or concerns?

### Phase 3: Provide Implementation Guidance
Create a structured implementation plan:
1. **Approach**: High-level implementation strategy
2. **Phase Breakdown**: Steps for implementation
3. **Per-Connector Plan**: Specific guidance for each connector

### Phase 4: Clarifying Questions (if needed)
If requirements are unclear:
- Ask specific questions about connector structure
- Request more information about edge cases
- Clarify scope boundaries

## GRACE-STYLE ANTI-PATTERNS TO WATCH FOR

1. **Missing Reference Implementation**: If no similar payment method exists
2. **Unclear Transformer Pattern**: If the RouterDataV2 conversion is not documented
3. **Incomplete File Analysis**: If critical files weren't identified
4. **Low Score (< 4)**: Connector not ready for implementation

## OUTPUT FORMAT

Return ONLY a JSON object (no markdown fences, no extra text):
{
  "approved": boolean,
  "notes": "string (markdown with ## headings)",
  "adjustedCriteria": ["updated acceptance criteria"],
  "clarifyingQuestions": ["questions if not approved"],
  "references": {
    "connectors": ["Stripe", "Adyen"],
    "similarPaymentMethods": ["Card"],
    "patternFiles": ["src/connectors/stripe/transformers.rs"]
  },
  "implementationPlan": {
    "approach": "string - high-level strategy",
    "considerations": ["risk 1", "risk 2"],
    "phases": [
      {
        "name": "Phase 1: Types and Enums",
        "description": "Add payment method variants",
        "dependsOn": []
      },
      {
        "name": "Phase 2: Transformers",
        "description": "Implement request/response transformers",
        "dependsOn": ["Phase 1"]
      }
    ],
    "perConnectorPlan": {
      "Stripe": {
        "files": ["src/connectors/stripe/transformers.rs"],
        "patternToFollow": "Card implementation",
        "estimatedEffort": "2-3 hours"
      }
    }
  },
  "confirmedComplexity": "low" | "medium" | "high",
  "readyForImplementation": boolean
}

Rules:
- If approved=true, implementationPlan.approach is REQUIRED
- If approved=false, include clarifyingQuestions
- Use references from Requirements Discovery
- confirmedComplexity must match or override task.estimatedComplexity
- readyForImplementation=true only if score >= 7 and no major blockers`;

export const productAlignmentCheckpoint: Checkpoint = {
  id: "product_alignment",
  name: "Product alignment",
  description: "PM review with clarifying-question loop.",
  retryFrom: "product_alignment",
  maxRetries: 2,
  timeout: 24 * 60 * 60 * 1000,
  async run(ctx) {
    const task = ctx.artifacts.task;
    if (!task) return { passed: false, errors: ["Missing task artifact"] };

    // Early-return if a prior approved review is already saved.
    const saved = ctx.artifacts.productAlignment;
    if (saved && saved.approved && !(saved as any).pendingQuestions) {
      ctx.log(
        `Re-using prior PM approval from saved state.`,
        "info"
      );
      return { passed: true, artifacts: { productAlignment: saved } };
    }

    // Hydrate prior clarifying-question rounds from saved artifacts so that
    // resumed runs don't loop on the same questions.
    const priorAnswers: Array<{
      questions: string[];
      answers: Record<string, string>;
    }> = Array.isArray(
      (ctx.artifacts as Record<string, unknown>).pmClarifications
    )
      ? ((ctx.artifacts as Record<string, unknown>).pmClarifications as Array<{
          questions: string[];
          answers: Record<string, string>;
        }>)
      : [];

    if (priorAnswers.length > 0) {
      ctx.log(
        `Resuming with ${priorAnswers.length} prior clarification round(s) — PM will see them as context`,
        "info"
      );
    }

    // Up to 3 rounds of clarifying questions in a single run()
    for (let round = 0; round < 3; round++) {
      let raw = "";
      try {
        raw = await callLlm({
          system: SYSTEM,
          user: JSON.stringify({ task, priorClarifications: priorAnswers }, null, 2),
          label: `product_alignment:round${round + 1}`,
        });
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        ctx.log(`LLM call failed: ${msg}`, "error");
        return {
          passed: false,
          errors: [`PM review LLM call failed: ${msg}`],
        };
      }

      const parsed = safeParseJson<PmReviewResult>(raw);
      if (!parsed || typeof parsed.approved !== "boolean") {
        ctx.log(`PM returned non-JSON. Raw: ${raw.slice(0, 200)}`, "error");
        return {
          passed: false,
          errors: [`PM returned non-JSON response: ${raw.slice(0, 300)}`],
        };
      }

      const hasQuestions =
        Array.isArray(parsed.clarifyingQuestions) &&
        parsed.clarifyingQuestions.length > 0;

      // Auto mode → reviewer agent answers the questions, no human pause.
      if (hasQuestions && ctx.options.autoMode) {
        ctx.log(
          `Auto-mode: reviewer agent answering ${parsed.clarifyingQuestions!.length} clarifying question(s)`,
          "info"
        );
        try {
          const auto = await autoAnswerClarifyingQuestions(
            ctx,
            parsed.clarifyingQuestions!
          );
          priorAnswers.push({
            questions: parsed.clarifyingQuestions!,
            answers: auto,
          });
          (ctx.artifacts as Record<string, unknown>).pmClarifications = priorAnswers;
          continue;
        } catch (err) {
          ctx.log(
            `Auto-reviewer failed: ${err instanceof Error ? err.message : String(err)}`,
            "warn"
          );
          // Fall through to UI mode below.
        }
      }

      // UI mode + questions → pause and collect answers, then loop.
      if (hasQuestions && ctx.options.taskFromUi && ctx.bus) {
        ctx.log(
          `PM has ${parsed.clarifyingQuestions!.length} clarifying question(s) — awaiting answers`,
          "warn"
        );
        ctx.bus.emitHumanWaiting("product_alignment", {
          notes: parsed.notes,
          questions: parsed.clarifyingQuestions,
        });
        ctx.bus.emit("artifact:update", "product_alignment", {
          artifacts: {
            productAlignment: {
              approved: false,
              notes: parsed.notes,
              pendingQuestions: parsed.clarifyingQuestions,
            },
          },
        });

        // Inner loop: keep asking until all questions have an answer or an attachment.
        let answers: Record<string, string> | null = null;
        while (!answers) {
          const response = await ctx.bus.waitFor<{
            answers: Record<string, string>;
            attachments?: Record<string, Attachment[]>;
          }>("human:product_alignment");

          // Persist any image attachments to disk.
          const savedPaths = response.attachments
            ? await saveAttachments(ctx, response.attachments)
            : {};

          const collected: Record<string, string> = {};
          let missing: string | null = null;
          for (const q of parsed.clarifyingQuestions!) {
            const textPart = (response.answers?.[q] ?? "").trim();
            const attPart = savedPaths[q] ?? [];
            if (!textPart && attPart.length === 0) {
              missing = q;
              break;
            }
            // Compose the answer the LLM will see: text + attached image paths.
            const attachmentBlock =
              attPart.length > 0
                ? "\n\nAttached image file paths:\n" +
                  attPart.map((p) => `- ${p}`).join("\n")
                : "";
            collected[q] = (textPart || "(image attached)") + attachmentBlock;
          }
          if (missing) {
            ctx.bus.emit("human:rejected", "product_alignment", {
              reason: `All questions need a text answer or an attached image. Missing: "${missing}"`,
            });
            continue;
          }
          answers = collected;
        }

        priorAnswers.push({ questions: parsed.clarifyingQuestions!, answers });
        (ctx.artifacts as Record<string, unknown>).pmClarifications = priorAnswers;
        ctx.bus.emit("human:resolved", "product_alignment", { decision: "answered" });
        ctx.log(
          `Clarifying answers received — re-running PM review`,
          "info"
        );
        // Loop back to top — ask the PM again with new answers in context.
        continue;
      }

      // No questions (or no UI) → finalize and pass.
      if (!parsed.approved) {
        ctx.log(
          `⚠ PM concerns (non-blocking): ${parsed.notes.slice(0, 200)}`,
          "warn"
        );
      } else {
        ctx.log(`✓ Approved`, "success");
      }
      return {
        passed: true,
        artifacts: {
          productAlignment: {
            approved: parsed.approved,
            notes: parsed.notes,
            adjustedCriteria: parsed.adjustedCriteria,
            references: parsed.references,
            implementationPlan: parsed.implementationPlan,
            confirmedComplexity: parsed.confirmedComplexity,
            readyForImplementation: parsed.readyForImplementation,
          } as ProductAlignmentDoc,
        },
      };
    }

    // Safety — passed max rounds without convergence. Proceed anyway.
    ctx.log(
      "Max clarification rounds reached — proceeding with last known state.",
      "warn"
    );
    return {
      passed: true,
      artifacts: {
        productAlignment: {
          approved: true,
          notes: "Proceeded after 3 rounds of clarification without PM final approval.",
          implementationPlan: {
            approach: "Proceed with implementation based on available information from task definition.",
          },
        } as ProductAlignmentDoc,
      },
    };
  },
};
