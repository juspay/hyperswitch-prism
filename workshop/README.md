# hyperswitch-prism workshop

A hands-on workshop for the **hyperswitch-prism** unified payment library. Run a
payment through one payment service provider (PSP), switch processors with a
one-line change, build a small payment orchestrator (routing + retry), extend
the library, and run a test suite — all from working, runnable code.

## Agenda → where it lives

| # | Workshop step | Covered by |
|---|---------------|-----------|
| 1 | Execute the app to see the library in action with **PSP-1** | `npm run run:payment` |
| 2 | View the payment experience with **PSP-1** | console output of the same command |
| 3 | Make a **minimal code change** to switch PSP-1 → PSP-2 | edit one line in `config/active-psp.ts` |
| 4 | Re-execute to see the library with **PSP-2** | `npm run run:payment` again |
| 5 | Repeat in 2–3 languages (participant preference) | see **Languages** below |
| 6 | Build a payment orchestrator: **(a) condition-based routing, (b) payment retry** | `npm run run:routing`, `npm run run:retry` |
| 7 | Extend the library: **new flow or new processor** | `npm run run:extend` |
| 8 | Execute the **test suite** to ensure the change is robust | `npm test` |

## Start here

➡️ **[`javascript/`](./javascript/)** — the complete JS/TS workshop. Open
[`javascript/STEPS.md`](./javascript/STEPS.md) for the guided, step-by-step
walkthrough.

```bash
# Clone and switch to the workshop branch
git clone https://github.com/juspay/hyperswitch-prism.git
cd hyperswitch-prism
git fetch origin claude/funny-edison-nap6wt
git checkout claude/funny-edison-nap6wt

# Install and run
cd workshop/javascript
npm install
npm test                 # verify setup (no credentials needed)
npm run run:payment      # run your first payment
```

## Languages (step 5)

This folder implements the workshop end-to-end in **JavaScript/TypeScript**.

For the "repeat in another language" portion, the repository's top-level
[`examples/`](../examples) directory contains the **same connectors** (Stripe,
Adyen, Cybersource, and 90+ more) implemented in:

- **Python** — `examples/<connector>/<connector>.py`
- **Kotlin/Java** — `examples/<connector>/<connector>.kt`
- **Rust** — `examples/<connector>/<connector>.rs`

The SDKs for those languages live under [`sdk/`](../sdk) (`python`, `java`,
`rust`). The concepts demonstrated here — unified request/response, swapping
PSPs, routing, retry — translate directly; only the syntax changes. A natural
extension of this workshop is to port `javascript/` to Python or Kotlin using
those SDKs.

## No credentials? Still works.

Every demo runs without API keys: the unified library builds and sends the
request, and you'll see a connector error instead of a charge. Add sandbox keys
to `javascript/.env` (copy from `.env.example`) to see real approvals. The test
suite needs **no** credentials.
