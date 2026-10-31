import { test as base, type TestType } from "@playwright/test";
import { HyperswitchClient } from "./hyperswitch-client.js";
import { setupMerchant, type TestMerchant } from "./merchant-setup.js";
import { PaymentOperations } from "./payment-operations.js";
import { UcsAudit } from "./ucs-audit.js";
import { Report } from "./report.js";
import { ScenarioOverrideResolver } from "./scenario-overrides.js";
import type {
  ConnectorDefinition,
  JsonObject,
  Scenario,
} from "./types.js";

type ScenarioFixtures = {
  connector: ConnectorDefinition;
  scenario: Scenario;
  merchant: Pick<
    TestMerchant,
    "merchantId" | "profileId" | "merchantConnectorId"
  >;
  payments: PaymentOperations;
  report: Pick<Report, "step" | "check">;
  expected: <T extends JsonObject>(checkId: string, base: T) => T;
};

type InternalFixtures = {
  _hs: HyperswitchClient;
  _merchant: TestMerchant;
  _overrides: ScenarioOverrideResolver;
  _audit: UcsAudit;
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
      if (
        Object.keys(overrides.requests ?? {}).length
        || Object.keys(overrides.expectations ?? {}).length
      ) {
        info.annotations.push({
          type: "Connector overrides",
          description: [
            ...Object.keys(overrides.requests ?? {}).map((name) => `request:${name}`),
            ...Object.keys(overrides.expectations ?? {}).map((name) =>
              `expectation:${name}`),
          ].join(", "),
        });
      }
      const resolver = new ScenarioOverrideResolver(overrides);
      await use(resolver);
      resolver.assertAllConsumed();
    },
    { auto: true },
  ],
  _audit: async ({}, use, info) => {
    const audit = new UcsAudit(info);
    try {
      await use(audit);
    } finally {
      await audit.attachPending();
    }
  },
  _hs: async ({ _audit }, use) => {
    const hs = await HyperswitchClient.create((call) => _audit.record(call));
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
  report: async ({ _audit }, use, info) => {
    await use(new Report(_audit, info));
  },
  payments: async ({ _hs, _merchant, _overrides }, use) => {
    await use(new PaymentOperations(_hs, _merchant, _overrides));
  },
  expected: async ({ _overrides }, use) => {
    await use((checkId, base) => _overrides.expected(checkId, base));
  },
});

// Publish only the supported scenario API. This assignment is checked by TypeScript.
export const test: TestType<ScenarioFixtures, {}> = fixtures;
