// STEP 8 — test suite: unified result normalization
// We test the PURE normalizer that maps raw SDK responses to UnifiedResult.
// No PaymentClient is constructed, so there is no network call.
import { test } from 'node:test';
import assert from 'node:assert/strict';

import { types } from 'hyperswitch-prism';
import { normalizeAuthorize } from '../src/library/unified-payments.js';

test('CHARGED is a successful (ok) result', () => {
  const r = normalizeAuthorize('stripe', {
    status: types.PaymentStatus.CHARGED,
    connectorTransactionId: 'txn_123',
  });
  assert.equal(r.ok, true);
  assert.equal(r.transactionId, 'txn_123');
  assert.equal(r.statusText, 'CHARGED');
  assert.equal(r.error, undefined);
});

test('AUTHORIZED is also ok (auth without capture)', () => {
  const r = normalizeAuthorize('adyen', { status: types.PaymentStatus.AUTHORIZED });
  assert.equal(r.ok, true);
});

test('FAILURE is not ok and surfaces the connector error message', () => {
  const r = normalizeAuthorize('stripe', {
    status: types.PaymentStatus.FAILURE,
    error: { connectorDetails: { message: 'card declined' } },
  });
  assert.equal(r.ok, false);
  assert.equal(r.error, 'card declined');
});

test('PENDING is flagged pending and not treated as ok', () => {
  const r = normalizeAuthorize('cybersource', { status: types.PaymentStatus.PENDING });
  assert.equal(r.ok, false);
  assert.equal(r.pending, true);
});

test('carries the PSP name through unchanged', () => {
  const r = normalizeAuthorize('adyen', { status: types.PaymentStatus.CHARGED });
  assert.equal(r.psp, 'adyen');
  assert.equal(r.operation, 'authorize');
});
