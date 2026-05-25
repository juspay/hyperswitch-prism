import {
  type Checkpoint,
  type CheckpointId,
  type CheckpointResult,
  PipelineAbortError,
  type PipelineContext,
  SessionBusyError,
  type TaskDefinition,
} from "./types.js";
import type { StateManager } from "./state.js";
import { DEFAULT_SESSION_ID } from "./state.js";
import { withTimeout } from "./utils.js";
import type { PipelineEventBus } from "./logger.js";

export interface PipelineEngineOptions {
  /**
   * Optional callback to recompute the pipeline when the task's
   * `workflowType` changes mid-run (e.g. the user picked "Standard task"
   * at session-create time, then toggled to "Integrate a new connector"
   * inside the Task Definition form). Called after each checkpoint passes
   * — if it returns a list whose checkpoint ids differ from the current
   * `this.checkpoints`, the engine swaps to the new list and re-emits a
   * `pipeline:list` event so the sidebar updates.
   */
  rebuildPipeline?: (task: TaskDefinition) => Checkpoint[];
}

export class PipelineEngine {
  constructor(
    private checkpoints: Checkpoint[],
    private state: StateManager,
    private bus?: PipelineEventBus,
    private options: PipelineEngineOptions = {}
  ) {}

  async run(ctx: PipelineContext, startFrom?: CheckpointId): Promise<void> {
    const sessionId = ctx.sessionId ?? DEFAULT_SESSION_ID;
    if (!this.state.claimSession(sessionId, ctx.runId)) {
      const session = this.state.getSession(sessionId);
      throw new SessionBusyError(sessionId, session?.currentRunId ?? undefined);
    }
    this.state.markRunRunning(ctx.runId);

    // Default to "failed" so any throw releases the lock with a non-success
    // status. Successful completion explicitly bumps this to "succeeded".
    // User-initiated aborts come through run.ts's IPC path and are stamped
    // as "cancelled" before we ever return here, so this branch always
    // resolves to either succeeded or failed.
    let final: "succeeded" | "failed" | "cancelled" = "failed";

    // Periodic background heartbeat. Without this, UI-blocking checkpoints
    // (task, human-review, design-gate) stop emitting beats — they sit on
    // `bus.waitFor(...)` for minutes — and the supervisor's stale-lock
    // reaper would mistakenly mark the session as `error`. The Node event
    // loop will skip the timer if it's truly wedged, so this still
    // distinguishes a live-but-waiting engine from a stuck one.
    const heartbeatTimer = setInterval(() => {
      try {
        this.state.heartbeat(ctx.runId);
      } catch {
        /* swallow — we don't want a transient DB hiccup to crash the engine */
      }
    }, 15_000);
    heartbeatTimer.unref?.();

    // Tell the dashboard which checkpoints this run will actually execute.
    // The dashboard's PIPELINE constant is a superset across all workflows;
    // it uses this list to filter the sidebar / journey / phase tracker so
    // new-connector runs hide L2/L3 rows and add-payment-method runs hide
    // scaffold / per-flow rows.
    this.bus?.emit("pipeline:list", undefined, {
      checkpoints: this.checkpoints.map((c) => ({
        id: c.id,
        name: c.name,
      })),
    });

    try {
      await this.runCheckpoints(ctx, startFrom);
      final = "succeeded";
    } finally {
      clearInterval(heartbeatTimer);
      this.state.releaseSession(sessionId, ctx.runId, final);
    }
  }

