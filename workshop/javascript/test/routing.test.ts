// STEP 8 — test suite: condition-based routing
// Pure functions → no network, no credentials. Run with:  npm test
import { test } from 'node:test';
import assert from 'node:assert/strict';

import { selectPsp, DEFAULT_ROUTING_PLAN, type RoutingPlan } from '../src/orchestrator/routing.js';

test('high-value payments route to adyen', () => {
  const d = selectPsp(DEFAULT_ROUTING_PLAN, { minorAmount: 9900, currency: 'USD' });
  assert.equal(d.psp, 'adyen');
});

test('EUR payments route to cybersource', () => {
  const d = selectPsp(DEFAULT_ROUTING_PLAN, { minorAmount: 2000, currency: 'EUR' });
  assert.equal(d.psp, 'cybersource');
});

test('small USD payments fall back to stripe', () => {
  const d = selectPsp(DEFAULT_ROUTING_PLAN, { minorAmount: 1500, currency: 'USD' });
  assert.equal(d.psp, 'stripe');
});

test('first matching rule wins (high-value EUR → adyen, not cybersource)', () => {
  // 9900 (>5000) matches the high-value rule before the EUR rule.
  const d = selectPsp(DEFAULT_ROUTING_PLAN, { minorAmount: 9900, currency: 'EUR' });
  assert.equal(d.psp, 'adyen');
});

test('threshold is exclusive (exactly 5000 is NOT high-value)', () => {
  const d = selectPsp(DEFAULT_ROUTING_PLAN, { minorAmount: 5000, currency: 'USD' });
  assert.equal(d.psp, 'stripe');
});

test('custom plans are honored', () => {
  const plan: RoutingPlan = {
    rules: [{ reason: 'GBP → adyen', when: (c) => c.currency === 'GBP', use: 'adyen' }],
    fallback: 'cybersource',
  };
  assert.equal(selectPsp(plan, { minorAmount: 100, currency: 'GBP' }).psp, 'adyen');
  assert.equal(selectPsp(plan, { minorAmount: 100, currency: 'USD' }).psp, 'cybersource');
});
