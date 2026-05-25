import { callLlm, type ToolDef, type ToolHandler } from "../llm.js";
import { loadAgentSkill } from "./skill-loader.js";
import { createRepoTools } from "./repo.js";

/**
 * Factory that returns a `spawn_agent` tool the main LLM can call to
 * delegate focused exploration to a named subagent.
 *
 * On each call:
 *   1. Load the agent's skill markdown (e.g. skills/agents/file-locator.md)
 *      and use its body as the subagent's system prompt.
 *   2. Build a fresh set of repo tools scoped to the same project root.
 *   3. Run a nested callLlm with the subagent system prompt, the topic
 *      passed in by the parent, and a tool loop of its own.
 *   4. Return the subagent's final text (its report) back to the parent.
 *
 * The parent agent therefore only sees the condensed report — raw file
 * contents, grep hits, and directory listings stay inside the subagent's
 * private context window.
 */
export function createSpawnAgentTool(
  projectRoot: string,
  parentLabel: string,
  allowedAgents: string[]
): { tool: ToolDef; handler: ToolHandler } {
  const allowed = new Set(allowedAgents);

  const handler: ToolHandler = async (args) => {
    const agentName = String(args.agent ?? args.name ?? "").trim();
    const topic = String(args.topic ?? args.task ?? "").trim();
    const inputJson =
      typeof args.input === "object" && args.input !== null
        ? JSON.stringify(args.input, null, 2)
        : "";

    if (!agentName) {
      return `ERROR: 'agent' (subagent slug) is required`;
    }
    if (allowedAgents.length > 0 && !allowed.has(agentName)) {
      return `ERROR: agent "${agentName}" is not in the allowed set for this step. Allowed: ${allowedAgents.join(", ")}`;
    }
    if (!topic && !inputJson) {
      return `ERROR: provide a 'topic' string (what the subagent should investigate)`;
    }

    const skill = loadAgentSkill(agentName);
    if (!skill) {
      return `ERROR: no skill file found at packages/core/skills/agents/${agentName}.md`;
    }

    // Each subagent gets its own private repo tool set. No cross-talk.
    const { tools: repoTools, handlers: repoHandlers } =
      createRepoTools(projectRoot);

    const subagentSystem = skill.body;
    const subagentUser = JSON.stringify(
      {
        topic,
        input: args.input ?? null,
        instructions:
          "Use the read_file, list_dir, glob, and grep tools to explore the repo. Return your final answer as a single JSON object in the shape specified by your skill. Do not include prose outside the JSON.",
      },
      null,
      2
    );

    try {
      const report = await callLlm({
        system: subagentSystem,
        user: subagentUser,
        label: `${parentLabel}/@${agentName}`,
        tools: repoTools,
        toolHandlers: repoHandlers,
        maxToolSteps: 12,
        // Subagents produce compact reports — keep the ceiling modest.
        maxTokens: 4000,
      });
      return report.trim() || `ERROR: subagent @${agentName} returned empty output`;
    } catch (err) {
      return `ERROR: subagent @${agentName} failed: ${err instanceof Error ? err.message : String(err)}`;
    }
  };

  const tool: ToolDef = {
    type: "function",
    function: {
      name: "spawn_agent",
      description:
        `Delegate focused repo exploration to a named subagent. The subagent runs in its own conversation with its own tool loop, explores the repo, and returns a condensed report. Use this BEFORE writing your final output — your own context window is preserved since raw file contents live inside the subagent's conversation, not yours. Allowed agents for this step: ${allowedAgents.join(", ")}.`,
      parameters: {
        type: "object",
        properties: {
          agent: {
            type: "string",
            description: `The subagent slug. Must be one of: ${allowedAgents.join(", ")}`,
          },
          topic: {
            type: "string",
            description:
              "A short (1–2 sentence) description of what this subagent should investigate. Be specific — include the task nouns and the target modules.",
          },
          input: {
            type: "object",
            description:
              "Optional structured input (e.g. the work items, the L3 integration points). The subagent will receive this verbatim as extra context.",
          },
        },
        required: ["agent", "topic"],
      },
    },
  };

  return { tool, handler };
}
