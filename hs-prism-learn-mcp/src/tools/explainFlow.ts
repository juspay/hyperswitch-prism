import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { explainFlowShape } from "../schemas.js";
import { getConcept, findDiscrepancies, connectorsForFlow } from "../data/knowledge.js";
import { result, errorResult } from "../util.js";
import { cite, renderSources, type Citation } from "../citations.js";
import { ANNOTATIONS, depthText } from "./_shared.js";

// Core flows: slug + prerequisites (per .skills/.../flow-dependencies.md, the authoritative graph).
const CORE: Record<string, { slug: string; dependsOn: string[]; aliases: string[]; coverageQuery: string }> = {
  authorize: { slug: "flow-authorize", dependsOn: [], aliases: ["auth"], coverageQuery: "authorize" },
  capture: { slug: "flow-capture", dependsOn: ["authorize"], aliases: [], coverageQuery: "capture" },
  void: { slug: "flow-void", dependsOn: ["authorize"], aliases: ["cancel"], coverageQuery: "void" },
  refund: { slug: "flow-refund", dependsOn: ["authorize", "capture"], aliases: [], coverageQuery: "refund" },
  psync: { slug: "flow-psync", dependsOn: ["authorize"], aliases: ["payment sync", "paymentsync", "sync"], coverageQuery: "Pay.Get" },
  rsync: { slug: "flow-rsync", dependsOn: ["refund"], aliases: ["refund sync", "refundsync"], coverageQuery: "Refund.Get" },
};

function resolveFlow(q: string): string | undefined {
  const n = q.trim().toLowerCase();
  if (CORE[n]) return n;
  for (const [key, v] of Object.entries(CORE)) if (v.aliases.includes(n)) return key;
  for (const key of Object.keys(CORE)) if (n.includes(key)) return key;
  return undefined;
}

export function registerExplainFlow(server: McpServer): void {
  server.registerTool(
    "prism_learn_explain_flow",
    {
      title: "Explain a payment flow and its dependencies",
      description:
        "Explain a flow (authorize, capture, void, refund, psync, rsync) in plain language: what it does, which flows it depends on, where its implementation pattern lives, and how many connectors support it. Surfaces known doc discrepancies.",
      inputSchema: explainFlowShape,
      annotations: ANNOTATIONS,
    },
    (args) => {
      const key = resolveFlow(args.flow);
      if (!key) {
        return errorResult(
          `"${args.flow}" is not one of the six core flows I have a card for (authorize, capture, void, refund, psync, rsync). ` +
            `For advanced flows (webhooks, mandates, disputes), see the dependency graph or use prism_learn_search.`,
          { flow: args.flow, coreFlows: Object.keys(CORE) },
        );
      }
      const meta = CORE[key]!;
      const card = getConcept(meta.slug);
      const cov = connectorsForFlow(meta.coverageQuery);
      const supportCount = cov.reduce((max, c) => Math.max(max, c.connectors.length), 0);

      const citations: Citation[] = [
        cite(`.skills/_shared/references/flow-patterns/${key}.md`, "the implementation pattern for this flow"),
        cite(".skills/add-connector-flow/references/flow-dependencies.md", "the authoritative flow dependency graph"),
      ];

      // Surface a relevant known discrepancy (e.g. Refund prerequisites).
      const discs = findDiscrepancies(key).filter((d) => d.topic === "flows");

      const lines: string[] = [];
      lines.push(`# Flow: ${key}`);
      if (card) {
        lines.push("");
        lines.push(`**In one line:** ${card.one_liner}`);
        lines.push("");
        lines.push(`**Analogy:** ${card.analogy}`);
        lines.push("");
        lines.push(depthText(card, "standard"));
      }
      lines.push("");
      lines.push(`**Depends on:** ${meta.dependsOn.length ? meta.dependsOn.join(", ") : "(nothing — it is the root flow)"}`);
      if (supportCount > 0) lines.push(`**Connector support:** ~${supportCount} connectors implement this flow (see prism_learn_coverage).`);
      if (discs.length) {
        lines.push("");
        lines.push(`⚠️ **Heads up (known doc discrepancy):** ${discs[0]!.summary} ${discs[0]!.resolution}`);
      }

      return result(lines.join("\n") + renderSources(citations), {
        ok: true,
        flow: key,
        dependsOn: meta.dependsOn,
        patternDoc: `.skills/_shared/references/flow-patterns/${key}.md`,
        connectorSupportCount: supportCount,
        discrepancies: discs,
        citations,
      });
    },
  );
}
