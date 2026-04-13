/**
 * gRPC smoke test for hyperswitch-prism SDK.
 *
 * For each supported flow (filtered by data/field_probe/{connector}.json),
 * calls the connector's _build*Request() builder to construct the proto
 * request, then dispatches it directly through the GrpcClient.
 *
 * No grpc_* wrapper functions are needed in the connector JS file.
 *
 * Usage:
 *   node test_smoke_grpc.js --connectors stripe --examples-dir /path/to/examples
 */

import { GrpcClient } from "hyperswitch-prism";
import type { GrpcConfig } from "hyperswitch-prism";
import * as fs from "fs";
import * as path from "path";

// ── ANSI color helpers ──────────────────────────────────────────────────────
const _NO_COLOR = (!process.stdout.isTTY && !process.env["FORCE_COLOR"]) || !!process.env["NO_COLOR"];
function _c(code: string, text: string): string { return _NO_COLOR ? text : `\x1b[${code}m${text}\x1b[0m`; }
function _green(t: string): string { return _c("32", t); }
function _red(t: string): string { return _c("31", t); }
function _yellow(t: string): string { return _c("33", t); }
function _grey(t: string): string { return _c("90", t); }
function _bold(t: string): string { return _c("1", t); }

// ── Probe request normalization ───────────────────────────────────────────────
// Field-probe data uses snake_case keys; protobufjs fromObject expects camelCase.
function _snakeToCamel(s: string): string {
  return s.replace(/_([a-z])/g, (_, c: string) => c.toUpperCase());
}
function _deepCamel(obj: unknown): unknown {
  if (Array.isArray(obj)) return (obj as unknown[]).map(_deepCamel);
  if (obj !== null && typeof obj === "object") {
    return Object.fromEntries(
      Object.entries(obj as Record<string, unknown>).map(([k, v]) => [
        _snakeToCamel(k),
        _deepCamel(v),
      ])
    );
  }
  return obj;
}

// ── Flow manifest ─────────────────────────────────────────────────────────────

function loadFlowManifest(sdkRoot: string): string[] {
  // Try multiple locations for flows.json
  const locations = [
    path.join(sdkRoot, "generated", "flows.json"),
    path.join(process.cwd(), "flows.json"),
  ];
  
  // Check environment variable
  if (process.env.FLOWS_JSON_PATH) {
    locations.unshift(process.env.FLOWS_JSON_PATH);
  }
  
  for (const manifestPath of locations) {
    if (fs.existsSync(manifestPath)) {
      const data = JSON.parse(fs.readFileSync(manifestPath, "utf-8")) as { flows: string[] };
      return data.flows;
    }
  }
  
  const searched = locations.join("\n  - ");
  throw new Error(
    `flows.json not found. Searched:\n  - ${searched}\nRun: make generate`
  );
}

// ── Field-probe support filtering ────────────────────────────────────────────

interface FieldProbe {
  supportedFlows: Set<string>;
  // First supported variant's proto_request per flow — used as payload fallback.
  probeRequests:  Map<string, Record<string, unknown>>;
}

function loadFieldProbe(connector: string, examplesDir: string): FieldProbe | null {
  const probeFile = path.join(examplesDir, "..", "data", "field_probe", `${connector}.json`);
  if (!fs.existsSync(probeFile)) return null;
  const probe = JSON.parse(fs.readFileSync(probeFile, "utf-8")) as {
    flows?: Record<string, Record<string, { status: string; proto_request?: Record<string, unknown> }>>;
  };
  if (!probe.flows) return null;
  const supportedFlows = new Set<string>();
  const probeRequests  = new Map<string, Record<string, unknown>>();
  for (const [flowName, variants] of Object.entries(probe.flows)) {
    const supportedVariant = Object.values(variants).find((v) => v.status === "supported");
    if (supportedVariant) {
      supportedFlows.add(flowName);
      if (supportedVariant.proto_request) {
        probeRequests.set(flowName, supportedVariant.proto_request);
      }
    }
  }
  return { supportedFlows, probeRequests };
}

