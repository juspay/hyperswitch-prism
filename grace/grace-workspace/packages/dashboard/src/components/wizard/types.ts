import type {
  AuthLocation,
  AuthScheme,
  CurrencyUnit,
  DocType,
  Region,
} from "./enums";
import { expandWithPrerequisites, TOGGLE_FORCED_FLOWS } from "./enums";
import type {
  SessionWithTaskInput,
  TaskDefinition,
} from "../UnifiedCreateSessionModal";
import type { SessionCopyStrategy } from "../../hooks/useSessions";

export interface DocUrl {
  title: string;
  url: string;
  type: DocType;
}

export interface DiscoveryFieldMeta {
  sourceUrl?: string;
  confidence: "high" | "medium" | "low";
}

export interface WizardState {
  step: number;

  // Auto-discovery state
  discoveryStatus: "idle" | "running" | "done" | "error" | "cancelled";
  discoveryError: string | null;
  discoveryJobId: string | null;
  discoveryNotes: string | null;
  // Per-field source/confidence metadata. Keyed by WizardState field name.
  discoveryMeta: Record<string, DiscoveryFieldMeta>;
  // Fields the user has manually edited after discovery (so we can show "✎ Edited" badges).
  editedFields: Set<string>;
  // Full tech-spec markdown produced by discovery. When present, launching
  // ships it as task.techSpecMarkdown so preflight writes it to disk and
  // L2_planning can short-circuit both of its LLM phases.
  specMarkdown: string | null;

  // Step 1 — Basics
  connectorName: string;
  displayName: string;
  description: string;
  openApiUrl: string;
  postmanCollectionUrl: string;
  integrationGuideUrl: string;
  docs: DocUrl[];

  // Step 2 — Technical
  authScheme: AuthScheme | "";
  authDetails: string;
  authLocation: AuthLocation | "";
  credentialFields: string[];
  currencyUnit: CurrencyUnit | "";
  baseUrl: string;
  sandboxUrl: string;
  supports3DS: boolean;
  supportsWebhooks: boolean;
  supportsRecurring: boolean;
  supportedFlows: string[];
  primaryPaymentMethod: string;
  selectedPMs: Record<string, string[]>; // category -> sub-methods
  webhookUrlPattern: string;
  regions: Region[];
  supportedCurrencies: string;
  sandboxCredentialsHint: string;

  // Step 3 — Acceptance & Runner
  acceptanceCriteria: string[];
  prerequisites: string[];
  estimatedComplexity: "low" | "medium" | "high";
  priority: "critical" | "high" | "medium" | "low";
  connectorNotes: string;
  runner: "opencode" | "claude-code" | "";
  runnerModel: string;

  // Session-side
  sourcePath: string;
  copyStrategy: SessionCopyStrategy;
}

/**
 * Schema version for the persisted wizard draft. Bump when WizardState shape
 * changes in a way that an old serialized blob can't safely hydrate; old
 * blobs with a different version are dropped on load.
 */
export const WIZARD_DRAFT_VERSION = 1;

/**
 * Serializable subset of WizardState — drops transient discovery state and
 * Sets that don't survive JSON.stringify cleanly. Used by useSessionDraft.
 */
export interface WizardDraft {
  version: number;
  data: Omit<
    WizardState,
    | "step"
    | "discoveryStatus"
    | "discoveryError"
    | "discoveryJobId"
    | "discoveryNotes"
    | "discoveryMeta"
    | "specMarkdown"
    | "editedFields"
  > & {
    editedFields: string[];
  };
}

export function serializeWizardState(s: WizardState): WizardDraft {
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  const {
    step,
    discoveryStatus,
    discoveryError,
    discoveryJobId,
    discoveryNotes,
    discoveryMeta,
    specMarkdown,
    editedFields,
    ...rest
  } = s;
  return {
    version: WIZARD_DRAFT_VERSION,
    data: { ...rest, editedFields: Array.from(editedFields) },
  };
}

