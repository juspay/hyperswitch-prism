// WORKSHOP STEP 7 — Extend the library (new flow + new processor)
// ─────────────────────────────────────────────────────────────────────────────
//   npm run run:extend
//
// This step demonstrates the two ways participants extend the unified library:
//
//   (A) ADD A NEW PROCESSOR
//       We added 'cybersource' (Cybersource) to config/psp-registry.ts. That ONE
//       registry entry made it usable everywhere — routing, retry, and the demos
//       below — with no other code changes. Try adding a 4th processor yourself!
//
//   (B) ADD A NEW FLOW
//       We added voidPayment() to src/library/unified-payments.ts. Below we exercise
//       a brand-new composite flow built from it: authorize with MANUAL capture,
//       then VOID (cancel) the authorization before it is captured.
//
// See test/unified-payments.test.ts for the matching unit test that proves the
// new flow's result-normalization is correct — that's STEP 8 (the test suite).
// ─────────────────────────────────────────────────────────────────────────────

import 'dotenv/config';

import { types } from 'hyperswitch-prism';
import { authorize, voidPayment } from '../library/unified-payments.js';
import { getPsp, listPsps } from '../../config/psp-registry.js';
import { APPROVED_CARD, type Order } from '../library/cards.js';
import { banner, step, money } from '../library/format.js';

async function main() {
  // (A) Show that the registry is the single extension point for processors.
  banner('(A) New processor — the registry is the only thing you touch');
  for (const name of listPsps()) {
    const psp = getPsp(name);
    const configured = psp.isConfigured() ? 'credentials present' : 'no credentials (add to .env)';
    console.log(`    • ${name.padEnd(10)} ${psp.displayName.padEnd(14)} [${configured}]`);
  }

  // (B) Exercise the brand-new authorize(MANUAL) → void flow.
  const psp = listPsps()[0]; // use the first PSP for the demo
  banner(`(B) New flow — manual authorize then VOID, via ${getPsp(psp).displayName}`);

  const order: Order = {
    merchantTransactionId: `extend_${Date.now()}`,
    minorAmount: 4200, // $42.00
    currency: 'USD',
    card: APPROVED_CARD,
  };

  step(`Authorizing ${money(order.minorAmount, order.currency)} with MANUAL capture (funds reserved, not taken) ...`);
  const auth = await authorize(psp, order, { captureMethod: types.CaptureMethod.MANUAL });
  console.log(`    → authorize: ${auth.statusText} (${auth.status})${auth.error ? ` — ${auth.error}` : ''}`);

  if (!auth.transactionId) {
    banner('No transaction id (likely missing credentials). Add keys to .env to run the full void flow.');
    return;
  }

  step(`Voiding authorization ${auth.transactionId} (NEW flow) ...`);
  const voided = await voidPayment(psp, order, auth.transactionId);
  console.log(`    → void: ${voided.statusText} (${voided.status})${voided.error ? ` — ${voided.error}` : ''}`);

  banner('Extension demo complete — a new processor and a new flow, no orchestrator changes.');
}

main().catch((e) => {
  console.error('Fatal:', e);
  process.exit(1);
});
