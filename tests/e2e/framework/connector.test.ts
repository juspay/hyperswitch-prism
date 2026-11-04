import assert from "node:assert/strict";
import test from "node:test";
import { connectorFromFile, isConnector } from "./connector.js";

test("derives and brands a connector from its definition filename", () => {
  const connector = connectorFromFile(
    "file:///repo/tests/e2e/connectors/stripe.ts",
    {},
  );

  assert.equal(connector.id, "stripe");
  assert.equal(isConnector(connector), true);
});

test("rejects connector definitions outside the direct connector directory", () => {
  assert.throws(
    () => connectorFromFile("file:///repo/tests/e2e/examples/stripe.ts", {}),
    /must be located under connectors/,
  );
  assert.throws(
    () => connectorFromFile("file:///repo/tests/e2e/connectors/nested/stripe.ts", {}),
    /must be direct children of connectors/,
  );
});
