# Braintree GSM row set — smart retry enablement

## Why this is not a PR

"Smart retry enablement with GSM error code update" is **runtime data, not code.**
GSM rows live in the Postgres `gateway_status_map` table. The six migrations under
`hyperswitch/migrations/*gsm*` create and extend the *table*; they seed no rows, and
**no connector's rows are checked into the HS repo** — not Stripe's, not anyone's.
Rows are created through the admin API (`crates/router/src/routes/gsm.rs`, `POST /gsm`).

So the deliverable is the row set plus an applier, not a diff.

## Files

- `braintree_gsm_rows.json` — 316 rows: 79 Braintree processor response codes ×
  4 payment sub-flows (`Authorize`, `Capture`, `Void`, `PSync`).
- `braintree_gsm_apply.sh` — POSTs them; re-runnable.

## How the keys were derived (not guessed)

`get_gsm_record` (`crates/router/src/core/payments/helpers.rs:8150`) →
`find_gsm_rule(connector, flow, sub_flow, code, message)`. From
`retry.rs:321` the flow is `consts::PAYMENT_FLOW_STR` = **`"Payment"`** and the
sub_flow is the router flow name. The lookup matches on **both code and message**,
so the message text must be byte-identical to what UCS emits.

The code/message pairs were confirmed against the **live sandbox**, not just the docs.
Braintree's amount-triggered declines produce:

| amount | UCS `code` | UCS `message` | Braintree's own decline_type |
|---|---|---|---|
| 2000.00 | `2000` | `Do Not Honor` | SOFT |
| 2001.00 | `2001` | `Insufficient Funds` | SOFT |
| 2004.00 | `2004` | `Expired Card` | HARD |
| 3000.00 | `3000` | `Processor Network Unavailable - Try Again` | (Failed) |

**The docs render en-dashes; the wire uses ASCII hyphens.** Every `–` in the
reference table was normalised to `-` here. Getting this wrong makes the row never
match — the lookup would silently miss and the payment would not retry.

## Classification

`decision` follows Braintree's own soft/hard classification (which it also reports
inline as `decline_type=SOFT|HARD`): soft → `retry`, hard → `do_default`.
27 codes retry, 52 do not.

`error_category` is chosen deliberately, because
`ErrorCategory::should_perform_elimination_routing`
(`common_enums/src/enums.rs:10939`) returns true **only** for `ProcessorDowntime`
and `ProcessorDeclineUnauthorized`. So the category decides whether HS routes away
from Braintree entirely, not just whether it retries:

- `PROCESSOR_DOWNTIME` — `3000` only. Braintree is unreachable; try another connector.
- `PROCESSOR_DECLINE_UNAUTHORIZED` — `2025`, `2026`, `2040`, `2042`, `2080`. Merchant
  or credential misconfiguration; retrying on Braintree will keep failing.
- `ISSUE_WITH_PAYMENT_METHOD` — expired/invalid/closed/lost cards. A different
  connector will not help; do not eliminate.
- `PROCESSOR_DECLINE_INCORRECT_DATA` — AVS/CVV/amount/currency/tax data problems.
- `SOFT_DECLINE` / `HARD_DECLINE` — everything else.

`step_up_possible` is set for `2078`, `2099`, `2101` — the codes a 3DS step-up can
actually rescue. `clear_pan_possible` for `2010`, `2059`, `2060`.

## The UCS dependency is already satisfied

GSM's issuer-code lookup path builds `Network:{card_network}|IssuerCode:{code}` from
`ErrorResponse.network_decline_code`. The Authorize run wired
`network_advice_code` / `network_decline_code` / `network_error_message` from
Braintree's processor response, and a live decline confirms they arrive populated:

```json
"issuerDetails": { "networkDetails": {
  "adviceCode": "01", "declineCode": "XX",
  "errorMessage": "sample network response text" } }
```

Hyperswitch's own Braintree hardcodes all three to `None` at every error site, so
this data has no value on the direct (non-UCS) path — it only works because the
payment is routed through UCS.

## Caveats

1. Codes `2063`, `2066`, `2068`–`2077`, `2079`, `2081`–`2098`, `2100` are PayPal- and
   Venmo-specific and are **excluded** — this connector's card flows cannot produce them.
2. `2109`–`2999` is documented as a single "Processor Declined" range. It is not
   expanded here; add a catch-all row if your GSM supports range or prefix matching.
3. Approval codes `1000`–`1005` need no rows.
4. The message text must track Braintree's wire text. If Braintree rewords a response,
   the row stops matching and the code silently falls back to no-retry.
