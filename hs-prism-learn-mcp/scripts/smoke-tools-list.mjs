/**
 * Smoke test: spawn the built server over stdio, run initialize + tools/list,
 * assert all 14 learn tools + 7 resources are present, then round-trip a few
 * read-only calls and assert structuredContent.ok / expected grounding.
 */
import { spawn } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const entry = join(root, "dist", "index.js");

const EXPECTED_TOOLS = [
  "prism_learn_start_here",
  "prism_learn_architecture_overview",
  "prism_learn_explain_concept",
  "prism_learn_glossary",
  "prism_learn_repo_map",
  "prism_learn_search",
  "prism_learn_read_doc",
  "prism_learn_how_to",
  "prism_learn_explain_flow",
  "prism_learn_connector_walkthrough",
  "prism_learn_learning_path",
  "prism_learn_troubleshoot",
  "prism_learn_coverage",
  "prism_learn_faq",
];

const EXPECTED_RESOURCES = [
  "prism://learn/start",
  "prism://learn/architecture",
  "prism://learn/glossary",
  "prism://learn/repo-map",
  "prism://learn/skills-index",
  "prism://learn/payments-101",
  "prism://learn/llms-txt",
];

function rpc(child, id, method, params) {
  return new Promise((resolve, reject) => {
    const payload = JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n";
    let buffer = "";
    const onData = (chunk) => {
      buffer += chunk.toString();
      let idx;
      while ((idx = buffer.indexOf("\n")) >= 0) {
        const line = buffer.slice(0, idx).trim();
        buffer = buffer.slice(idx + 1);
        if (!line) continue;
        let msg;
        try {
          msg = JSON.parse(line);
        } catch {
          continue;
        }
        if (msg.id === id) {
          child.stdout.off("data", onData);
          if (msg.error) reject(new Error(`${method} -> ${JSON.stringify(msg.error)}`));
          else resolve(msg.result);
          return;
        }
      }
    };
    child.stdout.on("data", onData);
    child.stdin.write(payload);
    setTimeout(() => {
      child.stdout.off("data", onData);
      reject(new Error(`${method} timed out`));
    }, 15000);
  });
}

function assert(cond, msg) {
  if (!cond) {
    console.error(`❌ ${msg}`);
    process.exitCode = 1;
    throw new Error(msg);
  }
  console.log(`✅ ${msg}`);
}

async function callTool(child, id, name, args) {
  return rpc(child, id, "tools/call", { name, arguments: args });
}

async function main() {
  const child = spawn("node", [entry], { stdio: ["pipe", "pipe", "inherit"] });

  try {
    await rpc(child, 1, "initialize", {
      protocolVersion: "2024-11-05",
      capabilities: {},
      clientInfo: { name: "smoke", version: "0.0.0" },
    });
    console.log("✅ initialize");

    const list = await rpc(child, 2, "tools/list", {});
    const names = list.tools.map((t) => t.name).sort();
    for (const t of EXPECTED_TOOLS) assert(names.includes(t), `tools/list includes ${t}`);
    assert(
      list.tools.length === EXPECTED_TOOLS.length,
      `exactly ${EXPECTED_TOOLS.length} tools (got ${list.tools.length})`,
    );
    for (const t of list.tools) {
      assert(t.description && t.description.length > 30, `${t.name} has a description`);
      assert(t.inputSchema && t.inputSchema.type === "object", `${t.name} has an object inputSchema`);
      assert(t.annotations?.readOnlyHint === true, `${t.name} readOnlyHint=true`);
    }

    const resources = await rpc(child, 3, "resources/list", {});
    const uris = resources.resources.map((r) => r.uri).sort();
    for (const u of EXPECTED_RESOURCES) assert(uris.includes(u), `resources/list includes ${u}`);

    let id = 10;
    const start = await callTool(child, id++, "prism_learn_start_here", { role: "explorer" });
    assert(start.structuredContent?.ok === true, "start_here ok");

    const concept = await callTool(child, id++, "prism_learn_explain_concept", { concept: "connector", depth: "tldr" });
    assert(concept.structuredContent?.ok === true, "explain_concept connector ok");
    const conceptCites = JSON.stringify(concept.structuredContent?.citations ?? []);
    assert(conceptCites.includes("stripe.rs"), "connector card cites a real stripe file");

    const flow = await callTool(child, id++, "prism_learn_explain_flow", { flow: "refund" });
    const deps = (flow.structuredContent?.dependsOn ?? []).map((d) => String(d).toLowerCase());
    assert(deps.includes("capture"), "refund flow depends on capture");

    const search = await callTool(child, id++, "prism_learn_search", { query: "refund" });
    assert(Array.isArray(search.structuredContent?.results) && search.structuredContent.results.length > 0, "search returns results");

    const doc = await callTool(child, id++, "prism_learn_read_doc", {
      path: "docs/architecture/concepts/error-handling.md",
    });
    assert(doc.structuredContent?.ok === true && String(doc.structuredContent?.body || "").length > 100, "read_doc returns verbatim body");

    const glossary = await callTool(child, id++, "prism_learn_glossary", {});
    assert(Array.isArray(glossary.structuredContent?.terms) && glossary.structuredContent.terms.length > 5, "glossary lists terms");

    const howto = await callTool(child, id++, "prism_learn_how_to", { task: "add a connector" });
    assert(String(howto.structuredContent?.skillMdPath || "").includes("new-connector"), "how_to routes to new-connector skill");

    const miss = await callTool(child, id++, "prism_learn_explain_concept", { concept: "zzzznotathing" });
    assert(miss.isError === true, "unknown concept returns graceful error");
    assert(Array.isArray(miss.structuredContent?.suggestions), "unknown concept offers suggestions");

    console.log("\n🎉 smoke test passed");
  } finally {
    child.kill();
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
