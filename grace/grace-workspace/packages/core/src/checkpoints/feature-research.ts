import type { Checkpoint, FeatureResearchReport } from "../types.js";
import { runAI } from "../tools/runner-factory.js";

// ─── Agent 1: Existing Structure ──────────────────────────────────────
// Explores the target repo and returns a markdown report of what's already
// there: screens, components, hooks, types, tests, naming conventions.

const EXISTING_STRUCTURE_SYSTEM = `You are **Agent 1 — Existing Structure Scout**.

Your sole job: explore the target repo and report back WHAT ALREADY EXISTS that is relevant to the task. You are NOT planning, NOT recommending, NOT writing code. Just mapping what's on disk.

Use your tools aggressively:
- glob / list_dir to find relevant directories and files
- read_file to inspect the most important ones (top-level types, component signatures, hook exports)
- grep to find usages, imports, and patterns

Produce a THOROUGH markdown report covering:

# Existing Structure Report

## Owning screen / area
<Which screen or module in the repo is the natural home for this feature? Give the exact path.>

## Existing files to extend (MOST IMPORTANT)
For each relevant file:
- **Path**: exact relative path
- **What it does**: 1–2 sentences
- **Key exports**: the top-level let-bindings, types, or components it exports
- **Why relevant**: how it relates to the task

## Existing types / records
- Name, path, and field list for each type the new feature could reuse

## Existing hooks / API surfaces
- Hook or function name, path, what endpoint it hits, return type

## Existing tests
- Path and what each test covers

## Conventions observed
- Import style (open X vs module Y = Z)
- Component patterns (functional? hooks? state management?)
- File naming conventions

Be exhaustive. 10–20 findings is typical. Every path must be real — you verified it with your tools.`;

// ─── Agent 2: Ideal Flow (Web Research) ───────────────────────────────
// Searches the web for how similar features are built in the real world.

const IDEAL_FLOW_SYSTEM = `You are **Agent 2 — Ideal Flow Researcher**.

Your sole job: use websearch and webfetch to research how this kind of feature is typically implemented in production products, open-source libraries, and industry best practices. You are NOT reading the repo. You are NOT planning for this specific project. Just gathering external evidence.

Use your tools:
- websearch to find articles, docs, open-source implementations
- webfetch to read the most promising URLs in detail

Produce a markdown report covering:

# Ideal Flow Report

## How this feature typically works
<3–5 sentence summary of the standard approach across the industry>

## Common UI patterns
- Pattern name — what it looks like — where it's used (cite product or library)

## Recommended data flow / state machine
- What states the feature goes through (e.g. idle → loading → submitted → confirmed)
- Where state typically lives (local, context, URL, server)

## Reference implementations
For each:
- **Source**: product/library name + URL
- **What they do well**: 1 sentence
- **What to avoid**: 1 sentence (if applicable)

## Edge cases people commonly miss
- List of gotchas, error states, race conditions that show up in production

## Accessibility & UX best practices
- Keyboard navigation, screen reader, mobile considerations

## Sources
- Every URL you actually fetched, one per line

6–12 substantive findings is the right size. If the topic is too niche, say so honestly.`;

// ─── Agent 3: Final Decision ──────────────────────────────────────────
// Runs AFTER agents 1+2. Receives both reports and produces the decision.

const FINAL_DECISION_SYSTEM = `You are **Agent 3 — Decision Maker**.

You receive:
1. The original task (title, description, acceptance criteria)
2. **Agent 1's report**: what already exists in the repo
3. **Agent 2's report**: how this feature is typically built in the industry

Your job: produce the FINAL DECISION on what to build and how. This decision is consumed directly by the L2 planning stage, so it must be concrete and actionable.

Rules:
- Every claim must trace back to either Agent 1 (repo) or Agent 2 (web). Cite which.
- STRONGLY PREFER extending existing files over creating new ones. If Agent 1 found a relevant screen, the default answer is "add it there."
- When Agent 1 and Agent 2 conflict (e.g. web says "use a separate hooks file" but repo has hooks inline), side with the repo's existing conventions. Explain why.
- Be specific about file paths. "Modify src/screens/OrchestrationV1/ProductionIntentForm.res" not "modify the form component."

Return ONLY valid JSON:

{
  "finalDecision": "3–5 sentence summary of what to build and the recommended approach. Reference both agents.",
  "actionItems": [
    "Extend <exact path> to add <specific thing> — this is the natural home because <Agent 1 finding>",
    "Follow the <pattern name> pattern from <Agent 2 reference> for the state management",
    "Reuse existing <type/hook name> at <path> instead of creating a new one",
    "..."
  ]
}

5–10 action items is the right size. Each must name a real file path (from Agent 1) or a real pattern (from Agent 2).`;

