// WORKSHOP STEP 6b — Payment retry / fallback
// ─────────────────────────────────────────────────────────────────────────────
//   npm run run:retry
//
// We combine BOTH orchestrator use-cases:
//   1. routing picks the primary PSP for the cart
//   2. retry tries the primary first, then falls back to the other PSPs until
//      one approves (or all are exhausted)
//
// The retry engine (withRetry) is pure and processor-agnostic — see
// test/retry.test.ts where we drive it with fakes (success on Nth try, all-fail,
// etc.) and assert it behaves, with zero network calls.
// ─────────────────────────────────────────────────────────────────────────────

import 'dotenv/config';

import { selectPsp, DEFAULT_ROUTING_PLAN } from '../orchestrator/routing.js';
import { withRetry, buildPlan } from '../orchestrator/retry.js';
import { authorize, type UnifiedResult } from '../library/unified-payments.js';
import { getPsp, listPsps } from '../../config/psp-registry.js';
import { APPROVED_CARD, type Order } from '../library/cards.js';
import { banner, step, money } from '../library/format.js';

async function main() {
  const cart = { minorAmount: 7500, currency: 'USD' }; // high-value → routes to adyen

  banner(`Routing + retry for ${money(cart.minorAmount, cart.currency)}`);

  // 1. Route to a primary PSP.
  const decision = selectPsp(DEFAULT_ROUTING_PLAN, cart);
  step(`primary PSP: ${getPsp(decision.psp).displayName}  (${decision.reason})`);

  // 2. Build a fallback plan: primary first, then everyone else.
  const fallbacks = listPsps().filter((p) => p !== decision.psp);
  const plan = buildPlan(decision.psp, fallbacks);
  step(`retry plan: ${plan.map((p) => getPsp(p).displayName).join('  →  ')}\n`);

  const order: Order = {
    merchantTransactionId: `retry_${Date.now()}`,
    minorAmount: cart.minorAmount,
    currency: cart.currency,
    card: APPROVED_CARD,
  };

  // 3. Run the retry loop against the real unified library.
  const outcome = await withRetry<UnifiedResult>({
    plan,
    attempt: (psp) => authorize(psp, { ...order, merchantTransactionId: `${order.merchantTransactionId}_${psp}` }),
    onAttempt: (psp, i, res) => {
      const tag = res.ok ? 'APPROVED' : 'declined/failed';
      console.log(`    attempt ${i + 1} — ${getPsp(psp).displayName}: ${res.statusText} → ${tag}${res.error ? ` (${res.error})` : ''}`);
    },
  });

  if (outcome.succeeded && outcome.winningPsp) {
    banner(`Approved by ${getPsp(outcome.winningPsp).displayName} after ${outcome.attempts.length} attempt(s).`);
  } else {
    banner(`All ${outcome.attempts.length} PSP(s) failed. (Add sandbox credentials in .env to see an approval.)`);
  }
}

main().catch((e) => {
  console.error('Fatal:', e);
  process.exit(1);
});
