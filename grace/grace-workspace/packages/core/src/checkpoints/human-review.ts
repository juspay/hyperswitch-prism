import { promises as fs } from "node:fs";
import path from "node:path";
import { execa } from "execa";
import type {
  CheckpointId,
  CheckpointResult,
  HumanReviewDecision,
  PipelineContext,
  SpecReviewSession,
} from "../types.js";
import { ask, askChoice } from "../prompts/cli-prompts.js";
import { artifactsDir, ensureDir, nowIso } from "../utils.js";
import { autoReviewSpec } from "../agents/auto-reviewer.js";

export interface HumanReviewOptions<T> {
  checkpointId: CheckpointId;
  specType: "l2" | "l3" | "l4";
  loadSpec: (ctx: PipelineContext) => T | undefined;
  render: (spec: T) => void;
  validate: (spec: unknown) => spec is T;
  extraValidate?: (spec: T) => string | null;
  onEditSaved?: (ctx: PipelineContext, edited: T) => Promise<void>;
  regenerateKey: "l2RegeneratePrompt" | "l3RegeneratePrompt" | "l4RegeneratePrompt";
  previousKey: "previousL2" | "previousL3" | "previousL4";
  sessionKey: "l2Review" | "l3Review" | "l4Review";
  applySpec: (ctx: PipelineContext, spec: T) => void;
}

interface HumanReviewResponse<T> {
  decision: HumanReviewDecision;
  editedSpec?: T;
  regeneratePrompt?: string;
  notes?: string;
}

async function openInEditor(filePath: string): Promise<void> {
  const editor = process.env.EDITOR || process.env.VISUAL || "nano";
  try {
    await execa(editor, [filePath], { stdio: "inherit" });
  } catch {
    try {
      await execa("nano", [filePath], { stdio: "inherit" });
    } catch {
      await execa("vi", [filePath], { stdio: "inherit" });
    }
  }
}

export async function runHumanReview<T>(
  ctx: PipelineContext,
  opts: HumanReviewOptions<T>
): Promise<CheckpointResult> {
  const spec = opts.loadSpec(ctx);
  if (!spec) {
    return { passed: false, errors: [`Missing ${opts.specType} spec`] };
  }

  // Auto mode: a stand-in reviewer agent decides.
  if (ctx.options.autoMode) {
    try {
      const decision = await autoReviewSpec(ctx, opts.specType, spec);
      if (decision.decision === "approve") {
        const session: SpecReviewSession = {
          checkpointId: opts.checkpointId,
          specType: opts.specType,
          specSnapshot: spec,
          decision: "approve",
          reviewedAt: nowIso(),
          durationMs: 0,
          reviewerNotes: "auto-reviewer approved",
        };
        return {
          passed: true,
          artifacts: { [opts.sessionKey]: session } as Record<string, unknown>,
        };
      }
      // regenerate
      (ctx.artifacts as Record<string, unknown>)[opts.regenerateKey] =
        decision.regeneratePrompt ?? "Regenerate with more detail.";
      (ctx.artifacts as Record<string, unknown>)[opts.previousKey] = spec;
      const session: SpecReviewSession = {
        checkpointId: opts.checkpointId,
        specType: opts.specType,
        specSnapshot: spec,
        decision: "regenerate",
        reviewedAt: nowIso(),
        durationMs: 0,
        reviewerNotes: decision.regeneratePrompt,
      };
      (ctx.artifacts as Record<string, unknown>)[opts.sessionKey] = session;
      return {
        passed: false,
        errors: [`auto-reviewer requested regeneration: ${decision.regeneratePrompt}`],
      };
    } catch (err) {
      ctx.log(
        `auto-reviewer failed, falling back: ${err instanceof Error ? err.message : String(err)}`,
        "warn"
      );
    }
  }

  // Auto-approve (CI) path
  if (ctx.options.autoApproveReviews) {
    const session: SpecReviewSession = {
      checkpointId: opts.checkpointId,
      specType: opts.specType,
      specSnapshot: spec,
      decision: "approve",
      reviewedAt: nowIso(),
      durationMs: 0,
      reviewerNotes: "auto-approved via --auto-approve-reviews",
    };
    ctx.log(`[${opts.checkpointId}] auto-approved (CI flag)`, "success");
    return {
      passed: true,
      artifacts: { [opts.sessionKey]: session } as Record<string, unknown>,
    };
  }

  // UI mode — wait for human response over WebSocket
  if (ctx.options.taskFromUi && ctx.bus) {
    return runUiReview(ctx, opts, spec);
  }

  // Fallback: CLI prompts
  return runCliReview(ctx, opts, spec);
}

