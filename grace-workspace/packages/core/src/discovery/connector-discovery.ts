import { runClaudeCode } from "../tools/claude-code-runner.js";
import {
  CONNECTOR_DISCOVERY_SYSTEM,
  buildConnectorDiscoveryUserPayload,
} from "../generators/connector-discovery-prompt.js";
import { deriveClaudeSessionId } from "../checkpoints/session-id.js";
import type { ConnectorDocFinding } from "../types.js";
import { getConfig } from "../config.js";

export type Confidence = "high" | "medium" | "low";

export interface DiscoveryField<T> {
  value: T;
  sourceUrl?: string;
  confidence: Confidence;
}

export interface DiscoveryFields {
  authScheme?: DiscoveryField<string>;
  authLocation?: DiscoveryField<string>;
  credentialFields?: DiscoveryField<string[]>;
  currencyUnit?: DiscoveryField<string>;
  baseUrl?: DiscoveryField<string>;
  sandboxUrl?: DiscoveryField<string>;
  supportedFlows?: DiscoveryField<string[]>;
  supportedPaymentMethodsByConnector?: DiscoveryField<Record<string, string[]>>;
  supports3DS?: DiscoveryField<boolean>;
  supportsWebhooks?: DiscoveryField<boolean>;
  supportsRecurring?: DiscoveryField<boolean>;
  webhookUrlPattern?: DiscoveryField<string>;
  regions?: DiscoveryField<string[]>;
  supportedCurrencies?: DiscoveryField<string[]>;
  sandboxCredentialsHint?: DiscoveryField<string>;
}

export interface DiscoveryResult {
  connectorName: string;
  connectorDocs: ConnectorDocFinding[];
  fields: DiscoveryFields;
  /** Full UCS-template tech spec markdown, if the LLM produced one. */
  specMarkdown?: string;
  notes?: string;
  /** Claude session id of the discovery call (for resume on re-run). */
  sessionId: string;
  /** Wall-clock duration in ms. */
  durationMs: number;
}

interface RawAgentResult {
  connectorDocs?: Array<{
    title?: string;
    url?: string;
    type?: string;
    verificationScore?: number;
    verificationStatus?: string;
  }>;
  fields?: Record<string, { value: unknown; sourceUrl?: string; confidence?: string } | undefined>;
  specMarkdown?: string;
  notes?: string;
}

export interface DiscoverConnectorOptions {
  connectorName: string;
  /** cwd for the runner. Defaults to workspace root (where grace lives). */
  cwd?: string;
  /** Override model. */
  model?: string;
  /** Hard timeout, default 5 min. */
  timeoutMs?: number;
  /** Per-line progress callback. v1 emits coarse milestones only. */
  onProgress?: (msg: string) => void;
}

/**
 * Wizard-side connector discovery: web-search + extract structured fields.
 *
 * Reuses `runAI()` (claude-code runner has built-in WebSearch + WebFetch tools).
 * Returns a typed DiscoveryResult; the dashboard pre-populates the wizard's
 * Review step from this, and the human edits/confirms before launching the
 * pipeline. Downstream, the populated `task.connectorDocs[]` lets L2 Phase 1
 * skip its own web search.
 */
export async function discoverConnector(
  opts: DiscoverConnectorOptions,
): Promise<DiscoveryResult> {
  const name = opts.connectorName.trim();
  if (!name) throw new Error("connectorName is required");

  // Default cwd: the workspace root that hosts grace/.
  const cwd =
    opts.cwd ??
    process.env.TENXGRACE_WORKSPACE_ROOT ??
    process.cwd();

  // Note: we intentionally don't pass `preferredSessionId` here. The previous
  // version derived a stable UUID from (name, "wizard", "discovery") so re-runs
  // could resume the prior conversation, but Claude rejects starting a new
  // session with a UUID already in its store ("Session ID is already in use").
  // Letting the runner generate a fresh UUID per call trades the resume
  // capability for retry reliability.
  opts.onProgress?.(`Searching the web for ${name} API documentation…`);

  const cfg = getConfig();
  const started = Date.now();

  const { result, sessionId } = await runClaudeCode<RawAgentResult>({
    skillBody: CONNECTOR_DISCOVERY_SYSTEM,
    userPayload: buildConnectorDiscoveryUserPayload(name),
    cwd,
    label: "wizard:discover",
    sessionLabel: `${slugify(name)}-discovery-${Date.now().toString(36)}`,
    timeoutMs: opts.timeoutMs ?? 15 * 60_000,
    model: opts.model ?? cfg.claudeCode?.model ?? cfg.llm?.model,
  });

  opts.onProgress?.("Extracting structured fields…");

  const docs = normalizeDocs(result.connectorDocs);
  const fields = normalizeFields(result.fields);
  const specMarkdown =
    typeof result.specMarkdown === "string" && result.specMarkdown.trim().length > 100
      ? result.specMarkdown
      : undefined;

  return {
    connectorName: name,
    connectorDocs: docs,
    fields,
    specMarkdown,
    notes: typeof result.notes === "string" ? result.notes : undefined,
    sessionId,
    durationMs: Date.now() - started,
  };
}

// --- Normalizers ----------------------------------------------------------
//
// The model is instructed to return strict JSON, but trust nothing. Drop
// malformed entries silently — the wizard's review step is the ultimate
// validator (the human sees and corrects whatever survives).

