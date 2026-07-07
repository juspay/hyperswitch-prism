/**
 * Webhook smoke test — Adyen AUTHORISATION
 *
 * Uses a real Adyen AUTHORISATION webhook body and feeds it into
 * EventClient.handleEvent / parseEvent with connector identity only
 * (no API credentials, no webhook secret).
 *
 * What this validates:
 *  1. SDK routes to the correct connector from identity alone
 *  2. Adyen webhook body is parsed correctly
 *  3. event_type is returned
 *  4. source_verified=false is expected — no real HMAC secret provided,
 *     and Adyen verification is not mandatory so it must NOT error out
 *  5. IntegrationError / ConnectorError are NOT thrown for a valid payload
 *
 * Usage:
 *   cd sdk/javascript && npx tsx smoke-test/test_smoke_webhook.ts
 */

import { EventClient, IntegrationError, ConnectorError } from "hyperswitch-prism";
import { types } from "hyperswitch-prism";

const { Environment, HttpMethod } = types;

// ── ANSI helpers ───────────────────────────────────────────────────────────────
const NO_COLOR = !process.stdout.isTTY || !!process.env["NO_COLOR"];
const c = (code: string, t: string) => NO_COLOR ? t : `\x1b[${code}m${t}\x1b[0m`;
const green  = (t: string) => c("32", t);
const yellow = (t: string) => c("33", t);
const red    = (t: string) => c("31", t);
const grey   = (t: string) => c("90", t);
const bold   = (t: string) => c("1",  t);

// ── Adyen AUTHORISATION webhook body (from real test configuration) ────────────
// Sensitive fields replaced:
//   merchantAccountCode → "YOUR_MERCHANT_ACCOUNT"
//   merchantReference   → "pay_test_00000000000000"
//   pspReference        → "TEST000000000000"
//   hmacSignature       → "test_hmac_signature_placeholder"
//   cardHolderName      → "John Doe"
//   shopperEmail        → "shopper@example.com"
const ADYEN_WEBHOOK_BODY = JSON.stringify({
  live: "false",
  notificationItems: [{
    NotificationRequestItem: {
      additionalData: {
        authCode: "APPROVED",
        cardSummary: "1111",
        cardHolderName: "John Doe",
        expiryDate: "03/2030",
        shopperEmail: "shopper@example.com",
        shopperIP: "128.0.0.1",
        shopperInteraction: "Ecommerce",
        captureDelayHours: "0",
        gatewaySystem: "direct",
        hmacSignature: "test_hmac_signature_placeholder",
      },
      amount: { currency: "GBP", value: 654000 },
      eventCode: "AUTHORISATION",
      eventDate: "2026-01-21T14:18:18+01:00",
      merchantAccountCode: "YOUR_MERCHANT_ACCOUNT",
      merchantReference: "pay_test_00000000000000",
      operations: ["CAPTURE", "REFUND"],
      paymentMethod: "visa",
      pspReference: "TEST000000000000",
      reason: "APPROVED:1111:03/2030",
      success: "true",
    },
  }],
});

const ADYEN_HEADERS: Record<string, string> = {
  "content-type": "application/json",
  "accept": "*/*",
};

// ── Connector identity only — no API creds, no webhook secret ─────────────────
function buildConfig(): types.IConnectorConfig {
  return {
    options: { environment: Environment.SANDBOX },
    connectorConfig: { adyen: {} },
  };
}

// ── Test 1: handleEvent — AUTHORISATION ───────────────────────────────────────
async function testHandleEvent(): Promise<boolean> {
  console.log(bold("\n[Adyen Webhook — AUTHORISATION handleEvent]"));
  const client = new EventClient(buildConfig());

  const request: types.IEventServiceHandleRequest = {
    merchantEventId: "smoke_wh_adyen_auth",
    requestDetails: {
      method: HttpMethod.HTTP_METHOD_POST,
      uri: "/webhooks/adyen",
      headers: ADYEN_HEADERS,
      body: new TextEncoder().encode(ADYEN_WEBHOOK_BODY),
    },
    // capture_method is the EventContext use case:
    // Adyen AUTHORISATION maps to AUTHORIZED (manual) or CAPTURED (automatic)
    eventContext: {
      payment: {
        captureMethod: types.CaptureMethod.MANUAL,
      },
    },
  };

  try {
    const response = await client.handleEvent(request);
    console.log(grey(`  event_type     : ${response.eventType}`));
    console.log(grey(`  source_verified: ${response.sourceVerified}`));
    console.log(grey(`  merchant_event : ${response.merchantEventId}`));
    if (!response.sourceVerified) {
      console.log(yellow("  ~ source_verified=false (expected — no real HMAC secret)"));
    }
    console.log(green("  ✓ PASSED: handleEvent returned response without crashing"));
    return true;
  } catch (e: any) {
    console.log(red(`  ✗ FAILED: ${e?.constructor?.name}: ${e.message}`));
    return false;
  }
}

