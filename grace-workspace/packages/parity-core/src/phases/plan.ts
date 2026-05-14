import { runAI } from "@byne/core";
import type { ParityConfig } from "../config.js";
import { PLAN_SYSTEM, buildPlanUser } from "../prompts/plan.js";
import type { Leaf, PlanResult } from "../types.js";

export function parsePlan(text: string): PlanResult {
  if (text.includes("## Still Missing")) {
    const m = text.match(/## Still Missing\s*([\s\S]+)/);
    return {
      ok: false,
      escalation: { blocker: "Agent reported plan-time gaps", tried: "ran PLAN once", question: m?.[1]?.trim().slice(0, 800) ?? "see Still Missing" },
    };
  }
  const planMatch = text.match(/(## Implementation Plan[\s\S]+)/);
  if (!planMatch) {
    return { ok: false, escalation: { blocker: "No Implementation Plan block emitted", tried: "regex on agent output", question: "Why did the agent not emit the required plan?" } };
  }
  const md = planMatch[1];
  const confidence = md.match(/Plan Confidence:\s*100%/);
  if (!confidence) {
    return { ok: false, markdown: md, escalation: { blocker: "Plan confidence not 100%", tried: "regex on Plan Confidence line", question: "What's blocking 100% confidence?" } };
  }
  if (/\bTBD\b|\bTODO\b/.test(md) || /similar to/i.test(md)) {
    return { ok: false, markdown: md, escalation: { blocker: "Plan contains TBD/TODO/similar-to placeholder", tried: "scan for forbidden tokens", question: "Spell out every change explicitly." } };
  }

  const filesMatch = [...md.matchAll(/^- `([^`]+\.rs)`/gm)].map((m) => m[1]);
  return { ok: true, markdown: md, files: filesMatch };
}

export async function runPlan(opts: { cfg: ParityConfig; leaf: Leaf; understandMarkdown: string; sessionId?: string }): Promise<PlanResult & { sessionId?: string }> {
  const ai = await runAI({
    system: PLAN_SYSTEM,
    user: buildPlanUser(opts.leaf, opts.understandMarkdown),
    label: `parity/plan/${opts.leaf.number}`,
    model: opts.cfg.llm.planModel || undefined,
    sessionId: opts.sessionId,
  } as any);
  const text = typeof (ai as any).output === "string" ? (ai as any).output : JSON.stringify((ai as any).output ?? "");
  const parsed = parsePlan(text);
  return { ...parsed, sessionId: (ai as any).sessionId };
}
