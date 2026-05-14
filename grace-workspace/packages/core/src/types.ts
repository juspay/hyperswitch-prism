export type CheckpointId =
  | "task"
  | "preflight"
  | "requirements"
  | "product_alignment"
  | "feature_research"
  | "design_gate"
  | "l2_planning"
  | "l2_review"
  | "l3_analysis"
  | "l3_review"
  | "implementation"
  | "compiler"
  | "compiler_check"
  | "grpc_test"
  | "design_match"
  | "cypress"
  | "playwright"
  | "pr_review"
  | "regression";

export type CheckpointStatus =
  | "idle"
  | "running"
  | "passed"
  | "failed"
  | "skipped"
  | "waiting_for_retry";

export interface CheckpointResult {
  passed: boolean;
  output?: string;
  artifacts?: Record<string, unknown>;
  errors?: string[];
}

/**
 * One row per checkpoint attempt — recorded by the engine on every
 * pass-or-fail completion, so the dashboard can replay retry history even
 * after a browser refresh or for a completed run loaded from disk.
 *
 * `attemptIndex` matches the engine's `retries` counter at the moment the
 * checkpoint executed: 0 for the first attempt, 1 for the first retry, etc.
 */
export interface AttemptRecord {
  runId: string;
  checkpointId: CheckpointId;
  attemptIndex: number;
  status: "passed" | "failed";
  errors?: string[];
  output?: string | null;
  artifacts?: Record<string, unknown> | null;
  startedAt?: number;
  completedAt: number;
}

export interface Checkpoint {
  id: CheckpointId;
  name: string;
  description: string;
  retryFrom: CheckpointId;
  run: (ctx: PipelineContext) => Promise<CheckpointResult>;
  onFail?: (ctx: PipelineContext, result: CheckpointResult) => Promise<void>;
  maxRetries?: number;
  timeout?: number;
  /** If true, continue to next checkpoint when max retries exceeded instead of aborting */
  continueOnFailure?: boolean;
}

export interface TaskAttachment {
  /** Original file name as uploaded. */
  name: string;
  /** MIME type, best-effort. */
  mimeType: string;
  /** UTF-8 text contents (for text files). Either this or `dataBase64` is set. */
  text?: string;
  /** Base64-encoded contents (for binary files like images / PDFs). */
  dataBase64?: string;
  /** Original byte size. */
  size: number;
}

/**
 * GRACE WORKFLOW: Payment Method Categories
 */
export type PaymentMethodCategory =
  | "card"
  | "wallet"
  | "bank_transfer"
  | "bank_debit"
  | "bnpl"
  | "crypto"
  | "voucher"
  | "gift_card"
  | "pay_later"
  | string;

/**
 * GRACE WORKFLOW: Task Definition for Payment Method Implementation
 *
 * Like Grace's connector implementation workflow:
 * - {PAYMENT_METHOD}: The payment method to implement (Card, Wallet, etc.)
 * - {TARGET_CONNECTORS}: Array of connector names to implement for
 * - {CONNECTORS_FILE}: Equivalent to feature/target specification
 */
export interface TaskDefinition {
  /** Task title */
  title: string;
  /** Detailed description */
  description: string;
  /** Acceptance criteria */
  acceptanceCriteria: string[];
  /** Connector reference document URLs */
  connectorDocUrls?: string[];
  /** Target file paths */
  targetFiles?: string[];
  /**
   * Project root. For a session-scoped run this is the per-session worktree
   * path resolved by SessionManager — checkpoints stay agnostic to sessions
   * and just consume this field.
   */
  projectRoot: string;
  /**
   * Owning session. Defaults to "default" for runs created before the
   * session-management migration. Stored on the run row via
   * StateManager.save() and used by engine.run() for the claim/release
   * lock dance.
   */
  sessionId?: string;
  /**
   * Phase 10: per-session gRPC server port. Computed at engine boot from
   * session.portSlot as `8000 + portSlot`. Checkpoints (grpc-test,
   * grpc-server-lifecycle) read this so concurrent sessions don't fight
   * for the same listener. Defaults to 8000 if unset.
   */
  grpcPort?: number;
  /**
   * Phase 10: per-session dummy-connector HTTP port. Computed at engine
   * boot from session.portSlot as `8080 + portSlot`. Preflight rewrites
   * `<projectRoot>/config/development.toml` so the connector's
   * `base_url` points at this port. Defaults to 8080 if unset.
   */
  dummyConnectorPort?: number;
  /** Files uploaded with the task */
  attachments?: TaskAttachment[];

