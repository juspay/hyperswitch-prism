import { StateManager } from "@10xgrace/core";

export async function historyCommand(): Promise<void> {
  const state = new StateManager();
  const runs = await state.listRuns();
  if (runs.length === 0) {
    // eslint-disable-next-line no-console
    console.log("No runs.");
    return;
  }
  // eslint-disable-next-line no-console
  console.log("Run ID".padEnd(46), "Last CP".padEnd(20), "Status".padEnd(10), "Title");
  for (const r of runs) {
    // eslint-disable-next-line no-console
    console.log(
      r.runId.padEnd(46),
      (r.lastCheckpoint ?? "-").padEnd(20),
      (r.lastStatus ?? "-").padEnd(10),
      r.title
    );
  }
  state.close();
}
