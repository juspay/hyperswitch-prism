# KillBill × Hyperswitch Prism — API test harness

The server-to-server analog of the Medusa demo app (there's no browser: KillBill is a billing backend, so the
flow is driven entirely through KillBill's REST API). It exercises the `killbill-hyperswitch` plugin end to end:

```
create/reuse tenant → upload plugin config → create account → add payment method (card)
  → purchase → get → refund → get → replay a webhook
```

## Prerequisites

1. A **running KillBill** with the plugin installed and started (default `http://127.0.0.1:8080`, RBAC
   `admin:password`). Quickest: the official `killbill/killbill` Docker image (or Kaui + KillBill compose),
   then install the built bundle with `kpm install_java_plugin killbill-hyperswitch --from-source-file=…`.
2. `curl` and `jq` on your PATH.
3. A **real connector sandbox key** — every payment call actually reaches the connector (e.g. Stripe test key
   `sk_test_…`). Use the connector's own test card.

## Run

```bash
chmod +x run.sh

# Stripe (default)
STRIPE_API_KEY=sk_test_xxxxx ./run.sh

# Any other parity connector — supply a full properties file (see ../src/main/resources/hyperswitch.properties):
CONNECTOR=adyen PLUGIN_CONFIG_FILE=./adyen.properties CARD_NUMBER=4111111111111111 ./run.sh
```

The script prints each step, the transaction status, and the captured ids. It fails fast if KillBill is
unreachable or a step errors.

## Run for EVERY connector (parity sweep)

```bash
# 1. create configs/<connector>.properties for each connector to test (contents = the plugin config; keys are
#    in ../src/main/resources/hyperswitch.properties). e.g. for Stripe:
cat > configs/stripe.properties <<'PROPS'
org.killbill.billing.plugin.hyperswitch.connector=stripe
org.killbill.billing.plugin.hyperswitch.environment=SANDBOX
org.killbill.billing.plugin.hyperswitch.stripe.apiKey=sk_test_xxx
PROPS
# 2. run the whole set and get a PASS/FAIL table:
./run-all.sh
```

`run-all.sh` runs the full flow for each of the 6 parity connectors (each in its own tenant `hs-<connector>`),
captures per-connector output in `logs/<connector>.log`, and prints a summary. Connectors without a
`configs/<name>.properties` are skipped. **PayPal is skipped by design** — it's a redirect/wallet connector, so
the raw-card server-to-server flow doesn't apply (it needs the 3DS/redirect flow, deferred in v1). Real
credentials in `configs/*.properties` are git-ignored.

Example summary:

```
================ CONNECTOR SUMMARY ================
  stripe       PASS — purchase SUCCESS
  adyen        PASS — purchase SUCCESS
  braintree    PASS — purchase SUCCESS
  cybersource  PASS — purchase SUCCESS
  forte        RAN — no SUCCESS status; inspect logs/forte.log
  paypal       SKIP — redirect connector; raw-card flow N/A
```

### Useful env vars (all optional)

| var | default | purpose |
|---|---|---|
| `KB_URL` | `http://127.0.0.1:8080` | KillBill base URL |
| `KB_USER` / `KB_PASSWORD` | `admin` / `password` | RBAC credentials |
| `KB_TENANT_KEY` / `KB_TENANT_SECRET` | `hyperswitch-test` / `…-secret` | tenant created/reused |
| `CONNECTOR` | `stripe` | connector to route to (parity set: stripe, adyen, braintree, cybersource, paypal, forte) |
| `STRIPE_API_KEY` | — | Stripe sandbox key (default-connector shortcut) |
| `PLUGIN_CONFIG_FILE` | — | full `.properties` body for non-Stripe connectors |
| `AMOUNT` / `CURRENCY` | `10.00` / `USD` | amount charged/refunded |
| `CARD_*` | Stripe test card | card number/exp/cvc |
| `WEBHOOK_FILE` | `webhooks/stripe-payment_intent-succeeded.json` | body posted in step 9 |

## Files

- **`run.sh`** — the runnable end-to-end driver (primary).
- **`killbill-flow.http`** — the same flow as individual requests for the VS Code REST Client / IntelliJ HTTP
  Client (interactive, step-by-step). Import it into Postman if you prefer.
- **`webhooks/stripe-payment_intent-succeeded.json`** — sample connector event replayed in step 9.

## Notes

- **Webhook step:** replaying a raw event **won't pass the connector's signature verification**, so the plugin
  correctly returns `{"status":"rejected"}` (source not verified). To exercise a real webhook, use the
  connector dashboard's "send test event" against `…/plugins/killbill-hyperswitch/webhook` and set
  `…hyperswitch.webhookSecret` in the plugin config. The endpoint reachability + tenant resolution are still
  validated by the replay.
- **Auth/capture/void:** the default flow uses `PURCHASE` (sale). To test auth-only, POST an `AUTHORIZE`
  payment, then `POST /1.0/kb/payments/{id}` to capture or `DELETE /1.0/kb/payments/{id}` to void — see the
  optional requests in `killbill-flow.http`.
- **Recurring/MIT:** after the card is added (a mandate is set up), a later `PURCHASE` with **no** card
  properties charges the stored mandate off-session (Phase B) — drive it by calling step 5 again without card
  data, or via a KillBill subscription invoice.
- This harness assumes the plugin is **built, installed, and its P0 native-lib/OSGi load verified** first (see
  the top-level plugin README). It does not compile or deploy anything.