  // ========== AI RUNNER CONFIGURATION ==========

  /** AI runner to use for this task - "opencode" or "claude-code" */
  runner?: "opencode" | "claude-code";
  /** Optional model override for the selected runner */
  runnerModel?: string;

  // ========== GRACE/10XGRACE WORKFLOW FIELDS ==========

  /**
   * GRACE: {PAYMENT_METHOD} equivalent
   * The payment method to implement (Card, Wallet, BankTransfer, etc.)
   */
  paymentMethod?: string;

  /**
   * GRACE: Flow / API to implement (e.g. "Pay.Capture", "Pay.Refund",
   * "Event.HandleEvent"). Used when the task targets a connector flow
   * gap instead of a payment-method gap. Mutually exclusive with
   * paymentMethod in practice but both fields are optional.
   */
  flow?: string;

  /**
   * GRACE: Target connectors
   * Array of connector names to implement this payment method for
   * Like Grace's {CONNECTORS_FILE}
   */
  targetConnectors?: string[];

  /**
   * GRACE: Payment method category
   */
  paymentMethodCategory?: PaymentMethodCategory;

  /**
   * GRACE: Priority classification
   */
  priority?: "critical" | "high" | "medium" | "low";

  /**
   * GRACE: Connector documentation URLs
   */
  connectorDocs?: Array<{
    connector: string;
    urls: Array<{
      title: string;
      url: string;
      type:
        | "api_reference"
        | "payment_method_guide"
        | "authentication_guide"
        | "webhooks_guide"
        | "testing_guide"
        | "error_reference";
      verified?: boolean;
    }>;
  }>;

  /**
   * GRACE: Implementation prerequisites
   */
  prerequisites?: string[];

  /**
   * GRACE: Estimated complexity
   */
  estimatedComplexity?: "low" | "medium" | "high";

  /**
   * 10XGRACE: Requirements discovery results
   * Populated by Requirements checkpoint
   */
  requirements?: {
    status?: "valid" | "problematic" | "insufficient";
    overallScore?: number;
    connectorsAnalyzed?: Array<{
      name: string;
      files: {
        root: string;
        mainFiles: string[];
        transformers: string[];
        types: string[];
      };
      currentPaymentMethods: string[];
      filesToModify: Array<{
        path: string;
        reason: string;
        changeType: "modify" | "create";
      }>;
      patterns: {
        existingMethod: string;
        registrationPattern: string;
        transformerPattern: string;
      };
      score: number;
      keyGaps?: string[];
    }>;
    commonPatterns?: {
      registration?: string;
      transformers?: string;
    };
    recommendations?: string[];
  };
}

export interface ConnectorDocFinding {
  connector: string;
  urls: string[];
  keyDetails: string;
  /** 10-point verification score (0-10) */
  verificationScore: number;
  /** Status based on verification score */
  verificationStatus: "valid" | "problematic" | "insufficient";
}

export interface L2ResearchFinding {
  connectorDocs: ConnectorDocFinding[];
  paymentMethodInfo: {
    source: string;
    details: string;
  };
  implementationPatterns: string[];
  /** Documentation gaps identified during 10-point verification */
  documentationGaps?: string[];
}

/**
 * GRACE WORKFLOW: Detailed logging for L2 generation phases
 */
export interface WorkflowExecutionLog {
  phase: "links_discovery" | "techspec_generation";
  workflowFile: string;
  readAt: string;
  output: string;
  status: "success" | "failed";
}

export interface WebSearchResult {
  title: string;
  url: string;
  snippet?: string;
}

export interface WebSearchQuery {
  query: string;
  timestamp: string;
  results: WebSearchResult[];
  resultCount: number;
}

export interface CommandExecution {
  command: string;
  workingDir: string;
  output?: string;
  durationMs?: number;
  status: "success" | "failed";
}

export interface FileCreated {
  path: string;
  description: string;
  sizeBytes?: number;
}

