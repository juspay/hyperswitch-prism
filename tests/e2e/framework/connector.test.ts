import assert from "node:assert/strict";
import test from "node:test";
import { connectorFromFile, isConnector } from "./connector.js";

test("derives and brands a connector from its definition filename", () => {
  const connector = connectorFromFile(
    "file:///repo/tests/e2e/connectors/stripe/merchant_01.ts",
    { run: ["scenario"] },
  );

  assert.equal(connector.id, "stripe");
  assert.equal(connector.merchantVariant, "merchant_01");
  assert.equal(isConnector(connector), true);
});

test("rejects connector definitions outside synthetic merchant paths", () => {
  assert.throws(
    () => connectorFromFile("file:///repo/tests/e2e/examples/stripe.ts", { run: [] }),
    /must be located under connectors/,
  );
  assert.throws(
    () => connectorFromFile("file:///repo/tests/e2e/connectors/stripe.ts", { run: [] }),
    /must be connectors/,
  );
});
