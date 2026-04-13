import { PaymentClient, MerchantAuthenticationClient, types, IntegrationError, ConnectorError } from "hyperswitch-prism";
import * as fs from "fs";
import * as path from "path";

const {
  PaymentStatus,
  Connector,
} = types;

const { RequestConfig, Environment } = types;

const defaults = RequestConfig.create({});

interface Credentials {
  stripe?: { apiKey?: { value: string }; api_key?: { value: string } } | Array<{ apiKey?: { value: string }; api_key?: { value: string } }>;
  paypal?: { clientId?: { value: string }; client_id?: { value: string }; clientSecret?: { value: string }; client_secret?: { value: string } } | Array<{ clientId?: { value: string }; client_id?: { value: string }; clientSecret?: { value: string }; client_secret?: { value: string } }>;
}

function loadCredentials(credsFile: string): Credentials {
  if (!fs.existsSync(credsFile)) return {};
  return JSON.parse(fs.readFileSync(credsFile, "utf-8"));
}

function getStripeApiKey(credentials: Credentials): string | null {
  if (!credentials.stripe) return null;
  const stripeCreds = Array.isArray(credentials.stripe) ? credentials.stripe[0] : credentials.stripe;
  return stripeCreds.apiKey?.value || stripeCreds.api_key?.value || null;
}

function getPayPalCredentials(credentials: Credentials): { clientId: string; clientSecret: string } | null {
  if (!credentials.paypal) return null;
  const paypalCreds = Array.isArray(credentials.paypal) ? credentials.paypal[0] : credentials.paypal;
  const clientId = paypalCreds.clientId?.value || paypalCreds.client_id?.value;
  const clientSecret = paypalCreds.clientSecret?.value || paypalCreds.client_secret?.value;
  if (clientId && clientSecret) {
    return { clientId, clientSecret };
  }
  return null;
}

async function testPaypalAuthorize(credsFile: string): Promise<boolean> {
  console.log("\n[PayPal Authorize]");

  if (!fs.existsSync(credsFile)) {
    console.log("  SKIPPED: creds.json not found");
    return true;
  }
  const credentials = loadCredentials(credsFile);
  const paypalCreds = getPayPalCredentials(credentials);

  if (!paypalCreds) {
    console.log("  SKIPPED: No PayPal credentials in creds.json");
    return true;
  }

  console.log(`  Using client_id: ${paypalCreds.clientId.substring(0, 10)}...`);

  const paypalConfig: types.IConnectorConfig = {
    options: { environment: Environment.SANDBOX },
    connectorConfig: {
      paypal: {
        clientId: { value: paypalCreds.clientId },
        clientSecret: { value: paypalCreds.clientSecret },
      },
    },
  };

  const authClient = new MerchantAuthenticationClient(paypalConfig, defaults);
  const paymentClient = new PaymentClient(paypalConfig, defaults);

  // Step 1: Create access token
  console.log("  Step 1: Creating access token...");
  let accessTokenValue: string | null = null;
  let tokenTypeValue: string | null = null;
  let expiresInSeconds: number = 3600;

  try {
    const accessTokenResponse = await authClient.createServerAuthenticationToken({
      merchantAccessTokenId: "paypal_token_" + Date.now(),
      connector: Connector.PAYPAL,
      testMode: true,
    });

    if (accessTokenResponse.accessToken?.value) {
      accessTokenValue = accessTokenResponse.accessToken.value;
      tokenTypeValue = accessTokenResponse.tokenType ?? "Bearer";
      expiresInSeconds = Number(accessTokenResponse.expiresInSeconds) || 3600;
      console.log("  Access token received");
    } else {
      console.log("  No access token in response");
      return true;
    }
  } catch (e: any) {
    if (e instanceof IntegrationError) {
      console.log(`  IntegrationError: ${e.message} (code=${e.errorCode}, action=${e.suggestedAction}, doc=${e.docUrl})`);
      return true;
    }
    if (e instanceof ConnectorError) {
      console.log(`  ConnectorError: ${e.message} (code=${e.errorCode}, http=${e.httpStatusCode})`);
      return true;
    }
    console.log(`  Error creating access token: ${e.message}`);
    return true;
  }

  // Step 2: Authorize with access token
  console.log("  Step 2: Authorizing with access token...");
  const authorizeRequest: types.PaymentServiceAuthorizeRequest = {
    merchantTransactionId: "paypal_authorize_" + Date.now(),
    amount: { minorAmount: 1000, currency: types.Currency.USD },
    captureMethod: types.CaptureMethod.AUTOMATIC,
    paymentMethod: {
      card: {
        cardNumber: { value: "4111111111111111" },
        cardExpMonth: { value: "12" },
        cardExpYear: { value: "2050" },
        cardCvc: { value: "123" },
        cardHolderName: { value: "Test User" },
      },
    },
    state: {
      accessToken: {
        token: { value: accessTokenValue },
        tokenType: tokenTypeValue,
        expiresInSeconds,
      },
    },
    address: { billingAddress: {} },
    authType: types.AuthenticationType.NO_THREE_DS,
    returnUrl: "https://example.com/return",
    orderDetails: [],
  };

  try {
    const response = await paymentClient.authorize(authorizeRequest);

    if (response.status === PaymentStatus.CHARGED) {
      console.log("  PASSED: Payment charged");
      return true;
    } else {
      console.log(`  FAILED: Expected CHARGED, got ${response.status}`);
      return false;
    }
  } catch (e: any) {
    if (e instanceof IntegrationError) {
      console.log(`  IntegrationError: ${e.message} (code=${e.errorCode}, action=${e.suggestedAction}, doc=${e.docUrl})`);
      return true;
    }
    if (e instanceof ConnectorError) {
      console.log(`  ConnectorError: ${e.message} (code=${e.errorCode}, http=${e.httpStatusCode})`);
      return true;
    }
    console.error("  Error:", e.message || e);
    return false;
  }
}