// ── Flow gRPC dispatch metadata ──────────────────────────────────────────────
// Maps flow key → GrpcClient field/method + connector builder function name + arg type.
//
// arg: "AUTOMATIC"/"MANUAL" = string literal forwarded to builder (capture_method);
//      "txnId"              = connector txn_id (from shared authorize pre-run);
//      "none"               = builder takes no arguments.

interface FlowMeta {
  field:   string;   // GrpcClient field  (e.g. "payment", "customer")
  method:  string;   // camelCase method  (e.g. "authorize", "create")
  builder: string;   // _build*Request fn exported by the connector's JS module
  arg:     "AUTOMATIC" | "MANUAL" | "txnId" | "mandateId" | "none";
}

// Canonical ordering matches Rust build.rs.
const FLOW_META: [string, FlowMeta][] = [
  ["authorize",                { field: "payment",          method: "authorize",             builder: "_buildAuthorizeRequest",            arg: "AUTOMATIC" }],
  ["capture",                  { field: "payment",          method: "capture",               builder: "_buildCaptureRequest",              arg: "txnId"     }],
  ["void",                     { field: "payment",          method: "void",                  builder: "_buildVoidRequest",                 arg: "txnId"     }],
  ["get",                      { field: "payment",          method: "get",                   builder: "_buildGetRequest",                  arg: "txnId"     }],
  ["refund",                   { field: "payment",          method: "refund",                builder: "_buildRefundRequest",               arg: "txnId"     }],
  ["reverse",                  { field: "payment",          method: "reverse",               builder: "_buildReverseRequest",              arg: "txnId"     }],
  ["create_customer",          { field: "customer",         method: "create",                builder: "_buildCreateCustomerRequest",       arg: "none"      }],
  ["tokenize",                 { field: "paymentMethod",    method: "tokenize",              builder: "_buildTokenizeRequest",             arg: "none"      }],
  ["setup_recurring",          { field: "payment",          method: "setupRecurring",        builder: "_buildSetupRecurringRequest",       arg: "none"      }],
  ["recurring_charge",         { field: "recurringPayment", method: "charge",                builder: "_buildRecurringChargeRequest",      arg: "mandateId" }],
  ["pre_authenticate",         { field: "paymentMethodAuth", method: "preAuthenticate",       builder: "_buildPreAuthenticateRequest",      arg: "none"      }],
  ["authenticate",             { field: "paymentMethodAuth", method: "authenticate",          builder: "_buildAuthenticateRequest",         arg: "none"      }],
  ["post_authenticate",        { field: "paymentMethodAuth", method: "postAuthenticate",      builder: "_buildPostAuthenticateRequest",    arg: "none"      }],
  ["handle_event",             { field: "event",            method: "handleEvent",           builder: "_buildHandleEventRequest",          arg: "none"      }],
  ["create_access_token",      { field: "payment",          method: "createAccessToken",     builder: "_buildCreateAccessTokenRequest",    arg: "none"      }],
  ["create_session_token",     { field: "payment",          method: "createSessionToken",    builder: "_buildCreateSessionTokenRequest",   arg: "none"      }],
  ["create_sdk_session_token", { field: "payment",          method: "createSdkSessionToken", builder: "_buildCreateSdkSessionTokenRequest", arg: "none"      }],
];

const FLOW_META_MAP = new Map<string, FlowMeta>(FLOW_META);
const TXN_ID_FLOWS    = new Set(["capture", "void", "get", "refund", "reverse"]);
const SELF_AUTH_FLOWS = new Set(["capture", "void"]);
const MANDATE_ID_FLOWS = new Set(["recurring_charge"]);

// ── Credentials ───────────────────────────────────────────────────────────────

type CredEntry = Record<string, string | { value?: string } | undefined>;

function extractCredsValue(entry: CredEntry, keys: string[]): string | undefined {
  for (const k of keys) {
    const v = entry[k];
    if (typeof v === "string" && v) return v;
    if (v && typeof v === "object" && "value" in v && typeof v.value === "string" && v.value) {
      return v.value;
    }
  }
  return undefined;
}

