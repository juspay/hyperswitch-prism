import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import {
  ALL_CHECKPOINTS,
  DEFAULT_SESSION_ID,
  PipelineEngine,
  PipelineEventBus,
  SessionBusyError,
  StateManager,
  loadConfig,
  newRunId,
  setConfig,
  checkAIHealth,
  type CheckpointId,
  type CheckpointStatus,
  type PipelineContext,
  type PipelineOptions,
  type TaskDefinition,
} from "@10xgrace/core";

interface RunOpts {
  taskFile?: string;
  startFrom?: CheckpointId;
  project?: string;
  dryRun?: boolean;
  maxRetries?: number;
  threshold?: number;
  dashboard?: boolean;
  autoApproveReviews?: boolean;
  reviewTimeout?: number;
  resume?: string;
  config?: string;
  taskFromUi?: boolean;
  autoMode?: boolean;
  /** Session this engine instance is bound to. Falls back to "default". */
  session?: string;
  /** Override config.yml wsPort. Used by the supervisor's per-session port allocator. */
  wsPort?: number;
}

/**
 * Which artifact keys each checkpoint produces. Used by the rewind path on
 * resume-from-step so re-running a stage doesn't see its own stale output
 * in the LLM context.
 */
const ARTIFACT_KEYS_BY_STAGE: Record<string, string[]> = {
  task: [],
  product_alignment: ["productAlignment", "pmClarifications"],
  feature_research: ["featureResearch"],
  design_gate: ["designGate"],
  // Phase 12: include per-phase Claude session ids so manual rewind forces a
  // fresh `claude --session-id <new>` call instead of resuming a stale
  // conversation. The on-disk jsonl files at ~/.claude/projects/<...>/<uuid>.jsonl
  // survive until the `10xgrace sessions prune` command reaps them.
  l2_gen: ["l2", "l2LinksSessionId", "l2TechspecSessionId"],
  // Phase 13: also clear l2GraceIssueUrl so a post-rewind re-approval files
  // a fresh issue at juspay/grace instead of being silently skipped by the
  // idempotency guard in maybeCreateGraceIssue.
  l2_review: ["l2Review", "l2RegeneratePrompt", "previousL2", "l2GraceIssueUrl"],
  l3_gen: ["l3", "l3SessionId"],
  l3_review: ["l3Review", "l3RegeneratePrompt", "previousL3"],
  l4_gen: ["l4"],
  l4_review: ["l4Review", "l4RegeneratePrompt", "previousL4"],
  implementation: [
    "implementation",
    "implementationFiles",
    "implementationSessionId",
  ],
  compiler: ["compiledFiles"],
  design_match: ["designDiff"],
  cypress: ["cypressReport"],
  playwright: ["playwrightReport"],
  // Phase 12: also clear grpc_test's persistent session id on rewind so a
  // do-over starts the test conversation fresh. grpc_test's stage id is
  // not in the existing keyed-by-stage scheme (it shares "implementation"
  // for the legacy rollback path), so we add a dedicated entry here.
  grpc_test: ["grpcTest", "grpcurlOutput", "grpcTestErrors", "grpcTestSessionId"],
  pr_review: ["prReview", "prReviewSessionId"],
  regression: ["regression"],
};

