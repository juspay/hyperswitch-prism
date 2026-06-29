import { runCommand } from "./run.js";
import type { CheckpointId } from "@10xgrace/core";

interface ReplayOpts {
  from: CheckpointId;
}

export async function replayCommand(runId: string, opts: ReplayOpts): Promise<void> {
  await runCommand({ resume: runId, startFrom: opts.from });
}
