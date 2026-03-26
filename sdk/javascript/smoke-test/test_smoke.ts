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

import { PaymentClient, types, NetworkError, IntegrationError, ConnectorResponseTransformationError } from "hyperswitch-prism";
import * as fs from "fs";
import * as path from "path";

const { ConnectorConfig, ConnectorSpecificConfig, SdkOptions, Environment } = types;

// ── ANSI color helpers ──────────────────────────────────────────────────────
const _NO_COLOR = !process.stdout.isTTY || !!process.env["NO_COLOR"];
function _c(code: string, text: string): string { return _NO_COLOR ? text : `\x1b[${code}m${text}\x1b[0m`; }
function _green (t: string): string { return _c("32", t); }
function _yellow(t: string): string { return _c("33", t); }
function _red   (t: string): string { return _c("31", t); }
function _grey  (t: string): string { return _c("90", t); }
function _bold  (t: string): string { return _c("1",  t); }

// Placeholder values that indicate credentials are not configured
const PLACEHOLDER_VALUES = new Set(["", "placeholder", "test", "dummy", "sk_test_placeholder"]);

// Canonical scenario order for consolidated-file discovery
const SCENARIO_NAMES = [
    "checkout_autocapture",
    "checkout_card",
    "checkout_wallet",
    "checkout_bank",
    "refund",
    "recurring",
    "void_payment",
    "get_payment",
    "create_customer",
    "tokenize",
    "authentication",
];

interface AuthConfig {
    [key: string]: string | object;
    metadata?: any;
}

interface Credentials {
    [connector: string]: AuthConfig | AuthConfig[];
}

