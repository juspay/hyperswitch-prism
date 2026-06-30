# hyperswitch-prism workshop — JavaScript / TypeScript

A self-contained, runnable workshop that teaches the **hyperswitch-prism**
unified payment library through eight short steps: run a payment with one PSP,
switch processors with a one-line change, build a payment orchestrator
(condition-based routing + retry), extend the library with a new flow/processor,
and run a test suite.

> 📖 **Follow the guided walkthrough in [STEPS.md](./STEPS.md)** — it maps every
> command below to a workshop step.

## Quick start

```bash
cd workshop/javascript
npm install
cp .env.example .env        # optional — add sandbox keys to see real approvals

npm run run:payment         # Steps 1–4: run a payment (switch PSP in config/active-psp.ts)
npm run run:routing         # Step 6a: condition-based routing
npm run run:retry           # Step 6b: payment retry / fallback
npm run run:extend          # Step 7: add a new processor / new flow
npm test                    # Step 8: the test suite (no credentials needed)
```

## How it works

```
┌─────────────────────────────────────────────────────────────┐
│  steps/*.ts        — demos you run                          │
├─────────────────────────────────────────────────────────────┤
│  orchestrator/     — routing + retry (pure, processor-free) │
├─────────────────────────────────────────────────────────────┤
│  library/unified-payments.ts — the unified library          │
│      authorize() · capture() · void() · refund()            │
├─────────────────────────────────────────────────────────────┤
│  config/psp-registry.ts   — every PSP lives here            │
│  config/active-psp.ts     — the one-line switch             │
├─────────────────────────────────────────────────────────────┤
│  hyperswitch-prism (npm)  — unified SDK → 100+ processors   │
└─────────────────────────────────────────────────────────────┘
```

The whole point: **application code never names a processor.** Swapping Stripe
for Adyen is a one-line config change; adding a processor is one registry entry.

## The PSPs in this workshop

| Key | Processor | Role | Env vars |
|-----|-----------|------|----------|
| `stripe` | Stripe | PSP-1 | `STRIPE_API_KEY` |
| `adyen` | Adyen | PSP-2 | `ADYEN_API_KEY`, `ADYEN_MERCHANT_ACCOUNT` |
| `cybersource` | Cybersource | PSP-3 (added in Step 7) | `CYBERSOURCE_API_KEY`, `CYBERSOURCE_MERCHANT_ACCOUNT`, `CYBERSOURCE_API_SECRET` |

No credentials? Everything still runs — you'll see the request get built and sent
and a connector error come back instead of a charge. The test suite needs no keys.

## Requirements

- Node.js 18+ (LTS recommended)
- Linux x64, macOS, or Windows via WSL2 (the SDK ships a native x86_64 library)
- No build step — scripts run TypeScript directly via `tsx`
