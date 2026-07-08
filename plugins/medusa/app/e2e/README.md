# E2E tests (Playwright)

Browser + route-level end-to-end tests that exercise **medusa-custom-payments** and
**medusa-custom-payments-react** together through the `app/` harness (Express server on
:3000 serving the built React client and the `/store`, `/admin`, `/hooks` APIs same-origin).

## Layout

```
app/e2e/
├── fixtures/
│   ├── connectors.ts      # creds.json availability + per-connector test material
│   ├── test-cards.ts      # documented sandbox test cards
│   ├── pages.ts           # page objects (checkout init, order page actions)
│   ├── iframes.ts         # hosted-field fillers (Stripe/Adyen/GlobalPay) + PayPal popup
│   └── adyen-webhook.ts   # builds + injects a real HMAC-signed Adyen notification
├── ui/                    # browser specs: stripe, adyen, paypal, globalpay
└── api/                   # route-level specs: braintree, cybersource, mollie
```

## Prerequisites

1. **Credentials** — provide `creds.json` (default `~/Workspace/creds.json`, or set
   `CREDS_PATH`). See `creds.example.json` at the repo root for the schema. Connectors
   absent from the file are **skipped** automatically.
   - Adyen browser test also needs `adyen.hmac_key` (used to sign the injected webhook).
   - PayPal browser test also needs `paypal.sandbox_buyer { email, password }`.
2. **Browser** — `npx playwright install chromium`.

## Running

```bash
npm run test:e2e          # everything (builds, starts server, runs UI + API)
npm run test:e2e:ui       # the 4 browser connector specs
npm run test:e2e:api      # the 3 route-level specs
npx playwright test app/e2e/ui/stripe.spec.ts --headed --debug   # watch one flow
npx playwright show-report
```

The Playwright `webServer` builds the plugin, the React package, and the client, then runs
`npm run server:dev`. Tests run **serially** (`workers: 1`) because the harness uses a single
hardcoded cart (`cart_test_01`) and an in-memory session store.

## Expected outcomes

- `stripe`, `adyen` → **authorized** → capture → refund (and an authorize → void branch).
- `paypal`, `globalpay` → **captured** (auto-capture) → refund.
- `braintree`, `cybersource`, `mollie` → initiate returns the connector-specific session
  shape (these can't complete card auth headlessly: wallet nonce / transient token / redirect).

## Caveats

- The hosted-field selectors in `iframes.ts` target current connector SDK markup and are
  **best-effort**; if a connector ships a DOM change, update the frame/field locators there.
- PayPal's popup approval is the most fragile flow and depends on PayPal's sandbox UI.
- These hit **live sandboxes** — failures may reflect sandbox account state (e.g. a
  connector account "not activated"), not a code regression.