export function hydrateWizardState(
  raw: unknown,
  defaultSourcePath: string,
): WizardState | null {
  if (!raw || typeof raw !== "object") return null;
  const draft = raw as Partial<WizardDraft>;
  if (draft.version !== WIZARD_DRAFT_VERSION) return null;
  if (!draft.data || typeof draft.data !== "object") return null;
  const base = makeInitialState(defaultSourcePath);
  const data = draft.data as WizardDraft["data"];
  return {
    ...base,
    ...data,
    editedFields: new Set<string>(Array.isArray(data.editedFields) ? data.editedFields : []),
    // Transient discovery state is always reset to idle on hydrate.
    step: 1,
    discoveryStatus: "idle",
    discoveryError: null,
    discoveryJobId: null,
    discoveryNotes: null,
    discoveryMeta: {},
    specMarkdown: null,
  };
}

export function makeInitialState(defaultSourcePath: string): WizardState {
  return {
    step: 1,
    discoveryStatus: "idle",
    discoveryError: null,
    discoveryJobId: null,
    discoveryNotes: null,
    discoveryMeta: {},
    editedFields: new Set<string>(),
    specMarkdown: null,
    connectorName: "",
    displayName: "",
    description: "",
    openApiUrl: "",
    postmanCollectionUrl: "",
    integrationGuideUrl: "",
    docs: [],
    authScheme: "",
    authDetails: "",
    authLocation: "",
    credentialFields: [],
    currencyUnit: "",
    baseUrl: "",
    sandboxUrl: "",
    supports3DS: false,
    supportsWebhooks: false,
    supportsRecurring: false,
    supportedFlows: ["Authorize"],
    primaryPaymentMethod: "",
    selectedPMs: {},
    webhookUrlPattern: "",
    regions: [],
    supportedCurrencies: "",
    sandboxCredentialsHint: "",
    acceptanceCriteria: [
      "Authorize flow compiles and runs",
      "gRPC Authorize call succeeds against sandbox",
    ],
    prerequisites: [],
    estimatedComplexity: "medium",
    priority: "medium",
    connectorNotes: "",
    runner: "",
    runnerModel: "",
    sourcePath: defaultSourcePath,
    copyStrategy: "git-worktree",
  };
}

export type WizardAction =
  | { type: "set"; patch: Partial<WizardState> }
  | { type: "toggleFlow"; flow: string }
  | { type: "togglePM"; category: string; method: string }
  | { type: "addDoc"; doc: DocUrl }
  | { type: "removeDoc"; index: number }
  | { type: "updateDoc"; index: number; patch: Partial<DocUrl> }
  | { type: "addCriterion"; text: string }
  | { type: "removeCriterion"; index: number }
  | { type: "updateCriterion"; index: number; text: string }
  | { type: "addPrereq"; text: string }
  | { type: "removePrereq"; index: number }
  | { type: "addCredField"; text: string }
  | { type: "removeCredField"; index: number }
  | { type: "setToggle"; key: "supports3DS" | "supportsWebhooks" | "supportsRecurring"; value: boolean }
  | { type: "discoveryStart"; jobId: string }
  | { type: "discoveryError"; error: string }
  | { type: "discoveryCancel" }
  | { type: "discoveryResult"; result: DiscoveryApiResult }
  | { type: "next" }
  | { type: "back" }
  | { type: "goto"; step: number };

/**
 * Shape of result.fields from the /api/discover-connector response.
 * Mirrors core's DiscoveryFields but mirrored here so the dashboard
 * package doesn't pull in core's compiled types.
 */
export interface DiscoveryApiResult {
  connectorName: string;
  connectorDocs: Array<{
    connector: string;
    urls: string[];
    keyDetails: string;
    verificationScore: number;
    verificationStatus: "valid" | "problematic" | "insufficient";
  }>;
  fields: Record<
    string,
    { value: unknown; sourceUrl?: string; confidence: "high" | "medium" | "low" } | undefined
  >;
  /** Full UCS-template tech spec markdown, if discovery produced one. */
  specMarkdown?: string;
  notes?: string;
  sessionId: string;
  durationMs: number;
}

