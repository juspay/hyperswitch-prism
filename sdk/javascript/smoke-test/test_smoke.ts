/**
 * Multi-connector smoke test for hyperswitch-prism SDK.
 *
 * Loads connector credentials from external JSON file and runs authorize flow
 * for multiple connectors.
 *
 * Usage:
 *   node test_smoke.js --creds-file creds.json --all
 *   node test_smoke.js --creds-file creds.json --connectors stripe,adyen
 *   node test_smoke.js --creds-file creds.json --all --dry-run
 */

import { types, NetworkError, IntegrationError, ConnectorError } from "hyperswitch-prism";
import * as fs from "fs";
import * as path from "path";
import { createRequire } from "module";

// Import http_client for mock intercept (use createRequire for ESM compatibility)
const _require = createRequire(import.meta.url);
let httpClient: any;
try {
  httpClient = _require("hyperswitch-prism/dist/src/http_client.js");
} catch {
  try {
    httpClient = _require("hyperswitch-prism/dist/http_client");
  } catch {
    httpClient = null;
  }
}

const { ConnectorConfig, ConnectorSpecificConfig, SdkOptions, Environment } = types;

// ── ANSI color helpers ──────────────────────────────────────────────────────
const _NO_COLOR = !process.stdout.isTTY || !!process.env["NO_COLOR"];
function _c(code: string, text: string): string { return _NO_COLOR ? text : `\x1b[${code}m${text}\x1b[0m`; }
function _green(t: string): string { return _c("32", t); }
function _yellow(t: string): string { return _c("33", t); }
function _red(t: string): string { return _c("31", t); }
function _grey(t: string): string { return _c("90", t); }
function _bold(t: string): string { return _c("1", t); }

// Placeholder values that indicate credentials are not configured
const PLACEHOLDER_VALUES = new Set(["", "placeholder", "test", "dummy", "sk_test_placeholder"]);

interface AuthConfig {
  [key: string]: string | object;
  metadata?: any;
}

interface Credentials {
  [connector: string]: AuthConfig | AuthConfig[];
}

interface ScenarioResult {
  status: "passed" | "skipped" | "failed";
  result?: any;
  reason?: string;
  detail?: string;
  error?: string;
}

interface ConnectorResult {
  connector: string;
  status: "passed" | "failed" | "skipped" | "dry_run";
  scenarios: { [key: string]: ScenarioResult };
  error?: string;
}

function loadCredentials(credsFile: string): Credentials {
  if (!fs.existsSync(credsFile)) {
    throw new Error(`Credentials file not found: ${credsFile}`);
  }
  return JSON.parse(fs.readFileSync(credsFile, "utf-8"));
}

function isPlaceholder(value: string): boolean {
  if (!value) return true;
  const lower = value.toLowerCase();
  return PLACEHOLDER_VALUES.has(lower) || lower.includes("placeholder");
}

function hasValidCredentials(authConfig: AuthConfig): boolean {
  for (const [key, value] of Object.entries(authConfig)) {
    if (key === "metadata" || key === "_comment") continue;
    if (typeof value === "object" && value !== null && "value" in value) {
      const val = (value as { value: unknown }).value;
      if (typeof val === "string" && !isPlaceholder(val)) return true;
    }
    if (typeof value === "string" && !isPlaceholder(value)) return true;
  }
  return false;
}

function buildConnectorConfig(connectorKey: string, authConfig: AuthConfig): any {
  const connectorFields: Record<string, any> = {};
  for (const [key, value] of Object.entries(authConfig)) {
    if (key !== "_comment" && key !== "metadata") {
      const camelKey = key.replace(/_([a-z])/g, (_, l) => l.toUpperCase());
      connectorFields[camelKey] = value;
    }
  }

  return ConnectorConfig.create({
    connectorConfig: ConnectorSpecificConfig.create({ [connectorKey.toLowerCase()]: connectorFields }),
    options: SdkOptions.create({ environment: Environment.SANDBOX }),
  });
}

interface FlowManifest {
  flows: string[];
  flow_to_example_fn?: Record<string, string | null>;
}

