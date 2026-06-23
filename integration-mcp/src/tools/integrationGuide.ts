import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { integrationGuideShape } from "../schemas.js";
import { buildGuide } from "../guide.js";
import { result } from "../util.js";

export function registerIntegrationGuide(server: McpServer): void {
  server.registerTool(
    "prism_integration_guide",
    {
      title: "Prism integration guide (start here)",
      description:
        "The entry-point tool: returns an ordered, end-to-end plan for integrating hyperswitch-prism into a given " +
        "framework, with each step mapped to the next MCP tool to call. " +
        "Input: { framework: 'express'|'next'|'nestjs'|'fastify'|'node', flows: [...], connector? }. " +
        "Flows: authorize, capture, void, refund, sync, webhook. " +
        "Call this FIRST, then follow the steps (requirements → config → scaffold → test_charge).",
      inputSchema: integrationGuideShape,
      annotations: { readOnlyHint: true, openWorldHint: false },
    },
    (args) => {
      const guide = buildGuide(args.framework, args.flows, args.connector);
      return result(guide.markdown, {
        ok: true,
        framework: guide.framework,
        flows: guide.flows,
        connector: guide.connector,
        steps: guide.steps,
      });
    },
  );
}
