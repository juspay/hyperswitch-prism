import { test as base, type TestType } from "@playwright/test";
import { Assertions } from "./assertions.js";
import { HyperswitchClient } from "./hyperswitch-client.js";
import { setupMerchant, type TestMerchant } from "./merchant-setup.js";
import { PaymentOperations } from "./payment-operations.js";
import { UcsAudit } from "./ucs-audit.js";
import type {
  ConnectorDefinition,
  Scenario,
  ScenarioOverrides,
} from "./types.js";

type ScenarioFixtures = {
  connector: ConnectorDefinition;
  scenario: Scenario;
  merchant: Pick<
    TestMerchant,
    "merchantId" | "profileId" | "merchantConnectorId"
  >;
  payments: PaymentOperations;
  checks: Pick<Assertions, "fields" | "oneOf">;
};

type InternalFixtures = {
  _hs: HyperswitchClient;
  _merchant: TestMerchant;
  _overrides: ScenarioOverrides;
  _checks: Assertions;
};

// Setup fixtures have no payment journey. Raw HTTP and API keys remain internal.
const fixtures = base.extend<ScenarioFixtures & InternalFixtures>({
  connector: [
    async () => {
      throw new Error("Configure the connector with test.use({ connector })");
    },
    { option: true },
  ],
  scenario: [
    async () => {
      throw new Error(
        "Configure scenario metadata with test.use({ scenario })",
      );
    },
    { option: true },
  ],
  _overrides: [
    async ({ connector, scenario }, use, info) => {
      const exclusion = connector.exclude?.[scenario.id];
      if (exclusion && !exclusion.reason.trim()) {
        throw new Error("Scenario exclusions require a reason");
      }
      base.skip(!!exclusion, exclusion?.reason);
      info.annotations.push({
        type: "Scenario",
        description: scenario.description,
      });
      info.annotations.push({ type: "Verifies", description: scenario.verifies });
      const overrides = connector.overrides?.[scenario.id] ?? {};
      if (Object.keys(overrides.requests ?? {}).length) {
        info.annotations.push({
          type: "Connector request overrides",
          description: Object.keys(overrides.requests!).join(", "),
        });
      }
      await use(overrides);
    },
    { auto: true },
  ],
  _hs: async ({}, use, info) => {
    const audit = new UcsAudit(info);
    const hs = await HyperswitchClient.create((call) => audit.record(call));
    try {
      await use(hs);
    } finally {
      await hs.dispose();
    }
  },
  _merchant: async ({ _hs, connector, scenario }, use) => {
    await use(
      await setupMerchant(_hs, connector, scenario.paymentMethodsEnabled),
    );
  },
  merchant: async ({ _merchant }, use) => {
    await use({
      merchantId: _merchant.merchantId,
      profileId: _merchant.profileId,
      merchantConnectorId: _merchant.merchantConnectorId,
    });
  },
  _checks: async ({}, use, info) => {
    await use(new Assertions(info));
  },
  checks: async ({ _checks }, use) => {
    await use({
      fields: _checks.fields.bind(_checks),
      oneOf: _checks.oneOf.bind(_checks),
    });
  },
  payments: async ({ _hs, _merchant, _checks, _overrides }, use) => {
    await use(new PaymentOperations(_hs, _merchant, _checks, _overrides));
  },
});

// Publish only the supported scenario API. This assignment is checked by TypeScript.
export const test: TestType<ScenarioFixtures, {}> = fixtures;
