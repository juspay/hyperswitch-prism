import { StateManager, ALL_CHECKPOINTS } from "@10xgrace/core";

const COLOR: Record<string, string> = {
  idle: "\x1b[90m",
  running: "\x1b[36m",
  passed: "\x1b[32m",
  failed: "\x1b[31m",
  skipped: "\x1b[33m",
};

export async function statusCommand(runId?: string): Promise<void> {
  const state = new StateManager();
  let id = runId;
  if (!id) {
    const runs = await state.listRuns();
    if (runs.length === 0) {
      // eslint-disable-next-line no-console
      console.log("No runs found.");
      return;
    }
    id = runs[0]!.runId;
  }
  const saved = await state.load(id);
  if (!saved) {
    // eslint-disable-next-line no-console
    console.log(`No run: ${id}`);
    return;
  }
  // eslint-disable-next-line no-console
  console.log(`\x1b[1mRun: ${saved.runId}\x1b[0m — ${saved.task.title || "(no title)"}`);
  // eslint-disable-next-line no-console
  console.log("───────────────────────────────────────────────");
  for (const cp of ALL_CHECKPOINTS) {
    const st = saved.checkpointStates[cp.id] ?? "idle";
    const color = COLOR[st] ?? "";
    const retries = saved.retryCount[cp.id] ?? 0;
    // eslint-disable-next-line no-console
    console.log(
      `  ${color}${st.padEnd(8)}\x1b[0m  ${cp.id.padEnd(20)}  ${cp.name}${retries ? `  (retries: ${retries})` : ""}`
    );
  }
  state.close();
}
