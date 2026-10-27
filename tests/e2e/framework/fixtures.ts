import { test as base, type TestType } from "@playwright/test";
import { Assertions } from "./assertions.js";
import { HyperswitchClient } from "./hyperswitch-client.js";
import { setupMerchant, type TestMerchant } from "./merchant-setup.js";
import { PaymentOperations } from "./payment-operations.js";
import { UcsAudit } from "./ucs-audit.js";
import { Report } from "./report.js";
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
  report: Pick<Report, "step">;
};

type InternalFixtures = {
  _hs: HyperswitchClient;
  _merchant: TestMerchant;
  _overrides: ScenarioOverrides;
  _checks: Assertions;
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
  _checks: async ({}, use, info) => {
    await use(new Assertions(info));
  },
  checks: async ({ _checks }, use) => {
    await use({
      fields: _checks.fields.bind(_checks),
      oneOf: _checks.oneOf.bind(_checks),
    });
  },
  report: async ({ _audit }, use) => {
    await use(new Report(_audit));
  },
  payments: async ({ _hs, _merchant, _checks, _overrides }, use) => {
    await use(new PaymentOperations(_hs, _merchant, _checks, _overrides));
  },
});

// Publish only the supported scenario API. This assignment is checked by TypeScript.
export const test: TestType<ScenarioFixtures, {}> = fixtures;