export const featureResearchCheckpoint: Checkpoint = {
  id: "feature_research",
  name: "Feature research",
  description:
    "Three agents: (1) existing repo structure, (2) ideal flow from web research, (3) final decision synthesizing both.",
  retryFrom: "feature_research",
  timeout: 30 * 60 * 1000,
  async run(ctx) {
    const task = ctx.artifacts.task;
    if (!task) {
      return { passed: false, errors: ["Missing task"] };
    }
    const productAlignment = ctx.artifacts.productAlignment;

    // ── Phase 1: run Agent 1 and Agent 2 in parallel ──────────────
    ctx.log(
      `[feature_research] Phase 1: spawning Agent 1 (existing structure) + Agent 2 (ideal flow) in parallel`,
      "info"
    );

    const taskPayload = {
      task: { title: task.title, description: task.description, acceptanceCriteria: task.acceptanceCriteria },
      productAlignment,
    };

    let existingStructure = "";
    let idealFlow = "";

    try {
      const [repoReport, webReport] = await Promise.all([
        runAI<string>({
          skillBody: EXISTING_STRUCTURE_SYSTEM,
          userPayload: {
            ...taskPayload,
            instructions: "Explore the repo thoroughly using your tools. Return the markdown report described in your system prompt.",
          },
          cwd: ctx.task.projectRoot,
          label: "feature_research:agent1-existing",
          rawText: true,
        }).then((r) => r.result),
        runAI<string>({
          skillBody: IDEAL_FLOW_SYSTEM,
          userPayload: {
            ...taskPayload,
            instructions: "Research the web using websearch + webfetch. Return the markdown report described in your system prompt.",
          },
          cwd: ctx.task.projectRoot,
          label: "feature_research:agent2-idealflow",
          rawText: true,
        }).then((r) => r.result),
      ]);
      existingStructure = String(repoReport ?? "").trim();
      idealFlow = String(webReport ?? "").trim();
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      ctx.log(`[feature_research] Phase 1 failed: ${msg}`, "error");
      return { passed: false, errors: [`Phase 1 (parallel agents) failed: ${msg}`] };
    }

    ctx.log(
      `[feature_research] Phase 1 done — Agent 1: ${existingStructure.length}ch · Agent 2: ${idealFlow.length}ch`,
      "success"
    );

    if (!existingStructure && !idealFlow) {
      return {
        passed: false,
        errors: ["Both agents returned empty — cannot proceed to final decision"],
      };
    }

    // ── Phase 2: run Agent 3 (final decision) after both are done ─
    ctx.log(
      `[feature_research] Phase 2: spawning Agent 3 (final decision) — depends on Agent 1 + Agent 2`,
      "info"
    );

    let finalDecision = "";
    let actionItems: string[] | undefined;

    try {
      const { result: decision } = await runAI<{
        finalDecision?: string;
        actionItems?: unknown;
      }>({
        skillBody: FINAL_DECISION_SYSTEM,
        userPayload: {
          task: { title: task.title, description: task.description, acceptanceCriteria: task.acceptanceCriteria },
          productAlignment,
          agent1Report_existingStructure: existingStructure,
          agent2Report_idealFlow: idealFlow,
          instructions:
            "Read both agent reports carefully. Produce the final decision JSON. Every action item must cite a real file path from Agent 1 or a real pattern from Agent 2.",
        },
        cwd: ctx.task.projectRoot,
        label: "feature_research:agent3-decision",
      });

      finalDecision = typeof decision?.finalDecision === "string" ? decision.finalDecision.trim() : "";
      actionItems = Array.isArray(decision?.actionItems)
        ? (decision.actionItems as unknown[])
            .map((r) => (typeof r === "string" ? r.trim() : ""))
            .filter((r) => r.length > 0)
        : undefined;
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      ctx.log(`[feature_research] Phase 2 (Agent 3) failed: ${msg}`, "error");
      return { passed: false, errors: [`Agent 3 (final decision) failed: ${msg}`] };
    }

    if (!finalDecision) {
      return {
        passed: false,
        errors: [`Agent 3 returned no finalDecision`],
      };
    }

    const featureResearch: FeatureResearchReport = {
      existingStructure,
      idealFlow,
      finalDecision,
      actionItems,
    };

    ctx.log(
      `[feature_research] Done — decision: ${finalDecision.length}ch, ${actionItems?.length ?? 0} action items`,
      "success"
    );
    return { passed: true, artifacts: { featureResearch } };
  },
};
