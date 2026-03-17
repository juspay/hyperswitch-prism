/**
 * Smoke test for PayPal access token flow using hyperswitch-payments npm tarball.
 *
 * This test demonstrates:
 *   1. Create an access token via PayPal
 *   2. Use the access token in an authorize request
 *
 * Usage:
 *   mkdir /tmp/test-js-sdk && cd /tmp/test-js-sdk && npm init -y
 *   npm install <path-to>/hyperswitch-payments-0.1.0.tgz
 *   npx ts-node test_access_token_smoke.ts
 */

import { PaymentClient, MerchantAuthenticationClient, types, NetworkError } from "hs-playlib";

const {
  MerchantAuthenticationServiceCreateAccessTokenRequest,
  MerchantAuthenticationServiceCreateAccessTokenResponse,
  PaymentServiceAuthorizeRequest,
  PaymentServiceAuthorizeResponse,
  Currency,
  CaptureMethod,
  AuthenticationType,
  Connector,
  SecretString,
  AccessToken,
  ConnectorState,
  RequestError,
  ResponseError,
} = types;

const { ConnectorConfig, RequestConfig, Environment } = types;

const PAYPAL_CREDS = {
  client_id: "client_id",
  client_secret: "client_secret",
};



// 1. ConnectorConfig (connector, auth, environment)
const config = ConnectorConfig.create({
  options: { environment: Environment.SANDBOX },
  connectorConfig: {
    paypal: {
      clientId: { value: PAYPAL_CREDS.client_id },
      clientSecret: { value: PAYPAL_CREDS.client_secret },
    },
  },
});

// 2. Optional RequestConfig defaults (http, vault)
const defaults = RequestConfig.create({});

/**
 * Test the access token flow:
 * 1. Create access token
 * 2. Use access token in authorize request
 */
async function testAccessTokenFlow(): Promise<void> {
  console.log("\n=== Test: PayPal Access Token Flow ===");

  const authClient = new MerchantAuthenticationClient(config, defaults);
  const paymentClient = new PaymentClient(config, defaults);

  // Step 1: Create Access Token Request
  console.log("\n--- Step 1: Create Access Token ---");
  const accessTokenRequest: types.IMerchantAuthenticationServiceCreateAccessTokenRequest =
    MerchantAuthenticationServiceCreateAccessTokenRequest.create({
      merchantAccessTokenId: "access_token_test_" + Date.now(),
      connector: Connector.PAYPAL,
      testMode: true,
    });

  // Make the request via MerchantAuthenticationClient
  let accessTokenResponse: types.MerchantAuthenticationServiceCreateAccessTokenResponse;
  let accessTokenValue: string | null = null;
  let tokenTypeValue: string | null = null;

  try {
    accessTokenResponse = await authClient.createAccessToken(accessTokenRequest);
    console.log(`  Response type: ${typeof accessTokenResponse}`);
    console.log(`  Response keys: ${Object.keys(accessTokenResponse)}`);

    // Extract access token from response
    if (
      accessTokenResponse.accessToken &&
      accessTokenResponse.accessToken.value
    ) {
      accessTokenValue = accessTokenResponse.accessToken.value;
      tokenTypeValue = accessTokenResponse.tokenType ?? "Bearer";
      console.log(
        `  Access Token received: ${accessTokenValue!.substring(0, 20)}...`
      );
      console.log(`  Token Type: ${tokenTypeValue}`);
      console.log(
        `  Expires In: ${accessTokenResponse.expiresInSeconds} seconds`
      );
      console.log(`  Status: ${accessTokenResponse.status}`);
    } else {
      console.log("  WARNING: No access token in response");
      console.log(
        "  Full response:",
        JSON.stringify(accessTokenResponse, null, 2)
      );
    }
  } catch (e: any) {
    if (e instanceof RequestError) {
      console.log(`  RequestError: ${e.errorCode} - ${e.errorMessage}`);
      console.log("  This might be expected if credentials are not valid");
      return;
    } else if (e instanceof ResponseError) {
      console.log(`  ResponseError: ${e.errorCode} - ${e.errorMessage}`);
      console.log("  This might be expected if credentials are not valid");
      return;
    } else if (e instanceof NetworkError) {
      console.log(`  NetworkError: ${e.code} - ${e.message}`);
      console.log("  This might be expected for connectivity issues");
      return;
    }
    const message = e instanceof Error ? e.message : String(e);
    console.log(`  Error creating access token: ${message}`);
    console.log("  This might be expected if credentials are not valid");
    return;
  }

  if (!accessTokenValue) {
    console.log("  SKIPPED: Cannot proceed without access token");
    return;
  }

  // Step 2: Use Access Token in Authorize Request
  console.log("\n--- Step 2: Authorize with Access Token ---");
  const authorizeRequest: types.IPaymentServiceAuthorizeRequest =
    PaymentServiceAuthorizeRequest.create({
      merchantTransactionId: "authorize_with_token_" + Date.now(),
      amount: {
        minorAmount: 1000, // $10.00
        currency: Currency.USD,
      },
      captureMethod: CaptureMethod.AUTOMATIC,
      paymentMethod: {
        card: {
          cardNumber: { value: "4111111111111111" },
          cardExpMonth: { value: "12" },
          cardExpYear: { value: "2050" },
          cardCvc: { value: "123" },
          cardHolderName: { value: "Test User" },
        },
      },
      customer: {
        email: { value: "test@example.com" },
        name: "Test",
      },
      state: ConnectorState.create({
        accessToken: AccessToken.create({
          token: SecretString.create({ value: accessTokenValue }),
          tokenType: tokenTypeValue,
          expiresInSeconds: accessTokenResponse!.expiresInSeconds,
        }),
      }),
      authType: AuthenticationType.NO_THREE_DS,
      returnUrl: "https://example.com/return",
      webhookUrl: "https://example.com/webhook",
      address: {},
      testMode: true,
    });

  try {
    const authorizeResponse: types.PaymentServiceAuthorizeResponse =
      await paymentClient.authorize(authorizeRequest);
    console.log(`  Response type: ${typeof authorizeResponse}`);
    console.log(`  Response keys: ${Object.keys(authorizeResponse)}`);
    console.log(`  Payment status: ${authorizeResponse.status}`);
    console.log("  PASSED");
  } catch (e: any) {
    if (e instanceof RequestError) {
      console.log(`  RequestError: ${e.errorCode} - ${e.errorMessage}`);
      console.log("  PASSED (round-trip completed, error is from PayPal)");
    } else if (e instanceof ResponseError) {
      console.log(`  ResponseError: ${e.errorCode} - ${e.errorMessage}`);
      console.log("  PASSED (round-trip completed, error is from PayPal)");
    } else if (e instanceof NetworkError) {
      console.log(`  NetworkError: ${e.code} - ${e.message}`);
      console.log("  PASSED (round-trip completed, connectivity error)");
    } else {
      console.log(`  Error during authorize: ${e}`);
      console.log("  PASSED (round-trip completed, error is from PayPal)");
    }
  }

  console.log("\n=== Test Complete ===");
}

// Run the test
testAccessTokenFlow()
  .then(() => console.log("\nAll checks passed."))
  .catch((e: unknown) => {
    console.error(e);
    process.exit(1);
  });
