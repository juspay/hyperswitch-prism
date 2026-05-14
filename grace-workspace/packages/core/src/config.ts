import fs from "node:fs";
import path from "node:path";
import dotenv from "dotenv";
import YAML from "yaml";

export type LlmProtocol = "openai" | "anthropic";

/**
 * AI runner type - determines which CLI tool to use for AI execution
 */
export type RunnerType = "opencode" | "claude-code";

export interface LlmConfig {
  baseUrl: string;
  apiKey: string;
  model: string;
  protocol: LlmProtocol;
  maxTokens: number;
  temperature: number;
  timeoutMs: number;
  authHeader?: string;
  authScheme?: string;
  extraHeaders?: Record<string, string>;
  /**
   * Per-checkpoint model overrides. The key is the checkpoint id (e.g.
   * "l3_gen", "l4_gen", "implementation") and the value is a model slug
   * supported by the gateway. Falls back to `model` when unset.
   */
  models?: Record<string, string>;
}

export interface OpencodeConfig {
  /** Model slug passed to `opencode run --model`, e.g. "litellm/open-large". */
  model: string;
  /** URL of a running `opencode serve` instance. Empty string disables --attach. */
  attachUrl: string;
  /** Per-call hard timeout in ms. */
  timeoutMs: number;
  /**
   * How many per-file implementation calls to run in parallel. Default 4.
   * Higher = faster but more load on the opencode server and gateway.
   */
  implementationConcurrency: number;
}

export interface ClaudeCodeConfig {
  /** Model slug passed to `claude` CLI, e.g. "claude-sonnet-4-6". */
  model: string;
  /** Per-call hard timeout in ms. */
  timeoutMs: number;
  /**
   * How many per-file implementation calls to run in parallel. Default 4.
   * Higher = faster but more load on the system.
   */
  implementationConcurrency: number;
  /** Whether to use global ~/.claude/settings.json configuration. */
  useGlobalConfig: boolean;
  /** Additional CLI arguments to pass to claude. */
  extraArgs: string[];
}

export interface CsddConfig {
  projectRoot: string;
  /**
   * Absolute path to the connector `creds.json` file. When set,
   * SessionManager.create() symlinks this into each new session's
   * worktree as `<projectRoot>/creds.json` so credentials propagate
   * to every isolated workspace without copy-and-rotate. Override
   * via TENXGRACE_CREDS_PATH env var (recommended — keeps secrets out of
   * the committed config.yml).
   */
  credsPath?: string;
  devServerUrl: string;
  designMatchThreshold: number;
  maxRetries: number;
  dashboardPort: number;
  wsPort: number;
  llm: LlmConfig;
  /** AI runner to use - "opencode" or "claude-code". Defaults to "opencode" for backward compatibility. */
  runner: RunnerType;
  /** OpenCode configuration (used when runner is "opencode"). */
  opencode: OpencodeConfig;
  /** Claude Code configuration (used when runner is "claude-code"). */
  claudeCode: ClaudeCodeConfig;
  checkpoints: {
    compiler: { command: string; args: string[]; enabled?: boolean };
    cypress: { command: string; args: string[]; enabled?: boolean };
    playwright: { command: string; args: string[]; enabled?: boolean };
    regression: { command: string; args: string[]; enabled?: boolean };
    design_match: { enabled: boolean; screenshotRoute: string };
    pr_review: { requireHumanApproval: boolean; humanApprovalTimeoutMs: number };
    /**
     * Phase 13: GitHub side-effect on l2 tech spec approval. When the
     * reviewer (human or auto-mode) approves the L2 spec, the engine
     * shells out to `gh issue create` against `graceIssueRepo` so the
     * approved spec lands in a central tracker. Failure to create the
     * issue logs a warning and lets l2_review still pass (external system
     * isn't a correctness gate).
     */
    l2_review: {
      createGraceIssue: boolean;
      graceIssueRepo: string;
      graceIssueLabels: string[];
    };
  };
}

const DEFAULTS: CsddConfig = {
  projectRoot: "../hyperswitch-control-center",
  devServerUrl: "http://localhost:9000",
  designMatchThreshold: 0.9,
  maxRetries: 3,
  dashboardPort: 3141,
  wsPort: 3142,
  llm: {
    baseUrl: "",
    apiKey: "",
    model: "claude-sonnet-4-20250514",
    protocol: "openai",
    maxTokens: 4000,
    temperature: 0,
    timeoutMs: 60_000,
    authHeader: "Authorization",
    authScheme: "Bearer",
    extraHeaders: {},
    models: {},
  },
  runner: "opencode",
  opencode: {
    model: "litellm/open-large",
    attachUrl: "http://127.0.0.1:4096",
    timeoutMs: 600_000,
    implementationConcurrency: 4,
  },
  claudeCode: {
    model: "claude-sonnet-4-6",
    timeoutMs: 600_000,
    implementationConcurrency: 4,
    useGlobalConfig: true,
    extraArgs: [],
  },
  checkpoints: {
    compiler: { command: "npm", args: ["run", "re:build"] },
    cypress: { command: "npx", args: ["cypress", "run", "--reporter", "json"] },
    playwright: { command: "npx", args: ["playwright", "test", "--reporter=json"] },
    regression: { command: "npm", args: ["run", "re:build"] },
    design_match: { enabled: true, screenshotRoute: "/" },
    pr_review: { requireHumanApproval: true, humanApprovalTimeoutMs: 300_000 },
    l2_review: {
      createGraceIssue: true,
      graceIssueRepo: "juspay/grace",
      // `connector-integration` exists on juspay/grace (verified via
      // `gh label list`). gh issue create --label NAME errors if NAME
      // doesn't exist on the target repo, so we deliberately ship a
      // single label that's known to be present.
      graceIssueLabels: ["connector-integration"],
    },
  },
};