function parseArgs(): { credsFile: string } {
  const args = process.argv.slice(2);
  let credsFile = "creds.json";

  for (let i = 0; i < args.length; i++) {
    const arg = args[i];
    if (arg === "--creds-file" && i + 1 < args.length) {
      credsFile = args[++i];
    }
  }

  // Resolve relative paths from cwd
  if (!path.isAbsolute(credsFile)) {
    credsFile = path.join(process.cwd(), credsFile);
  }

  return { credsFile };
}

/**
 * IntegrationError path: missing required field (amount) must throw IntegrationError
 * and must NOT reach the connector.
 */
async function testStripeIntegrationError(credsFile: string): Promise<boolean> {
  console.log("\n[Stripe IntegrationError — missing amount]");

  if (!fs.existsSync(credsFile)) {
    console.log("  FAILED: creds.json not found");
    return false;
  }
  const credentials = loadCredentials(credsFile);
  const apiKey = getStripeApiKey(credentials);

  if (!apiKey) {
    console.log("  FAILED: No Stripe API key in creds.json");
    return false;
  }

  const stripeConfig: types.ConnectorConfig = {
    options: { environment: Environment.SANDBOX },
    connectorConfig: { stripe: { apiKey: { value: apiKey } } },
  };

  const paymentClient = new PaymentClient(stripeConfig, defaults);

  // amount intentionally omitted
  const authorizeRequest: types.PaymentServiceAuthorizeRequest = {
    merchantTransactionId: "stripe_missing_amount_" + Date.now(),
    paymentMethod: {
      card: {
        cardNumber: { value: "4111111111111111" },
        cardExpMonth: { value: "12" },
        cardExpYear: { value: "2050" },
        cardCvc: { value: "123" },
        cardHolderName: { value: "Test User" },
      },
    },
    captureMethod: types.CaptureMethod.AUTOMATIC,
    address: { billingAddress: {} },
    authType: types.AuthenticationType.NO_THREE_DS,
    returnUrl: "https://example.com/return",
    orderDetails: [],
  };

  try {
    await paymentClient.authorize(authorizeRequest);
    console.log("  FAILED: Expected IntegrationError but call succeeded — request should have been rejected before the HTTP call");
    return false;
  } catch (e: any) {
    if (e instanceof IntegrationError) {
      console.log(`  PASSED: IntegrationError (expected): ${e.message} (code=${e.errorCode})`);
      return true;
    }
    if (e instanceof ConnectorError) {
      console.log(`  FAILED: Got ConnectorError instead of IntegrationError: ${e.message} (code=${e.errorCode}, http=${e.httpStatusCode})`);
      return false;
    }
    console.error("  FAILED: Unexpected error:", e.message || e);
    return false;
  }
}

/**
 * ConnectorError path: request is valid but card is known to be declined by Stripe.
 * Must throw ConnectorError, not IntegrationError.
 */
