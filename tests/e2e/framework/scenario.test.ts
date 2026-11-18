import assert from "node:assert/strict";
import test from "node:test";
import { createScenarioResolver, defineScenarios } from "./scenario.js";

const catalog = defineScenarios({
  "payments/create-confirm/card/one-time/no3ds/create-confirm": {
    title: "Example",
    description: "Example scenario",
    verifies: "Example result",
    requirements: {
      connectorAccount: { paymentMethodsEnabled: [] },
    },
  },
});
const scenarioFromFile = createScenarioResolver(catalog);

test("derives the canonical scenario ID from its spec path", () => {
  const scenario = scenarioFromFile(
    "file:///repo/tests/e2e/scenarios/payments/create-confirm/card/one-time/no3ds/create-confirm.spec.ts",
  );

  assert.equal(
    scenario.id,
    "payments/create-confirm/card/one-time/no3ds/create-confirm",
  );
  assert.equal(scenario.title, "Example");
});

test("rejects a spec without the matching catalog entry", () => {
  assert.throws(
    () =>
      scenarioFromFile(
        "file:///repo/tests/e2e/scenarios/payments/create-confirm/card/recurring/connector-mandate/no3ds/setup-then-recurring.spec.ts",
      ),
    /No catalog entry for scenario: payments\/create-confirm\/card\/recurring\/connector-mandate\/no3ds\/setup-then-recurring/,
  );
});

test("rejects non-spec scenario files", () => {
  assert.throws(
    () =>
      scenarioFromFile(
        "file:///repo/tests/e2e/scenarios/payments/create-confirm/card/one-time/no3ds/create-confirm.planned.ts",
      ),
    /Scenario file must end in \.spec\.ts or \.spec\.js/,
  );
});
