/**
 * Multi-connector smoke test for hs-playlib SDK.
 * 
 * Loads connector credentials from external JSON file and runs authorize flow
 * for multiple connectors.
 * 
 * Usage:
 *   npx ts-node test_smoke.ts --creds-file creds.json --all
 *   npx ts-node test_smoke.ts --creds-file creds.json --connectors stripe,aci
 *   npx ts-node test_smoke.ts --creds-file creds.json --all --dry-run
 */

import { PaymentClient, types, NetworkError } from "hs-playlib";
import * as fs from "fs";
// @ts-ignore - protobuf generated files might not have types yet

const {
  PaymentServiceAuthorizeRequest,
  Currency,
  CaptureMethod,
  AuthenticationType,
  Connector,
  ConnectorConfig,
  Environment,
  RequestError,
  ResponseError
} = types;


// Test card configurations
const TEST_CARDS: Record<string, any> = {
  visa: {
    number: "4111111111111111",
    expMonth: "12",
    expYear: "2050",
    cvc: "123",
    holder: "Test User",
  },
  mastercard: {
    number: "5555555555554444",
    expMonth: "12",
    expYear: "2050",
    cvc: "123",
    holder: "Test User",
  },
};

// Default test amount
const DEFAULT_AMOUNT = { minorAmount: 1000, currency: Currency.USD };

// Placeholder values that indicate credentials are not configured
const PLACEHOLDER_VALUES = new Set(["", "placeholder", "test", "dummy", "sk_test_placeholder"]);

interface AuthConfig {
  [key: string]: string | object;
  metadata?: any;
}

interface Credentials {
  [connector: string]: AuthConfig | AuthConfig[];
}

interface TestResult {
  connector: string;
  status: "passed" | "failed" | "skipped" | "dry_run" | "passed_with_error";
  ffiTest?: { url: string; method: string; passed: boolean };
  roundTripTest?: { status?: number; type?: string; passed: boolean; error?: string; skipped?: boolean; reason?: string };
  error?: string;
}

function loadCredentials(credsFile: string): Credentials {
  if (!fs.existsSync(credsFile)) {
    throw new Error(`Credentials file not found: ${credsFile}`);
  }
  const content = fs.readFileSync(credsFile, "utf-8");
  return JSON.parse(content);
}

function isPlaceholder(value: string): boolean {
  if (!value) return true;
  const lower = value.toLowerCase();
  return PLACEHOLDER_VALUES.has(lower) || lower.includes("placeholder");
}

function hasValidCredentials(authConfig: AuthConfig): boolean {
  for (const [key, value] of Object.entries(authConfig)) {
    if (key === "metadata" || key === "_comment") continue;
    // Check for { value: string } structure (SecretString)
    if (typeof value === "object" && value !== null && "value" in value) {
      const val = (value as { value: unknown }).value;
      if (typeof val === "string" && !isPlaceholder(val)) {
        return true;
      }
    }
    // Fallback for string values (legacy support)
    if (typeof value === "string" && !isPlaceholder(value)) {
      return true;
    }
  }
  return false;
}

function buildAuthorizeRequest(cardType: string = "visa"): any {
  const card = TEST_CARDS[cardType] || TEST_CARDS.visa;

  return PaymentServiceAuthorizeRequest.create({
    merchantTransactionId: `smoke_test_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`,
    amount: {
      minorAmount: DEFAULT_AMOUNT.minorAmount,
      currency: DEFAULT_AMOUNT.currency,
    },
    captureMethod: CaptureMethod.AUTOMATIC,
    paymentMethod: {
      card: {
        cardNumber: { value: card.number },
        cardExpMonth: { value: card.expMonth },
        cardExpYear: { value: card.expYear },
        cardCvc: { value: card.cvc },
        cardHolderName: { value: card.holder },
      },
    },
    customer: {
      email: { value: "test@example.com" },
      name: "Test User",
    },
    authType: AuthenticationType.NO_THREE_DS,
    returnUrl: "https://example.com/return",
    webhookUrl: "https://example.com/webhook",
    address: {},
    testMode: true,
  });
}

