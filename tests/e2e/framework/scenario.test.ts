import assert from "node:assert/strict";
import test from "node:test";
import { createScenarioResolver } from "./scenario.js";

const catalog = [
  "payments/create-confirm/card/one-time/no3ds/automatic-capture-create-confirm",
] as const;
const metadata = {
  title: "Example",
  description: "Example scenario",
  verifies: "Example result",
  requirements: {
    connectorAccount: { paymentMethodsEnabled: [] },
  },
};
const scenarioFromFile = createScenarioResolver(catalog);

test("derives the canonical scenario ID from its spec path", () => {
  const scenario = scenarioFromFile(
    "file:///repo/tests/e2e/scenarios/payments/create-confirm/card/one-time/no3ds/automatic-capture-create-confirm.spec.ts",
    metadata,
  );

  assert.equal(
    scenario.id,
    "payments/create-confirm/card/one-time/no3ds/automatic-capture-create-confirm",
  );
  assert.equal(scenario.title, "Example");
});

test("rejects a spec without the matching catalog entry", () => {
  assert.throws(
    () =>
      scenarioFromFile(
        "file:///repo/tests/e2e/scenarios/payments/create-confirm/card/recurring/connector-mandate/no3ds/setup-then-recurring.spec.ts",
        metadata,
      ),
    /No catalog entry for scenario: payments\/create-confirm\/card\/recurring\/connector-mandate\/no3ds\/setup-then-recurring/,
  );
});

test("rejects non-spec scenario files", () => {
  assert.throws(
    () =>
      scenarioFromFile(
        "file:///repo/tests/e2e/scenarios/payments/create-confirm/card/one-time/no3ds/create-confirm.planned.ts",
        metadata,
      ),
    /Scenario file must end in \.spec\.ts or \.spec\.js/,
  );
});

test("metadata belongs to the calling spec and must describe its purpose", () => {
  const file =
    "file:///repo/tests/e2e/scenarios/payments/create-confirm/card/one-time/no3ds/automatic-capture-create-confirm.spec.ts";
  const custom = scenarioFromFile(file, {
    ...metadata,
    title: "Caller-owned title",
  });
  assert.equal(custom.title, "Caller-owned title");
  assert.deepEqual(custom.requirements, metadata.requirements);
  for (const field of ["title", "description", "verifies"] as const) {
    assert.throws(
      () => scenarioFromFile(file, { ...metadata, [field]: " " }),
      new RegExp(`requires a nonempty ${field}`),
    );
  }
});
