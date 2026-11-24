import assert from "node:assert/strict";
import test from "node:test";
import { ScenarioOverrideResolver } from "./scenario-overrides.js";

test("applies omit before recursive patch without mutating the shared request", () => {
  const base = {
    metadata: { shared: true, replace: "base" },
    shipping: { address: { line1: "1 Test Street" }, phone: "2025550100" },
    order_details: [{ product_id: "shared" }],
    explicit_null: "replace-me",
  };
  const resolver = new ScenarioOverrideResolver({
    requests: {
      createPayment: {
        "initial-payment": {
          reason: "Connector sandbox requires these request fields",
          omit: [["shipping", "phone"]],
          patch: {
            metadata: { connector_variant: true, replace: "patched" },
            order_details: [{ product_id: "replacement" }],
            explicit_null: null,
          },
        },
      },
    },
  });

  assert.deepEqual(resolver.request("createPayment", "initial-payment", base), {
    metadata: {
      shared: true,
      connector_variant: true,
      replace: "patched",
    },
    shipping: { address: { line1: "1 Test Street" } },
    order_details: [{ product_id: "replacement" }],
    explicit_null: null,
  });
  assert.equal(base.shipping.phone, "2025550100");
  resolver.assertAllConsumed();
});

test("patches a stable expected-value check", () => {
  const resolver = new ScenarioOverrideResolver({
    expectations: {
      "payment-result": {
        reason: "Connector returns its documented sandbox reference",
        patch: { nested: { state: "connector-state" } },
      },
    },
  });

  assert.deepEqual(
    resolver.expected("payment-result", {
      status: "succeeded",
      nested: { state: "shared", retained: true },
    }),
    {
      status: "succeeded",
      nested: { state: "connector-state", retained: true },
    },
  );
  resolver.assertAllConsumed();
});

test("rejects unused overrides and duplicate stable call IDs", () => {
  const unused = new ScenarioOverrideResolver({
    requests: {
      createPayment: {
        "misspelled-call": {
          reason: "Synthetic test case",
          patch: { currency: "EUR" },
        },
      },
    },
  });
  assert.throws(() => unused.assertAllConsumed(), /Unused connector overrides/);

  const duplicate = new ScenarioOverrideResolver({});
  duplicate.request("createPayment", "initial-payment", { currency: "USD" });
  assert.throws(
    () => duplicate.request("createPayment", "initial-payment", { currency: "USD" }),
    /operation call ID used more than once/,
  );
});

test("rejects invalid omit paths and empty reasons", () => {
  assert.throws(
    () => new ScenarioOverrideResolver({
      requests: {
        createPayment: {
          "initial-payment": { reason: "   ", patch: { currency: "EUR" } },
        },
      },
    }),
    /requires a nonempty reason/,
  );

  const resolver = new ScenarioOverrideResolver({
    requests: {
      createPayment: {
        "initial-payment": {
          reason: "Synthetic test case",
          omit: [["shipping", "missing"]],
        },
      },
    },
  });
  assert.throws(
    () => resolver.request("createPayment", "initial-payment", {
      shipping: { address: { line1: "1 Test Street" } },
    }),
    /omit path does not exist/,
  );
});
