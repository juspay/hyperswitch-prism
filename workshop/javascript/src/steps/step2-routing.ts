// WORKSHOP STEP 6a — Condition-based routing
// ─────────────────────────────────────────────────────────────────────────────
//   npm run run:routing
//
// We feed several different "carts" through the SAME routing plan and watch the
// orchestrator pick a PSP based on amount and currency. Then we actually run the
// chosen PSP through the unified library.
//
// The routing decision (selectPsp) is a pure function — see test/routing.test.ts
// for how we prove it picks correctly without any network calls.
// ─────────────────────────────────────────────────────────────────────────────

import 'dotenv/config';

import { selectPsp, DEFAULT_ROUTING_PLAN } from '../orchestrator/routing.js';
import { authorize } from '../library/unified-payments.js';
import { getPsp } from '../../config/psp-registry.js';
import { APPROVED_CARD, type Order } from '../library/cards.js';
import { banner, step, money } from '../library/format.js';

const CARTS: Array<{ label: string; minorAmount: number; currency: string }> = [
  { label: 'Small USD order', minorAmount: 1500, currency: 'USD' }, // → stripe (fallback)
  { label: 'EUR order', minorAmount: 2000, currency: 'EUR' }, // → cybersource
  { label: 'High-value order', minorAmount: 9900, currency: 'USD' }, // → adyen
];

async function main() {
  banner('Condition-based routing');
  console.log('  Routing plan (first matching rule wins):');
  DEFAULT_ROUTING_PLAN.rules.forEach((r, i) => console.log(`    ${i + 1}. ${r.reason}`));
  console.log(`    ${DEFAULT_ROUTING_PLAN.rules.length + 1}. otherwise → ${DEFAULT_ROUTING_PLAN.fallback} (fallback)`);

  for (const cart of CARTS) {
    const decision = selectPsp(DEFAULT_ROUTING_PLAN, cart);
    const psp = getPsp(decision.psp);

    banner(`${cart.label}: ${money(cart.minorAmount, cart.currency)}`);
    step(`routed to ${psp.displayName}  —  ${decision.reason}`);

    const order: Order = {
      merchantTransactionId: `routing_${Date.now()}`,
      minorAmount: cart.minorAmount,
      currency: cart.currency,
      card: APPROVED_CARD,
    };

    const result = await authorize(decision.psp, order);
    console.log(`    → ${psp.displayName}: ${result.statusText} (${result.status})${result.error ? ` — ${result.error}` : ''}`);
  }

  banner('Routing demo complete.');
}

main().catch((e) => {
  console.error('Fatal:', e);
  process.exit(1);
});