export function wizardReducer(s: WizardState, a: WizardAction): WizardState {
  switch (a.type) {
    case "set":
      return { ...s, ...a.patch };
    case "toggleFlow": {
      const cur = new Set(s.supportedFlows);
      if (cur.has(a.flow)) cur.delete(a.flow);
      else cur.add(a.flow);
      return { ...s, supportedFlows: expandWithPrerequisites(Array.from(cur)) };
    }
    case "togglePM": {
      const cat = s.selectedPMs[a.category] ?? [];
      const next = cat.includes(a.method)
        ? cat.filter((m) => m !== a.method)
        : [...cat, a.method];
      const selectedPMs = { ...s.selectedPMs, [a.category]: next };
      if (next.length === 0) delete selectedPMs[a.category];
      // Clear primary PM if it was deselected
      const stillSelected = Object.entries(selectedPMs)
        .flatMap(([c, ms]) => ms.map((m) => `${c}:${m}`));
      const primary = stillSelected.includes(s.primaryPaymentMethod)
        ? s.primaryPaymentMethod
        : (stillSelected[0] ?? "");
      return { ...s, selectedPMs, primaryPaymentMethod: primary };
    }
    case "addDoc":
      return { ...s, docs: [...s.docs, a.doc] };
    case "removeDoc":
      return { ...s, docs: s.docs.filter((_, i) => i !== a.index) };
    case "updateDoc":
      return {
        ...s,
        docs: s.docs.map((d, i) => (i === a.index ? { ...d, ...a.patch } : d)),
      };
    case "addCriterion":
      return { ...s, acceptanceCriteria: [...s.acceptanceCriteria, a.text] };
    case "removeCriterion":
      return {
        ...s,
        acceptanceCriteria: s.acceptanceCriteria.filter((_, i) => i !== a.index),
      };
    case "updateCriterion":
      return {
        ...s,
        acceptanceCriteria: s.acceptanceCriteria.map((c, i) =>
          i === a.index ? a.text : c,
        ),
      };
    case "addPrereq":
      return { ...s, prerequisites: [...s.prerequisites, a.text] };
    case "removePrereq":
      return {
        ...s,
        prerequisites: s.prerequisites.filter((_, i) => i !== a.index),
      };
    case "addCredField":
      return { ...s, credentialFields: [...s.credentialFields, a.text] };
    case "removeCredField":
      return {
        ...s,
        credentialFields: s.credentialFields.filter((_, i) => i !== a.index),
      };
    case "setToggle": {
      const next = { ...s, [a.key]: a.value };
      // When a toggle flips on, force the corresponding flows in.
      if (a.value) {
        const forcedMap = TOGGLE_FORCED_FLOWS as Record<string, readonly string[]>;
        const forced = forcedMap[a.key];
        if (forced) {
          const flows = new Set([...s.supportedFlows, ...forced]);
          next.supportedFlows = expandWithPrerequisites(Array.from(flows));
        }
      }
      return next;
    }
    case "discoveryStart":
      return {
        ...s,
        discoveryStatus: "running",
        discoveryJobId: a.jobId,
        discoveryError: null,
      };
    case "discoveryError":
      return {
        ...s,
        discoveryStatus: "error",
        discoveryError: a.error,
        discoveryJobId: null,
      };
    case "discoveryCancel":
      return {
        ...s,
        discoveryStatus: "cancelled",
        discoveryJobId: null,
      };
    case "discoveryResult":
      return applyDiscoveryResult(s, a.result);
    case "next":
      return { ...s, step: Math.min(s.step + 1, 4) };
    case "back":
      return { ...s, step: Math.max(s.step - 1, 1) };
    case "goto":
      return { ...s, step: a.step };
  }
}

/**
 * Merges discovery output into wizard state. Only fills fields the user
 * has not manually edited (preserves any in-progress typing). The user
 * can override any field afterward — we'll track it via editedFields.
 */