export interface L2GenerationLog {
  workflowExecutions: WorkflowExecutionLog[];
  webSearchQueries: WebSearchQuery[];
  filesCreated: FileCreated[];
  commandsExecuted: CommandExecution[];
}

export interface L2Plan {
  summary: string;
  scope: string;
  outOfScope: string;
  technicalConstraints: string[];
  estimatedComplexity: "low" | "medium" | "high";
  researchFindings?: L2ResearchFinding;
  generationLog?: L2GenerationLog;
  /** Full technical specification content from Tech Spec Agent */
  specContent?: string;
}

/** @deprecated Use L2Plan instead */
export type L2Spec = L2Plan;

/**
 * Codegen Fix Log Entry for tracking build/test loop iterations
 */
export interface CodegenFixLogEntry {
  iteration: number;
  error: string;
  fileChanged: string;
  changeDescription: string;
}

/**
 * Codegen Execution Log for tracking phases and commands
 */
export interface CodegenExecutionLog {
  phasesCompleted: string[];
  commandsExecuted: CommandExecution[];
  serverLogsChecked: boolean;
}

/**
 * Codegen Result from 2.3_codegen.md workflow
 */
export interface CodegenResult {
  success: boolean;
  connector: string;
  flow: string;
  buildIterations: number;
  grpcurlResult: "PASS" | "FAIL" | "NOT_RUN";
  filesModified: string[];
  fixLog: CodegenFixLogEntry[];
  grpcurlOutput: string;
  executionLog: CodegenExecutionLog;
  reason?: string;
}

/**
 * Field specification for Request/Response structs
 */
export interface FieldSpec {
  /** Snake_case Rust field name */
  name: string;
  /** Original field name from API spec (camelCase) */
  originalName: string;
  /** Rust type (String, i64, bool, Option<String>, etc.) */
  type: string;
  /** true if spec marks it required */
  required: boolean;
  /** Full serde attribute if different from field name */
  serdeAnnotation?: string;
  /** Description copied/adapted from L2 spec */
  doc: string;
  /** How to get this value from RouterDataV2 */
  sourceMapping: string;
}

/**
 * Request or Response struct specification
 */
export interface RequestResponseSpec {
  /** Struct name, e.g., "StripeAuthorizeRequest" */
  name: string;
  /** Derive macros, e.g., ["Serialize", "Debug"] */
  derives: string[];
  /** Doc comment for the struct */
  doc: string;
  /** Field specifications */
  fields: FieldSpec[];
}

/**
 * Individual field mapping for TryFrom
 */
export interface FieldMapping {
  /** Source path in RouterDataV2, e.g., "payment_data.amount" */
  source: string;
  /** Target field name in struct */
  target: string;
  /** Transformation description, e.g., "Convert minor to major units" */
  transformation: string;
  /** Rust type */
  type: string;
}

/**
 * TryFrom implementation specification
 */
export interface TryFromImplSpec {
  /** "request" for RouterDataV2→Request, "response" for Response→RouterDataV2 */
  implType: "request" | "response";
  /** Source type */
  from: string;
  /** Target type */
  to: string;
  /** Field-by-field mappings */
  mappings: FieldMapping[];
  /** Special handling notes */
  specialHandling: string[];
}

/**
 * Macro connector implementation parameters
 */
export interface MacroParamsSpec {
  flow: string;
  httpMethod: "GET" | "POST" | "PUT" | "PATCH" | "DELETE";
  urlPath: string;
  contentType: string;
  headers: string[];
  requiresCurlRequest: boolean;
}

/**
 * Changes to connector.rs
 */
export interface ConnectorChangesSpec {
  /** Flow enum variant name, e.g., "AuthorizeV2" */
  flowEnumVariant: string;
  /** Exact syntax to add to create_all_prerequisites! */
  createAllPrerequisitesAddition: string;
  macroInvocation: {
    macroName: string;
    parameters: MacroParamsSpec;
  };
}

/**
 * Supporting type (enum/struct) specification
 */
export interface SupportingTypeSpec {
  name: string;
  type: "Enum" | "Struct" | "TypeAlias";
  variants?: string[];
  fields?: FieldSpec[];
  purpose: string;
}

/**
 * Status mapping specification
 */
