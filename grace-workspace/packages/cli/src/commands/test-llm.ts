import { callLlm, getConfig, loadConfig, setConfig } from "@10xgrace/core";

export async function testLlmCommand(): Promise<void> {
  const cfg = loadConfig();
  setConfig(cfg);

  // eslint-disable-next-line no-console
  console.log(`\x1b[1m10XGRACE · LLM health check\x1b[0m`);
  // eslint-disable-next-line no-console
  console.log(`  baseUrl: ${cfg.llm.baseUrl || "\x1b[31m<not set>\x1b[0m"}`);
  // eslint-disable-next-line no-console
  console.log(`  model:   ${cfg.llm.model}`);
  // eslint-disable-next-line no-console
  console.log(`  protocol: ${cfg.llm.protocol}`);
  // eslint-disable-next-line no-console
  console.log(`  apiKey:  ${cfg.llm.apiKey ? "****" + cfg.llm.apiKey.slice(-4) : "\x1b[31m<not set>\x1b[0m"}`);
  // eslint-disable-next-line no-console
  console.log("");

  const started = Date.now();
  try {
    const out = await callLlm({
      system:
        "You are a test responder. Reply ONLY with the exact JSON: {\"ok\": true, \"msg\": \"pong\"}",
      user: "ping",
    });
    const elapsed = Date.now() - started;
    // eslint-disable-next-line no-console
    console.log(`\x1b[32m✓ LLM responded in ${elapsed}ms\x1b[0m`);
    // eslint-disable-next-line no-console
    console.log(`\x1b[90m--- raw output ---\x1b[0m`);
    // eslint-disable-next-line no-console
    console.log(out);
    // eslint-disable-next-line no-console
    console.log(`\x1b[90m--- end ---\x1b[0m`);
  } catch (err) {
    const elapsed = Date.now() - started;
    // eslint-disable-next-line no-console
    console.error(`\x1b[31m✕ LLM call failed after ${elapsed}ms\x1b[0m`);
    // eslint-disable-next-line no-console
    console.error(err instanceof Error ? err.stack ?? err.message : String(err));
    process.exitCode = 1;
  }
  // Ensure process exits even if a socket is held open
  setTimeout(() => process.exit(process.exitCode ?? 0), 50).unref();
}
