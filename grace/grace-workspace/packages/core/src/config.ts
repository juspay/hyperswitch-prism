import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import dotenv from "dotenv";
import YAML from "yaml";
import { findWorkspaceRoot } from "./utils.js";

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

/**
 * PR Resolver — polls GitHub for review comments tagged with `trigger`,
 * groups them by connector, then drives Claude through Byne's
 * `runClaudeCode` runner to make the edits and run a cargo build/clippy
 * fix-loop. See `packages/core/src/pr-resolver/` for the implementation
 * and `grace/pr-resolver/prompts/` for the prompt markdown.
 */
export interface PrResolverConfig {
  /** Master switch. When false, the supervisor doesn't boot the polling loop. */
  enabled: boolean;
  /**
   * Phase A: when true, the resolver pushes commits automatically after the
   * cargo build/clippy fix loop succeeds. When false (default), the PR
   * enters `awaiting_approval` and the user must click Approve in the
   * dashboard before commits leave the worktree.
   */
  autoApprove: boolean;
  /** GitHub repository in `owner/name` format. Empty disables boot even when `enabled` is true. */
  githubRepo: string;
  /** Trigger tag the resolver matches in comments (case-insensitive). */
  trigger: string;
  /** Poll interval in seconds. */
  pollInterval: number;
  /** Max PRs processed per cycle. MVP defaults to 1 (serial processing). */
  maxConcurrent: number;
  /** Max iterations of the cargo build/clippy fix loop per sub-task. */
  maxBuildLoops: number;
  /** Cap on triggered comments processed per cycle. */
  maxCommentsPerCycle: number;
  /**
   * Absolute path to the JSON state file. Empty string is resolved at load
   * time to `~/.tenxgrace/pr-resolver-state.json` (or, for legacy installs,
   * `~/.byne/pr-resolver-state.json` if that file already exists).
   */
  stateFilePath: string;
  /**
   * Absolute path to the working git clone used for fixes. Empty string is
   * resolved at load time to `~/.tenxgrace/pr-resolver/worktree` (or legacy
   * `~/.byne/pr-resolver/worktree` if already present).
   */
  worktreePath: string;
  /**
   * Absolute path to the prompts directory. Empty string is resolved at
   * load time to `<projectRoot>/grace/pr-resolver/prompts`.
   */
  promptsDir: string;
  /** Cargo build command + args used between fix-loop iterations. */
  cargoBuild: { command: string; args: string[] };
  /** Cargo clippy command + args. Runs after a successful build. */
  cargoClippy: { command: string; args: string[] };
  /**
   * GitHub `authorAssociation` values allowed to trigger the bot.
   * Defaults to MEMBER, OWNER, COLLABORATOR — matches the Python service.
   */
  allowedAssociations: string[];
  /** Hard allow-list of GitHub logins (bypasses association check). */
  allowedUsers: string[];
  /** Block-list of GitHub logins (always rejected). */
  blockedUsers: string[];
  /**
   * Phase B: run a grpcurl-based verification step after cargo build/clippy.
   * Off by default — once you're confident on the test extractor + generator,
   * flip it on via the dashboard.
   */
  grpcTestEnabled: boolean;
  /** Port the per-worktree `cargo run -p grpc-server` listens on. */
  grpcPort: number;
  /** Per-grpcurl-invocation timeout in ms. */
  grpcTestTimeoutMs: number;
  /**
   * Wall clock budget for `cargo run -p grpc-server` to spawn + compile +
   * answer its first `grpcurl list` probe. Default 10 min — enough for a
   * cold-cache grpc-server build on hyperswitch-prism. Subsequent runs in
   * the same worktree are typically a few seconds.
   */
  grpcServerStartTimeoutMs: number;
  /** Max command count we'll execute per sub-task (caps both extractor and generator). */
  maxGrpcCommands: number;
  /**
   * Hard timeout for each `cargo build` / `cargo clippy` invocation. Defaults
   * to 30 min — cold-cache builds on hyperswitch-prism can exceed 15 min,
   * so this is intentionally generous. Lower it for fast machines if you
   * want quicker failure on stuck builds.
   */
  cargoTimeoutMs: number;
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
  /** PR Resolver configuration. */
  prResolver: PrResolverConfig;
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
  prResolver: {
    enabled: false,
    autoApprove: false,
    githubRepo: "",
    trigger: "@HS-prism-bot",
    pollInterval: 300,
    maxConcurrent: 1,
    maxBuildLoops: 3,
    maxCommentsPerCycle: 20,
    stateFilePath: "",
    worktreePath: "",
    promptsDir: "",
    cargoBuild: {
      command: "cargo",
      args: ["build", "--package", "connector-integration"],
    },
    cargoClippy: {
      command: "cargo",
      args: [
        "clippy",
        "--package",
        "connector-integration",
        "--",
        "-D",
        "warnings",
      ],
    },
    allowedAssociations: ["MEMBER", "OWNER", "COLLABORATOR"],
    allowedUsers: [],
    blockedUsers: [],
    grpcTestEnabled: false,
    grpcPort: 8000,
    grpcTestTimeoutMs: 30_000,
    maxGrpcCommands: 5,
    cargoTimeoutMs: 30 * 60 * 1000, // 30 min — cold-cache build budget
    grpcServerStartTimeoutMs: 10 * 60 * 1000, // 10 min — cold cargo run + first probe
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
  // The dashboard's Vite middleware spawns Claude Code from its own cwd
  // (packages/dashboard/), not the workspace root. Without finding the
  // workspace root, .env and config.yml lookups would fall back to defaults
  // and silently ignore the user's customisations (e.g. claudeCode.model).
  const wsRoot = findWorkspaceRoot();

  // Load .env from workspace root first (the canonical location), then from
  // cwd as a per-package override. Existing process.env values win on both
  // passes (override: false).
  if (wsRoot) {
    dotenv.config({ path: path.join(wsRoot, ".env"), override: false });
  }
  dotenv.config({ path: path.resolve(process.cwd(), ".env"), override: false });

  const candidates = [
    explicitPath,
    // Workspace-root variants first so a sibling cwd (dashboard/) doesn't
    // miss the canonical config.yml that lives alongside pnpm-workspace.yaml.
    wsRoot ? path.join(wsRoot, "config.yml") : undefined,
    wsRoot ? path.join(wsRoot, "config.yaml") : undefined,
    wsRoot ? path.join(wsRoot, "10xgrace.config.yml") : undefined,
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

  // PR Resolver env overrides. Keeping the github repo and enable flag out
  // of committed config.yml is the path of least friction for shared dev
  // boxes — drop a value in .env and toggle the feature.
  //
  // Migration: prefer TENXGRACE_PR_RESOLVER_* (the documented form in
  // .env.example/README); fall back to legacy BYNE_PR_RESOLVER_* so users
  // on the old envs don't break. Drop the BYNE_ fallbacks once nobody is
  // running pre-rename configs.
  const githubRepo =
    process.env.TENXGRACE_PR_RESOLVER_GITHUB_REPO ??
    process.env.BYNE_PR_RESOLVER_GITHUB_REPO;
  if (githubRepo) merged.prResolver.githubRepo = githubRepo;

  const trigger =
    process.env.TENXGRACE_PR_RESOLVER_TRIGGER ??
    process.env.BYNE_PR_RESOLVER_TRIGGER;
  if (trigger) merged.prResolver.trigger = trigger;

  const enabledRaw =
    process.env.TENXGRACE_PR_RESOLVER_ENABLED ??
    process.env.BYNE_PR_RESOLVER_ENABLED;
  if (enabledRaw !== undefined) {
    merged.prResolver.enabled = enabledRaw === "true" || enabledRaw === "1";
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

  // PR Resolver path defaults — resolve once at load time so consumers
  // (CLI, supervisor, dashboard) never need to know the fallback rules.
  //
  // Migration: default to ~/.tenxgrace/, but if a pre-existing ~/.byne/
  // state file is present (legacy), keep using it so existing users don't
  // lose their resolver state. New installs land in ~/.tenxgrace/ cleanly.
  const tenxHome = path.join(os.homedir(), ".tenxgrace");
  const legacyHome = path.join(os.homedir(), ".byne");
  const legacyStateFile = path.join(legacyHome, "pr-resolver-state.json");
  const legacyWorktree = path.join(legacyHome, "pr-resolver", "worktree");
  if (!merged.prResolver.stateFilePath) {
    merged.prResolver.stateFilePath = fs.existsSync(legacyStateFile)
      ? legacyStateFile
      : path.join(tenxHome, "pr-resolver-state.json");
  }
  if (!merged.prResolver.worktreePath) {
    merged.prResolver.worktreePath = fs.existsSync(legacyWorktree)
      ? legacyWorktree
      : path.join(tenxHome, "pr-resolver", "worktree");
  }
  if (!merged.prResolver.promptsDir) {
    merged.prResolver.promptsDir = path.join(
      merged.projectRoot,
      "grace",
      "pr-resolver",
      "prompts"
    );
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