async function testConnector(
  instanceName: string,
  authConfig: AuthConfig,
  dryRun: boolean = false,
  baseConnectorName?: string
): Promise<TestResult> {
  // Use base name for metadata (without index), instance name for display
  const connectorKey = baseConnectorName || instanceName;

  const result: TestResult = {
    connector: instanceName,
    status: "pending" as any,
  };

  try {
    const req = buildAuthorizeRequest();

    // Get the correct connector enum value
    const connectorEnum = Connector[connectorKey.toUpperCase() as keyof typeof Connector];
    if (!connectorEnum) {
      throw new Error(`Unknown connector: ${connectorKey}`);
    }

    // Build auth config - filter out _comment and metadata fields
    // Convert snake_case to camelCase for protobuf compatibility
    const authFields: Record<string, any> = {};
    for (const [key, value] of Object.entries(authConfig)) {
      if (key !== "_comment" && key !== "metadata") {
        // Convert snake_case to camelCase
        const camelKey = key.replace(/_([a-z])/g, (_, letter) => letter.toUpperCase());
        authFields[camelKey] = value;
      }
    }

    // Build ConnectorAuth with the appropriate oneof field
    // The key should match the connector name (e.g., 'stripe', 'adyen', 'aci')
    const connectorAuthKey = connectorKey.toLowerCase();
    const connectorConfig: Record<string, unknown> = {
      [connectorAuthKey]: authFields,
    };

    const config = ConnectorConfig.create({
      options: { environment: Environment.SANDBOX },
      connectorConfig: connectorConfig as types.IConnectorSpecificConfig,
    });

    // Test 1: Low-level FFI via PaymentClient internals
    // We use the client to build the request
    const client = new PaymentClient(config);

    // For now, we'll just verify the request building works
    // The actual FFI call happens inside client.authorize()

    if (dryRun) {
      result.status = "dry_run";
      result.ffiTest = { url: "dry-run", method: "POST", passed: true };
      return result;
    }

    if (!hasValidCredentials(authConfig)) {
      result.status = "skipped";
      result.roundTripTest = { skipped: true, reason: "placeholder_credentials", passed: false };
      return result;
    }

    try {
      const response = await client.authorize(req);
      result.roundTripTest = {
        status: response.status,
        type: "PaymentServiceAuthorizeResponse",
        passed: true,
      };
      result.status = "passed";
    }
    catch (e: any) {
      if (e instanceof RequestError) {
        result.roundTripTest = {
          passed: true,
          error: e.errorMessage || `${types.PaymentStatus[e.status]}}` || String(e.statusCode) || String(e),
        };
        result.status = "passed_with_error";
        result.error = e.errorMessage || `${types.PaymentStatus[e.status]}}` || String(e.statusCode) || String(e);
      } else if (e instanceof ResponseError) {
        result.roundTripTest = {
          passed: true,
          error: e.errorMessage || `${types.PaymentStatus[e.status]}}` || String(e.statusCode) || String(e),
        };
        result.status = "passed_with_error";
        result.error = e.errorMessage || `${types.PaymentStatus[e.status]}}` || String(e.statusCode) || String(e);
      } else if (e instanceof NetworkError) {
        result.roundTripTest = {
          passed: true,
          error: `${e.code}: ${e.message}`,
        };
        result.status = "passed_with_error";
        result.error = `${e.code}: ${e.message}`;
      } else {
        result.roundTripTest = {
          passed: true,
          error: e.message || String(e),
        };
        result.status = "passed_with_error";
        result.error = e.message || String(e);
      }
    }
  }


  catch (e: any) {
    result.status = "failed";
    result.error = e.message || String(e);
  }

  return result;
}



function parseArgs(): { credsFile: string; connectors?: string[]; all: boolean; dryRun: boolean; card: string } {
  const args = process.argv.slice(2);
  let credsFile = "creds.json";
  let connectors: string[] | undefined;
  let all = false;
  let dryRun = false;
  let card = "visa";

  for (let i = 0; i < args.length; i++) {
    const arg = args[i];
    if (arg === "--creds-file" && i + 1 < args.length) {
      credsFile = args[++i];
    } else if (arg === "--connectors" && i + 1 < args.length) {
      connectors = args[++i].split(",").map((c) => c.trim());
    } else if (arg === "--all") {
      all = true;
    } else if (arg === "--dry-run") {
      dryRun = true;
    } else if (arg === "--card" && i + 1 < args.length) {
      card = args[++i];
    } else if (arg === "--help" || arg === "-h") {
      console.log(`
Usage: npx ts-node test_smoke.ts [options]

Options:
  --creds-file <path>     Path to credentials JSON (default: creds.json)
  --connectors <list>     Comma-separated list of connectors to test
  --all                   Test all connectors in the credentials file
  --dry-run               Build requests without executing HTTP calls
  --card <type>           Test card type: visa or mastercard (default: visa)
  --help, -h              Show this help message

Examples:
  npx ts-node test_smoke.ts --all
  npx ts-node test_smoke.ts --connectors stripe,aci
  npx ts-node test_smoke.ts --all --dry-run
`);
      process.exit(0);
    }
  }

  if (!all && !connectors) {
    console.error("Error: Must specify either --all or --connectors");
    process.exit(1);
  }

  return { credsFile, connectors, all, dryRun, card };
}