function applyDiscoveryResult(s: WizardState, r: DiscoveryApiResult): WizardState {
  // Auto-fill description if the user left it empty — discovery's notes (or a
  // generic fallback) gives downstream agents enough context to proceed.
  const autoDescription =
    !s.description.trim() && s.connectorName.trim()
      ? r.notes?.split(/[.!?]/)[0]?.slice(0, 300).trim() ||
        `Auto-discovered integration for ${s.connectorName.trim()}.`
      : s.description;

  const next: WizardState = {
    ...s,
    description: autoDescription,
    discoveryStatus: "done",
    discoveryJobId: null,
    discoveryError: null,
    discoveryNotes: r.notes ?? null,
    specMarkdown: r.specMarkdown ?? null,
  };

  const meta: Record<string, DiscoveryFieldMeta> = { ...s.discoveryMeta };
  const edited = s.editedFields;
  const skipIfEdited = (key: string) => edited.has(key);

  const f = r.fields ?? {};

  const setField = <K extends keyof WizardState>(
    key: K,
    raw: { value: unknown; sourceUrl?: string; confidence: "high" | "medium" | "low" } | undefined,
    value: WizardState[K] | undefined,
  ) => {
    if (!raw || value === undefined || skipIfEdited(key as string)) return;
    (next as unknown as Record<string, unknown>)[key as string] = value;
    meta[key as string] = {
      sourceUrl: raw.sourceUrl,
      confidence: raw.confidence,
    };
  };

  // String enums — only accept if the discovered value is one of the allowed enum members.
  const authScheme = f.authScheme;
  if (authScheme && typeof authScheme.value === "string") {
    setField("authScheme", authScheme, authScheme.value as WizardState["authScheme"]);
  }
  const authLocation = f.authLocation;
  if (authLocation && typeof authLocation.value === "string") {
    setField("authLocation", authLocation, authLocation.value as WizardState["authLocation"]);
  }
  const currencyUnit = f.currencyUnit;
  if (currencyUnit && typeof currencyUnit.value === "string") {
    setField("currencyUnit", currencyUnit, currencyUnit.value as WizardState["currencyUnit"]);
  }

  const credentialFields = f.credentialFields;
  if (credentialFields && Array.isArray(credentialFields.value)) {
    setField("credentialFields", credentialFields, credentialFields.value as string[]);
  }

  const baseUrl = f.baseUrl;
  if (baseUrl && typeof baseUrl.value === "string") {
    setField("baseUrl", baseUrl, baseUrl.value);
  }
  const sandboxUrl = f.sandboxUrl;
  if (sandboxUrl && typeof sandboxUrl.value === "string") {
    setField("sandboxUrl", sandboxUrl, sandboxUrl.value);
  }

  // supportedFlows — merge with any user-locked flows (Authorize is default).
  const supportedFlows = f.supportedFlows;
  if (supportedFlows && Array.isArray(supportedFlows.value) && !skipIfEdited("supportedFlows")) {
    const merged = expandWithPrerequisites([
      ...((supportedFlows.value as string[]) ?? []),
      "Authorize",
    ]);
    next.supportedFlows = merged;
    meta["supportedFlows"] = {
      sourceUrl: supportedFlows.sourceUrl,
      confidence: supportedFlows.confidence,
    };
  }

  // supportedPaymentMethodsByConnector → maps into selectedPMs (category -> [methods]).
  const pmByConnector = f.supportedPaymentMethodsByConnector;
  if (pmByConnector && pmByConnector.value && typeof pmByConnector.value === "object" && !skipIfEdited("selectedPMs")) {
    const valueObj = pmByConnector.value as Record<string, string[]>;
    const flat = Object.values(valueObj).flat();
    const selectedPMs: Record<string, string[]> = {};
    for (const cm of flat) {
      if (typeof cm !== "string") continue;
      const [cat, method] = cm.split(":");
      if (!cat || !method) continue;
      if (!selectedPMs[cat]) selectedPMs[cat] = [];
      if (!selectedPMs[cat].includes(method)) selectedPMs[cat].push(method);
    }
    next.selectedPMs = selectedPMs;
    // Pre-pick primary PM as the first one if none chosen.
    if (!s.primaryPaymentMethod && flat[0]) {
      next.primaryPaymentMethod = flat[0];
    }
    meta["selectedPMs"] = {
      sourceUrl: pmByConnector.sourceUrl,
      confidence: pmByConnector.confidence,
    };
  }

  const supports3DS = f.supports3DS;
  if (supports3DS && typeof supports3DS.value === "boolean") {
    setField("supports3DS", supports3DS, supports3DS.value);
  }
  const supportsWebhooks = f.supportsWebhooks;
  if (supportsWebhooks && typeof supportsWebhooks.value === "boolean") {
    setField("supportsWebhooks", supportsWebhooks, supportsWebhooks.value);
  }
  const supportsRecurring = f.supportsRecurring;
  if (supportsRecurring && typeof supportsRecurring.value === "boolean") {
    setField("supportsRecurring", supportsRecurring, supportsRecurring.value);
  }

  const webhookUrlPattern = f.webhookUrlPattern;
  if (webhookUrlPattern && typeof webhookUrlPattern.value === "string") {
    setField("webhookUrlPattern", webhookUrlPattern, webhookUrlPattern.value);
  }
  const regions = f.regions;
  if (regions && Array.isArray(regions.value)) {
    setField("regions", regions, regions.value as WizardState["regions"]);
  }
  const supportedCurrencies = f.supportedCurrencies;
  if (supportedCurrencies && Array.isArray(supportedCurrencies.value)) {
    // Wizard stores currencies as a comma-separated string.
    setField("supportedCurrencies", supportedCurrencies, (supportedCurrencies.value as string[]).join(", "));
  }
  const sandboxCredentialsHint = f.sandboxCredentialsHint;
  if (sandboxCredentialsHint && typeof sandboxCredentialsHint.value === "string") {
    setField("sandboxCredentialsHint", sandboxCredentialsHint, sandboxCredentialsHint.value);
  }

  // Promote first discovered URL into structured docs (api_reference type) so
  // task.connectorDocs[] lands populated when we submit.
  if (r.connectorDocs && r.connectorDocs.length > 0 && !skipIfEdited("docs")) {
    const flat = r.connectorDocs.flatMap((d) => d.urls.slice(0, 3));
    const newDocs = flat.slice(0, 5).map((url, idx) => ({
      title: idx === 0 ? "Discovered Doc" : `Discovered Doc ${idx + 1}`,
      url,
      type: "api_reference" as const,
    }));
    // Don't overwrite if user already added docs.
    if (s.docs.length === 0) {
      next.docs = newDocs;
      meta["docs"] = { confidence: "medium" };
    }
  }

  next.discoveryMeta = meta;
  return next;
}

