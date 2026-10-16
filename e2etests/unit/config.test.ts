import { test } from "node:test";
import assert from "node:assert/strict";
import { assertOtp, credentials, merge } from "../support/config.js";
import { merchants } from "../otp/merchants.js";

test("merchant patches retain nested connector defaults, replace arrays, and do not mutate shared input", () => {
  const defaults = {
    metadata: { retained: true, replaced: 1 },
    methods: ["card"],
  };
  const result = merge(defaults, {
    metadata: { replaced: 2 },
    methods: ["wallet"],
  });
  assert.deepEqual(result, {
    metadata: { retained: true, replaced: 2 },
    methods: ["wallet"],
  });
  assert.equal(defaults.metadata.replaced, 1);
  assert.deepEqual(defaults.methods, ["card"]);
  assert.deepEqual(merge({ metadata: {} }, { metadata: null }), {
    metadata: null,
  });
});

test("HS credential layouts support direct entries, the first account, and explicit account selection", () => {
  const auth = {
    connector_account_details: {
      auth_type: "HeaderKey",
      api_key: "sandbox-placeholder",
    },
  };
  const document = {
    stripe: auth,
    adyen: {
      note: "ignored",
      connector_1: auth,
      connector_2: { ...auth, metadata: { alternate: true } },
    },
  };
  assert.deepEqual(credentials(document, "stripe"), auth);
  assert.deepEqual(credentials(document, "adyen"), auth);
  assert.equal(
    credentials(document, "adyen", "connector_2").metadata.alternate,
    true,
  );
  assert.throws(
    () => credentials(document, "adyen", "missing"),
    /Missing HS sandbox credentials/,
  );
  assert.throws(
    () => credentials(document, "missing"),
    /Missing HS sandbox credentials/,
  );
});

test("OTP overrides cannot accidentally switch the suite to mandate, wallet, or 3DS flows", () => {
  const base = {
    amount: 100,
    authentication_type: "no_three_ds",
    payment_method: "card",
    payment_method_data: { card: {} },
  };
  assert.doesNotThrow(() => assertOtp(base));
  for (const patch of [
    { authentication_type: "three_ds" },
    { payment_method: "wallet" },
    { amount: 0 },
    { amount: NaN },
    { amount: 1.5 },
    { off_session: true },
    { setup_future_usage: "off_session" },
    { mandate_data: {} },
    { mandate_id: "saved" },
    { customer_acceptance: {} },
    { recurring_details: {} },
  ])
    assert.throws(() => assertOtp(merge(base, patch)), /OTP requires/);
});

test("merchant inventory contains each supplied card OTP merchant/connector exactly once", () => {
  const ids = merchants.map((m) => `${m.org}/${m.merchant}/${m.connector}`);
  assert.equal(merchants.length, 48);
  assert.equal(new Set(ids).size, ids.length);
  assert.equal(new Set(merchants.map((m) => m.connector)).size, 6);
});

test("override merge rejects prototype keys from JSON input", () => {
  assert.throws(
    () => merge({}, JSON.parse('{"__proto__":{"polluted":true}}')),
    /Invalid override key/,
  );
  assert.equal(({} as any).polluted, undefined);
});
