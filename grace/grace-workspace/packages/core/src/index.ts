export * from "./types.js";
export * from "./engine.js";
export * from "./state.js";
export * from "./session-manager.js";
export * from "./session-supervisor.js";
export * from "./logger.js";
export * from "./config.js";
export * from "./utils.js";
export * from "./llm.js";
export * from "./checkpoints/index.js";

// OpenCode runner exports (original - maintained for backward compatibility)
export { checkOpencodeHealth } from "./tools/opencode-runner.js";
export type { OpencodeHealthResult } from "./tools/opencode-runner.js";

// Claude Code runner exports
export { runClaudeCode, checkClaudeCodeHealth } from "./tools/claude-code-runner.js";
export type { ClaudeCodeRunOptions, ClaudeCodeHealthResult } from "./tools/claude-code-runner.js";

// Unified runner factory exports (preferred for new code)
export { runAI, checkAIHealth, getConfiguredRunner, withRunner } from "./tools/runner-factory.js";
export type { AIRunOptions, AIHealthResult, RunnerInterface } from "./tools/runner-factory.js";

// PR Resolver exports
export * from "./pr-resolver/index.js";

// Wizard-side connector discovery
export { discoverConnector } from "./discovery/connector-discovery.js";
export type {
  DiscoveryResult,
  DiscoveryFields,
  DiscoveryField,
  DiscoverConnectorOptions,
  Confidence,
} from "./discovery/connector-discovery.js";