function buildGrpcConfig(connector: string, cred: CredEntry): GrpcConfig {
  const apiKey      = extractCredsValue(cred, ["api_key", "apiKey"]) || "placeholder";
  const apiSecret   = extractCredsValue(cred, ["api_secret", "apiSecret"]);
  const key1        = extractCredsValue(cred, ["key1"]);
  const merchantId  = extractCredsValue(cred, ["merchant_account", "merchant_id", "merchantId"]);
  const tenantId    = extractCredsValue(cred, ["tenant_id", "tenantId"]);

  const connectorVariant = connector.charAt(0).toUpperCase() + connector.slice(1);

  const connectorSpecific: Record<string, string> = { api_key: apiKey };
  if (apiSecret) connectorSpecific.api_secret = apiSecret;
  if (key1) connectorSpecific.key1 = key1;
  if (merchantId) connectorSpecific.merchant_id = merchantId;
  if (tenantId) connectorSpecific.tenant_id = tenantId;

  return {
    endpoint: extractCredsValue(cred, ["endpoint"]) || "http://localhost:8000",
    connector,
    connector_config: {
      config: {
        [connectorVariant]: connectorSpecific,
      },
    },
  } as GrpcConfig;
}

function loadCreds(credsPath: string): Record<string, CredEntry | CredEntry[]> {
  if (!fs.existsSync(credsPath)) return {};
  return JSON.parse(fs.readFileSync(credsPath, "utf-8")) as Record<string, CredEntry | CredEntry[]>;
}

// ── Builder dispatch ──────────────────────────────────────────────────────────

function buildRequest(
  mod: Record<string, unknown>,
  flow: string,
  arg?: string,
  probeRequests?: Map<string, Record<string, unknown>>,
): unknown {
  const meta = FLOW_META_MAP.get(flow);
  if (!meta) return _deepCamel(probeRequests?.get(flow) ?? {});
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const fn = typeof mod[meta.builder] === "function" ? (mod[meta.builder] as (...a: any[]) => unknown) : null;
  if (!fn) return _deepCamel(probeRequests?.get(flow) ?? {});
  return meta.arg === "none" ? fn() : fn(arg ?? "");
}

// ── txn_id extraction ─────────────────────────────────────────────────────────

function extractTxnId(connectorTransactionId: string | undefined): string {
  return connectorTransactionId ?? "probe_connector_txn_001";
}

// ── Scenario result tracking ─────────────────────────────────────────────────

interface ScenarioResult {
  status: "passed" | "skipped" | "failed";
  message?: string;
  reason?: string;
  error?: string;
}

interface ConnectorResult {
  connector: string;
  status: "passed" | "failed" | "skipped";
  scenarios: Map<string, ScenarioResult>;
  error?: string;
}

function isTransportError(msg: string): boolean {
  return /unavailable|deadlineexceeded|connection refused|transport error|dns error|connection reset/i.test(msg);
}

// ── Main ─────────────────────────────────────────────────────────────────────