export async function runCommand(opts: RunOpts): Promise<void> {
  const cfg = loadConfig(opts.config);
  setConfig(cfg);

  if (opts.project) cfg.projectRoot = path.resolve(opts.project);
  // CLI --ws-port wins over config. The supervisor uses this to give each
  // child engine a distinct port from its allocation pool.
  if (typeof opts.wsPort === "number" && Number.isFinite(opts.wsPort)) {
    cfg.wsPort = opts.wsPort;
  }

  const state = new StateManager();

  // Make sure the default session exists and points at the configured
  // projectRoot. The schema migration plants a placeholder row; this fills
  // in the real path on every boot so config changes propagate.
  state.ensureDefaultSession(cfg.projectRoot);

  // Clean out any empty/abandoned runs from prior sessions.
  try {
    const pruned = await state.pruneEmptyRuns();
    if (pruned > 0) {
      // eslint-disable-next-line no-console
      console.log(`\x1b[90m[10xgrace] pruned ${pruned} empty run(s)\x1b[0m`);
    }
  } catch {
    /* ignore */
  }

  // Reap any session locks left over from a crashed prior boot. Without this,
  // a SIGKILL'd engine leaves `sessions.current_run_id` set and the very
  // next claim attempt would fail with SessionBusyError.
  //
  // Aggressive 5s cutoff is correct for Phase 1 single-engine: at boot, by
  // definition no other engine process is alive, so any lock visible here
  // belongs to a prior incarnation and is stale. Phase 3's process-per-
  // session supervisor will replace this with PID-based liveness checks.
  try {
    const cleared = state.recoverStaleSessions(5_000);
    if (cleared > 0) {
      // eslint-disable-next-line no-console
      console.log(
        `\x1b[33m[10xgrace] recovered ${cleared} stale session lock(s) from a prior crash\x1b[0m`
      );
    }
  } catch {
    /* ignore */
  }

  // node --watch handoff: check for a pending resume intent written by a prior
  // dashboard "resume from stage" click.
  const resumeFile = path.join(os.homedir(), ".10xgrace", "resume.json");
  let autoResume: { runId: string; startFrom?: CheckpointId } | undefined;
  try {
    if (fs.existsSync(resumeFile)) {
      const txt = fs.readFileSync(resumeFile, "utf-8");
      autoResume = JSON.parse(txt);
      fs.unlinkSync(resumeFile); // consume it
      // eslint-disable-next-line no-console
      console.log(
        `\x1b[35m[10xgrace] handoff file consumed: resume=${autoResume?.runId} startFrom=${autoResume?.startFrom ?? "(auto)"}\x1b[0m`
      );
    }
  } catch (err) {
    // eslint-disable-next-line no-console
    console.error(`[10xgrace] bad handoff file:`, err);
  }
  if (autoResume && !opts.resume) opts.resume = autoResume.runId;
  if (autoResume?.startFrom && !opts.startFrom)
    opts.startFrom = autoResume.startFrom;

  let runId: string;
  let task: TaskDefinition;
  let artifacts: Record<string, unknown> = {};
  let retryCount: Record<string, number> = {};

  if (opts.resume) {
    const preLoad = await state.load(opts.resume);
    if (!preLoad) {
      // eslint-disable-next-line no-console
      console.warn(
        `\x1b[33m[10xgrace] resume target ${opts.resume} not found — falling back to a fresh run\x1b[0m`
      );
      opts.resume = undefined;
      opts.startFrom = undefined;
    }
  }

  if (opts.resume) {
    const saved = await state.load(opts.resume);
    if (!saved) {
      throw new Error(`No saved run with id ${opts.resume}`);
    }
    runId = saved.runId;
    task = saved.task;
    artifacts = saved.artifacts;
    retryCount = saved.retryCount;
    // Ensure the task is also in the artifacts so the task checkpoint
    // doesn't re-prompt when resuming.
    if (!artifacts.task) artifacts.task = task;

    // eslint-disable-next-line no-console
    console.log(
      `\x1b[90m[10xgrace] loaded run ${runId} — saved checkpoint states:\x1b[0m`
    );
    for (const cp of ALL_CHECKPOINTS) {
      const st = saved.checkpointStates[cp.id] ?? "(none)";
      // eslint-disable-next-line no-console
      console.log(`\x1b[90m         ${cp.id.padEnd(20)} ${st}\x1b[0m`);
    }

    // Auto-compute resume point: first non-passed checkpoint in pipeline order.
    if (!opts.startFrom) {
      const firstUnfinished = ALL_CHECKPOINTS.find(
        (c) =>
          (saved.checkpointStates[c.id] as CheckpointStatus | undefined) !==
          "passed"
      );
      if (firstUnfinished) {
        opts.startFrom = firstUnfinished.id;
        // eslint-disable-next-line no-console
        console.log(
          `\x1b[35m[10xgrace] auto-resuming at "${firstUnfinished.id}" (first non-passed)\x1b[0m`
        );
      } else {
        // eslint-disable-next-line no-console
        console.log(
          `\x1b[32m[10xgrace] run was already complete — nothing to resume\x1b[0m`
        );
        return;
      }
    } else {
      // eslint-disable-next-line no-console
      console.log(
        `\x1b[35m[10xgrace] explicit resume at "${opts.startFrom}"\x1b[0m`
      );
    }

    // Resume semantics: "give it another go, fresh". Reset the retry counter
    // for the stage we're resuming at + every downstream stage, AND wipe the
    // stored artifacts those stages produced so the re-run doesn't see its
    // own stale output in context.
    if (opts.startFrom) {
      const startIdx = ALL_CHECKPOINTS.findIndex((c) => c.id === opts.startFrom);
      if (startIdx >= 0) {
        const clearedStages: CheckpointId[] = [];
        const clearedKeys: string[] = [];
        for (let i = startIdx; i < ALL_CHECKPOINTS.length; i++) {
          const cpId = ALL_CHECKPOINTS[i]!.id;
          clearedStages.push(cpId);
          retryCount[cpId] = 0;
          for (const key of ARTIFACT_KEYS_BY_STAGE[cpId] ?? []) {
            if (artifacts[key] !== undefined) {
              delete artifacts[key];
              clearedKeys.push(key);
            }
          }
        }
        // eslint-disable-next-line no-console
        console.log(
          `\x1b[35m[10xgrace] rewind: reset retry counters for ${clearedStages.length} stage(s) from "${opts.startFrom}"\x1b[0m`
        );
        if (clearedKeys.length > 0) {
          // eslint-disable-next-line no-console
          console.log(
            `\x1b[35m[10xgrace] rewind: cleared artifacts: ${clearedKeys.join(", ")}\x1b[0m`
          );
        }
        // Persist the rewound state so the DB reflects reality before engine.run
        await state.rewindRun(runId, artifacts, retryCount, clearedStages);
      }
    }
  } else {
    runId = newRunId();
    if (opts.taskFile) {
      const raw = fs.readFileSync(opts.taskFile, "utf-8");
      task = JSON.parse(raw) as TaskDefinition;
      if (!task.projectRoot) task.projectRoot = cfg.projectRoot;
      artifacts = { task };
    } else {
      task = {
        title: "",
        description: "",
        acceptanceCriteria: [],
        projectRoot: cfg.projectRoot,
      };
    }
  }

  // Phase 12: Override config runner with task-specified runner (from unified create modal)
  // This allows per-task runner selection from the dashboard
  if (task.runner) {
    cfg.runner = task.runner;
    // Also set model if specified in task
    if (task.runnerModel) {
      cfg.llm.model = task.runnerModel;
      if (task.runner === "claude-code") {
        cfg.claudeCode.model = task.runnerModel;
      } else if (task.runner === "opencode") {
        cfg.opencode.model = task.runnerModel;
      }
    }
    // eslint-disable-next-line no-console
    console.log(`[10xgrace] Using task-specified runner: ${cfg.runner}${task.runnerModel ? ` (${task.runnerModel})` : ''}`);
  }

  // Resolve the owning session. Precedence: CLI flag > task.sessionId > default.
  // The supervisor passes --session when spawning a child engine; standalone
  // engines fall back to whatever the resumed run says, or finally the default.
  const sessionId = opts.session ?? task.sessionId ?? DEFAULT_SESSION_ID;
  task.sessionId = sessionId;
  // `--session` is set exclusively by the supervisor when spawning a child.
  // Use that as the supervised-mode flag so runs:new / runs:resume route
  // through the stdout-marker respawn protocol instead of the legacy
  // resume.json + watch-bounce dance.
  const supervised = !!opts.session;
  // Override projectRoot from the session row so the engine always sees the
  // session-scoped path. For the default session this is the same as
  // cfg.projectRoot — but for any future session it's the per-session
  // worktree under ~/.10xgrace/sessions/<id>/<projectName>.
  const session = state.getSession(sessionId);
  if (session?.projectRoot) {
    task.projectRoot = session.projectRoot;
    // `task` and `artifacts.task` are deserialized from separate DB columns
    // in the resume block above, so they are distinct object refs. L3 reads
    // ctx.artifacts.task.projectRoot — without this propagation, writes
    // land in the original source repo instead of the session worktree.
    if (artifacts.task) {
      (artifacts.task as TaskDefinition).projectRoot = session.projectRoot;
    }
  }
  // Phase 10: derive per-session gRPC + dummy-connector ports from the
  // session's portSlot so concurrent sessions don't collide. Default
  // session is slot 0 → unshifted 8000/8080 (back-compat).
  if (session) {
    const grpcPort = 8000 + session.portSlot;
    const dummyConnectorPort = 8080 + session.portSlot;
    task.grpcPort = grpcPort;
    task.dummyConnectorPort = dummyConnectorPort;
    if (artifacts.task) {
      (artifacts.task as TaskDefinition).grpcPort = grpcPort;
      (artifacts.task as TaskDefinition).dummyConnectorPort = dummyConnectorPort;
    }
  }

  // Phase 5: bus is now a *client* that connects outbound to the supervisor's
  // control WS. Multiple engines share the same control port; the supervisor
  // routes messages by sessionId. Standalone (`pnpm engine`) still works
  // because it implicitly assumes a supervisor on cfg.wsPort — if none is
  // running, the bus simply drops events (process still functions).
  const controlWsUrl = `ws://localhost:${cfg.wsPort}`;
  const bus =
    opts.dashboard === false
      ? undefined
      : new PipelineEventBus(runId, controlWsUrl, sessionId);

  // Inbound dashboard commands: abort, list runs, resume a run.
  if (bus) {
    (
      bus as unknown as {
        onInbound: (h: (m: { type: string; payload?: any }) => void) => void;
      }
    ).onInbound(async (msg) => {
      if (msg.type === "pipeline:abort") {
        // eslint-disable-next-line no-console
        console.log(
          "\x1b[31m[10xgrace] pipeline:abort received from dashboard — releasing session lock and exiting\x1b[0m"
        );
        bus.emit("pipeline:abort", undefined, { error: "cancelled from dashboard" });
        // Release the session lock with a 'cancelled' terminal status so
        // the very next engine boot doesn't see an orphaned lock and refuse
        // to start a new run on this session. This is the half of the abort
        // path that the engine's own `finally` can't run — process.exit()
        // below would skip it.
        try {
          state.releaseSession(sessionId, ctx.runId, "cancelled");
        } catch (err) {
          // eslint-disable-next-line no-console
          console.error(`[10xgrace] abort: releaseSession failed:`, err);
        }
        try {
          const entry = process.argv[1];
          if (entry && fs.existsSync(entry)) {
            const now = new Date();
            fs.utimesSync(entry, now, now);
          }
        } catch {
          /* ignore */
        }
        setTimeout(() => process.exit(0), 150);
        return;
      }

      if (msg.type === "auto-mode:set") {
        const p = msg.payload as { enabled: boolean; agentName?: string };
        pipelineOptions.autoMode = !!p?.enabled;
        pipelineOptions.agentName = p?.agentName?.trim() || undefined;
        const label = pipelineOptions.agentName || "auto-reviewer";
        // eslint-disable-next-line no-console
        console.log(
          `\x1b[35m[10xgrace] auto-mode ${pipelineOptions.autoMode ? "ON" : "OFF"} (agent: ${label})\x1b[0m`
        );
        bus.emit("auto-mode:state", undefined, {
          enabled: pipelineOptions.autoMode,
          agentName: pipelineOptions.agentName,
        });
        return;
      }

      if (msg.type === "runs:list") {
        try {
          const runs = await state.listRuns();
          // Fetch per-checkpoint status for each so the UI can show a quick summary
          const withHistory = await Promise.all(
            runs.map(async (r) => ({
              ...r,
              checkpoints: await state.getCheckpointHistory(r.runId),
            }))
          );
          // eslint-disable-next-line no-console
          console.log(
            `\x1b[90m[10xgrace] runs:list → ${withHistory.length} run(s)\x1b[0m`
          );
          bus.emit("runs:list:response", undefined, { runs: withHistory });
        } catch (err) {
          bus.emit("runs:list:response", undefined, {
            runs: [],
            error: err instanceof Error ? err.message : String(err),
          });
        }
        return;
      }

      if (msg.type === "attempts:request") {
        // Dashboard sends this on connect (and after run-switch) to rehydrate
        // per-attempt retry history that the WS replay buffer doesn't preserve.
        const target = (msg.payload as { runId?: string })?.runId ?? runId;
        try {
          const attempts = await state.listAttempts(target);
          bus.emit("attempts:response", undefined, { runId: target, attempts });
        } catch (err) {
          bus.emit("attempts:response", undefined, {
            runId: target,
            attempts: [],
            error: err instanceof Error ? err.message : String(err),
          });
        }
        return;
      }

      if (msg.type === "runs:new") {
        // In supervised mode (Phase 3+), exit cleanly with a respawn marker
        // on stdout — the parent supervisor will spawn a fresh child for this
        // session. Standalone mode falls back to the legacy file-bounce so
        // `pnpm engine` still works without a supervisor.
        bus.emit("runs:new:ack", undefined, { ok: true });
        if (supervised) {
          // eslint-disable-next-line no-console
          console.log(
            `\x1b[35m[10xgrace] runs:new received — emitting respawn intent and exiting cleanly\x1b[0m`
          );
          await emitRespawnAndExit(state, sessionId, ctx.runId, {});
          return;
        }
        try {
          const resumePath = path.join(os.homedir(), ".10xgrace", "resume.json");
          if (fs.existsSync(resumePath)) fs.unlinkSync(resumePath);
          // eslint-disable-next-line no-console
          console.log(
            `\x1b[35m[10xgrace] runs:new received — restarting engine for a fresh run\x1b[0m`
          );
          try {
            const entry =
              process.argv[1] && fs.existsSync(process.argv[1])
                ? process.argv[1]
                : null;
            if (entry) {
              const content = fs.readFileSync(entry);
              fs.writeFileSync(entry, content);
            }
          } catch (err) {
            // eslint-disable-next-line no-console
            console.error(`[10xgrace] runs:new: failed to bounce entry file:`, err);
          }
          setTimeout(() => process.exit(0), 250);
        } catch (err) {
          bus.emit("runs:new:ack", undefined, {
            ok: false,
            error: err instanceof Error ? err.message : String(err),
          });
        }
        return;
      }

      if (msg.type === "runs:resume") {
        const target = msg.payload as { runId: string; startFrom?: CheckpointId };
        if (!target?.runId) return;

        if (supervised) {
          bus.emit("runs:resuming", undefined, target);
          // eslint-disable-next-line no-console
          console.log(
            `\x1b[35m[10xgrace] resume requested for ${target.runId}${target.startFrom ? ` from ${target.startFrom}` : ""} — emitting respawn intent and exiting\x1b[0m`
          );
          await emitRespawnAndExit(state, sessionId, ctx.runId, {
            runId: target.runId,
            startFrom: target.startFrom,
          });
          return;
        }

        try {
          fs.mkdirSync(path.join(os.homedir(), ".10xgrace"), { recursive: true });
          fs.writeFileSync(
            path.join(os.homedir(), ".10xgrace", "resume.json"),
            JSON.stringify(target),
            "utf-8"
          );
          bus.emit("runs:resuming", undefined, target);
          // eslint-disable-next-line no-console
          console.log(
            `\x1b[35m[10xgrace] resume requested for ${target.runId}${target.startFrom ? ` from ${target.startFrom}` : ""} — restarting engine\x1b[0m`
          );
          try {
            const entry =
              process.argv[1] && fs.existsSync(process.argv[1])
                ? process.argv[1]
                : null;
            if (entry) {
              const content = fs.readFileSync(entry);
              fs.writeFileSync(entry, content);
            }
          } catch (err) {
            // eslint-disable-next-line no-console
            console.error(`[10xgrace] resume: failed to bounce entry file:`, err);
          }
          setTimeout(() => process.exit(0), 250);
        } catch (err) {
          bus.emit("runs:resuming", undefined, {
            error: err instanceof Error ? err.message : String(err),
          });
        }
        return;
      }
    });
  }

  // Auto-enable UI task input when the dashboard is on and we're not in CI-auto mode.
  // (We intentionally keep this on for resume — the dashboard is still the input surface.)
  const autoTaskFromUi =
    opts.dashboard !== false && !opts.autoApproveReviews;

  const pipelineOptions: PipelineOptions = {
    autoApproveReviews: opts.autoApproveReviews,
    reviewTimeoutMs: opts.reviewTimeout,
    dryRun: opts.dryRun,
    maxRetries: opts.maxRetries ?? cfg.maxRetries,
    designMatchThreshold: opts.threshold ?? cfg.designMatchThreshold,
    devServerUrl: cfg.devServerUrl,
    regressionCommand: cfg.checkpoints.regression.command,
    dashboard: opts.dashboard !== false,
    model: cfg.llm.model,
    taskFromUi: opts.taskFromUi ?? autoTaskFromUi,
    autoMode: opts.autoMode,
  };

  const ctx: PipelineContext = {
    runId,
    sessionId,
    task,
    artifacts,
    retryCount,
    options: pipelineOptions,
    bus: bus
      ? {
          waitFor: (type, timeoutMs) => bus.waitFor(type, timeoutMs),
          emit: (type, cp, payload) => bus.emit(type, cp, payload),
          emitHumanWaiting: (cp, spec) => bus.emitHumanWaiting(cp, spec),
        }
      : undefined,
    log: (msg, level = "info") => {
      const cleaned = msg.replace(/^\[\w+\]\s*/, "");
      if (bus) bus.log(inferCheckpoint(msg), cleaned, level);
      else {
        const ts = new Date().toISOString();
        // eslint-disable-next-line no-console
        console.log(`${ts} ${level.toUpperCase()} ${msg}`);
      }
    },
  };

  // When resuming, replay saved checkpoint statuses and artifacts so the dashboard
  // rebuilds the sidebar state immediately. Use the in-memory `artifacts` (which
  // already has `task` merged in via line 92) rather than re-reading from the DB —
  // runs that never finished the `task` checkpoint won't have it in artifacts_json.
  if (opts.resume && bus) {
    const saved = await state.load(opts.resume);
    if (saved) {
      for (const [cpId, status] of Object.entries(saved.checkpointStates)) {
        bus.emit("checkpoint:status", cpId as CheckpointId, { status });
      }
      // Strip the placeholder task seeded by SessionSupervisor.startSession
      // from the replay payload — it has an empty title and would otherwise
      // pollute artifacts.task on every newly-connecting dashboard tab,
      // making `taskAlreadySubmitted` flicker truthy and hiding TaskForm.
      const taskTitle = (task as { title?: string }).title?.trim() ?? "";
      const replayArtifacts =
        taskTitle.length > 0
          ? artifacts
          : Object.fromEntries(
              Object.entries(artifacts).filter(([k]) => k !== "task")
            );
      bus.emit("artifact:update", undefined, { artifacts: replayArtifacts });
      // Push per-attempt history so the dashboard's RetryHistory renders
      // immediately on resume instead of waiting for an attempts:request.
      if (saved.attempts && saved.attempts.length > 0) {
        bus.emit("attempts:response", undefined, {
          runId: saved.runId,
          attempts: saved.attempts,
        });
      }
      // Only confirm acceptance for genuinely-populated tasks. The supervisor
      // seeds a placeholder run row before the engine boots, so an empty
      // title means "first boot, still waiting for UI submission".
      if (taskTitle.length > 0) {
        bus.emit("task:accepted", "task", { task });
      }
    }
  }

  // Broadcast initial auto-mode state so new dashboard connections see the toggle.
  if (bus) {
    bus.emit("auto-mode:state", undefined, {
      enabled: !!pipelineOptions.autoMode,
      agentName: pipelineOptions.agentName,
    });
  }

  // eslint-disable-next-line no-console
  console.log(`\x1b[1m\x1b[35m[10xgrace] runId=${runId}\x1b[0m`);
  // eslint-disable-next-line no-console
  console.log(`\x1b[90m[10xgrace] project=${task.projectRoot}\x1b[0m`);
  // eslint-disable-next-line no-console
  console.log(
    `\x1b[90m[10xgrace] taskFromUi=${pipelineOptions.taskFromUi} dashboard=${pipelineOptions.dashboard} llm=${cfg.llm.model}@${cfg.llm.baseUrl}\x1b[0m`
  );
  // eslint-disable-next-line no-console
  console.log(`\x1b[90m[10xgrace] built from ${new Date().toISOString()} runtime load\x1b[0m`);

  // Pre-flight: check AI runner connectivity
  const aiHealth = await checkAIHealth();
  const runnerLabel = aiHealth.runner === "claude-code" ? "Claude Code" : "OpenCode";
  if (aiHealth.connected) {
    // eslint-disable-next-line no-console
    console.log(
      `\x1b[32m[10xgrace] ✓ ${runnerLabel} connected (${aiHealth.connectionInfo}${aiHealth.latencyMs ? `, ${aiHealth.latencyMs}ms` : ""})\x1b[0m`
    );
  } else {
    // eslint-disable-next-line no-console
    console.log(
      `\x1b[31m[10xgrace] ✕ ${runnerLabel} NOT connected — ${aiHealth.error}\x1b[0m`
    );
    if (aiHealth.runner === "opencode") {
      // eslint-disable-next-line no-console
      console.log(
        `\x1b[33m[10xgrace]   Implementation steps may fail. Start opencode with: opencode serve\x1b[0m`
      );
    } else {
      // eslint-disable-next-line no-console
      console.log(
        `\x1b[33m[10xgrace]   Implementation steps may fail. Ensure 'claude' CLI is installed.\x1b[0m`
      );
    }
  }
  if (bus) {
    bus.emit("ai:health", undefined, aiHealth);
  }

  const engine = new PipelineEngine(ALL_CHECKPOINTS, state, bus);

  try {
    await engine.run(ctx, opts.startFrom);
  } catch (err) {
    if (err instanceof SessionBusyError) {
      // Another run holds the session lock. This shouldn't happen during
      // normal Phase 1 single-engine operation — it means a prior crash
      // left state behind that recoverStaleSessions didn't clear, OR a
      // future Phase 3 supervisor double-spawned. Surface the message
      // without polluting the log with a stack trace.
      // eslint-disable-next-line no-console
      console.error(`\x1b[31m[10xgrace] ${err.message}\x1b[0m`);
      bus?.emit("pipeline:abort", undefined, { error: err.message });
      process.exitCode = 2;
    } else {
      ctx.log(
        `Pipeline aborted: ${err instanceof Error ? err.message : String(err)}`,
        "error"
      );
      bus?.emit("pipeline:abort", undefined, {
        error: err instanceof Error ? err.message : String(err),
      });
      process.exitCode = 1;
    }
  } finally {
    bus?.close();
    state.close();
  }
}

