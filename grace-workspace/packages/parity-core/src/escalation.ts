import type { ParityConfig } from "./config.js";
import { LABELS, transition } from "./github/labels.js";
import { emit } from "./progress.js";

export interface EscalateOpts {
  cfg: ParityConfig;
  issue: number;
  step: "understand" | "plan" | "execute" | "validate" | "grpc-verify" | "handoff" | "sweep";
  blocker: string;
  tried: string;
  question: string;
  cc?: string;
  /** When true, print the escalation to stdout but do not apply parity:blocked or comment on GitHub. */
  dryRun?: boolean;
}

export async function escalate(opts: EscalateOpts): Promise<void> {
  const body = [
    `## Autopilot Escalation: ${opts.blocker.slice(0, 80)}`,
    "",
    "### Blocked At",
    `- Step: ${opts.step}`,
    `- Attempt: 1`,
    "",
    "### Specific Blocker",
    opts.blocker,
    "",
    "### What I Tried",
    opts.tried,
    "",
    "### Question",
    opts.question,
    "",
    opts.cc ? `cc @${opts.cc.replace(/^@/, "")}` : "",
  ].join("\n");

  emit({ phase: "escalate", status: "ok", step: opts.step, blocker: opts.blocker });

  if (opts.dryRun) {
    // eslint-disable-next-line no-console
    console.log(`[dry-run] escalation (would post + apply parity:blocked on #${opts.issue}):\n${body}`);
    return;
  }

  await transition({
    repo: `${opts.cfg.github.owner}/${opts.cfg.github.repo}`,
    issue: opts.issue,
    add: [LABELS.BLOCKED],
    comment: body,
  });
}