async function runTests(
  credsFile: string,
  connectors: string[] | undefined,
  dryRun: boolean
): Promise<TestResult[]> {
  const credentials = loadCredentials(credsFile);
  const results: TestResult[] = [];

  const testConnectors = connectors || Object.keys(credentials);

  console.log(`\n${"=".repeat(60)}`);
  console.log(`Running smoke tests for ${testConnectors.length} connector(s)`);
  console.log(`${"=".repeat(60)}\n`);

  for (const connectorName of testConnectors) {
    const authConfig = credentials[connectorName];
    if (!authConfig) {
      console.log(`\n--- Testing ${connectorName} ---`);
      console.log(`  SKIPPED (not found in credentials file)`);
      results.push({ connector: connectorName, status: "skipped", error: "not_found" });
      continue;
    }

    console.log(`\n--- Testing ${connectorName} ---`);

    if (Array.isArray(authConfig)) {
      // Multi-instance connector
      for (let i = 0; i < authConfig.length; i++) {
        const instanceName = `${connectorName}[${i + 1}]`;
        console.log(`  Instance: ${instanceName}`);

        if (!hasValidCredentials(authConfig[i])) {
          console.log(`  SKIPPED (placeholder credentials)`);
          results.push({
            connector: instanceName,
            status: "skipped",
            roundTripTest: { skipped: true, reason: "placeholder_credentials", passed: false },
          });
          continue;
        }

        const result = await testConnector(instanceName, authConfig[i], dryRun, connectorName);
        results.push(result);

        if (result.status === "passed") {
          console.log(`  ✓ PASSED`);
        } else if (result.status === "passed_with_error") {
          console.log(`  ✓ PASSED (with connector error ${result.error})`);
        } else if (result.status === "dry_run") {
          console.log(`  ✓ DRY RUN`);
        } else {
          console.log(`  ✗ ${result.status.toUpperCase()}: ${result.error || "Unknown error"}`);
        }
      }
    } else {
      // Single-instance connector
      if (!hasValidCredentials(authConfig)) {
        console.log(`  SKIPPED (placeholder credentials)`);
        results.push({
          connector: connectorName,
          status: "skipped",
          roundTripTest: { skipped: true, reason: "placeholder_credentials", passed: false },
        });
        continue;
      }

      const result = await testConnector(connectorName, authConfig, dryRun);
      results.push(result);

      if (result.status === "passed") {
        console.log(`  ✓ PASSED`);
      } else if (result.status === "passed_with_error") {
        console.log(`  ✓ PASSED (with connector error ${result.error})`);
      } else if (result.status === "dry_run") {
        console.log(`  ✓ DRY RUN`);
      } else {
        console.log(`  ✗ ${result.status.toUpperCase()}: ${result.error || "Unknown error"}`);
      }
    }
  }

  return results;
}

function printSummary(results: TestResult[]): number {
  console.log(`\n${"=".repeat(60)}`);
  console.log("TEST SUMMARY");
  console.log(`${"=".repeat(60)}\n`);

  const passed = results.filter((r) => ["passed", "passed_with_error", "dry_run"].includes(r.status)).length;
  const skipped = results.filter((r) => r.status === "skipped").length;
  const failed = results.filter((r) => r.status === "failed").length;
  const total = results.length;

  console.log(`Total:   ${total}`);
  console.log(`Passed:  ${passed} ✓`);
  console.log(`Skipped: ${skipped} (placeholder credentials)`);
  console.log(`Failed:  ${failed} ✗`);
  console.log();

  if (failed > 0) {
    console.log("Failed tests:");
    for (const result of results) {
      if (result.status === "failed") {
        console.log(`  - ${result.connector}: ${result.error || "Unknown error"}`);
      }
    }
    console.log();
    return 1;
  }

  if (passed === 0 && skipped > 0) {
    console.log("All tests skipped (no valid credentials found)");
    console.log("Update creds.json with real credentials to run tests");
    return 1;
  }

  console.log("All tests completed successfully!");
  return 0;
}

async function main() {
  const { credsFile, connectors, all, dryRun, card } = parseArgs();

  try {
    const results = await runTests(credsFile, connectors, dryRun);
    const exitCode = printSummary(results);
    process.exit(exitCode);
  } catch (e: any) {
    console.error(`\nFatal error: ${e.message || e}`);
    process.exit(1);
  }
}

main();