export interface StatusMappingSpec {
  /** All possible status values from connector */
  connectorStatuses: string[];
  /** How mapping is determined */
  mappingLogic: string;
  /** Connector status → Hyperswitch status */
  mappings: Record<string, string>;
  /** Fallback status if no match */
  fallback: string;
  /** Error case mappings */
  errorCases?: Record<string, string>;
}

/**
 * Ambiguous/unclear specification item
 */
export interface AmbiguousSpec {
  field: string;
  issue: string;
  recommendation: string;
}

/**
 * GitHub-style file change preview for review UI
 */
export interface FileChangePreview {
  /** File path relative to project root */
  path: string;
  /** Type of change */
  changeType: "modified" | "created" | "deleted";
  /** Estimated lines to be added */
  linesAdded: number;
  /** Estimated lines to be removed */
  linesRemoved: number;
  /** Human-readable description of what changes */
  description: string;
  /** Code snippet preview (optional) */
  previewSnippet?: string;
}

/**
 * Complete implementation specification from L3 Analysis
 */
export interface ImplementationSpecification {
  requestStruct: RequestResponseSpec;
  responseStruct: RequestResponseSpec;
  tryFromImplementations: TryFromImplSpec[];
  connectorChanges: ConnectorChangesSpec;
  supportingTypes: SupportingTypeSpec[];
  statusMapping: StatusMappingSpec;
  ambiguities?: AmbiguousSpec[];
  filesChangedPreview: FileChangePreview[];
}

/**
 * Implementation type classification for GRACE workflow
 * Distinguishes between new flows and payment method additions
 */
export type ImplementationType =
  | "new_flow"
  | "payment_method_addition"
  | "flow_completion";

/**
 * GRACE WORKFLOW: L3 Analysis Result (Phase 4 from 2.3_codegen.md)
 * Analyzes patterns, files, and prerequisites for implementation
 */
export interface L3Analysis {
  connector: string;
  flow: string;
  paymentMethod?: string;
  implementationType: ImplementationType;
  parentFlow?: string;
  analysis: {
    l2SpecVersion?: string;
    patternsIdentified: string[];
    filesToModify: string[];
    existingFlows: string[];
    flowAlreadyExists?: boolean;
    prerequisitesStatus: "complete" | "incomplete";
    missingPrerequisites?: string[];
  };
  specification: ImplementationSpecification;
  implementationNotes: string;
  riskAssessment?: string[];
  executionLog: {
    filesRead: string[];
    analysisComplete: boolean;
  };
}

/** @deprecated Use L3Analysis instead */
export type L3Spec = L3Analysis;

/** File change type - kept for ImplementationFile compatibility */
export type L4ChangeType = "create" | "modify" | "delete";

export interface ImplementationFile {
  path: string;
  changeType: L4ChangeType;
  bytes: number;
  /** Unified diff (before → after). Present for modify/create, absent for delete. */
  diff?: string;
  /** Which MINIme worker processed this file. */
  workerId?: number;
}

export interface WorkerState {
  id: number;
  status: "idle" | "queued" | "running" | "done";
  file?: string;
  changeType?: string;
  completedCount: number;
}

export interface ImplementationResult {
  files: ImplementationFile[];
  workers?: WorkerState[];
}

export interface TestFailure {
  testName: string;
  error: string;
  screenshot?: string;
}

export interface TestReport {
  totalTests: number;
  passed: number;
  failed: number;
  failures: TestFailure[];
}

export interface DiffRegion {
  x: number;
  y: number;
  width: number;
  height: number;
  description: string;
}

export interface DesignDiffResult {
  similarityScore: number;
  diffImagePath: string;
  regions: DiffRegion[];
}

export interface PRComment {
  file: string;
  line?: number;
  comment: string;
  severity: "info" | "warning" | "blocking";
}

export interface PRReviewResult {
  approved: boolean;
  comments: PRComment[];
  specComplianceScore: number;
  /**
   * "SUCCESS" if the connector passed validation, "FAILED" if the workflow
   * raised a do-not-merge PR for visibility. Mirrors the workflow's STATUS
   * field — set even when approved=true.
   */
  status?: "SUCCESS" | "FAILED";
  /** URL of the PR raised by the agent on juspay/hyperswitch-prism. */
  prUrl?: string;
  /** PR number, derived from prUrl by the engine for convenience. */
  prNumber?: number;
  /** GRACE branch pushed to origin, e.g. feat/grace-cybersource-bankdebit. */
  branchName?: string;
  /** Hash of the commit cherry-picked onto the PR branch. */
  commitHash?: string;
  /** Free-text reason when status is FAILED. */
  reason?: string;
}

