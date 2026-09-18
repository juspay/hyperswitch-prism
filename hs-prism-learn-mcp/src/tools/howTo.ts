import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { howToShape } from "../schemas.js";
import { findSkillForTask, listSkills } from "../data/knowledge.js";
import { result, errorResult } from "../util.js";
import { cite, renderSources, type Citation } from "../citations.js";
import { ANNOTATIONS } from "./_shared.js";

export function registerHowTo(server: McpServer): void {
  server.registerTool(
    "prism_learn_how_to",
    {
      title: "How do I do X? — route a task to the right skill",
      description:
        "Map a task ('add a connector', 'add a refund flow', 'add a wallet', 'write a tech spec', 'review a PR') to the repo's matching .skills playbook, and summarize its steps and reference docs. The skill is what you actually run to do the work.",
      inputSchema: howToShape,
      annotations: ANNOTATIONS,
    },
    (args) => {
      const match = findSkillForTask(args.task);
      if (!match) {
        return errorResult(
          `I couldn't map "${args.task}" to a known skill. Available skills:\n` +
            listSkills().map((s) => `- **${s.name}** — ${s.description.slice(0, 120)}...`).join("\n"),
          { task: args.task, skills: listSkills().map((s) => s.name) },
        );
      }
      const s = match.skill;
      const citations: Citation[] = [cite(s.skillMdPath, `the ${s.name} playbook`)];
      for (const r of s.references.slice(0, 6)) citations.push(cite(r.path, "reference used by this skill"));

      const text =
        `# How to: ${args.task}\n\n` +
        `**Use the \`${s.name}\` skill.** ${s.description}\n\n` +
        `**Run it from:** \`${s.skillMdPath}\`\n` +
        (s.triggers.length ? `\n**Use it when:** ${s.triggers.join("; ")}\n` : "") +
        (s.references.length
          ? `\n**Reference docs it relies on:**\n${s.references.map((r) => `- \`${r.path}\``).join("\n")}\n`
          : "") +
        renderSources(citations);

      return result(text, {
        ok: true,
        task: args.task,
        skill: s.name,
        skillMdPath: s.skillMdPath,
        description: s.description,
        triggers: s.triggers,
        references: s.references,
        citations,
      });
    },
  );
}