async function runUiReview<T>(
  ctx: PipelineContext,
  opts: HumanReviewOptions<T>,
  spec: T
): Promise<CheckpointResult> {
  const startedAt = Date.now();
  ctx.log(
    `[${opts.checkpointId}] ⏳ Awaiting human review from the dashboard — click the ${opts.specType.toUpperCase()} step to respond.`,
    "warn"
  );
  ctx.bus!.emitHumanWaiting(opts.checkpointId, spec);

  while (true) {
    const response = await ctx.bus!.waitFor<HumanReviewResponse<T>>(
      `human:${opts.checkpointId}`
    );

    if (response.decision === "approve") {
      const session: SpecReviewSession = {
        checkpointId: opts.checkpointId,
        specType: opts.specType,
        specSnapshot: spec,
        decision: "approve",
        reviewedAt: nowIso(),
        durationMs: Date.now() - startedAt,
        reviewerNotes: response.notes,
      };
      ctx.log(`[${opts.checkpointId}] ✓ Approved by reviewer`, "success");
      ctx.bus!.emit("human:resolved", opts.checkpointId, { decision: "approve" });
      return {
        passed: true,
        artifacts: { [opts.sessionKey]: session } as Record<string, unknown>,
      };
    }

    if (response.decision === "edit") {
      if (!response.editedSpec) {
        ctx.bus!.emit("human:rejected", opts.checkpointId, {
          reason: "Edit decision requires an editedSpec payload",
        });
        continue;
      }
      if (!opts.validate(response.editedSpec)) {
        ctx.bus!.emit("human:rejected", opts.checkpointId, {
          reason: `Edited ${opts.specType.toUpperCase()} spec failed schema validation. Required fields missing or empty. Check task ids and titles are non-empty, and at least one task exists.`,
        });
        continue;
      }
      if (opts.extraValidate) {
        const extra = opts.extraValidate(response.editedSpec);
        if (extra) {
          ctx.bus!.emit("human:rejected", opts.checkpointId, { reason: extra });
          continue;
        }
      }
      opts.applySpec(ctx, response.editedSpec);
      if (opts.onEditSaved) await opts.onEditSaved(ctx, response.editedSpec);
      const session: SpecReviewSession = {
        checkpointId: opts.checkpointId,
        specType: opts.specType,
        specSnapshot: response.editedSpec,
        decision: "edit",
        reviewedAt: nowIso(),
        durationMs: Date.now() - startedAt,
        reviewerNotes: response.notes,
      };
      ctx.log(
        `[${opts.checkpointId}] ✓ Edited spec accepted as source of truth`,
        "success"
      );
      ctx.bus!.emit("human:resolved", opts.checkpointId, { decision: "edit" });
      return {
        passed: true,
        artifacts: { [opts.sessionKey]: session } as Record<string, unknown>,
      };
    }

    if (response.decision === "regenerate") {
      const guidance = response.regeneratePrompt?.trim() ?? "";
      if (guidance.length < 5) {
        ctx.bus!.emit("human:rejected", opts.checkpointId, {
          reason: "Regenerate guidance must be at least 5 characters",
        });
        continue;
      }
      (ctx.artifacts as Record<string, unknown>)[opts.regenerateKey] = guidance;
      (ctx.artifacts as Record<string, unknown>)[opts.previousKey] = spec;
      const session: SpecReviewSession = {
        checkpointId: opts.checkpointId,
        specType: opts.specType,
        specSnapshot: spec,
        decision: "regenerate",
        reviewedAt: nowIso(),
        durationMs: Date.now() - startedAt,
        reviewerNotes: guidance,
      };
      (ctx.artifacts as Record<string, unknown>)[opts.sessionKey] = session;
      ctx.log(
        `[${opts.checkpointId}] Reviewer requested regeneration: ${guidance}`,
        "warn"
      );
      ctx.bus!.emit("human:resolved", opts.checkpointId, {
        decision: "regenerate",
      });
      return {
        passed: false,
        errors: [`Reviewer requested regeneration: ${guidance}`],
      };
    }

    ctx.bus!.emit("human:rejected", opts.checkpointId, {
      reason: `Unknown decision: ${response.decision}`,
    });
  }
}