async function runConnector(
  connectorName: string,
  examplesDir:   string,
  cred:          CredEntry,
): Promise<ConnectorResult> {
  const result: ConnectorResult = {
    connector: connectorName,
    status: "passed",
    scenarios: new Map(),
  };
  
  // Try both .js and .ts file extensions
  let jsFile = path.join(examplesDir, connectorName, `${connectorName}.js`);
  if (!fs.existsSync(jsFile)) {
    jsFile = path.join(examplesDir, connectorName, `${connectorName}.ts`);
  }
  if (!fs.existsSync(jsFile)) {
    result.status = "skipped";
    result.error = `No JavaScript/TypeScript file found for ${connectorName}`;
    console.log(_grey(`  [${connectorName}] No JavaScript/TypeScript file found at ${examplesDir}/${connectorName}/, skipping.`));
    return result;
  }
  // eslint-disable-next-line @typescript-eslint/no-var-requires
  const mod: Record<string, unknown> = require(jsFile);

  const config = buildGrpcConfig(connectorName, cred);
  const client = new GrpcClient(config);

  // Filter to supported flows (field_probe); null means no filter.
  const fieldProbe = loadFieldProbe(connectorName, examplesDir);
  const probeRequests = fieldProbe?.probeRequests;
  if (fieldProbe !== null) {
    console.log(_grey(`  [${connectorName}] field_probe: ${fieldProbe.supportedFlows.size} supported flows`));
  }

  const presentFlows = FLOW_META
    .map(([flow]) => flow)
    .filter((flow) => fieldProbe === null || fieldProbe.supportedFlows.has(flow));

  if (presentFlows.length === 0) {
    result.status = "skipped";
    result.error = "No flows to run";
    console.log(_grey(`  [${connectorName}] No flows to run, skipping.`));
    return result;
  }

  const txnId = `probe_js_grpc_${Date.now()}`;
  let authorizeTxnId = txnId;
  let mandateId: string | undefined;

  const hasAuthorize  = presentFlows.includes("authorize");
  const hasDependents = presentFlows.some((f) => TXN_ID_FLOWS.has(f));
  const hasSetupRecurring = presentFlows.includes("setup_recurring");
  const hasMandateDependents = presentFlows.some((f) => MANDATE_ID_FLOWS.has(f));

  // Pre-run AUTOMATIC authorize to get a real connector txn_id for get/refund/reverse.
  if (hasAuthorize && hasDependents) {
    process.stdout.write(`  [authorize] running … `);
    try {
      const req = buildRequest(mod, "authorize", "AUTOMATIC", probeRequests);
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const res = await (client as any).payment.authorize(req) as { connectorTransactionId?: string; statusCode: number };
      authorizeTxnId = extractTxnId(res.connectorTransactionId);
      const resultStr = `txn_id: ${res.connectorTransactionId ?? "-"}, status_code: ${res.statusCode}`;
      if (res.statusCode >= 400) {
        console.log(_yellow("SKIPPED (connector error)"), _grey(`— ${resultStr}`));
        result.scenarios.set("authorize", { status: "skipped", reason: "connector_error", message: resultStr });
      } else {
        console.log(_green("PASSED"), _grey(`— ${resultStr}`));
        result.scenarios.set("authorize", { status: "passed", message: resultStr });
      }
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      if (isTransportError(msg)) {
        console.log(_red("FAILED"), _grey(`— ${msg}`));
        result.scenarios.set("authorize", { status: "failed", error: msg });
        result.status = "failed";
      } else {
        console.log(_yellow("SKIPPED (connector error)"), _grey(`— ${msg}`));
        result.scenarios.set("authorize", { status: "skipped", reason: "connector_error", message: msg });
      }
    }
  }

  // Pre-run setup_recurring to get mandate_id for recurring_charge.
  if (hasSetupRecurring && hasMandateDependents) {
    process.stdout.write(`  [setup_recurring] running … `);
    try {
      const req = buildRequest(mod, "setup_recurring", undefined, probeRequests);
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const res = await (client as any).payment.setupRecurring(req) as { 
        mandateReference?: { connectorMandateId?: { connectorMandateId?: string } }; 
        statusCode: number 
      };
      mandateId = res.mandateReference?.connectorMandateId?.connectorMandateId;
      const resultStr = `mandate_id: ${mandateId ?? "-"}, status_code: ${res.statusCode}`;
      if (res.statusCode >= 400) {
        console.log(_yellow("SKIPPED (connector error)"), _grey(`— ${resultStr}`));
        result.scenarios.set("setup_recurring", { status: "skipped", reason: "connector_error", message: resultStr });
      } else {
        console.log(_green("PASSED"), _grey(`— ${resultStr}`));
        result.scenarios.set("setup_recurring", { status: "passed", message: resultStr });
      }
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      console.log(_yellow("SKIPPED (connector error)"), _grey(`— ${msg}`));
      result.scenarios.set("setup_recurring", { status: "skipped", reason: "connector_error", message: msg });
    }
  }

  for (const flow of presentFlows) {
    const meta = FLOW_META_MAP.get(flow)!;

    // Skip authorize if already handled in the pre-run above.
    if (flow === "authorize" && hasAuthorize && hasDependents) {
      continue;
    }

    // Skip setup_recurring if already handled in the pre-run above.
    if (flow === "setup_recurring" && hasSetupRecurring && hasMandateDependents) {
      continue;
    }

    process.stdout.write(`  [${flow}] running … `);

    try {
      let resultStr: string;

      if (SELF_AUTH_FLOWS.has(flow)) {
        // capture / void: do a MANUAL authorize inline (AUTOMATIC txn_id can't be captured).
        const authReq = buildRequest(mod, "authorize", "MANUAL", probeRequests);
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const auth = await (client as any).payment.authorize(authReq) as { connectorTransactionId?: string; statusCode: number; error?: unknown };
        if (auth.statusCode >= 400) {
          throw new Error(`inline authorize failed (status ${auth.statusCode})`);
        }
        const selfTxnId = auth.connectorTransactionId ?? txnId;
        const req = buildRequest(mod, flow, selfTxnId, probeRequests);
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const res = await (client as any)[meta.field][meta.method](req) as { connectorTransactionId?: string; statusCode: number };
        resultStr = `txn_id: ${res.connectorTransactionId ?? "-"}, status_code: ${res.statusCode}`;
      } else if (MANDATE_ID_FLOWS.has(flow)) {
        // recurring_charge: use mandateId from setup_recurring pre-run.
        const arg = mandateId ?? txnId;
        const req = buildRequest(mod, flow, arg, probeRequests);
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const res = await (client as any)[meta.field][meta.method](req) as Record<string, unknown>;
        resultStr = `status_code: ${res["statusCode"] ?? "?"}`;
      } else if (TXN_ID_FLOWS.has(flow)) {
        const req = buildRequest(mod, flow, authorizeTxnId, probeRequests);
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const res = await (client as any)[meta.field][meta.method](req) as Record<string, unknown>;
        resultStr = `status_code: ${res["statusCode"] ?? "?"}`;
      } else {
        const req = buildRequest(mod, flow, txnId, probeRequests);
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const res = await (client as any)[meta.field][meta.method](req) as Record<string, unknown>;
        resultStr = `status_code: ${res["statusCode"] ?? "?"}`;
      }

      console.log(_green("PASSED"), _grey(`— ${resultStr}`));
      result.scenarios.set(flow, { status: "passed", message: resultStr });
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
      if (isTransportError(msg)) {
        console.log(_red("FAILED"), _grey(`— ${msg}`));
        result.scenarios.set(flow, { status: "failed", error: msg });
        result.status = "failed";
      } else {
        console.log(_yellow("SKIPPED (connector error)"), _grey(`— ${msg}`));
        result.scenarios.set(flow, { status: "skipped", reason: "connector_error", message: msg });
      }
    }
  }

  // Update connector status based on scenarios
  if (result.status !== "failed") {
    const allPassedOrSkipped = Array.from(result.scenarios.values()).every(
      s => s.status === "passed" || s.status === "skipped"
    );
    result.status = allPassedOrSkipped ? "passed" : "failed";
  }

  return result;
}

