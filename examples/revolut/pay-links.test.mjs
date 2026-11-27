import { test } from 'node:test';
import assert from 'node:assert/strict';
import { getUniversalLink } from './pay-links.mjs';
const session = { checkout_base_url: 'https://sandbox-checkout.revolut.com', order_token: 'token+&', public_key: 'pk_test' };
test('same-device link retrieval encodes return URL and does not send credentials', async () => {
  const result = await getUniversalLink(session, { appReturnUrl: 'myapp://return?attempt=1&next=2' }, async (url, options) => {
    const parsed = new URL(url);
    assert.equal(parsed.pathname, '/get-universal-link');
    assert.equal(parsed.searchParams.get('orderToken'), 'token+&');
    assert.equal(parsed.searchParams.get('appReturnUrl'), 'myapp://return?attempt=1&next=2');
    assert.equal(options.credentials, 'omit');
    return { ok: true, json: async () => ({ universal_link: 'https://merchant.revolut.com/pay' }) };
  });
  assert.equal(result, 'https://merchant.revolut.com/pay');
});
test('cross-device source and provider failures', async () => {
  await assert.rejects(getUniversalLink(session, { source: 'invalid' }));
  await assert.rejects(getUniversalLink(session, { source: 'INPERSON_KIOSK' }, async url => {
    assert.equal(new URL(url).searchParams.get('src'), 'INPERSON_KIOSK');
    return { ok: false, status: 500 };
  }), /500/);
  await assert.rejects(getUniversalLink(session, { appReturnUrl: 'myapp://return' }, async () => ({ok: true, json: async () => ({universal_link: 'javascript:alert(1)'})})), /protocol/);
});