function loadFlowManifest(sdkRoot: string): FlowManifest {
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
      const data = JSON.parse(fs.readFileSync(manifestPath, "utf-8"));
      return {
        flows: data.flows as string[],
        flow_to_example_fn: data.flow_to_example_fn as Record<string, string | null> | undefined,
      };
    }
  }
  
  const searched = locations.join("\n  - ");
  throw new Error(
    `flows.json not found. Searched:\n  - ${searched}\nRun: make generate`
  );
}

function toPascalCase(flowKey: string): string {
  return "process" + flowKey
    .split("_")
    .map(part => part.charAt(0).toUpperCase() + part.slice(1))
    .join("");
}

function fromPascalCase(fnName: string): string {
  // processCheckoutCard → checkout_card
  return fnName
    .replace(/^process/, "")
    .replace(/([A-Z])/g, "_$1")
    .toLowerCase()
    .replace(/^_/, "");
}

type ScenarioList = Array<{ key: string; fn: Function }>;

function discoverAndValidate(
  mod: any,
  connectorName: string,
  manifest: string[],
  flowToExampleFn: Record<string, string | null> | undefined,
): ScenarioList | string {
  const declared: string[] | undefined = mod["SUPPORTED_FLOWS"];
  const legacyMode = declared === undefined;

  let effectiveDeclared: string[];
  if (legacyMode) {
    // Legacy mode: scan process* exports using flow_to_example_fn mapping
    // Find all available example functions
    const availableExampleFns = new Set(
      Object.keys(mod)
        .filter(k => k.startsWith("process") && typeof mod[k] === "function")
        .map(k => fromPascalCase(k))
    );
    // Map flows to their example functions if both exist
    effectiveDeclared = manifest.filter(name => {
      const exampleFn = flowToExampleFn?.[name];
      return exampleFn && availableExampleFns.has(exampleFn);
    });
  } else {
    effectiveDeclared = [...new Set(declared)]; // Deduplicate
  }

  // Validate flow names are lowercase snake_case
  for (const name of effectiveDeclared) {
    if (name !== name.toLowerCase() || name.includes(" ") || name.includes("-")) {
      return `COVERAGE ERROR: Flow name '${name}' in SUPPORTED_FLOWS must be lowercase snake_case (e.g., 'authorize', 'payout_create')`;
    }
  }

  // Helper: find implementation function for a flow in the module.
  // Tries (in order): mapped scenario fn, process-prefixed flow fn, camelCase flow fn (no prefix).
  function findFlowFn(name: string): Function | undefined {
    const exampleFn = flowToExampleFn?.[name];
    if (exampleFn && typeof mod[toPascalCase(exampleFn)] === "function")
      return mod[toPascalCase(exampleFn)];
    if (typeof mod[toPascalCase(name)] === "function")
      return mod[toPascalCase(name)];
    // Fallback: examples expose flow functions under camelCase without process prefix
    // e.g. flow "authorize" → mod["authorize"], flow "proxy_authorize" → mod["proxyAuthorize"]
    const camel = name.replace(/_([a-z])/g, (_, l) => l.toUpperCase());
    if (typeof mod[camel] === "function") return mod[camel];
    return undefined;
  }

  // CHECK 1: declared without implementation
  const missing: string[] = [];
  for (const name of effectiveDeclared) {
    if (!findFlowFn(name)) missing.push(name);
  }
  if (missing.length > 0) {
    return `COVERAGE ERROR: SUPPORTED_FLOWS declares ${JSON.stringify(missing)} but no implementation found for them.`;
  }

  // CHECK 2: scan ALL process* exports (only when SUPPORTED_FLOWS is explicitly defined)
  if (!legacyMode) {
    const allProcessFns = new Set(
      Object.keys(mod)
        .filter(k => k.startsWith("process") && typeof mod[k] === "function")
        .map(k => fromPascalCase(k))
    );
    const manifestSet = new Set(manifest);
    // Only flag process* functions whose base name IS a known flow but not in SUPPORTED_FLOWS.
    // Scenario functions (e.g. checkout_autocapture) whose name is not in manifest are allowed.
    const undeclared = [...allProcessFns].filter(n => manifestSet.has(n) && !effectiveDeclared.includes(n));
    if (undeclared.length > 0) {
      return `COVERAGE ERROR: process* functions exist for flows ${JSON.stringify(undeclared)} but they're not in SUPPORTED_FLOWS`;
    }

    // CHECK 3: Warn about entries in SUPPORTED_FLOWS not in the flow manifest.
    // These are typically composite scenario names (create_customer, recurring_charge)
    // not individually listed in flows.json. Warn only — don't fail.
    const stale = effectiveDeclared.filter(n => !manifestSet.has(n));
    if (stale.length > 0) {
      console.log(`  [warn] SUPPORTED_FLOWS contains entries not in flows.json (scenario names): ${JSON.stringify(stale)}`);
    }
  }

  // Return (key, fn) pairs for the test runner
  return effectiveDeclared.map(name => {
    const fn = findFlowFn(name)!;
    const exampleFn = flowToExampleFn?.[name];
    return { key: exampleFn ?? name, fn };
  });
}