function deepMerge<T>(base: T, over: Partial<T>): T {
  if (over === undefined || over === null) return base;
  if (typeof base !== "object" || base === null) return over as T;
  if (typeof over !== "object") return over as T;
  const out: Record<string, unknown> = { ...(base as Record<string, unknown>) };
  for (const k of Object.keys(over as Record<string, unknown>)) {
    const bv = (base as Record<string, unknown>)[k];
    const ov = (over as Record<string, unknown>)[k];
    if (
      bv &&
      ov &&
      typeof bv === "object" &&
      typeof ov === "object" &&
      !Array.isArray(bv) &&
      !Array.isArray(ov)
    ) {
      out[k] = deepMerge(bv, ov as Record<string, unknown>);
    } else {
      out[k] = ov;
    }
  }
  return out as T;
}

export function loadConfig(explicitPath?: string): CsddConfig {
  // Load .env from cwd before reading process.env so contributors can set
  // TENXGRACE_PROJECT_ROOT / TENXGRACE_LLM_API_KEY without touching their shell rc.
  // Existing process.env values win (override: false).
  dotenv.config({ path: path.resolve(process.cwd(), ".env"), override: false });

  const candidates = [
    explicitPath,
    path.resolve(process.cwd(), "config.yml"),
    path.resolve(process.cwd(), "config.yaml"),
    path.resolve(process.cwd(), "10xgrace.config.yml"),
  ].filter(Boolean) as string[];

  let loaded: Partial<CsddConfig> | undefined;
  let usedPath: string | undefined;
  for (const p of candidates) {
    if (fs.existsSync(p)) {
      const raw = fs.readFileSync(p, "utf-8");
      loaded = YAML.parse(raw) as Partial<CsddConfig>;
      usedPath = p;
      break;
    }
  }

  const merged = loaded ? deepMerge(DEFAULTS, loaded) : DEFAULTS;

  // Env var overrides for secrets
  if (process.env.TENXGRACE_LLM_API_KEY) merged.llm.apiKey = process.env.TENXGRACE_LLM_API_KEY;
  if (process.env.TENXGRACE_LLM_BASE_URL) merged.llm.baseUrl = process.env.TENXGRACE_LLM_BASE_URL;
  if (process.env.TENXGRACE_LLM_MODEL) merged.llm.model = process.env.TENXGRACE_LLM_MODEL;
  if (process.env.TENXGRACE_PROJECT_ROOT) merged.projectRoot = process.env.TENXGRACE_PROJECT_ROOT;
  // Phase 10: TENXGRACE_CREDS_PATH points at the user's connector creds.json so
  // SessionManager can symlink it into every new session worktree. Resolve
  // to absolute path so consumers don't need to know the supervisor's cwd.
  if (process.env.TENXGRACE_CREDS_PATH) {
    merged.credsPath = path.resolve(process.env.TENXGRACE_CREDS_PATH);
  } else if (merged.credsPath && !path.isAbsolute(merged.credsPath)) {
    merged.credsPath = path.resolve(merged.credsPath);
  }
  // Phase 13: env override for the grace issue repo. Lets developers point
  // at a fork (e.g. shuklatushar226/grace for testing) without editing
  // config.yml. Empty / unset → config.yml or DEFAULTS value wins.
  if (process.env.TENXGRACE_GRACE_ISSUE_REPO) {
    merged.checkpoints.l2_review.graceIssueRepo =
      process.env.TENXGRACE_GRACE_ISSUE_REPO;
  }

  if (usedPath) {
    // eslint-disable-next-line no-console
    console.log(`\x1b[90m[config] loaded ${usedPath}\x1b[0m`);
  } else {
    // eslint-disable-next-line no-console
    console.log(
      `\x1b[33m[config] no config.yml found; using defaults. Create one to set llm.baseUrl and llm.apiKey.\x1b[0m`
    );
  }

  // Resolve projectRoot relative to cwd
  if (!path.isAbsolute(merged.projectRoot)) {
    merged.projectRoot = path.resolve(process.cwd(), merged.projectRoot);
  }

  return merged;
}

let cached: CsddConfig | undefined;
export function getConfig(): CsddConfig {
  if (!cached) cached = loadConfig();
  return cached;
}

export function setConfig(cfg: CsddConfig): void {
  cached = cfg;
}
