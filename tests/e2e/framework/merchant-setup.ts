import { randomUUID } from "node:crypto";
import { expect, test } from "@playwright/test";
import type { ConnectorDefinition } from "./scenario.js";
import { loadConnectorCredentials } from "./credentials.js";
import { configuration } from "./configuration.js";
import { Hyperswitch } from "./hyperswitch.js";

export type TestMerchant = {
  merchantId: string;
  profileId: string;
  apiKey: string;
  merchantConnectorId: string;
};

export async function setupMerchant(
  hs: Hyperswitch,
  connector: ConnectorDefinition,
): Promise<TestMerchant> {
  return test.step("Set up merchant, connector account, and routing", async () => {
    await enableUcs(hs);
    const merchantId = `e2e_${randomUUID().replaceAll("-", "")}`;
    const account = await hs.call(
      "POST",
      "/accounts",
      configuration.adminApiKey,
      {
        merchant_id: merchantId,
        merchant_name: "UCS end-to-end test",
        primary_business_details: [{ country: "US", business: "default" }],
      },
    );
    const apiKeyResponse = await hs.call(
      "POST",
      `/api_keys/${merchantId}`,
      configuration.adminApiKey,
      {
        name: "UCS E2E",
        description: "Disposable sandbox test key",
        expiration: new Date(Date.now() + 86_400_000).toISOString(),
      },
    );
    expect(typeof apiKeyResponse.api_key, "Merchant API key is created").toBe(
      "string",
    );

    let profileId = account.default_profile as string | undefined;
    if (!profileId) {
      const profile = await hs.call(
        "POST",
        `/account/${merchantId}/business_profile`,
        apiKeyResponse.api_key,
        { profile_name: "ucs-e2e" },
      );
      profileId = profile.profile_id;
    }
    expect(typeof profileId, "Business profile is available").toBe("string");
    if (!profileId) throw new Error("Business profile was not created");

    const credentials = loadConnectorCredentials(connector.credentialKey);
    const merchantConnector = await hs.call(
      "POST",
      `/account/${merchantId}/connectors`,
      apiKeyResponse.api_key,
      {
        connector_name: connector.id,
        connector_label: `${connector.id}_e2e`,
        connector_type: "payment_processor",
        connector_account_details: credentials.connector_account_details,
        metadata: credentials.metadata ?? {},
        payment_methods_enabled: [],
        profile_id: profileId,
        test_mode: true,
        disabled: false,
      },
    );
    expect(
      typeof merchantConnector.merchant_connector_id,
      "Merchant connector account is created",
    ).toBe("string");

    const routing = await hs.call("POST", "/routing", apiKeyResponse.api_key, {
      name: "ucs-e2e",
      description: "Route to the connector under test",
      profile_id: profileId,
      algorithm: {
        type: "single",
        data: {
          connector: connector.id,
          merchant_connector_id: merchantConnector.merchant_connector_id,
        },
      },
    });
    await hs.call(
      "POST",
      `/routing/${routing.id}/activate`,
      apiKeyResponse.api_key,
      { profile_id: profileId },
    );

    return {
      merchantId,
      profileId,
      apiKey: apiKeyResponse.api_key,
      merchantConnectorId: merchantConnector.merchant_connector_id,
    };
  });
}

async function enableUcs(hs: Hyperswitch): Promise<void> {
  const response = await hs.fetch(
    "GET",
    "/configs/ucs_enabled",
    configuration.adminApiKey,
  );
  if (response.ok()) {
    const config = await response.json();
    if (config.value !== "true") {
      throw new Error("The local Hyperswitch stack has UCS disabled");
    }
    return;
  }
  if (response.status() !== 404) {
    throw new Error(
      `Cannot read Hyperswitch UCS configuration: HTTP ${response.status()}`,
    );
  }
  await hs.call("POST", "/configs/", configuration.adminApiKey, {
    key: "ucs_enabled",
    value: "true",
  });
}
