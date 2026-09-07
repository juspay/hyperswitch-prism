import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { RESOURCE_URIS, LLMS_TXT_URL } from "../constants.js";
import {
  getConcept,
  listConcepts,
  listSkills,
  listRepoAreas,
  topDirs,
  listGlossary,
  META,
} from "../data/knowledge.js";
import { depthText } from "../tools/_shared.js";

function startMd(): string {
  const c = getConcept("what-is-prism");
  return (
    "# Start here: hyperswitch-prism\n\n" +
    (c ? `${c.depth.tldr}\n\n` : "") +
    "Use the `prism_learn_*` tools to explore the repo. Every answer is grounded in real files.\n\n" +
    "- `prism_learn_start_here` — orientation + a path for your role\n" +
    "- `prism_learn_explain_concept` — what is X? (depth=tldr for non-engineers)\n" +
    "- `prism_learn_architecture_overview` — the big picture\n" +
    "- `prism_learn_repo_map` / `prism_learn_search` — where is X?\n" +
    "- `prism_learn_how_to` — how do I do X?\n"
  );
}

function architectureMd(): string {
  const tour = ["what-is-prism", "three-layer-architecture", "unified-request", "connector", "transformer", "flow", "payment-proto", "status-codes", "grace"];
  const cards = tour.map((s) => getConcept(s)).filter((c): c is NonNullable<typeof c> => Boolean(c));
  return "# How hyperswitch-prism works\n\n" + cards.map((c) => `## ${c.title}\n${depthText(c, "standard")}`).join("\n\n");
}

function paymentsMd(): string {
  const c = getConcept("payments-101");
  if (!c) return "# Payments 101\n\n(not available)";
  return `# ${c.title}\n\n${c.one_liner}\n\n**Analogy:** ${c.analogy}\n\n${c.depth.deep ?? c.depth.standard ?? c.depth.tldr}`;
}

function glossaryMd(): string {
  return (
    "# Glossary\n\n" +
    listGlossary().map((t) => `- **${t.term}** — ${t.definition}`).join("\n")
  );
}

function repoMapMd(): string {
  return (
    "# Repo map\n\n## Top-level folders\n" +
    topDirs().map((d) => `- \`${d}/\``).join("\n") +
    "\n\n## Key locations\n" +
    listRepoAreas().map((a) => `- **${a.topic}** — \`${a.path}\` — ${a.what}`).join("\n")
  );
}

function skillsMd(): string {
  return (
    "# Skills (task playbooks)\n\n" +
    listSkills()
      .map((s) => `## ${s.name}\n\`${s.skillMdPath}\`\n\n${s.description}`)
      .join("\n\n")
  );
}

export function registerResources(server: McpServer): void {
  const md = (uri: URL, text: string) => ({ contents: [{ uri: uri.href, mimeType: "text/markdown", text }] });

  server.registerResource(
    "learn-start",
    RESOURCE_URIS.start,
    { title: "Start here", description: "Orientation for newcomers to hyperswitch-prism.", mimeType: "text/markdown" },
    async (uri) => md(uri, startMd()),
  );
  server.registerResource(
    "learn-architecture",
    RESOURCE_URIS.architecture,
    { title: "Architecture overview", description: "How Prism works, end to end, in plain language.", mimeType: "text/markdown" },
    async (uri) => md(uri, architectureMd()),
  );
  server.registerResource(
    "learn-glossary",
    RESOURCE_URIS.glossary,
    { title: "Glossary", description: `A-Z payment/repo terminology (${META.glossaryCount} terms).`, mimeType: "text/markdown" },
    async (uri) => md(uri, glossaryMd()),
  );
  server.registerResource(
    "learn-repo-map",
    RESOURCE_URIS.repoMap,
    { title: "Repo map", description: "Where everything lives in the repo.", mimeType: "text/markdown" },
    async (uri) => md(uri, repoMapMd()),
  );
  server.registerResource(
    "learn-skills-index",
    RESOURCE_URIS.skillsIndex,
    { title: "Skills index", description: "The .skills task playbooks and what each does.", mimeType: "text/markdown" },
    async (uri) => md(uri, skillsMd()),
  );
  server.registerResource(
    "learn-payments-101",
    RESOURCE_URIS.payments101,
    { title: "Payments 101", description: "How an online payment works — for non-engineers.", mimeType: "text/markdown" },
    async (uri) => md(uri, paymentsMd()),
  );
  server.registerResource(
    "learn-llms-txt",
    RESOURCE_URIS.llmsTxt,
    { title: "llms.txt (repo LLM index)", description: `The repo's own LLM navigation index, fetched from ${LLMS_TXT_URL}.`, mimeType: "text/plain" },
    async (uri) => {
      try {
        const res = await fetch(LLMS_TXT_URL);
        const text = await res.text();
        return { contents: [{ uri: uri.href, mimeType: "text/plain", text }] };
      } catch (err) {
        return {
          contents: [
            {
              uri: uri.href,
              mimeType: "text/plain",
              text: `Could not fetch llms.txt (${err instanceof Error ? err.message : String(err)}).\nSource: ${LLMS_TXT_URL}`,
            },
          ],
        };
      }
    },
  );
}