function normalizeDocs(
  raw: RawAgentResult["connectorDocs"] | undefined,
): ConnectorDocFinding[] {
  if (!Array.isArray(raw)) return [];
  const findings: ConnectorDocFinding[] = [];
  const byConnector = new Map<string, ConnectorDocFinding>();
  for (const r of raw) {
    if (!r || typeof r.url !== "string" || !r.url.trim()) continue;
    const score = typeof r.verificationScore === "number" ? r.verificationScore : 5;
    const status: ConnectorDocFinding["verificationStatus"] =
      r.verificationStatus === "valid" || r.verificationStatus === "problematic" || r.verificationStatus === "insufficient"
        ? r.verificationStatus
        : score >= 7
        ? "valid"
        : score >= 4
        ? "problematic"
        : "insufficient";
    // ConnectorDocFinding groups urls by connector. Discovery returns one URL per row
    // so we collapse them under the discovered connector name later via the caller.
    const key = "DISCOVERED";
    const existing = byConnector.get(key);
    if (existing) {
      existing.urls.push(r.url.trim());
    } else {
      const f: ConnectorDocFinding = {
        connector: key,
        urls: [r.url.trim()],
        keyDetails: r.title ?? r.type ?? "",
        verificationScore: score,
        verificationStatus: status,
      };
      byConnector.set(key, f);
      findings.push(f);
    }
  }
  return findings;
}

const ALLOWED_AUTH_SCHEMES = new Set(["APIKey", "OAuth2", "BasicAuth", "Signature", "JWT", "Custom"]);
const ALLOWED_AUTH_LOCATIONS = new Set(["Header", "Query", "Body", "Custom"]);
const ALLOWED_CURRENCY_UNITS = new Set(["Minor", "StringMinor", "StringMajor", "Base"]);
const ALLOWED_CONFIDENCES = new Set<Confidence>(["high", "medium", "low"]);

function isConfidence(v: unknown): v is Confidence {
  return typeof v === "string" && ALLOWED_CONFIDENCES.has(v as Confidence);
}

function pickField<T>(
  raw: { value: unknown; sourceUrl?: string; confidence?: string } | undefined,
  predicate: (v: unknown) => v is T,
): DiscoveryField<T> | undefined {
  if (!raw) return undefined;
  if (!predicate(raw.value)) return undefined;
  return {
    value: raw.value,
    sourceUrl: typeof raw.sourceUrl === "string" ? raw.sourceUrl : undefined,
    confidence: isConfidence(raw.confidence) ? raw.confidence : "low",
  };
}

const isString = (v: unknown): v is string => typeof v === "string" && v.length > 0;
const isStringArray = (v: unknown): v is string[] =>
  Array.isArray(v) && v.every((s) => typeof s === "string" && s.length > 0);
const isBoolean = (v: unknown): v is boolean => typeof v === "boolean";
const isRecordOfStringArrays = (v: unknown): v is Record<string, string[]> =>
  !!v && typeof v === "object" && !Array.isArray(v) &&
  Object.values(v).every((arr) => isStringArray(arr));

function normalizeFields(raw: RawAgentResult["fields"] | undefined): DiscoveryFields {
  if (!raw || typeof raw !== "object") return {};
  const f: DiscoveryFields = {};

  const authScheme = pickField<string>(raw.authScheme, (v): v is string =>
    typeof v === "string" && ALLOWED_AUTH_SCHEMES.has(v));
  if (authScheme) f.authScheme = authScheme;

  const authLocation = pickField<string>(raw.authLocation, (v): v is string =>
    typeof v === "string" && ALLOWED_AUTH_LOCATIONS.has(v));
  if (authLocation) f.authLocation = authLocation;

  const credentialFields = pickField<string[]>(raw.credentialFields, isStringArray);
  if (credentialFields) f.credentialFields = credentialFields;

  const currencyUnit = pickField<string>(raw.currencyUnit, (v): v is string =>
    typeof v === "string" && ALLOWED_CURRENCY_UNITS.has(v));
  if (currencyUnit) f.currencyUnit = currencyUnit;

  const baseUrl = pickField<string>(raw.baseUrl, isString);
  if (baseUrl) f.baseUrl = baseUrl;

  const sandboxUrl = pickField<string>(raw.sandboxUrl, isString);
  if (sandboxUrl) f.sandboxUrl = sandboxUrl;

  const supportedFlows = pickField<string[]>(raw.supportedFlows, isStringArray);
  if (supportedFlows) f.supportedFlows = supportedFlows;

  const supportedPMs = pickField<Record<string, string[]>>(
    raw.supportedPaymentMethodsByConnector,
    isRecordOfStringArrays,
  );
  if (supportedPMs) f.supportedPaymentMethodsByConnector = supportedPMs;

  const supports3DS = pickField<boolean>(raw.supports3DS, isBoolean);
  if (supports3DS) f.supports3DS = supports3DS;

  const supportsWebhooks = pickField<boolean>(raw.supportsWebhooks, isBoolean);
  if (supportsWebhooks) f.supportsWebhooks = supportsWebhooks;

  const supportsRecurring = pickField<boolean>(raw.supportsRecurring, isBoolean);
  if (supportsRecurring) f.supportsRecurring = supportsRecurring;

  const webhookUrlPattern = pickField<string>(raw.webhookUrlPattern, isString);
  if (webhookUrlPattern) f.webhookUrlPattern = webhookUrlPattern;

  const regions = pickField<string[]>(raw.regions, isStringArray);
  if (regions) f.regions = regions;

  const supportedCurrencies = pickField<string[]>(raw.supportedCurrencies, isStringArray);
  if (supportedCurrencies) f.supportedCurrencies = supportedCurrencies;

  const sandboxCredentialsHint = pickField<string>(raw.sandboxCredentialsHint, isString);
  if (sandboxCredentialsHint) f.sandboxCredentialsHint = sandboxCredentialsHint;

  return f;
}

function slugify(s: string): string {
  return s.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/(^-|-$)/g, "");
}