  private async runCheckpoints(
    ctx: PipelineContext,
    startFrom?: CheckpointId
  ): Promise<void> {
    let i = startFrom
      ? this.checkpoints.findIndex((c) => c.id === startFrom)
      : 0;
    if (i === -1) throw new Error(`Unknown checkpoint: ${startFrom}`);

    while (i < this.checkpoints.length) {
      this.state.heartbeat(ctx.runId);
      const checkpoint = this.checkpoints[i]!;
      const retries = ctx.retryCount[checkpoint.id] ?? 0;
      const maxRetries = checkpoint.maxRetries ?? ctx.options.maxRetries ?? 3;

      if (retries >= maxRetries) {
        if (checkpoint.continueOnFailure) {
          // Graceful degradation: accept failure and continue to next checkpoint
          ctx.log(
            `[${checkpoint.id}] Max retries (${maxRetries}) exceeded. Continuing to next checkpoint (continueOnFailure=true).`,
            "warn"
          );
          await this.state.save(ctx, checkpoint.id, "failed");
          this.bus?.emitStatus(checkpoint.id, "failed");
          i++; // Continue to next checkpoint instead of aborting
          continue;
        }

        ctx.log(
          `[${checkpoint.id}] Max retries (${maxRetries}) exceeded. Pausing for manual retry.`,
          "error"
        );
        await this.state.save(ctx, checkpoint.id, "waiting_for_retry");
        this.bus?.emitStatus(checkpoint.id, "waiting_for_retry");
        this.bus?.emit("pipeline:waiting_for_retry", checkpoint.id, {
          maxRetries,
          lastError: "Max retries exceeded",
        });
        throw new PipelineAbortError(checkpoint.id, "Max retries exceeded - manual retry required");
      }

      ctx.log(`[${checkpoint.id}] Starting: ${checkpoint.name}`, "info");
      this.bus?.emitCheckpoint("checkpoint:start", checkpoint.id, { retries });
      this.bus?.emitStatus(checkpoint.id, "running");
      await this.state.save(ctx, checkpoint.id, "running");

      const startedAt = Date.now();
      let result: CheckpointResult;
      try {
        result = await withTimeout(
          checkpoint.run(ctx),
          checkpoint.timeout ?? 120_000,
          checkpoint.id
        );
      } catch (err) {
        result = {
          passed: false,
          errors: [err instanceof Error ? err.message : String(err)],
        };
      }

      // Always emit a single, unified artifact:update on completion — even
      // when the checkpoint produced no `result.artifacts`. The dashboard
      // uses this as the per-attempt history record (status + errors +
      // output, with artifacts when available); silently dropping the event
      // for failure-with-no-artifacts cases is what made retry pages empty.
      const emitAttemptUpdate = () => {
        this.bus?.emit("artifact:update", checkpoint.id, {
          artifacts: result.artifacts ?? {},
          retryAttempt: retries,
          status: result.passed ? "passed" : "failed",
          errors: result.errors ?? [],
          output: result.output ?? null,
        });
      };

      if (result.passed) {
        if (result.artifacts) {
          Object.assign(ctx.artifacts, result.artifacts);
        }
        emitAttemptUpdate();
        ctx.log(`[${checkpoint.id}] ✓ Passed`, "success");
        this.bus?.emitCheckpoint("checkpoint:pass", checkpoint.id);
        this.bus?.emitStatus(checkpoint.id, "passed");
        await this.state.save(ctx, checkpoint.id, "passed");
        await this.state.saveAttempt(
          ctx.runId,
          checkpoint.id,
          retries,
          "passed",
          null,
          result.output ?? null,
          result.artifacts ?? null,
          startedAt,
          Date.now()
        );

        // Reshape the pipeline if the user's workflowType pick (made via
        // the in-form Task-kind picker) demands a different sequence than
        // what we booted with. Typical trigger: session created with
        // workflowType=undefined (standard), user toggles to "Integrate a
        // new connector" inside the Task Definition form, submits — task
        // now carries workflowType="new-connector" and we need to swap
        // the standard L2/L3-bearing list for the scaffold + per-flow
        // subagent list.
        if (this.options.rebuildPipeline) {
          const next = this.options.rebuildPipeline(ctx.task);
          const nextIds = next.map((c) => c.id).join(",");
          const curIds = this.checkpoints.map((c) => c.id).join(",");
          if (nextIds !== curIds) {
            ctx.log(
              `[engine] pipeline reshape after ${checkpoint.id}: workflowType=${ctx.task.workflowType ?? "(unset)"} → ${next.length} checkpoints`,
              "info"
            );
            this.checkpoints = next;
            this.bus?.emit("pipeline:list", undefined, {
              checkpoints: next.map((c) => ({ id: c.id, name: c.name })),
            });
            // Find the just-passed checkpoint in the new list; advance to
            // the index after it. If the new pipeline doesn't contain the
            // passed checkpoint (unusual — e.g. preflight was renamed),
            // restart from the beginning and rely on saved-state lookups
            // to skip already-passed work.
            const passedIdx = next.findIndex((c) => c.id === checkpoint.id);
            i = passedIdx >= 0 ? passedIdx + 1 : 0;
            continue;
          }
        }

        i++;
      } else {
        ctx.log(
          `[${checkpoint.id}] ✕ Failed: ${result.errors?.join("; ") ?? "unknown"}`,
          "error"
        );
        this.bus?.emitCheckpoint("checkpoint:fail", checkpoint.id, {
          errors: result.errors,
        });
        this.bus?.emitStatus(checkpoint.id, "failed");

        // Preserve partial artifacts from the failed run (e.g. implementation
        // files that were written before the failure) so retries can resume.
        if (result.artifacts) {
          Object.assign(ctx.artifacts, result.artifacts);
        }
        emitAttemptUpdate();

        await this.state.save(ctx, checkpoint.id, "failed");
        await this.state.saveAttempt(
          ctx.runId,
          checkpoint.id,
          retries,
          "failed",
          result.errors ?? null,
          result.output ?? null,
          result.artifacts ?? null,
          startedAt,
          Date.now()
        );

        if (checkpoint.onFail) {
          ctx.log(`[${checkpoint.id}] Running failure handler...`, "info");
          try {
            await checkpoint.onFail(ctx, result);
          } catch (err) {
            ctx.log(
              `[${checkpoint.id}] onFail handler threw: ${err instanceof Error ? err.message : String(err)}`,
              "warn"
            );
          }
        }

        // Determine retry target - for grpc_test checkpoint with timeout errors,
        // retry only the grpc_test checkpoint itself instead of rolling back to implementation
        const isTimeoutError = result.errors?.some(
          (e) => e.includes("timed out") || e.includes("timeout")
        );
        const effectiveRetryFrom =
          checkpoint.id === "grpc_test" && isTimeoutError
            ? "grpc_test" // Timeout: retry only grpc_test, don't rebuild
            : checkpoint.retryFrom;

        const retryIdx = this.checkpoints.findIndex(
          (c) => c.id === effectiveRetryFrom
        );
        if (retryIdx === -1) {
          throw new PipelineAbortError(
            checkpoint.id,
            `Unknown retryFrom target: ${effectiveRetryFrom}`
          );
        }
        ctx.log(
          `[${checkpoint.id}] Rolling back to: ${effectiveRetryFrom} (retry ${retries + 1}/${maxRetries})`,
          "warn"
        );
        this.bus?.emitCheckpoint("checkpoint:retry", checkpoint.id, {
          rollbackTo: effectiveRetryFrom,
          attempt: retries + 1,
        });

        ctx.retryCount[checkpoint.id] = retries + 1;

        for (let j = retryIdx; j <= i; j++) {
          const cpId = this.checkpoints[j]!.id;
          if (cpId !== checkpoint.id) {
            await this.state.save(ctx, cpId, "idle");
            this.bus?.emitStatus(cpId, "idle");
          }
        }
        i = retryIdx;
      }
    }

    ctx.log("Pipeline complete. All checkpoints passed.", "success");
    this.bus?.emit("pipeline:complete");
  }
}