export type HumanReviewDecision = "approve" | "edit" | "regenerate";

export interface HumanReviewResult {
  decision: HumanReviewDecision;
  editorNotes?: string;
  regeneratePrompt?: string;
}

export interface SpecReviewSession {
  checkpointId: CheckpointId;
  specType: "l2" | "l3" | "l4";
  specSnapshot: unknown;
  decision: HumanReviewDecision;
  reviewedAt: string;
  durationMs: number;
  reviewerNotes?: string;
}

export interface ProductAlignmentDoc {
  approved: boolean;
  approver?: string;
  notes: string;
  adjustedCriteria?: string[];

  /** References from Requirements Discovery */
  references?: {
    connectors?: string[];
    similarPaymentMethods?: string[];
    patternFiles?: string[];
  };

  /** GRACE: Implementation plan (Tech Spec Lite) */
  implementationPlan?: {
    /** High-level approach */
    approach: string;
    /** Key considerations/risks */
    considerations?: string[];
    /** Implementation phases */
    phases?: Array<{
      name: string;
      description: string;
      dependsOn?: string[];
    }>;
    /** Per-connector plan */
    perConnectorPlan?: Record<
      string,
      {
        files: string[];
        patternToFollow: string;
        estimatedEffort?: string;
      }
    >;
  };

  /** GRACE: Confirmed complexity */
  confirmedComplexity?: "low" | "medium" | "high";

  /** GRACE: Ready for implementation */
  readyForImplementation?: boolean;

  /** Clarification Q&A history */
  clarifications?: Array<{
    round: number;
    questions: string[];
    answers: Record<string, string>;
    timestamp: string;
  }>;
}

export interface DesignGateResult {
  designRequired: boolean;
  docUrlsReady: boolean;
  connectorDocUrls?: string[];
  skipReason?: string;
}

export interface FeatureResearchReport {
  /** Agent 1: What already exists in the repo — screens, components, hooks, types. */
  existingStructure: string;
  /** Agent 2: What the ideal flow/approach is based on web/market research. */
  idealFlow: string;
  /**
   * Agent 3 (runs AFTER agents 1+2): Final decision — what to build, where it
   * lives, which existing files to extend. This is what L2 consumes first.
   */
  finalDecision: string;
  /** Concrete action items distilled from the final decision. */
  actionItems?: string[];
}

