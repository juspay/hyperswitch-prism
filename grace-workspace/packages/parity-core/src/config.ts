import { existsSync } from "node:fs";
import { join } from "node:path";
import { loadConfig as loadBaseConfig } from "@10xgrace/core";

export interface ParityConfig {
  prismPath: string;
  oracleReadOnlyPath: string;
  bridgeWritePath: string;
  credsPath: string;
  github: {
    owner: string;
    repo: string;
    rootIssue: number;
    actor: string;
  };
  grpc: {
    port: number;
    metricsPort: number;
    bootTimeoutMs: number;
    callTimeoutMs: number;
  };
  cache: {
    dir: string;
    treeTtlMs: number;
  };
  llm: {
    runner: "claude-code" | "opencode";
    understandModel: string;
    planModel: string;
    executeModel: string;
  };
  heartbeat: {
    pickOldestFirst: boolean;
    maxInflightClaimed: number;
    sweepStalePrDays: number;
    rcaStaleHours: number;
  };
  rules: {
    forbiddenOracleDirs: string[];
    forbiddenPrismDirs: string[];
  };
}

const DEFAULTS: ParityConfig = {
  prismPath: "",
  oracleReadOnlyPath: "",
  bridgeWritePath: "",
  credsPath: "",
  github: { owner: "juspay", repo: "hyperswitch-cloud", rootIssue: 15576, actor: "" },
  grpc: { port: 8000, metricsPort: 8080, bootTimeoutMs: 30_000, callTimeoutMs: 60_000 },
  cache: { dir: ".cache", treeTtlMs: 6 * 60 * 60 * 1000 },
  llm: { runner: "claude-code", understandModel: "", planModel: "", executeModel: "" },
  heartbeat: { pickOldestFirst: true, maxInflightClaimed: 1, sweepStalePrDays: 7, rcaStaleHours: 24 },
  rules: {
    forbiddenOracleDirs: [
      "crates/hyperswitch_connectors/",
      "crates/hyperswitch_domain_models/",
      "crates/api_models/",
      "crates/router/",
    ],
    forbiddenPrismDirs: ["crates/types-traits/"],
  },
};

function merge<T>(a: T, b: Partial<T> | undefined): T {
  if (!b) return a;
  const out: any = { ...a };
  for (const k of Object.keys(b) as (keyof T)[]) {
    const bv: any = b[k];
    if (bv && typeof bv === "object" && !Array.isArray(bv)) {
      out[k] = merge((a as any)[k] ?? {}, bv);
    } else if (bv !== undefined) {
      out[k] = bv;
    }
  }
  return out;
}

export function loadParityConfig(explicitPath?: string): ParityConfig {
  const base = loadBaseConfig(explicitPath) as any;
  const raw = base?.parityAutopilot as Partial<ParityConfig> | undefined;
  if (!raw) {
    throw new Error(
      "config.yml: missing parityAutopilot section. See grace-workspace/docs/parity-autopilot-plan.md for schema.",
    );
  }

  const cfg = merge(DEFAULTS, raw);

  cfg.prismPath = process.env.PARITY_PRISM_PATH ?? cfg.prismPath;
  cfg.oracleReadOnlyPath = process.env.PARITY_ORACLE_PATH ?? cfg.oracleReadOnlyPath;
  cfg.bridgeWritePath = process.env.PARITY_BRIDGE_PATH ?? cfg.bridgeWritePath;
  cfg.credsPath = process.env.PARITY_CREDS_PATH ?? cfg.credsPath;

  if (!cfg.prismPath) throw new Error("parityAutopilot.prismPath is empty (set PARITY_PRISM_PATH)");
  if (!existsSync(cfg.prismPath)) throw new Error(`prismPath does not exist: ${cfg.prismPath}`);

  // Bridge & oracle are optional. When set they must be sane — fail fast so the
  // operator hits a clear error before the agent burns 3-4 min on UNDERSTAND.
  if (cfg.bridgeWritePath) {
    if (!existsSync(cfg.bridgeWritePath)) {
      throw new Error(
        `parityAutopilot.bridgeWritePath does not exist: ${cfg.bridgeWritePath} (unset PARITY_BRIDGE_PATH to disable hs-bridge support)`,
      );
    }
    if (!existsSync(join(cfg.bridgeWritePath, ".git"))) {
      throw new Error(`bridgeWritePath is not a git repo: ${cfg.bridgeWritePath}`);
    }
    if (!existsSync(join(cfg.bridgeWritePath, "crates/external_services"))) {
      throw new Error(
        `bridgeWritePath does not look like a hyperswitch clone (missing crates/external_services): ${cfg.bridgeWritePath}`,
      );
    }
  }
  if (cfg.oracleReadOnlyPath && !existsSync(cfg.oracleReadOnlyPath)) {
    throw new Error(`oracleReadOnlyPath does not exist: ${cfg.oracleReadOnlyPath}`);
  }

  return cfg;
}
