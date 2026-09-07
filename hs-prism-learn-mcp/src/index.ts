#!/usr/bin/env node
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { createServer } from "./server.js";
import { SERVER_NAME, SERVER_VERSION } from "./constants.js";

async function main(): Promise<void> {
  const args = process.argv.slice(2);

  if (args.includes("--help") || args.includes("-h")) {
    process.stdout.write(
      `${SERVER_NAME} v${SERVER_VERSION}\n` +
        `MCP server that helps newcomers understand the hyperswitch-prism (UCS) codebase, in plain language.\n\n` +
        `Usage: hs-prism-learn-mcp [--http [--port <n>]]\n\n` +
        `  (default)      Run over stdio (for Claude Code / Cursor / Windsurf).\n` +
        `  --http         Run over streamable HTTP instead of stdio.\n` +
        `  --port <n>     HTTP port (default 3000; or PORT env).\n`,
    );
    return;
  }

  const server = createServer();

  if (args.includes("--http")) {
    await runHttp(server, args);
    return;
  }

  const transport = new StdioServerTransport();
  await server.connect(transport);
  // stdio: stay alive; logging to stderr only (stdout is the protocol channel).
  process.stderr.write(`${SERVER_NAME} v${SERVER_VERSION} listening on stdio\n`);
}

async function runHttp(server: ReturnType<typeof createServer>, args: string[]): Promise<void> {
  const { StreamableHTTPServerTransport } = await import(
    "@modelcontextprotocol/sdk/server/streamableHttp.js"
  );
  const http = await import("node:http");
  const portIdx = args.indexOf("--port");
  const port = Number(portIdx >= 0 ? args[portIdx + 1] : process.env.PORT ?? 3000);

  const transport = new StreamableHTTPServerTransport({ sessionIdGenerator: undefined });
  await server.connect(transport);

  const httpServer = http.createServer((req, res) => {
    transport.handleRequest(req, res).catch((err) => {
      process.stderr.write(`HTTP handler error: ${String(err)}\n`);
      if (!res.headersSent) {
        res.statusCode = 500;
        res.end("Internal Server Error");
      }
    });
  });
  httpServer.listen(port, () => {
    process.stderr.write(`${SERVER_NAME} v${SERVER_VERSION} listening on http://localhost:${port}/\n`);
  });
}

main().catch((err) => {
  process.stderr.write(`Fatal: ${err instanceof Error ? err.stack ?? err.message : String(err)}\n`);
  process.exit(1);
});