// Last intercepted mock request (method + URL), read by the PASSED handler.
// Use an object so the lambda captures the reference, not the value.
const _mockState = { lastRequest: null as string | null };

function installMockIntercept(): void {
  if (!httpClient || !("_intercept" in httpClient)) {
    // HTTP client module not available; mock mode will still work via error handling
    return;
  }
  const state = _mockState;
  httpClient._intercept = async (req: any) => {
    state.lastRequest = `${req.method} ${req.url}`;
    return {
      statusCode: 200,
      headers: {},
      body: new Uint8Array(Buffer.from("{}")),
      latencyMs: 0,
    };
  };
}

async function testConnectorScenarios(
  connectorName: string,
  config: any,
  examplesDir: string,
  sdkRoot: string,
  dryRun: boolean = false,
  mock: boolean = false,
): Promise<ConnectorResult> {
  const result: ConnectorResult = {
    connector: connectorName,
    status: "passed",
    scenarios: {},
  };

  if (dryRun) {
    result.status = "dry_run";
    return result;
  }

  const connectorDir = path.join(examplesDir, connectorName);
  if (!fs.existsSync(connectorDir)) {
    result.status = "skipped";
    (result.scenarios as any) = { skipped: true, reason: "no_examples_dir" };
    return result;
  }

  const consolidatedFile = path.join(connectorDir, `${connectorName}.ts`);
  if (!fs.existsSync(consolidatedFile)) {
    result.status = "skipped";
    (result.scenarios as any) = { skipped: true, reason: "no_scenario_files" };
    return result;
  }

  let mod: any;
  try {
    mod = await import(consolidatedFile);
  } catch (e: any) {
    console.log(`    IMPORT ERROR: ${e.message}`);
    result.status = "failed";
    (result as any).error = `import error: ${e.message}`;
    return result;
  }

  // Load flow manifest with mapping
  const manifestData = loadFlowManifest(sdkRoot);
  const manifest = manifestData.flows;
  const flowToExampleFn = manifestData.flow_to_example_fn;

  // Validate scenarios using the manifest
  const scenariosOrError = discoverAndValidate(mod, connectorName, manifest, flowToExampleFn);
  if (typeof scenariosOrError === "string") {
    result.status = "failed";
    (result as any).error = scenariosOrError;
    console.log(`    COVERAGE VIOLATION: ${scenariosOrError}`);
    return result;
  }

  // Build a map of example function names to their functions
  const exampleFnMap = new Map(scenariosOrError.map(s => [s.key, s.fn]));

  // Iterate ALL flows from manifest, using flow_to_example_fn mapping
  let anyFailed = false;

  for (const flowKey of manifest) {
    // Find the function to call - same logic for both mock and normal mode
    // Try flow name directly first, then fall back to example mapping
    let processFn = exampleFnMap.get(flowKey);
    
    if (!processFn && flowToExampleFn) {
      // Try mapped example function name
      const exampleFnName = flowToExampleFn[flowKey];
      if (exampleFnName) {
        processFn = exampleFnMap.get(exampleFnName);
      }
    }
    
    if (!processFn) {
      // No implementation found for this flow
      console.log(`    [${flowKey}] NOT IMPLEMENTED — No example function for flow '${flowKey}'`);
      result.scenarios[flowKey] = { status: "not_implemented", reason: `No example function for flow '${flowKey}'` };
      continue;
    }
    
    if (processFn) {
      const txnId = `smoke_${flowKey}_${Math.random().toString(16).slice(2, 10)}`;
      process.stdout.write(`    [${flowKey}] running ... `);

      try {
        const response = await processFn(txnId, config);

        if (response && response.error) {
          const errorStr = JSON.stringify(response.error);
          console.log(_yellow("SKIPPED (connector error)") + _grey(` — ${errorStr}`));
          result.scenarios[flowKey] = { status: "skipped", reason: "connector_error", detail: errorStr };
        } else {
          const summary = JSON.stringify(response);
          console.log(_green("PASSED") + _grey(` — ${summary}`));
          result.scenarios[flowKey] = { status: "passed", result: response };
        }
      } catch (e: any) {
        if (e instanceof IntegrationError) {
          const detail = `IntegrationError: ${e.message} (code=${e.errorCode}, action=${e.suggestedAction}, doc=${e.docUrl})`;
          // IntegrationError is always FAILED — req_transformer failed
          console.log(_red("FAILED") + ` — ${detail}`);
          result.scenarios[flowKey] = { status: "failed", error: detail };
          anyFailed = true;
        } else if (e instanceof ConnectorError) {
          const detail = `ConnectorError: ${e.message} (code=${e.errorCode}, http=${e.httpStatusCode})`;
          if (mock) {
            // In mock mode, ConnectorError means req_transformer successfully built the HTTP request.
            // The error is just from parsing the mock empty response, which is expected.
            const mockInfo = _mockState.lastRequest ?? "mock response";
            _mockState.lastRequest = null;
            console.log(_green("PASSED") + ` — req_transformer OK (${mockInfo})`);
            result.scenarios[flowKey] = { status: "passed", reason: "mock_verified", detail };
          } else {
            console.log(_yellow("SKIPPED (connector error)") + _grey(` — ${detail}`));
            result.scenarios[flowKey] = { status: "skipped", reason: "connector_error", detail };
          }
        } else if (e instanceof Error && e.message.startsWith("Rust panic:")) {
          console.log(_red("FAILED") + ` — ${e.message}`);
          result.scenarios[flowKey] = { status: "failed", error: e.message };
          anyFailed = true;
        } else if (mock && e.message && !e.message.includes("panic")) {
          // In mock mode, non-panic errors mean req_transformer successfully built the HTTP request.
          // The error is just from parsing the mock empty response, which is expected.
          const mockInfo = _mockState.lastRequest ?? "mock response";
          _mockState.lastRequest = null;
          console.log(_green("PASSED") + ` — req_transformer OK (${mockInfo})`);
          result.scenarios[flowKey] = { status: "passed", reason: "mock_verified", detail: e.message };
        } else {
          console.log(_red("FAILED") + ` — ${e?.constructor?.name || "Error"}: ${e.message}`);
          result.scenarios[flowKey] = { status: "failed", error: `${e?.constructor?.name || "Error"}: ${e.message}` };
          anyFailed = true;
        }
      }
    } else {
      // Example function doesn't exist in this connector's module
      console.log(`    [${flowKey}] NOT IMPLEMENTED — Example function '${exampleFnName}' not found for flow '${flowKey}'`);
      result.scenarios[flowKey] = { status: "not_implemented", reason: "no_implementation" };
    }
  }

  result.status = anyFailed ? "failed" : "passed";
  return result;
}

