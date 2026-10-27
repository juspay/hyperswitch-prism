import { randomUUID } from "node:crypto";
import { expect, test } from "@playwright/test";
import type { ConnectorDefinition } from "./scenario.js";
import { loadConnectorCredentials } from "./credentials.js";
import { configuration } from "./configuration.js";
import { Hyperswitch, stringField } from "./hyperswitch.js";

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
    const credentials = loadConnectorCredentials(
      connector.credentialKey,
      connector.credentialEntry,
    );
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
    const apiKey = stringField(apiKeyResponse, "api_key");

    let profileId =
      typeof account.default_profile === "string"
        ? account.default_profile
        : undefined;
    if (!profileId) {
      const profile = await hs.call(
        "POST",
        `/account/${merchantId}/business_profile`,
        apiKey,
        { profile_name: "ucs-e2e" },
      );
      profileId = stringField(profile, "profile_id");
    }
    expect(typeof profileId, "Business profile is available").toBe("string");
    if (!profileId) throw new Error("Business profile was not created");

    const merchantConnector = await hs.call(
      "POST",
      `/account/${merchantId}/connectors`,
      apiKey,
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
    const merchantConnectorId = stringField(
      merchantConnector,
      "merchant_connector_id",
    );

    const routing = await hs.call("POST", "/routing", apiKey, {
      name: "ucs-e2e",
      description: "Route to the connector under test",
      profile_id: profileId,
      algorithm: {
        type: "single",
        data: {
          connector: connector.id,
          merchant_connector_id: merchantConnectorId,
        },
      },
    });
    await hs.call(
      "POST",
      `/routing/${stringField(routing, "id")}/activate`,
      apiKey,
      { profile_id: profileId },
    );

    return {
      merchantId,
      profileId,
      apiKey,
      merchantConnectorId,
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
