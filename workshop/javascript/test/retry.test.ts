// STEP 8 — test suite: payment retry / fallback
// We drive withRetry() with FAKE attempt functions so we can assert the
// orchestration logic precisely, with zero network calls.
import { test } from 'node:test';
import assert from 'node:assert/strict';

import { withRetry, buildPlan, type AttemptOutcome } from '../src/orchestrator/retry.js';
import type { PspName } from '../config/psp-registry.js';

const fail = (statusText = 'FAILURE'): AttemptOutcome => ({ ok: false, statusText, error: 'declined' });
const pass = (statusText = 'CHARGED'): AttemptOutcome => ({ ok: true, statusText });

test('stops at the first successful PSP', async () => {
  const tried: PspName[] = [];
  const res = await withRetry({
    plan: ['stripe', 'adyen', 'cybersource'],
    attempt: async (psp) => {
      tried.push(psp);
      return psp === 'adyen' ? pass() : fail();
    },
  });
  assert.equal(res.succeeded, true);
  assert.equal(res.winningPsp, 'adyen');
  assert.deepEqual(tried, ['stripe', 'adyen']); // never reached cybersource
  assert.equal(res.attempts.length, 2);
});

test('returns failure after exhausting every PSP', async () => {
  const res = await withRetry({
    plan: ['stripe', 'adyen'],
    attempt: async () => fail(),
  });
  assert.equal(res.succeeded, false);
  assert.equal(res.winningPsp, null);
  assert.equal(res.attempts.length, 2);
  assert.equal(res.finalResult?.ok, false);
});

test('succeeds immediately when the first PSP approves', async () => {
  let calls = 0;
  const res = await withRetry({
    plan: ['stripe', 'adyen', 'cybersource'],
    attempt: async () => {
      calls++;
      return pass();
    },
  });
  assert.equal(calls, 1);
  assert.equal(res.winningPsp, 'stripe');
});

test('maxAttempts caps the number of tries', async () => {
  let calls = 0;
  const res = await withRetry({
    plan: ['stripe', 'adyen', 'cybersource'],
    maxAttempts: 2,
    attempt: async () => {
      calls++;
      return fail();
    },
  });
  assert.equal(calls, 2);
  assert.equal(res.attempts.length, 2);
});

test('onAttempt fires once per attempt with the right index', async () => {
  const seen: Array<[PspName, number]> = [];
  await withRetry({
    plan: ['stripe', 'adyen'],
    attempt: async () => fail(),
    onAttempt: (psp, i) => seen.push([psp, i]),
  });
  assert.deepEqual(seen, [['stripe', 0], ['adyen', 1]]);
});

test('buildPlan puts primary first and de-duplicates', () => {
  assert.deepEqual(buildPlan('adyen', ['stripe', 'adyen', 'cybersource']), ['adyen', 'stripe', 'cybersource']);
});