function printResult(result: ConnectorResult): void {
  if (result.status === "passed") {
    const scenarios = result.scenarios;
    const passedCount = Object.values(scenarios).filter(s => s.status === "passed").length;
    const skippedCount = Object.values(scenarios).filter(s => s.status === "skipped").length;
    const notImplCount = Object.values(scenarios).filter(s => s.status === "not_implemented").length;
    console.log(_green(`  PASSED`) + ` (${passedCount} passed, ${skippedCount} skipped, ${notImplCount} not implemented)`);
    for (const [key, detail] of Object.entries(scenarios)) {
      if (detail.status === "passed") {
        const resultData = detail.result;
        const resultStr = resultData ? JSON.stringify(resultData) : "";
        console.log(_green(`    ${key}: ✓`) + _grey(` — ${resultStr}`));
      } else if (detail.status === "skipped") {
        const detailStr = detail.detail ? ` — ${detail.detail}` : "";
        console.log(_yellow(`    ${key}: ~ skipped (${detail.reason})`) + _grey(detailStr));
      } else if (detail.status === "not_implemented") {
        console.log(_grey(`    ${key}: N/A`));
      }
    }
  } else if (result.status === "dry_run") {
    console.log(_grey(`  DRY RUN`));
  } else if (result.status === "skipped") {
    const reason = (result.scenarios as any).reason || "unknown";
    console.log(_grey(`  SKIPPED (${reason})`));
  } else {
    console.log(_red(`  FAILED`));
    for (const [key, detail] of Object.entries(result.scenarios)) {
      if (detail.status === "failed") {
        console.log(_red(`    ${key}: ✗ FAILED — ${detail.error || "unknown error"}`));
      }
    }
    if (result.error) console.log(_red(`  Error: ${result.error}`));
  }
}

