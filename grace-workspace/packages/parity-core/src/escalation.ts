import type { ParityConfig } from "./config.js";
import { LABELS, transition } from "./github/labels.js";

export interface EscalateOpts {
  cfg: ParityConfig;
  issue: number;
  step: "understand" | "plan" | "execute" | "validate" | "grpc-verify" | "handoff" | "sweep";
  blocker: string;
  tried: string;
  question: string;
  cc?: string;
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

  await transition({
    repo: `${opts.cfg.github.owner}/${opts.cfg.github.repo}`,
    issue: opts.issue,
    add: [LABELS.BLOCKED],
    comment: body,
  });
}
