import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import type { PrResolverConfig } from "../config.js";

/**
 * Runtime overlay for PrResolverConfig — what the dashboard can edit. Stored
 * at `~/.tenxgrace/pr-resolver-config.json` (or legacy `~/.byne/...` if that
 * file already exists; overridable via the second arg to the helpers). Only
 * the safe-to-edit subset of fields is allowed; cargo commands, allow-lists,
 * and path defaults stay file-only.
 */
export interface PrResolverRuntimeOverlay {
  enabled?: boolean;
  autoApprove?: boolean;
  githubRepo?: string;
  trigger?: string;
  pollInterval?: number;
  maxConcurrent?: number;
  maxBuildLoops?: number;
  maxCommentsPerCycle?: number;
  grpcTestEnabled?: boolean;
  grpcPort?: number;
  grpcServerStartTimeoutMs?: number;
}

const ALLOWED_KEYS = new Set<keyof PrResolverRuntimeOverlay>([
  "enabled",
  "autoApprove",
  "githubRepo",
  "trigger",
  "pollInterval",
  "maxConcurrent",
  "maxBuildLoops",
  "maxCommentsPerCycle",
  "grpcTestEnabled",
  "grpcPort",
  "grpcServerStartTimeoutMs",
]);

export function defaultRuntimeOverlayPath(): string {
  // Migration: prefer ~/.tenxgrace/, fall back to legacy ~/.byne/ overlay
  // if it already exists so existing users keep their saved settings.
  const tenx = path.join(os.homedir(), ".tenxgrace", "pr-resolver-config.json");
  const legacy = path.join(os.homedir(), ".byne", "pr-resolver-config.json");
  return fs.existsSync(legacy) ? legacy : tenx;
}

export function loadRuntimeOverlay(
  overlayPath = defaultRuntimeOverlayPath()
): PrResolverRuntimeOverlay {
  if (!fs.existsSync(overlayPath)) return {};
  try {
    const raw = fs.readFileSync(overlayPath, "utf-8");
    const parsed = JSON.parse(raw) as Record<string, unknown>;
    const out: PrResolverRuntimeOverlay = {};
    for (const key of Object.keys(parsed) as Array<keyof PrResolverRuntimeOverlay>) {
      if (!ALLOWED_KEYS.has(key)) continue;
      const value = parsed[key];
      // Skip empty strings — they shouldn't promote a default to an empty override.
      if (typeof value === "string" && value.length === 0) continue;
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (out as any)[key] = value;
    }
    return out;
  } catch (err) {
    // eslint-disable-next-line no-console
    console.warn(
      `[pr-resolver:runtime-config] Failed to load ${overlayPath}: ${err instanceof Error ? err.message : String(err)} — using empty overlay`
    );
    return {};
  }
}

export function saveRuntimeOverlay(
  overlay: PrResolverRuntimeOverlay,
  overlayPath = defaultRuntimeOverlayPath()
): void {
  fs.mkdirSync(path.dirname(overlayPath), { recursive: true });
  // Strip undefined / empty so the JSON only contains real overrides.
  const cleaned: PrResolverRuntimeOverlay = {};
  for (const key of Object.keys(overlay) as Array<keyof PrResolverRuntimeOverlay>) {
    if (!ALLOWED_KEYS.has(key)) continue;
    const value = overlay[key];
    if (value === undefined || value === null) continue;
    if (typeof value === "string" && value.length === 0) continue;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (cleaned as any)[key] = value;
  }
  fs.writeFileSync(
    overlayPath,
    JSON.stringify(cleaned, null, 2) + "\n",
    "utf-8"
  );
}

export function clearRuntimeOverlay(
  overlayPath = defaultRuntimeOverlayPath()
): void {
  if (fs.existsSync(overlayPath)) {
    try {
      fs.unlinkSync(overlayPath);
    } catch {
      /* best-effort */
    }
  }
}

export interface OverlayValidationResult {
  ok: boolean;
  errors: string[];
}

/**
 * Validate an overlay against simple invariants. Backed by both the
 * supervisor's `pr-resolver:configure` handler and the dashboard's save
 * button so the user gets immediate feedback.
 */
export function validateOverlay(
  overlay: PrResolverRuntimeOverlay
): OverlayValidationResult {
  const errors: string[] = [];
  if (overlay.githubRepo !== undefined && overlay.githubRepo !== "") {
    if (!/^[^\s/]+\/[^\s/]+$/.test(overlay.githubRepo)) {
      errors.push(
        `githubRepo must be 'owner/name' format, got: ${overlay.githubRepo}`
      );
    }
  }
  if (overlay.trigger !== undefined && overlay.trigger.trim().length === 0) {
    errors.push(`trigger must be non-empty`);
  }
  const positives: Array<[keyof PrResolverRuntimeOverlay, number | undefined]> = [
    ["pollInterval", overlay.pollInterval],
    ["maxConcurrent", overlay.maxConcurrent],
    ["maxBuildLoops", overlay.maxBuildLoops],
    ["maxCommentsPerCycle", overlay.maxCommentsPerCycle],
  ];
  for (const [name, value] of positives) {
    if (value === undefined) continue;
    if (!Number.isFinite(value) || value < 1) {
      errors.push(`${String(name)} must be a positive integer, got: ${value}`);
    }
  }
  if (
    overlay.enabled === true &&
    overlay.githubRepo !== undefined &&
    overlay.githubRepo === ""
  ) {
    errors.push(`Cannot enable with githubRepo empty`);
  }
  return { ok: errors.length === 0, errors };
}

/**
 * Merge an overlay onto a base config. Only fields explicitly set in the
 * overlay (and not empty strings) override.
 */
export function mergeWithOverlay(
  base: PrResolverConfig,
  overlay: PrResolverRuntimeOverlay
): PrResolverConfig {
  const out: PrResolverConfig = { ...base };
  for (const key of Object.keys(overlay) as Array<keyof PrResolverRuntimeOverlay>) {
    if (!ALLOWED_KEYS.has(key)) continue;
    const value = overlay[key];
    if (value === undefined || value === null) continue;
    if (typeof value === "string" && value.length === 0) continue;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (out as any)[key] = value;
  }
  return out;
}