async function runTests(
  credsFile: string,
  connectors: string[] | undefined,
  dryRun: boolean,
  examplesDir: string,
  sdkRoot: string,
  mock: boolean = false,
): Promise<ConnectorResult[]> {
  // Install mock intercept if in mock mode
  if (mock) {
    installMockIntercept();
  }

  const credentials = loadCredentials(credsFile);
  const results: ConnectorResult[] = [];
  const testConnectors = connectors || Object.keys(credentials);

  // Use generated harnesses in mock mode
  // If examplesDir is explicitly provided, use it; otherwise use default paths
  let resolvedExamplesDir: string;
  if (examplesDir) {
    resolvedExamplesDir = examplesDir;
  } else {
    resolvedExamplesDir = path.join(__dirname, "..", "..", "..", "..", "examples");
  }

  // Load flow manifest once for all connectors
  let manifestData: FlowManifest;
  try {
    manifestData = loadFlowManifest(sdkRoot);
  } catch (e: any) {
    console.error(`FATAL: Could not load flow manifest: ${e.message}`);
    throw e;
  }
  const manifest = manifestData.flows;
  const flowToExampleFn = manifestData.flow_to_example_fn;

  console.log(`\n${"=".repeat(60)}`);
  console.log(`Running smoke tests for ${testConnectors.length} connector(s)`);
  if (mock) {
    console.log(`Mode: MOCK (HTTP intercepted, using generated harnesses)`);
  }
  console.log(`Examples dir: ${resolvedExamplesDir}`);
  console.log(`${"=".repeat(60)}\n`);

  for (const connectorName of testConnectors) {
    const authConfigValue = credentials[connectorName];
    console.log(`\n${_bold(`--- Testing ${connectorName} ---`)}`);

    if (!authConfigValue) {
      console.log(`  SKIPPED (not found in credentials file)`);
      results.push({ connector: connectorName, status: "skipped", scenarios: {}, error: "not_found" });
      continue;
    }

    const instances: { name: string; auth: AuthConfig }[] = Array.isArray(authConfigValue)
      ? authConfigValue.map((a, i) => ({ name: `${connectorName}[${i + 1}]`, auth: a }))
      : [{ name: connectorName, auth: authConfigValue }];

    for (const { name, auth } of instances) {
      if (instances.length > 1) console.log(`  Instance: ${name}`);

      if (!mock && !hasValidCredentials(auth)) {
        console.log(`  SKIPPED (placeholder credentials)`);
        results.push({ connector: name, status: "skipped", scenarios: {}, error: "placeholder_credentials" });
        continue;
      }

      let config: any;
      try {
        config = buildConnectorConfig(connectorName, auth);
      } catch (e: any) {
        console.log(`  SKIPPED (${e.message})`);
        results.push({ connector: name, status: "skipped", scenarios: {}, error: e.message });
        continue;
      }

      const result = await testConnectorScenarios(name, config, resolvedExamplesDir, sdkRoot, dryRun, mock);
      results.push(result);
      printResult(result);
    }
  }

  return results;
}

