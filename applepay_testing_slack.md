# Apple Pay PR Testing — Context & Approach

---

## Why we can't test Apple Pay the same way as Google Pay

With Google Pay, we could test end-to-end directly from localhost — open the demo app on `http://localhost:5252`, click Pay, and GPay would work in any browser. That's not possible with Apple Pay because of three hard requirements Apple enforces:

1. **HTTPS only** — the Apple Pay JS API refuses to initialize on non-HTTPS pages. `http://localhost` is a dead end.
2. **Domain verification** — Apple requires a domain association file to be served at `/.well-known/apple-developer-merchantid-domain-association` on the exact domain registered with the merchant certificate. You can't fake this on localhost.
3. **Safari + Apple hardware only** — Apple Pay only works in Safari on a Mac/iPhone/iPad with Touch ID or Face ID. No other browser, no emulation.

This means a full browser-driven E2E test (the way we tested GPay) would require deploying to a live HTTPS domain with proper cert setup — which is overkill for PR validation.

---

## Apple Pay flows — equivalent to GPay's DIRECT vs PAYMENT_GATEWAY

Just like Google Pay, Apple Pay has two distinct flows depending on who decrypts the token:

| Flow | Who decrypts | How it works |
|---|---|---|
| **Decrypt at Hyperswitch** (pre-decrypt) | HS Router | HS router decrypts the Apple Pay token using our Payment Processing Certificate (PPC), then sends raw DPAN (card number, expiry, cryptogram, ECI) directly to the connector. Equivalent to GPay **DIRECT** flow. |
| **Decrypt at Connector** | Connector | HS router passes the encrypted token through as-is. The connector (e.g. Stripe, Adyen) decrypts it on their end using their own Apple Pay setup. Equivalent to GPay **PAYMENT_GATEWAY** flow. |

TrustPayments (PR #1046) uses the **Decrypt at Hyperswitch** flow — same pattern as the JPMorgan GPay PR we shipped.

---

## How we are testing

Since browser testing isn't viable, we test in two layers:

**Layer 1 — gRPC directly to UCS**

Skip the HS router entirely. Send decrypted DPAN data (card number, expiry, cryptogram, ECI) directly to UCS via `grpcurl`. This validates the UCS transformer and connector integration in isolation. We ran 4 scenarios (Visa/MC/Amex across GBP/USD/EUR) — all `CHARGED ✅`.

**Layer 2 — cURL to HS Router (full stack)**

Send a real encrypted Apple Pay token (EC_v1 format, base64-encoded) to `POST /payments` on the HS router. The router decrypts it using our PPC cert, routes to UCS, UCS hits TrustPayments. This validates the full decrypt-passthrough pipeline end to end. Result: `status: succeeded`, `requesttypedescription: AUTH`, `walletsource: APPLEPAY` ✅.

For Layer 2 we need a fresh encrypted token from a dev periodically (tokens expire). The token must be encrypted with our PPC cert (`merchant.com.stripe.sang`).