function inferCheckpoint(msg: string): "pipeline" | CheckpointId {
  const m = msg.match(/^\[(\w+)\]/);
  if (!m) return "pipeline";
  return m[1] as CheckpointId;
}

/**
 * Tells the parent supervisor to respawn this engine with the given intent
 * by printing a recognized marker line on stdout, then releases the session
 * lock and exits cleanly.
 *
 * The marker MUST be the literal `__TENXGRACE_RESPAWN__ <json>` so the supervisor's
 * line-buffered stdout parser picks it up. The JSON payload accepts:
 *   - `runId?: string`   — re-use this run row (resume); supervisor skips enqueueRun
 *   - `startFrom?: string` — start the resumed run at this checkpoint id
 * An empty `{}` means "fresh run" (supervisor enqueues a new run).
 *
 * The lock release with status='cancelled' lets the next claim succeed even
 * if the supervisor's reaper hasn't ticked yet — we own the cancellation
 * decision here, so we record it deterministically rather than waiting for
 * heartbeat-based recovery.
 */
async function emitRespawnAndExit(
  state: StateManager,
  sessionId: string,
  runId: string,
  intent: { runId?: string; startFrom?: CheckpointId }
): Promise<void> {
  // eslint-disable-next-line no-console
  console.log(`__TENXGRACE_RESPAWN__ ${JSON.stringify(intent)}`);
  try {
    state.releaseSession(sessionId, runId, "cancelled");
  } catch {
    /* best-effort — supervisor reaper will catch leftovers */
  }
  // Small beat so the bus can flush any pending acks to the dashboard, then
  // exit cleanly. The supervisor sees the exit and respawns.
  setTimeout(() => process.exit(0), 100);
}
