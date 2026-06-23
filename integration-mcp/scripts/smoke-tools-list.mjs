/**
 * Smoke test: spawn the built server over stdio, run JSON-RPC initialize + tools/list,
 * assert all 10 tools are present with complete schemas + correct annotations, then
 * round-trip one call per read-only tool and assert structuredContent.ok.
 */
import { spawn } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const entry = join(root, "dist", "index.js");

const EXPECTED_TOOLS = [
  "prism_integration_guide",
  "prism_list_connectors",
  "prism_connector_requirements",
  "prism_scaffold_integration",
  "prism_generate_config",
  "prism_validate_config",
  "prism_explain_error",
  "prism_status_reference",
  "prism_doctor",
  "prism_test_charge",
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
  const res = await rpc(child, id, "tools/call", { name, arguments: args });
  return res;
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
    for (const t of EXPECTED_TOOLS) {
      assert(names.includes(t), `tools/list includes ${t}`);
    }
    assert(list.tools.length === EXPECTED_TOOLS.length, `exactly ${EXPECTED_TOOLS.length} tools (got ${list.tools.length})`);

    for (const t of list.tools) {
      assert(t.description && t.description.length > 30, `${t.name} has a description`);
      assert(t.inputSchema && t.inputSchema.type === "object", `${t.name} has an object inputSchema`);
    }

    // Annotations: test_charge must NOT be read-only; the rest must be.
    const byName = Object.fromEntries(list.tools.map((t) => [t.name, t]));
    assert(byName.prism_test_charge.annotations?.readOnlyHint === false, "prism_test_charge readOnlyHint=false");
    assert(byName.prism_test_charge.annotations?.openWorldHint === true, "prism_test_charge openWorldHint=true");
    for (const t of EXPECTED_TOOLS.filter((n) => n !== "prism_test_charge")) {
      assert(byName[t].annotations?.readOnlyHint === true, `${t} readOnlyHint=true`);
    }

    // Resources.
    const resources = await rpc(child, 3, "resources/list", {});
    const uris = resources.resources.map((r) => r.uri).sort();
    for (const u of ["prism://guide", "prism://llm-txt", "prism://status-table"]) {
      assert(uris.includes(u), `resources/list includes ${u}`);
    }

    // Round-trip read-only tools.
    let id = 10;
    const guide = await callTool(child, id++, "prism_integration_guide", { framework: "express", flows: ["authorize", "capture"] });
    assert(guide.structuredContent?.ok === true, "prism_integration_guide ok");

    const reqStripe = await callTool(child, id++, "prism_connector_requirements", { connector: "stripe" });
    const stripeFields = reqStripe.structuredContent?.requiredFields?.map((f) => f.name) ?? [];
    assert(stripeFields.includes("apiKey"), "stripe requires apiKey");

    const reqAnet = await callTool(child, id++, "prism_connector_requirements", { connector: "authorizedotnet" });
    const anetFields = reqAnet.structuredContent?.requiredFields?.map((f) => f.name) ?? [];
    assert(anetFields.includes("name") && anetFields.includes("transactionKey"), "authorizedotnet requires name + transactionKey");

    const goodCfg = await callTool(child, id++, "prism_validate_config", {
      connector: "stripe",
      config: { connectorConfig: { stripe: { apiKey: { value: "sk_test_x" } } } },
    });
    assert(goodCfg.structuredContent?.valid === true, "validate accepts a good stripe config");

    const badCfg = await callTool(child, id++, "prism_validate_config", {
      connector: "authorizedotnet",
      config: { connectorConfig: { authorizedotnet: { name: { value: "x" } } } },
    });
    assert(badCfg.structuredContent?.valid === false, "validate rejects missing transactionKey");

    const status = await callTool(child, id++, "prism_status_reference", { status_code: 8 });
    assert(status.structuredContent?.status?.name === "CHARGED", "status 8 = CHARGED");

    const scaffold = await callTool(child, id++, "prism_scaffold_integration", {
      framework: "express",
      connector: "stripe",
      flows: ["authorize", "capture"],
      language: "ts",
      captureMethod: "MANUAL",
    });
    assert(Array.isArray(scaffold.structuredContent?.files) && scaffold.structuredContent.files.length >= 3, "scaffold returns files");

    const doctor = await callTool(child, id++, "prism_doctor", {});
    assert(typeof doctor.structuredContent?.healthy === "boolean", "doctor returns a health verdict");

    const explain = await callTool(child, id++, "prism_explain_error", { status: 21 });
    assert(String(explain.content?.[0]?.text || "").includes("soft decline"), "explain_error describes soft decline for 21");

    // test_charge must hard-fail when test_mode=false.
    const refused = await callTool(child, id++, "prism_test_charge", { connector: "stripe", amount: 1000, test_mode: false });
    assert(refused.isError === true, "test_charge refuses test_mode=false");

    console.log("\n🎉 smoke test passed");
  } finally {
    child.kill();
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