// ===== validation =====

export function hasApiReference(s: WizardState): boolean {
  if (s.openApiUrl.trim() || s.postmanCollectionUrl.trim()) return true;
  return s.docs.some((d) => d.type === "api_reference" && d.url.trim());
}

export function canAdvanceStep(s: WizardState): boolean {
  switch (s.step) {
    case 1:
      return (
        s.connectorName.trim().length > 0 &&
        s.description.trim().length > 0 &&
        hasApiReference(s)
      );
    case 2: {
      const hasAuthorize = s.supportedFlows.includes("Authorize");
      const pmCount = Object.values(s.selectedPMs).reduce(
        (n, arr) => n + arr.length,
        0,
      );
      return (
        s.authScheme !== "" &&
        s.authLocation !== "" &&
        s.currencyUnit !== "" &&
        hasAuthorize &&
        pmCount >= 1 &&
        s.primaryPaymentMethod !== ""
      );
    }
    case 3:
      return (
        s.acceptanceCriteria.filter((c) => c.trim()).length >= 2 &&
        s.runner !== ""
      );
    case 4:
      return true;
    default:
      return false;
  }
}

export function canSubmit(s: WizardState): boolean {
  return (
    canAdvanceStep({ ...s, step: 1 }) &&
    canAdvanceStep({ ...s, step: 2 }) &&
    canAdvanceStep({ ...s, step: 3 }) &&
    s.sourcePath.trim().length > 0 &&
    // Block ONLY while discovery is actively running. If it completed without
    // a spec, we still allow launch — the pipeline's L2 will generate one
    // the normal way (no short-circuit, but no blocker either).
    s.discoveryStatus !== "running"
  );
}

// ===== builder =====