async function runCliReview<T>(
  ctx: PipelineContext,
  opts: HumanReviewOptions<T>,
  spec: T
): Promise<CheckpointResult> {
  opts.render(spec);
  ctx.log(
    `[${opts.checkpointId}] ══════════ ${opts.specType.toUpperCase()} SPEC REVIEW — Awaiting human input ══════════`,
    "warn"
  );
  const startedAt = Date.now();
  const decision = await askChoice<HumanReviewDecision>(
    `${opts.specType.toUpperCase()} Spec Review — choose an action:`,
    [
      { key: "approve", label: "✅  Approve — proceed to next checkpoint" },
      { key: "edit", label: "✏️   Edit — open spec in editor, then continue" },
      { key: "regenerate", label: "🔄  Regenerate — discard and re-generate with my guidance" },
    ]
  );

  if (decision === "approve") {
    const session: SpecReviewSession = {
      checkpointId: opts.checkpointId,
      specType: opts.specType,
      specSnapshot: spec,
      decision,
      reviewedAt: nowIso(),
      durationMs: Date.now() - startedAt,
    };
    return {
      passed: true,
      artifacts: { [opts.sessionKey]: session } as Record<string, unknown>,
    };
  }

  if (decision === "edit") {
    const dir = artifactsDir(ctx.task.projectRoot);
    await ensureDir(dir);
    const filePath = path.join(dir, `${opts.specType}-spec-${ctx.runId}.json`);
    await fs.writeFile(filePath, JSON.stringify(spec, null, 2), "utf-8");
    let edited: T | undefined;
    while (!edited) {
      await openInEditor(filePath);
      const raw = await fs.readFile(filePath, "utf-8");
      let parsed: unknown;
      try {
        parsed = JSON.parse(raw);
      } catch {
        await ask("Invalid JSON. Press Enter to re-open the editor...");
        continue;
      }
      if (!opts.validate(parsed)) {
        await ask("Schema validation failed. Press Enter to re-open...");
        continue;
      }
      if (opts.extraValidate) {
        const extra = opts.extraValidate(parsed as T);
        if (extra) {
          await ask(`${extra}. Press Enter to re-open...`);
          continue;
        }
      }
      edited = parsed as T;
    }
    const editorNotes = await ask("Any notes on your edits? (optional): ");
    opts.applySpec(ctx, edited);
    if (opts.onEditSaved) await opts.onEditSaved(ctx, edited);
    const session: SpecReviewSession = {
      checkpointId: opts.checkpointId,
      specType: opts.specType,
      specSnapshot: edited,
      decision,
      reviewedAt: nowIso(),
      durationMs: Date.now() - startedAt,
      reviewerNotes: editorNotes || undefined,
    };
    return {
      passed: true,
      artifacts: { [opts.sessionKey]: session } as Record<string, unknown>,
    };
  }

  // regenerate
  let guidance = "";
  while (guidance.length < 10) {
    guidance = await ask(
      "Describe what should change in the regenerated spec (min 10 chars): "
    );
  }
  (ctx.artifacts as Record<string, unknown>)[opts.regenerateKey] = guidance;
  (ctx.artifacts as Record<string, unknown>)[opts.previousKey] = spec;
  const session: SpecReviewSession = {
    checkpointId: opts.checkpointId,
    specType: opts.specType,
    specSnapshot: spec,
    decision,
    reviewedAt: nowIso(),
    durationMs: Date.now() - startedAt,
    reviewerNotes: guidance,
  };
  (ctx.artifacts as Record<string, unknown>)[opts.sessionKey] = session;
  return {
    passed: false,
    errors: [`Reviewer requested regeneration: ${guidance}`],
  };
}
