import { runAI } from "@byne/core";
import type { ParityConfig } from "../config.js";
import { UNDERSTAND_SYSTEM, buildUnderstandUser } from "../prompts/understand.js";
import type { Leaf, Locus, UnderstandingResult } from "../types.js";

interface ParseResult extends UnderstandingResult {}

export function parseUnderstandingSummary(text: string): ParseResult {
  if (text.includes("## Need Info")) {
    const m = text.match(/## Need Info\s*([\s\S]+)/);
    return {
      ok: false,
      escalation: { blocker: "Agent reported insufficient info", tried: "ran UNDERSTAND once", question: m?.[1]?.trim().slice(0, 800) ?? "see Need Info block" },
    };
  }
  const summaryMatch = text.match(/(## Understanding Summary[\s\S]+?)(?=\n## (?!#)|$)/);
  if (!summaryMatch) {
    return { ok: false, escalation: { blocker: "No Understanding Summary block found", tried: "regex on agent output", question: "Why did the agent not emit the required summary?" } };
  }
  const md = summaryMatch[1];

  const confidenceMatch = md.match(/Understanding Confidence:\s*(HIGH|MEDIUM|LOW)/i);
  const confidence = (confidenceMatch?.[1].toUpperCase() as "HIGH" | "MEDIUM" | "LOW" | undefined) ?? "LOW";

  const declared = md.match(/Edit target repo:\s*(prism|hyperswitch-bridge|none)/);
  const declaredTarget = declared && declared[1] !== "none" ? (declared[1] as "prism" | "hyperswitch-bridge") : undefined;

  const locusPatterns: { re: RegExp; locus: Exclude<Locus, null> }[] = [
    { re: /-\s*\[x\]\s*Prism transformer/i, locus: "prism-transformer" },
    { re: /-\s*\[x\]\s*Hyperswitch UCS bridge/i, locus: "hs-bridge" },
    { re: /-\s*\[x\]\s*Hyperswitch connector/i, locus: "hs-connector" },
    { re: /-\s*\[x\]\s*Ambiguous/i, locus: "ambiguous" },
  ];
  const matched = locusPatterns.filter((p) => p.re.test(md)).map((p) => p.locus);
  if (matched.length === 0) {
    return { ok: false, markdown: md, escalation: { blocker: "No locus checkbox ticked", tried: "regex against Root-cause Locus section", question: "Which locus is the root cause?" } };
  }
  if (matched.length > 1) {
    return { ok: false, markdown: md, escalation: { blocker: "Multiple locus checkboxes ticked", tried: "regex against Root-cause Locus section", question: "Pick exactly one locus." } };
  }
  if (confidence !== "HIGH") {
    return { ok: false, markdown: md, escalation: { blocker: `Confidence ${confidence}, need HIGH`, tried: "single understand pass", question: "What specific facts would raise this to HIGH?" } };
  }
  return { ok: true, markdown: md, locus: matched[0], confidence, declaredTarget };
}

export async function runUnderstand(opts: { cfg: ParityConfig; leaf: Leaf; sessionId?: string }): Promise<UnderstandingResult & { sessionId?: string }> {
  const ai = await runAI({
    system: UNDERSTAND_SYSTEM,
    user: buildUnderstandUser(opts.leaf, opts.cfg),
    label: `parity/understand/${opts.leaf.number}`,
    model: opts.cfg.llm.understandModel || undefined,
    sessionId: opts.sessionId,
  } as any);
  const text = typeof (ai as any).output === "string" ? (ai as any).output : JSON.stringify((ai as any).output ?? "");
  const parsed = parseUnderstandingSummary(text);
  return { ...parsed, sessionId: (ai as any).sessionId };
}