export function buildSessionWithTaskInput(s: WizardState): SessionWithTaskInput {
  const [primaryCategory, primaryMethod] = s.primaryPaymentMethod.split(":");
  const category = primaryCategory ?? "";
  const paymentMethod = primaryMethod ?? primaryCategory ?? "";

  const supportedPMsByConnector: Record<string, string[]> = {
    [s.connectorName.trim()]: Object.entries(s.selectedPMs).flatMap(([c, ms]) =>
      ms.map((m) => `${c}:${m}`),
    ),
  };

  const connectorDocs: TaskDefinition["connectorDocs"] = [
    {
      connector: s.connectorName.trim(),
      urls: [
        ...(s.openApiUrl.trim()
          ? [
              {
                title: "OpenAPI Spec",
                url: s.openApiUrl.trim(),
                type: "api_reference" as DocType,
              },
            ]
          : []),
        ...(s.postmanCollectionUrl.trim()
          ? [
              {
                title: "Postman Collection",
                url: s.postmanCollectionUrl.trim(),
                type: "api_reference" as DocType,
              },
            ]
          : []),
        ...(s.integrationGuideUrl.trim()
          ? [
              {
                title: "Integration Guide",
                url: s.integrationGuideUrl.trim(),
                type: "payment_method_guide" as DocType,
              },
            ]
          : []),
        ...s.docs
          .filter((d) => d.url.trim())
          .map((d) => ({
            title: d.title.trim() || d.url.trim(),
            url: d.url.trim(),
            type: d.type,
          })),
      ],
    },
  ];

  // The new-connector wizard is the canonical entrypoint for the
  // `.gracerules`-aligned pipeline: scaffold → per-flow subagents (no
  // L2/L3 review gates, since the techspec was captured up-front). The
  // engine's `buildPipeline(task)` keys off this discriminator.
  const workflowType: TaskDefinition["workflowType"] = "new-connector";

  // Map the wizard's `supportedFlows` to the canonical `.gracerules` flow
  // names so `buildPipeline()` can fan out one checkpoint per flow.
  // Falls back to all 6 core flows when the wizard didn't capture an
  // explicit subset.
  const flows: string[] | undefined =
    s.supportedFlows && s.supportedFlows.length > 0
      ? s.supportedFlows
      : undefined;

  const initialTask: TaskDefinition = {
    title: `Add ${s.connectorName.trim()} connector`,
    description: s.description.trim(),
    workflowType,
    flows,
    paymentMethod,
    category,
    targetConnectors: [s.connectorName.trim()],
    priority: s.priority,
    acceptanceCriteria: s.acceptanceCriteria.filter((c) => c.trim()),
    runner: (s.runner || "claude-code") as "opencode" | "claude-code",
    runnerModel: s.runnerModel.trim() || undefined,

    // Wizard-only structured fields
    authScheme: (s.authScheme || undefined) as TaskDefinition["authScheme"],
    authDetails: s.authDetails.trim() || undefined,
    authLocation: (s.authLocation || undefined) as TaskDefinition["authLocation"],
    credentialFields: s.credentialFields.length ? s.credentialFields : undefined,
    currencyUnit: (s.currencyUnit || undefined) as TaskDefinition["currencyUnit"],
    baseUrl: s.baseUrl.trim() || undefined,
    sandboxUrl: s.sandboxUrl.trim() || undefined,
    supportedFlows: s.supportedFlows,
    supportedPaymentMethodsByConnector: supportedPMsByConnector,
    supports3DS: s.supports3DS || undefined,
    supportsWebhooks: s.supportsWebhooks || undefined,
    supportsRecurring: s.supportsRecurring || undefined,
    openApiUrl: s.openApiUrl.trim() || undefined,
    postmanCollectionUrl: s.postmanCollectionUrl.trim() || undefined,
    integrationGuideUrl: s.integrationGuideUrl.trim() || undefined,
    sandboxCredentialsHint: s.sandboxCredentialsHint.trim() || undefined,
    webhookUrlPattern: s.webhookUrlPattern.trim() || undefined,
    regions: s.regions.length ? s.regions : undefined,
    supportedCurrencies: s.supportedCurrencies.trim()
      ? s.supportedCurrencies.split(",").map((c) => c.trim()).filter(Boolean)
      : undefined,
    connectorNotes: s.connectorNotes.trim() || undefined,
    prerequisites: s.prerequisites.length ? s.prerequisites : undefined,
    estimatedComplexity: s.estimatedComplexity,
    connectorDocs,
    // If discovery produced a spec, ship it as-is. Preflight writes it to the
    // canonical grace path, and L2_planning short-circuits both its phases.
    techSpecMarkdown: s.specMarkdown ?? undefined,
    // Flat URL list (matches grace/workflow/2.1_links.md output contract).
    // Preflight merges these into <projectRoot>/data/integration-source-links.json
    // keyed by connector name. Sourced from both the primary doc fields (Step 1)
    // and the structured connectorDocs[] entries.
    discoveredConnectorUrls: [
      s.openApiUrl.trim(),
      s.postmanCollectionUrl.trim(),
      s.integrationGuideUrl.trim(),
      ...connectorDocs[0].urls.map((u) => u.url),
    ]
      .filter((u) => u && u.length > 0)
      .filter((u, i, arr) => arr.indexOf(u) === i),
  };

  return {
    name: `${s.connectorName.trim()} integration`,
    description: s.description.trim() || undefined,
    sourcePath: s.sourcePath.trim(),
    strategy: s.copyStrategy,
    initialTask,
  };
}