export interface PipelineArtifacts {
  task?: TaskDefinition;
  /** Run-scoped feature branch created by preflight, e.g. feat/grace-cybersource-bankdebit-3f69a1. */
  branch?: string;
  /** GRACE: Requirements discovery results */
  requirements?: TaskDefinition["requirements"];
  productAlignment?: ProductAlignmentDoc;
  featureResearch?: FeatureResearchReport;
  designGate?: DesignGateResult;
  l2?: L2Plan;
  l3?: L3Analysis;
  /** Path to the saved L3 spec JSON file for downstream checkpoints */
  l3SpecPath?: string;
  implementation?: ImplementationResult;
  compiledFiles?: string[];
  /** Compilation errors from previous build attempt for retry */
  compilationErrors?: string[];
  /** Full compiler build output for UI display */
  compilerOutput?: string;
  /** gRPC test result object */
  grpcTest?: {
    status?: string;
    grpcurl_result?: string;
    reason?: string;
    grpcurl_command?: string;
    grpcurl_output?: string;
    output?: string;
  };
  /** gRPC test errors from previous test attempt for retry */
  grpcTestErrors?: string[];
  /** Raw grpcurl output from test attempts */
  grpcurlOutput?: string;
  designDiff?: DesignDiffResult;
  cypressReport?: TestReport;
  playwrightReport?: TestReport;
  prReview?: PRReviewResult;
  l2RegeneratePrompt?: string;
  l3RegeneratePrompt?: string;
  previousL2?: L2Plan;
  previousL3?: L3Analysis;
  l2Review?: SpecReviewSession;
  l3Review?: SpecReviewSession;
  /**
   * Phase 13: GitHub issue URL created at `juspay/grace` (or whatever
   * config.checkpoints.l2_review.graceIssueRepo points at) when the L2 tech
   * spec is approved. Set by `maybeCreateGraceIssue` in l2-review.ts.
   * Cleared by manual rewind via ARTIFACT_KEYS_BY_STAGE.l2_review so a
   * post-rewind re-approval creates a fresh issue.
   */
  l2GraceIssueUrl?: string;
  /**
   * Phase 12: per-phase Claude CLI session ids. Each LLM-using checkpoint
   * captures a uuid on its first call (returned by the runner) and stores
   * it here; subsequent calls resume the same conversation via
   * `claude --resume <uuid>`, letting the model retain prior-turn context
   * (the L3 spec it already read, files it already wrote, etc.) across
   * retries and engine respawns.
   *
   * l2_planning has two sub-sessions (links + techspec agents); implementation
   * has one per parallel file (see `implementationFileSessionIds`).
   *
   * Cleared by manual rewind via ARTIFACT_KEYS_BY_STAGE, forcing a fresh
   * Claude conversation on the next run of the rewound phase.
   */
  l2LinksSessionId?: string;
  l2TechspecSessionId?: string;
  l3SessionId?: string;
  /**
   * Single session for the implementation phase. The codegen agent uses its
   * own Edit/Write tool calls to write multiple files within one Claude
   * conversation — the planned per-file fan-out doesn't exist in current
   * code (`implementationConcurrency` config is unused), so one session id
   * is correct here. compiler_check and grpc_test resume this session
   * directly to trigger fixes.
   */
  implementationSessionId?: string;
  /**
   * Phase 12: grpc_test owns its own Claude session for the test-driving
   * agent (uses SLIM_PROMPT). When the test fails and the inner fix loop
   * needs to retry, the grpc_test agent resumes its own session ("code has
   * been updated; retry") while the FIX itself happens via
   * implementationSessionId.
   */
  grpcTestSessionId?: string;
  prReviewSessionId?: string;
}

export interface InboundBus {
  waitFor<T = unknown>(type: string, timeoutMs?: number): Promise<T>;
  emit(type: string, checkpointId?: CheckpointId, payload?: unknown): void;
  emitHumanWaiting(checkpointId: CheckpointId, spec: unknown): void;
}

export interface PipelineContext {
  runId: string;
  /**
   * Owning session id — engine.run() uses this to claim a per-session lock
   * and release it on completion. Defaults to "default" for legacy paths.
   */
  sessionId: string;
  task: TaskDefinition;
  artifacts: PipelineArtifacts;
  retryCount: Record<string, number>;
  log: (
    msg: string,
    level?: "info" | "warn" | "error" | "success" | "debug"
  ) => void;
  options: PipelineOptions;
  bus?: InboundBus;
}

export interface PipelineOptions {
  autoApproveReviews?: boolean;
  reviewTimeoutMs?: number;
  dryRun?: boolean;
  maxRetries?: number;
  designMatchThreshold?: number;
  devServerUrl?: string;
  regressionCommand?: string;
  dashboard?: boolean;
  model?: string;
  /** If true, task checkpoint waits for a WS task:submit message from the dashboard instead of CLI prompting. */
  taskFromUi?: boolean;
  /** If true, an auto-reviewer LLM agent stands in for the human on all review gates. */
  autoMode?: boolean;
  /** Friendly name for the auto-reviewer agent, e.g. "Riddhi's subagent". */
  agentName?: string;
}

export class PipelineAbortError extends Error {
  constructor(public checkpointId: CheckpointId, message: string) {
    super(`[${checkpointId}] ${message}`);
    this.name = "PipelineAbortError";
  }
}

/**
 * Thrown by engine.run() when a different run already holds the session's
 * lock. Caller should surface this as a "session busy" message to the user
 * rather than treating it as a generic pipeline failure.
 */
export class SessionBusyError extends Error {
  constructor(public sessionId: string, public holderRunId?: string) {
    super(
      `Session ${sessionId} is busy${holderRunId ? ` (held by run ${holderRunId})` : ""}`
    );
    this.name = "SessionBusyError";
  }
}