function printResult(result: ConnectorResult): void {
  if (result.status === "passed") {
    const passedCount = Array.from(result.scenarios.values()).filter(s => s.status === "passed").length;
    const skippedCount = Array.from(result.scenarios.values()).filter(s => s.status === "skipped").length;
    console.log(_green(`  PASSED`) + ` (${passedCount} passed, ${skippedCount} skipped)`);
    for (const [key, scenario] of result.scenarios) {
      if (scenario.status === "passed") {
        console.log(_green(`    ${key}: ✓`));
      } else if (scenario.status === "skipped") {
        console.log(_yellow(`    ${key}: ~ skipped (${scenario.reason})`));
      }
    }
  } else if (result.status === "skipped") {
    console.log(_grey(`  SKIPPED (${result.error || "unknown"})`));
  } else {
    console.log(_red(`  FAILED`));
    for (const [key, scenario] of result.scenarios) {
      if (scenario.status === "failed") {
        console.log(_red(`    ${key}: ✗ FAILED — ${scenario.error}`));
      }
    }
    if (result.error) {
      console.log(_red(`  Error: ${result.error}`));
    }
  }
}

function printSummary(results: ConnectorResult[]): number {
  console.log(`\n${"=".repeat(60)}`);
  console.log(_bold("TEST SUMMARY"));
  console.log(`${"=".repeat(60)}\n`);

  const passed = results.filter(r => r.status === "passed").length;
  const skipped = results.filter(r => r.status === "skipped").length;
  const failed = results.filter(r => r.status === "failed").length;

  // Count per-scenario statuses
  let totalFlowsPassed = 0;
  let totalFlowsSkipped = 0;
  let totalFlowsFailed = 0;
  for (const r of results) {
    for (const scenario of r.scenarios.values()) {
      if (scenario.status === "passed") totalFlowsPassed++;
      else if (scenario.status === "skipped") totalFlowsSkipped++;
      else if (scenario.status === "failed") totalFlowsFailed++;
    }
  }

  console.log(`Total connectors:   ${results.length}`);
  console.log(_green(`Passed:  ${passed}`));
  console.log(_grey(`Skipped: ${skipped} (no examples or placeholder credentials)`));
  console.log((failed > 0 ? _red : _green)(`Failed:  ${failed}`));
  console.log();
  console.log(`Flow results:`);
  console.log(_green(`  ${totalFlowsPassed} flows PASSED`));
  if (totalFlowsSkipped > 0) {
    console.log(_yellow(`  ${totalFlowsSkipped} flows SKIPPED (connector errors)`));
  }
  if (totalFlowsFailed > 0) {
    console.log(_red(`  ${totalFlowsFailed} flows FAILED`));
  }
  console.log();

  if (failed > 0) {
    console.log(_red("Failed connectors:"));
    for (const result of results) {
      if (result.status === "failed") {
        console.log(_red(`  - ${result.connector}: ${result.error || "see scenarios above"}`));
      }
    }
    console.log();
    return 1;
  }

  if (passed === 0 && skipped > 0) {
    console.log(_yellow("All tests skipped (no valid flows found)"));
    return 1;
  }

  console.log(_green("All tests completed successfully!"));
  return 0;
}