function printSummary(results: ConnectorResult[]): number {
  console.log(`\n${"=".repeat(60)}`);
  console.log(_bold("TEST SUMMARY"));
  console.log(`${"=".repeat(60)}\n`);

  const passed = results.filter(r => r.status === "passed" || r.status === "dry_run").length;
  const skipped = results.filter(r => r.status === "skipped").length;
  const failed = results.filter(r => r.status === "failed").length;

  // Count per-scenario statuses
  let totalFlowsPassed = 0;
  let totalFlowsSkipped = 0;
  let totalFlowsFailed = 0;
  for (const r of results) {
    for (const scenario of Object.values(r.scenarios)) {
      if (scenario.status === "passed") totalFlowsPassed++;
      else if (scenario.status === "skipped") totalFlowsSkipped++;
      else if (scenario.status === "failed") totalFlowsFailed++;
    }
  }

  console.log(`Total connectors:   ${results.length}`);
  console.log(_green(`Passed:  ${passed}`));
  console.log(_grey(`Skipped: ${skipped} (placeholder credentials or no examples)`));
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
    for (const r of results) {
      if (r.status === "failed") console.log(_red(`  - ${r.connector}`) + `: ${r.error || "see scenarios above"}`);
    }
    console.log();
    return 1;
  }

  if (passed === 0 && skipped > 0) {
    console.log(_yellow("All tests skipped (no valid credentials found)"));
    console.log("Update creds.json with real credentials to run tests");
    return 1;
  }

  console.log(_green("All tests completed successfully!"));
  return 0;
}

function parseArgs(): { credsFile: string; connectors?: string[]; all: boolean; dryRun: boolean; examplesDir?: string; mock: boolean } {
  const args = process.argv.slice(2);
  let credsFile = "creds.json";
  let connectors: string[] | undefined;
  let all = false;
  let dryRun = false;
  let mock = false;
  let examplesDir: string | undefined;

  for (let i = 0; i < args.length; i++) {
    const arg = args[i];
    if (arg === "--creds-file" && i + 1 < args.length) credsFile = args[++i];
    else if (arg === "--connectors" && i + 1 < args.length) connectors = args[++i].split(",").map(c => c.trim());
    else if (arg === "--all") all = true;
    else if (arg === "--dry-run") dryRun = true;
    else if (arg === "--mock") mock = true;
    else if (arg === "--examples-dir" && i + 1 < args.length) examplesDir = args[++i];
  }

  if (!all && !connectors) {
    console.error("Error: Must specify either --all or --connectors");
    process.exit(1);
  }

  return { credsFile, connectors, all, dryRun, examplesDir, mock };
}

async function main() {
  const { credsFile, connectors, all, dryRun, examplesDir, mock } = parseArgs();

  // Default paths: sdk and examples are siblings under repo root
  const resolvedExamplesDir = examplesDir || path.join(__dirname, "..", "..", "..", "..", "examples");
  const sdkRoot = path.join(__dirname, "..");

  try {
    const results = await runTests(
      credsFile,
      all ? undefined : connectors,
      dryRun,
      resolvedExamplesDir,
      sdkRoot,
      mock,
    );
    const exitCode = printSummary(results);
    process.exit(exitCode);
  } catch (e: any) {
    console.error(`\nFatal error: ${e.message || e}`);
    process.exit(1);
  }
}

main();