interface ScenarioResult {
    passed: boolean;
    result?: any;
    connectorError?: string;
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

async function testConnectorScenarios(
    connectorName: string,
    config: any,
    examplesDir: string,
    dryRun: boolean = false,
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

    const connectorDir = path.join(examplesDir, connectorName, "javascript");
    if (!fs.existsSync(connectorDir)) {
        result.status = "skipped";
        (result.scenarios as any) = { skipped: true, reason: "no_examples_dir" };
        return result;
    }

    const consolidatedFile = path.join(connectorDir, `${connectorName}.js`);
    if (!fs.existsSync(consolidatedFile)) {
        result.status = "skipped";
        (result.scenarios as any) = { skipped: true, reason: "no_scenario_files" };
        return result;
    }

    let mod: any;
    try {
        delete require.cache[require.resolve(consolidatedFile)];
        mod = require(consolidatedFile);
    } catch (e: any) {
        console.log(`    IMPORT ERROR: ${e.message}`);
        result.status = "failed";
        (result as any).error = `import error: ${e.message}`;
        return result;
    }

    // Discover exported process* functions in canonical scenario order
    interface ScenarioFn { key: string; fn: Function }
    const scenarioFns: ScenarioFn[] = [];
    for (const name of SCENARIO_NAMES) {
        const funcName = "process" + name.replace(/_([a-z])/g, (_, l) => l.toUpperCase()).replace(/^(.)/, c => c.toUpperCase());
        if (typeof mod[funcName] === "function") {
            scenarioFns.push({ key: name, fn: mod[funcName] });
        }
    }

    if (scenarioFns.length === 0) {
        result.status = "skipped";
        (result.scenarios as any) = { skipped: true, reason: "no_scenario_files" };
        return result;
    }

    let anyFailed = false;

    for (const { key: scenarioKey, fn: processFn } of scenarioFns) {

        const txnId = `smoke_${scenarioKey}_${Math.random().toString(16).slice(2, 10)}`;
        process.stdout.write(`    [${scenarioKey}] running ... `);

        try {
            const response = await processFn(txnId, config);
            
            // Check if response contains error (connector returned error in response body)
            if (response && response.error) {
                const errorStr = JSON.stringify(response.error);
                console.log(_yellow("~ connector error") + _grey(` — ${errorStr}`));
                result.scenarios[scenarioKey] = { passed: true, connectorError: errorStr };
            } else {
                const summary = JSON.stringify(response);
                console.log(_green("✓ ok") + _grey(` — ${summary}`));
                result.scenarios[scenarioKey] = { passed: true, result: response };
            }
        } catch (e: any) {
            const errorName = e?.constructor?.name;
            const errorMessage = e?.message;
            
            let isConnectorError = false;
            switch (errorName) {
                case "IntegrationError":
                case "ConnectorResponseTransformationError":
                    isConnectorError = true;
                    break;
                default:
                    // FFI-level panics (e.g. HandlerError, InvalidWalletToken) surface as a plain
                    // Error with a "Rust panic: ..." message — treat them as connector errors.
                    if (typeof errorMessage === "string" && errorMessage.startsWith("Rust panic:")) {
                        isConnectorError = true;
                    }
                    break;
            }
            
            if (isConnectorError) {
                const msg = e.errorMessage || e.message || String(e);
                const code = e.errorCode;
                const detail = code ? `${code}: ${msg}` : msg;
                console.log(_yellow("~ connector error") + _grey(` — ${detail}`));
                result.scenarios[scenarioKey] = { passed: true, connectorError: detail };
            } else {
                console.log(_red("✗ FAILED") + ` — ${e?.constructor?.name || "Error"}: ${e.message}`);
                result.scenarios[scenarioKey] = { passed: false, error: `${e?.constructor?.name || "Error"}: ${e.message}` };
                anyFailed = true;
            }
        }
    }

    result.status = anyFailed ? "failed" : "passed";
    return result;
}

function printResult(result: ConnectorResult): void {
    if (result.status === "passed") {
        const n = Object.keys(result.scenarios).length;
        console.log(_green(`  PASSED`) + ` (${n} scenario(s))`);
    } else if (result.status === "dry_run") {
        console.log(_grey(`  DRY RUN`));
    } else if (result.status === "skipped") {
        const reason = (result.scenarios as any).reason || "unknown";
        console.log(_grey(`  SKIPPED (${reason})`));
    } else {
        console.log(_red(`  FAILED`));
        for (const [key, detail] of Object.entries(result.scenarios)) {
            if (!detail.passed) {
                console.log(_red(`    ${key}`) + ` — ${detail.error || "unknown error"}`);
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
): Promise<ConnectorResult[]> {
    const credentials = loadCredentials(credsFile);
    const results: ConnectorResult[] = [];
    const testConnectors = connectors || Object.keys(credentials);

    console.log(`\n${"=".repeat(60)}`);
    console.log(`Running smoke tests for ${testConnectors.length} connector(s)`);
    console.log(`Examples dir: ${examplesDir}`);
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

            if (!hasValidCredentials(auth)) {
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

            const result = await testConnectorScenarios(name, config, examplesDir, dryRun);
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

    const passed  = results.filter(r => r.status === "passed" || r.status === "dry_run").length;
    const skipped = results.filter(r => r.status === "skipped").length;
    const failed  = results.filter(r => r.status === "failed").length;

    console.log(`Total:   ${results.length}`);
    console.log(_green(`Passed:  ${passed}`));
    console.log(_grey(`Skipped: ${skipped} (placeholder credentials or no examples)`));
    console.log((failed > 0 ? _red : _green)(`Failed:  ${failed}`));
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

function parseArgs(): { credsFile: string; connectors?: string[]; all: boolean; dryRun: boolean; examplesDir?: string } {
    const args = process.argv.slice(2);
    let credsFile = "creds.json";
    let connectors: string[] | undefined;
    let all = false;
    let dryRun = false;
    let examplesDir: string | undefined;

    for (let i = 0; i < args.length; i++) {
        const arg = args[i];
        if (arg === "--creds-file" && i + 1 < args.length)       credsFile  = args[++i];
        else if (arg === "--connectors" && i + 1 < args.length)  connectors = args[++i].split(",").map(c => c.trim());
        else if (arg === "--all")                                  all        = true;
        else if (arg === "--dry-run")                              dryRun     = true;
        else if (arg === "--examples-dir" && i + 1 < args.length) examplesDir = args[++i];
    }

    if (!all && !connectors) {
        console.error("Error: Must specify either --all or --connectors");
        process.exit(1);
    }

    return { credsFile, connectors, all, dryRun, examplesDir };
}

async function main() {
    const { credsFile, connectors, all, dryRun, examplesDir } = parseArgs();

    // Default examples dir: 4 levels up from this file (repo_root/examples)
    const resolvedExamplesDir = examplesDir ||
        path.join(__dirname, "..", "..", "..", "..", "examples");

    try {
        const results = await runTests(
            credsFile,
            all ? undefined : connectors,
            dryRun,
            resolvedExamplesDir,
        );
        const exitCode = printSummary(results);
        process.exit(exitCode);
    } catch (e: any) {
        console.error(`\nFatal error: ${e.message || e}`);
        process.exit(1);
    }
}

main();
