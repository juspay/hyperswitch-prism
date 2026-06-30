# Workshop walkthrough (JavaScript / TypeScript)

This is the hands-on guide. Each section maps to a step in the workshop agenda.
Total time: ~45–60 minutes. Everything runs locally with `tsx` (no build step).

> **Credentials are optional.** Every demo runs without keys — you'll see the
> request being built and sent, then a connector error instead of a charge. Add
> sandbox keys in `.env` to see real approvals. The **test suite needs no keys**.

---

## Setup (2 min)

```bash
cd workshop/javascript
npm install
cp .env.example .env     # optional: fill in sandbox keys for the PSPs you have
```

Requirements: Node.js 18+ on Linux x64 / macOS / WSL2 (the SDK ships a native
x86_64 library — see the SDK README for platform notes).

---

## Step 1 & 2 — Run a payment and view the experience with PSP-1 (Stripe)

```bash
npm run run:payment
```

You'll see the unified library run a full **authorize → refund** lifecycle
through **Stripe** (the default `ACTIVE_PSP`). With a real `STRIPE_API_KEY` the
authorize returns `CHARGED` and the refund returns `REFUND_SUCCESS`/`REFUND_PENDING`.

> The "payment experience" is the console output: status, transaction id, and
> any error — all normalized by the library regardless of processor.

**What to look at:** `src/steps/step1-run-payment.ts` and `src/library/unified-payments.ts`.
Notice the app code never mentions "Stripe" — it just calls `authorize(ACTIVE_PSP, order)`.

---

## Step 3 — Switch the PSP from PSP-1 to PSP-2 (the minimal change)

Open **`config/active-psp.ts`** and change exactly one line:

```diff
- export const ACTIVE_PSP: PspName = 'stripe';
+ export const ACTIVE_PSP: PspName = 'adyen';
```

That's the entire change. No application, orchestrator, or request code is touched.

---

## Step 4 — Re-run with PSP-2 (Adyen)

```bash
npm run run:payment
```

Same command, same code — now the library drives **Adyen**. This is the core
lesson: **the processor is a configuration detail, not a code dependency.**

> Switch it back to `'stripe'` (or try `'cybersource'`) and run again to feel how
> cheap swapping processors becomes with a unified library.

---

## Step 6a — Condition-based routing (payment orchestrator)

```bash
npm run run:routing
```

Three sample carts are fed through one **routing plan**:

| Cart | Rule that matches | PSP chosen |
|------|-------------------|-----------|
| $15.00 USD | none → fallback | Stripe |
| €20.00 EUR | EUR currency | Cybersource |
| $99.00 USD | amount > $50.00 | Adyen |

The decision logic lives in **`src/orchestrator/routing.ts`** as a **pure
function** `selectPsp(plan, ctx)`. Edit `DEFAULT_ROUTING_PLAN` to add your own
rules (e.g. route GBP somewhere, or route by amount band).

---

## Step 6b — Payment retry / fallback (payment orchestrator)

```bash
npm run run:retry
```

This combines routing **and** retry: routing picks the primary PSP, then
`withRetry` tries it first and **falls back** to the other PSPs until one
approves (or all are exhausted). Watch the per-attempt log.

The retry engine is **`src/orchestrator/retry.ts`** — also pure and
processor-agnostic (it takes an `attempt` callback). That's what makes it
robust and testable.

---

## Step 7 — Extend the library: new processor and/or new flow

```bash
npm run run:extend
```

**(A) Add a new processor** — we added `cybersource` as PSP-3. Adding a
processor is a single entry in **`config/psp-registry.ts`**; routing, retry, and
all demos pick it up automatically. **Try it:** add a 4th processor from the SDK
(e.g. `bluesnap`, `nuvei`, `globalpay`) by copying an existing registry entry and
adjusting its auth fields.

**(B) Add a new flow** — we added `voidPayment()` to
**`src/library/unified-payments.ts`** and exercised a new composite flow:
`authorize(MANUAL)` → `void`. Every flow follows the same shape (build client →
call SDK method → normalize result), so adding `capture`, `sync`, `dispute`,
etc. follows the same recipe.

---

## Step 8 — Run the test suite (prove the change is robust)

```bash
npm test
```

17 unit tests, **no credentials or network required**:

- `test/routing.test.ts` — routing picks the right PSP for every condition,
  including rule precedence and threshold edges.
- `test/retry.test.ts` — retry stops at first success, exhausts correctly,
  honors `maxAttempts`, and fires `onAttempt` per try (driven by fakes).
- `test/unified-payments.test.ts` — result normalization maps SDK statuses to
  `ok` / `pending` / error correctly.

Also available: `npm run typecheck` (full TypeScript check).

> **Try the red→green loop:** flip a rule in `routing.ts`, run `npm test`, watch
> it fail, then fix it. That's the "ensure the change is robust" muscle.

---

## File map

```
workshop/javascript/
├── config/
│   ├── active-psp.ts        ← Step 3: the one-line PSP switch
│   └── psp-registry.ts      ← Step 7A: add a processor here (only place)
├── src/
│   ├── library/
│   │   ├── unified-payments.ts  ← the unified library (authorize/capture/refund/void)
│   │   ├── cards.ts             ← test cards + sample order
│   │   └── format.ts            ← output helpers
│   ├── orchestrator/
│   │   ├── routing.ts           ← Step 6a: condition-based routing (pure)
│   │   └── retry.ts             ← Step 6b: retry/fallback (pure)
│   └── steps/
│       ├── step1-run-payment.ts ← Steps 1–4
│       ├── step2-routing.ts     ← Step 6a demo
│       ├── step3-retry.ts       ← Step 6b demo
│       └── step4-extend.ts      ← Step 7 demo
└── test/                        ← Step 8: the test suite
```
