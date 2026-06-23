/**
 * Acts like an AI agent: connects to the built MCP server over stdio JSON-RPC,
 * calls prism_doctor + prism_scaffold_integration, and writes the returned files
 * into a target app directory. Usage: node demo-drive.mjs <appDir> <connector> <framework>
 */
import { spawn } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const entry = join(root, "dist", "index.js");
const [appDir, connector = "stripe", framework = "express"] = process.argv.slice(2);
if (!appDir) throw new Error("usage: demo-drive.mjs <appDir> [connector] [framework]");

function rpc(child, id, method, params) {
  return new Promise((resolve, reject) => {
    let buffer = "";
    const onData = (chunk) => {
      buffer += chunk.toString();
      let idx;
      while ((idx = buffer.indexOf("\n")) >= 0) {
        const line = buffer.slice(0, idx).trim();
        buffer = buffer.slice(idx + 1);
        if (!line) continue;
        let msg;
        try { msg = JSON.parse(line); } catch { continue; }
        if (msg.id === id) {
          child.stdout.off("data", onData);
          msg.error ? reject(new Error(JSON.stringify(msg.error))) : resolve(msg.result);
          return;
        }
      }
    };
    child.stdout.on("data", onData);
    child.stdin.write(JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n");
    setTimeout(() => { child.stdout.off("data", onData); reject(new Error(`${method} timeout`)); }, 15000);
  });
}

const child = spawn("node", [entry], { stdio: ["pipe", "pipe", "inherit"] });
try {
  await rpc(child, 1, "initialize", { protocolVersion: "2024-11-05", capabilities: {}, clientInfo: { name: "demo-agent", version: "0" } });

  const doctor = await rpc(child, 2, "tools/call", { name: "prism_doctor", arguments: {} });
  console.log("[agent] doctor.healthy =", doctor.structuredContent.healthy, "| willLoad =", doctor.structuredContent.native.willLoad);

  const reqs = await rpc(child, 3, "tools/call", { name: "prism_connector_requirements", arguments: { connector } });
  console.log("[agent] required fields for", connector, "=", reqs.structuredContent.requiredFields.map(f => f.name).join(", "));

  const scaf = await rpc(child, 4, "tools/call", { name: "prism_scaffold_integration", arguments: { framework, connector, flows: ["authorize", "refund"], language: "ts", captureMethod: "AUTOMATIC" } });
  for (const f of scaf.structuredContent.files) {
    const dest = join(appDir, f.path);
    mkdirSync(dirname(dest), { recursive: true });
    writeFileSync(dest, f.contents);
    console.log("[agent] wrote", f.path);
  }
  console.log("[agent] done — scaffolded via MCP over JSON-RPC");
} finally {
  child.kill();
}