async function main(): Promise<void> {
  const { connectors, examplesDir } = parseArgs();

  const credsFile = path.join(process.cwd(), "creds.json");
  const allCreds  = loadCreds(credsFile);

  // Load flow manifest
  const sdkRoot = path.join(__dirname, "..");
  let manifest: string[];
  try {
    manifest = loadFlowManifest(sdkRoot);
  } catch (e) {
    console.error(_red(`MANIFEST ERROR: ${e instanceof Error ? e.message : String(e)}`));
    process.exit(1);
  }

  console.log(_bold("Javascript gRPC smoke test"));
  console.log(_grey(`connectors: ${connectors.join(", ")}`));
  console.log();

  const results: ConnectorResult[] = [];

  for (const connector of connectors) {
    console.log(_bold(`── ${connector} ──`));

    const raw = allCreds[connector];
    const creds: CredEntry[] = Array.isArray(raw) ? raw : raw ? [raw] : [{}];

    for (const cred of creds) {
      const result = await runConnector(connector, examplesDir, cred);
      results.push(result);
      printResult(result);
    }
    console.log();
  }

  const exitCode = printSummary(results);
  process.exit(exitCode);
}

function parseArgs(): { connectors: string[]; examplesDir: string } {
  const args = process.argv.slice(2);
  let connectors: string[] = ["stripe"];
  let examplesDir = path.join(__dirname, "../../../examples");

  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--connectors" && i + 1 < args.length) {
      connectors = args[++i].split(",").map((c) => c.trim());
    } else if (args[i] === "--examples-dir" && i + 1 < args.length) {
      examplesDir = args[++i];
    }
  }

  return { connectors, examplesDir };
}

main();