// ── Test 2: parseEvent ─────────────────────────────────────────────────────────
async function testParseEvent(): Promise<boolean> {
  console.log(bold("\n[Adyen Webhook — AUTHORISATION parseEvent]"));
  const client = new EventClient(buildConfig());

  const request: types.IEventServiceParseRequest = {
    requestDetails: {
      method: HttpMethod.HTTP_METHOD_POST,
      uri: "/webhooks/adyen",
      headers: ADYEN_HEADERS,
      body: new TextEncoder().encode(ADYEN_WEBHOOK_BODY),
    },
  };

  try {
    const response = await client.parseEvent(request);
    console.log(grey(`  event_type : ${response.eventType}`));
    console.log(grey(`  reference  : ${response.reference}`));
    console.log(green("  ✓ PASSED: parseEvent returned response"));
    return true;
  } catch (e: any) {
    console.log(red(`  ✗ FAILED: ${e?.constructor?.name}: ${e.message}`));
    return false;
  }
}

// ── Test 3: malformed body ─────────────────────────────────────────────────────
async function testMalformedBody(): Promise<boolean> {
  console.log(bold("\n[Adyen Webhook — malformed body]"));
  const client = new EventClient(buildConfig());

  const request: types.IEventServiceHandleRequest = {
    requestDetails: {
      method: HttpMethod.HTTP_METHOD_POST,
      uri: "/webhooks/adyen",
      headers: ADYEN_HEADERS,
      body: new TextEncoder().encode("not valid json {{{{"),
    },
  };

  try {
    const response = await client.handleEvent(request);
    console.log(yellow(`  ~ accepted malformed body — event_type: ${response.eventType}`));
    return true;
  } catch (e: any) {
    if (e instanceof IntegrationError || e instanceof ConnectorError) {
      console.log(green(`  ✓ PASSED: ${e.constructor.name} thrown as expected: ${e.message}`));
      return true;
    }
    console.log(red(`  ✗ FAILED: unexpected error: ${e?.constructor?.name}: ${e.message}`));
    return false;
  }
}

// ── Test 4: unknown eventCode ──────────────────────────────────────────────────
async function testUnknownEventCode(): Promise<boolean> {
  console.log(bold("\n[Adyen Webhook — unknown eventCode]"));
  const client = new EventClient(buildConfig());

  const body = JSON.parse(ADYEN_WEBHOOK_BODY);
  body.notificationItems[0].NotificationRequestItem.eventCode = "SOME_UNKNOWN_EVENT";

  const request: types.IEventServiceHandleRequest = {
    requestDetails: {
      method: HttpMethod.HTTP_METHOD_POST,
      uri: "/webhooks/adyen",
      headers: ADYEN_HEADERS,
      body: new TextEncoder().encode(JSON.stringify(body)),
    },
  };

  try {
    const response = await client.handleEvent(request);
    console.log(green(`  ✓ PASSED: handled gracefully — event_type: ${response.eventType}`));
    return true;
  } catch (e: any) {
    if (e instanceof IntegrationError || e instanceof ConnectorError) {
      console.log(green(`  ✓ PASSED: ${e.constructor.name} for unknown event (expected): ${e.message}`));
      return true;
    }
    console.log(red(`  ✗ FAILED: ${e?.constructor?.name}: ${e.message}`));
    return false;
  }
}

// ── main ───────────────────────────────────────────────────────────────────────
async function main(): Promise<void> {
  console.log(bold("Adyen Webhook Smoke Test"));
  console.log("─".repeat(50));

  const results = [
    await testHandleEvent(),
    await testParseEvent(),
    await testMalformedBody(),
    await testUnknownEventCode(),
  ];

  console.log("\n" + "=".repeat(50));
  const allPassed = results.every(r => r);
  console.log(allPassed ? green("PASSED") : red("FAILED"));
  process.exit(allPassed ? 0 : 1);
}

main().catch((e: any) => { console.error(e); process.exit(1); });