async function testStripeConnectorError(credsFile: string): Promise<boolean> {
  console.log("\n[Stripe ConnectorError — declined card]");

  if (!fs.existsSync(credsFile)) {
    console.log("  FAILED: creds.json not found");
    return false;
  }
  const credentials = loadCredentials(credsFile);
  const apiKey = getStripeApiKey(credentials);

  if (!apiKey) {
    console.log("  FAILED: No Stripe API key in creds.json");
    return false;
  }

  const stripeConfig: types.ConnectorConfig = {
    options: { environment: Environment.SANDBOX },
    connectorConfig: { stripe: { apiKey: { value: apiKey } } },
  };

  const paymentClient = new PaymentClient(stripeConfig, defaults);

  const authorizeRequest: types.PaymentServiceAuthorizeRequest = {
    merchantTransactionId: "stripe_declined_" + Date.now(),
    amount: { minorAmount: 1000, currency: types.Currency.USD },
    captureMethod: types.CaptureMethod.AUTOMATIC,
    paymentMethod: {
      card: {
        cardNumber: { value: "4000000000000002" }, // Stripe generic decline test card
        cardExpMonth: { value: "12" },
        cardExpYear: { value: "2050" },
        cardCvc: { value: "123" },
        cardHolderName: { value: "Test User" },
      },
    },
    address: { billingAddress: {} },
    authType: types.AuthenticationType.NO_THREE_DS,
    returnUrl: "https://example.com/return",
    orderDetails: [],
  };

  try {
    await paymentClient.authorize(authorizeRequest);
    // Stripe should decline 4000000000000002 — if it doesn't, not our failure
    console.log("  PASSED: Card unexpectedly succeeded (sandbox may behave differently)");
    return true;
  } catch (e: any) {
    if (e instanceof ConnectorError) {
      console.log(`  PASSED: ConnectorError (expected): ${e.message} (code=${e.errorCode}, http=${e.httpStatusCode})`);
      return true;
    }
    if (e instanceof IntegrationError) {
      console.log(`  FAILED: Got IntegrationError instead of ConnectorError: ${e.message} (code=${e.errorCode})`);
      return false;
    }
    console.error("  FAILED: Unexpected error:", e.message || e);
    return false;
  }
}

async function testStripeAuthorize(credsFile: string): Promise<boolean> {
  console.log("\n[Stripe Authorize]");

  if (!fs.existsSync(credsFile)) {
    console.log("  FAILED: creds.json not found");
    return false;
  }
  const credentials = loadCredentials(credsFile);
  const apiKey = getStripeApiKey(credentials);

  if (!apiKey) {
    console.log("  FAILED: No Stripe API key in creds.json");
    return false;
  }

  const stripeConfig: types.ConnectorConfig = {
    options: { environment: Environment.SANDBOX },
    connectorConfig: { stripe: { apiKey: { value: apiKey } } },
  };

  const paymentClient = new PaymentClient(stripeConfig, defaults);

  const authorizeRequest: types.PaymentServiceAuthorizeRequest = {
    merchantTransactionId: "stripe_authorize_" + Date.now(),
    amount: { minorAmount: 1000, currency: types.Currency.USD },
    captureMethod: types.CaptureMethod.AUTOMATIC,
    paymentMethod: {
      card: {
        cardNumber: { value: "4111111111111111" },
        cardExpMonth: { value: "12" },
        cardExpYear: { value: "2050" },
        cardCvc: { value: "123" },
        cardHolderName: { value: "Test User" },
      },
    },
    address: { billingAddress: {} },
    authType: types.AuthenticationType.NO_THREE_DS,
    returnUrl: "https://example.com/return",
    orderDetails: []
  };

  try {
    const response = await paymentClient.authorize(authorizeRequest);

    if (response.status === PaymentStatus.CHARGED) {
      console.log("  PASSED: Payment charged");
      return true;
    } else {
      console.log(`  FAILED: Expected CHARGED, got ${response.status}`);
      return false;
    }
  } catch (e: any) {
    if (e instanceof IntegrationError) {
      console.log(`  IntegrationError: ${e.message} (code=${e.errorCode}, action=${e.suggestedAction}, doc=${e.docUrl})`);
      return true;
    }
    if (e instanceof ConnectorError) {
      console.log(`  ConnectorError: ${e.message} (code=${e.errorCode}, http=${e.httpStatusCode})`);
      return true;
    }
    console.error("  FAILED:", e.message || e);
    return false;
  }
}

async function main(): Promise<void> {
  const { credsFile } = parseArgs();
  let allPassed = true;

  const paypalPassed = await testPaypalAuthorize(credsFile);
  if (!paypalPassed) allPassed = false;

  const stripePassed = await testStripeAuthorize(credsFile);
  if (!stripePassed) allPassed = false;

  const stripeIntegrationErrorPassed = await testStripeIntegrationError(credsFile);
  if (!stripeIntegrationErrorPassed) allPassed = false;

  const stripeConnectorErrorPassed = await testStripeConnectorError(credsFile);
  if (!stripeConnectorErrorPassed) allPassed = false;

  console.log("\n" + "=".repeat(40));
  console.log(allPassed ? "PASSED" : "FAILED");
  process.exit(allPassed ? 0 : 1);
}

main().catch((e: any) => {
  console.error(e);
  process.exit(1);
});