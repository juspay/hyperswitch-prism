# Braintree — recurring reviewer issues (mined from the last 20 merged PRs)

Source: inline review comments on juspay/hyperswitch-prism PRs
2251, 2242, 2227, 2223, 2221, 2217, 2212, 2211, 2187, 2183, 2125, 2124, 2087,
2073, 2050, 2013, 1986, 1980, 1952, 1935 (111 reviewer comments).
Every item below was raised on **more than one** PR, or was raised as blocking.
Treat this as a hard checklist for all Braintree work on `feat/braintree_grace`.

## A. Types on the wire

1. **Currency is `common_enums::Currency`, never `String`.**
   Raised 6× on PR2223/2221/2212 by deepanshu-iiitu.
2. **Amounts use an amount type** — `StringMajorUnit` / `FloatMajorUnit` / `MinorUnit`,
   never `String` or `f64`, and the *same* type as every other amount field in the file.
   PR2221 (3×), PR2212, PR2187 ("amount should not be string").
3. **PII and credentials are `Secret<…>`** — tokens, keys, card holder name, email,
   any merchant identifier that is not public. PR2183 (3×), PR1986, PR2212.
4. **Fixed-value string fields become enums** (`payment_type`, `document_type`, status
   strings). PR2221, PR2212, PR2183.

## B. Status mapping — the single largest cluster

5. **Never hardcode `AttemptStatus::Failure` in `build_error_response`.**
   `ConnectorCommon::build_error_response` is flow-agnostic: Refund, RSync, Capture and
   Authenticate all route through it. `ForeignFrom<FlowStatus> for RefundStatus` maps
   `Payment(_)` → `RefundFailure`, so a transport error on a refund reports terminal
   failure. Blocking on PR2221 (2×), PR2223, PR2183.
6. **Unknown/unrecognised connector status → `Unspecified`**, not an invented `Pending`
   or `Failure`. The HS caller applies the previous-status fallback; UCS has no previous
   status. PR2227 (4 comments, gopikrishna000 + AmitsinghTanwar007).
7. **A terminal connector state must map to a terminal UCS state.** `CANCELED`,
   hard-declined, explicitly-refused → `Failure`/`RefundFailure`, never `Pending`, or the
   attempt polls forever. PR2183, PR2221 (2×).
8. **Do not map a connector state the pipeline cannot advance.** `Authorized` when
   Capture is unimplemented strands the payment. PR2221 (2×).
9. **Partial capture reports `PartialCharged`, not `Charged`.** PR2183.
10. **A 200 carrying a failure body must become an `ErrorResponse`** that carries
    `failure_code` / `failure_message` — not a silent `Ok` that drops the reason. PR2223.
11. **Refund error paths must set `attempt_status`.** The refund error builder reads only
    `e.attempt_status` and has no `resource_common_data.status` fallback, so leaving it
    `None` makes every refund error report `REFUND_STATUS_UNSPECIFIED`. PR2223, PR2183.

## C. Identifiers and idempotency

12. **Authorize and PSync must return the same resource id.** Returning the order id from
    one and a connector-side identifier from the other makes one payment report two
    reference ids. PR2221.
13. **Use `resource_common_data.get_merchant_request_id()` for idempotency keys**, never a
    freshly minted UUID per call — a retry after a client timeout then authorises twice.
    PR2183 (blocking).
14. **Prefer the per-request field over the connector-config copy.** `PaymentsAuthorizeData`
    already carries `merchant_config_currency`; read it first and fall back to config.
    PR2187 raised this twice and cited **`braintree/transformers.rs:437`** as the correct
    reference implementation — so Braintree must not regress it.

## D. Reuse and structure

15. **Reuse existing helpers**: `is_auto_capture()` / `is_manual_capture()`,
    `is_setup_mandate()`, `is_refund_failure()`, the connector `utils.rs` helpers. Do not
    hand-roll an equivalent. PR2221 (2×), PR2050 (2×), PR1986.
16. **Generic logic belongs in `utils.rs`**, not in a connector file. PR2212, PR2050.
17. **Do not fall back to `billing_full_name` for card holder name** — make the field
    explicit. PR2223, PR2221.
18. **Fill in `IntegrationErrorContext`** — `::default()` collapses distinct causes into one
    opaque `InvalidDataFormat` and drops `suggested_action` / `doc_url`. PR2187, PR2183.
19. **Comment non-obvious logic.** PR2223, PR2221.

## E. Evidence and hygiene

20. **Novel local logic needs a `#[cfg(test)]` test** — signature preimages, hashing,
    field ordering, status maps. A mock-server run in a PR description is not reproducible
    in CI. Blocking on PR2221 and PR2050.
21. **Never guess a production hostname** in `config/production.toml`. PR2187.
22. **No unrelated regenerated files riding along** in the PR diff. PR2187, PR2013.
