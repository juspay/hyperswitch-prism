import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { troubleshootShape } from "../schemas.js";
import { findDiscrepancies } from "../data/knowledge.js";
import { result } from "../util.js";
import { cite, renderSources, type Citation } from "../citations.js";
import { ANNOTATIONS } from "./_shared.js";

interface Entry {
  id: string;
  keywords: string[];
  symptom: string;
  cause: string;
  fix: string;
  citations: { path: string; why: string }[];
}

// Curated, grounded symptom -> cause -> fix. Every citation is a real repo file.
const ENTRIES: Entry[] = [
  {
    id: "router-data-not-found",
    keywords: ["routerdata", "router data", "not found", "cannot find", "unresolved", "connectorintegration"],
    symptom: "`RouterData` / `ConnectorIntegration` not found, or unresolved import.",
    cause: "This codebase uses the V2 types only. `RouterData` and `ConnectorIntegration` (the old names) do not exist here.",
    fix: "Use `RouterDataV2` and `ConnectorIntegrationV2`, imported from `domain_types` (never `hyperswitch_domain_models`).",
    citations: [
      { path: ".skills/new-connector/SKILL.md", why: "the Critical Conventions: V2 types, domain_types imports" },
      { path: ".skills/_shared/references/quality-checklist.md", why: "these violations block review" },
    ],
  },
  {
    id: "wrong-import-crate",
    keywords: ["hyperswitch_domain_models", "import", "crate", "wrong import"],
    symptom: "Imports from `hyperswitch_domain_models` fail or are rejected.",
    cause: "That is the legacy crate name. UCS connectors import their types from `domain_types`.",
    fix: "Replace `hyperswitch_domain_models` imports with `domain_types`.",
    citations: [{ path: ".skills/new-connector/SKILL.md", why: "the import conventions" }],
  },
  {
    id: "status-is-a-number",
    keywords: ["status", "number", "numeric", "response.status", "enum", "charged", "8"],
    symptom: "`response.status` is a number, not a string — how do I read it?",
    cause: "Status comes from numeric proto enums (PaymentStatus / RefundStatus).",
    fix: "Compare against the named codes: AUTHORIZED=6, CHARGED=8, VOIDED=11, PENDING=20, FAILURE=21. Map a processor's strings to these via From/TryFrom.",
    citations: [
      { path: "docs/architecture/concepts/error-handling.md", why: "status vs error, with the codes" },
      { path: "integration-mcp/src/status/statusMap.ts", why: "the numeric status semantics in code" },
    ],
  },
  {
    id: "decline-not-exception",
    keywords: ["decline", "declined", "no exception", "failed", "failure", "error", "200"],
    symptom: "A declined payment did not throw — my code thought it succeeded.",
    cause: "A soft decline is a normal response with a FAILURE status (often HTTP 200), not a thrown error.",
    fix: "Always check `response.status` first. Thrown errors (IntegrationError/ConnectorError/NetworkError) mean the call could not complete; declines come back inside the response.",
    citations: [{ path: "docs/architecture/concepts/error-handling.md", why: "exceptions vs payment errors" }],
  },
  {
    id: "payment-method-dropped",
    keywords: ["payment method", "dropped", "unsupported", "not implemented", "wallet", "card", "match"],
    symptom: "A payment method is silently ignored / unsupported.",
    cause: "A catch-all `_` arm swallowed an unsupported PaymentMethodData variant.",
    fix: "Never silently drop a method. Return `IntegrationError::NotImplemented` with a message (e.g. via the unimplemented-payment-method helper), and add a real match arm to support it.",
    citations: [
      { path: ".skills/add-payment-method/SKILL.md", why: "the PaymentMethodData match-arm pattern" },
      { path: ".skills/_shared/references/quality-checklist.md", why: "no silent catch-alls" },
    ],
  },
  {
    id: "flow-not-wired",
    keywords: ["flow", "macro", "not wired", "missing", "prerequisites", "implementation", "trait"],
    symptom: "A flow I implemented is not being called / not wired up.",
    cause: "A flow must appear in BOTH macros: `create_all_prerequisites!` and `macro_connector_implementation!`.",
    fix: "Add the flow to both macros. Listing it in only one leaves it unwired.",
    citations: [{ path: ".skills/_shared/references/macro-reference.md", why: "both macros and what each does" }],
  },
  {
    id: "refund-needs-capture",
    keywords: ["refund", "capture", "dependency", "prerequisite", "order"],
    symptom: "Refund won't work / what does Refund depend on?",
    cause: "Refund depends on Authorize AND Capture — you can only refund money that was captured.",
    fix: "Implement Authorize and Capture before Refund. Note: one skill table lists only Authorize; the flow-dependencies graph (authoritative) lists Authorize + Capture.",
    citations: [{ path: ".skills/add-connector-flow/references/flow-dependencies.md", why: "the authoritative dependency graph" }],
  },
];

export function registerTroubleshoot(server: McpServer): void {
  server.registerTool(
    "prism_learn_troubleshoot",
    {
      title: "Troubleshoot a common newcomer problem",
      description:
        "Match a symptom ('RouterData not found', 'status is a number', 'decline didn't throw', 'payment method dropped', 'refund won't compile') to a grounded cause and fix, with real-file citations. Also surfaces known doc discrepancies.",
      inputSchema: troubleshootShape,
      annotations: ANNOTATIONS,
    },
    (args) => {
      const tokens = args.symptom.toLowerCase().split(/[^a-z0-9.]+/).filter((t) => t.length > 1);
      const scored = ENTRIES.map((e) => {
        let score = 0;
        for (const t of tokens) for (const k of e.keywords) if (k.includes(t) || t.includes(k)) score += 1;
        return { e, score };
      })
        .filter((x) => x.score > 0)
        .sort((a, b) => b.score - a.score)
        .slice(0, 3)
        .map((x) => x.e);

      const discs = findDiscrepancies(args.symptom);

      if (!scored.length && !discs.length) {
        return result(
          `No matching symptom in the troubleshooting set for "${args.symptom}". ` +
            `Try prism_learn_search for the error text, or prism_learn_explain_concept for the concept involved.`,
          { ok: true, symptom: args.symptom, matches: [], discrepancies: [] },
        );
      }

      const citations: Citation[] = [];
      const blocks = scored.map((e) => {
        for (const c of e.citations) if (!citations.some((x) => x.path === c.path)) citations.push(cite(c.path, c.why));
        return `## ${e.symptom}\n- **Cause:** ${e.cause}\n- **Fix:** ${e.fix}`;
      });

      let text = `# Troubleshooting: "${args.symptom}"\n\n${blocks.join("\n\n")}`;
      if (discs.length) {
        text += `\n\n## Related known discrepancy\n${discs.map((d) => `- **${d.title}** — ${d.resolution}`).join("\n")}`;
        for (const d of discs) for (const s of d.sources) if (!citations.some((x) => x.path === s.path)) citations.push(cite(s.path, "discrepancy source"));
      }
      text += renderSources(citations);

      return result(text, {
        ok: true,
        symptom: args.symptom,
        matches: scored.map((e) => ({ symptom: e.symptom, cause: e.cause, fix: e.fix, citations: e.citations })),
        discrepancies: discs,
        citations,
      });
    },
  );
}
