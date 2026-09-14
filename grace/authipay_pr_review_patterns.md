# Recurring PR-Review Findings — last 20 merged PRs (`shuklatushar226`)

**Repo:** `juspay/hyperswitch-prism` · **Generated:** 2026-09-09
**Corpus:** 314 comments on 20 merged PRs; 176 are from human reviewers (author self-replies and
`github-actions[bot]` / `chatgpt-codex-connector[bot]` excluded).

**Reviewers:** `JeevaRamu0104` (80), `deepanshu-iiitu` (28), `AmitsinghTanwar007` (17),
`iemyashasvi` (16), `XyneSpaces` (15, automated reviewer bot with real substance),
`gopikrishna000` (13), `swangi-kumari` (6), `jarnura` (1).

**PRs analysed** (newest merge first):
`2251, 2223, 2242, 2212, 2221, 2211, 2227, 2187, 2217, 2183, 2050, 2125, 2124, 2087, 2073, 2013, 1910, 1980, 1812, 1986`
(PRs 2242, 2211, 2217, 2073, 2251 drew no reviewer comments.)

> Note on authorship: no GitHub account or git-log identity resolves for `george.p@juspay.in`.
> The account authenticated in this environment — and the one that owns the Authipay work
> (PR #587 *Authipay incremental auth impl*) — is `shuklatushar226`. That account's 20 most
> recently merged PRs are the corpus.

---

## Recurring issues

### 1. Reinventing logic that already exists in `utils.rs` / common helpers
**Rule:** Before writing a helper in `<connector>/transformers.rs`, grep
`crates/integrations/connector-integration/src/utils.rs`, `crates/common/common_utils/`, and
`domain_types` for it. If it is generic, put it in the shared util, not the connector file.

**8 distinct PRs:** #1812, #1910, #1980, #1986, #2050, #2187, #2212, #2221 — the single most
repeated class of comment.

- #2221 `utils.rs:558` — @AmitsinghTanwar007: *"you can remove this instead just use is manual
  capture"*; `utils.rs:583` *"here also is setup mandate function can be used"*.
- #2212 `d24/transformers.rs:261` — @deepanshu-iiitu: *"Can we move this function to validate
  chilean rut to connector utils file?"*; `:390`,`:399` *"Can we use/create a util for this?"*
- #2050 `domain_types/src/connector_response_masking.rs:407` — @AmitsinghTanwar007: *"there is a
  existing function for this functionality of removing prefixes i think we can use it"*.
- #1910 `domain_types/src/utils.rs:669` — @AmitsinghTanwar007: *"check this once i think we have
  some padding function for date"*.
- #1986 `paysafe/transformers.rs:242` — @deepanshu-iiitu: *"Use utls properly here"*.
- #1812 `airwallex/transformers.rs:629` — @deepanshu-iiitu: *"Can we move this logic to individual
  utils which can be reused across payment methods within airwallex?"*
- #2187 — @AmitsinghTanwar007 asked for the two-decimal wire-amount conversion to move to a common
  amount type with reusable `validate_unsigned(field_name)` / `validate_max_len(max_len, field_name)`
  helpers rather than being hand-rolled per connector.
- #1980 — @gopikrishna000: the fail-closed authorization block was copy-pasted into three release
  workflows; asked for one reusable `workflow_call` workflow so security fixes can't drift.

```rust
// anti-pattern — hand-rolled in <connector>/transformers.rs
let is_auto = matches!(req.capture_method, Some(CaptureMethod::Automatic) | None);
fn pad2(m: &str) -> String { format!("{:0>2}", m) }

// correct
use crate::utils::{is_manual_capture, is_setup_mandate_flow};
let expiry_month = card.get_card_expiry_month_2_digit()?;
```

---

### 2. Untested code / no live-sandbox proof
**Rule:** Ship `connector_specs/<connector>/specs.json` **and** `override.json`; run at least one
real scenario against the sandbox; add `#[cfg(test)]` for any pure logic (signature preimages,
untagged-enum round-trips, amount/status/error mapping). A grpcurl transcript in the description is
not evidence unless it was captured against the code as merged.

**8 distinct PRs:** #1812, #1910, #2050, #2124, #2183, #2187, #2212, #2221

- #2124 — @gopikrishna000: *"Missing `override.json`. The base `PaymentService/Authorize` scenario is
  a generic, connector-agnostic fixture… the harness has nothing connector-real to send."* and
  *"Have you personally run `./scripts/run-tests --connector citigate --suite PaymentService/Authorize
  --scenario <name> --interface grpc` and gotten a PASS? … `specs.json` and unit tests never leave the
  process, so they can't catch [a wrong field mapping]."* Also asked for `browser_automation_spec.json`
  for the 3DS-redirect leg.
- #2221 `paynearme/transformers.rs:141` — @JeevaRamu0104: *"Nothing tests this. It's the only novel
  logic in the PR … the sort order, the exempt-field skip and the null skip stay unverified."*
- #2050 — @JeevaRamu0104: *"b4431bc removed the unit tests and there's no `#[cfg(test)]` block, so
  `Run Tests` is green on 436 lines nothing exercises."* — then listed the exact required cases
  (keyless top-level JSON, arrays under allowed/denied keys, `text/plain`, XML comments/DOCTYPE,
  BOM-prefixed form bodies).
- #2183 `saferpay.rs:330` — the grpcurl block *predated* the flow it claimed to prove:
  *"`Initialize` here and the settle `Authorize` have no live verification … Can you re-run the 3DS
  legs against the current code?"*
- #2187, #1910, #2212, #1812 — same shape: mock-server or manual-only evidence, no spec, no CI gate.

---

### 3. Status mapping that strands or lies about money
**Rule:** Never hardcode a terminal status in a shared error builder. `build_error_response` is
flow-agnostic — pass `attempt_status: None` and let each flow's own transformer decide, *except*
on the refund path where `types.rs:9600` has no fallback and you must set
`FlowStatus::Refund(RefundStatus::Failure)` explicitly. Every connector status must map to a state
the caller can advance out of.

**5 distinct PRs:** #2183, #2187, #2221, #2223, #2227

- #2221 `paynearme.rs:227` — @JeevaRamu0104: *"Every non-2xx on every flow gets
  `Payment(AttemptStatus::Failure)` here … so a 400 from clock skew, a rate limit, or a PayNearMe 5xx
  reports a charged payment as FAILURE."* And: `get_error_response_v2` routes Refund/RSync through it
  too, and `ForeignFrom<FlowStatus> for RefundStatus` maps `Payment(_) -> RefundFailure`, so
  *"A merchant told the refund failed may re-issue it."*
- #2223 `paydotcom/transformers.rs:1336` — *"Hardcoding `Failure` discards the status the caller just
  computed: Capture derives `CaptureFailed`, Authenticate derives `AuthenticationFailed` … the
  merchant sees a failed payment while the hold is still intact and capturable."*
- #2223 `:1787` — the mirror-image bug: `attempt_status = None` on the refund path makes every refund
  error report `REFUND_STATUS_UNSPECIFIED`.
- #2183 `:1120` — a capture Saferpay reports `CANCELED` (terminal) mapped to `Pending`: *"the attempt
  polls forever."* `:763` — `to_refund_error_response` is dead code because `ConnectorCommon::
  build_error_response` is flow-agnostic, so *"A hard-declined refund stays `Pending` and keeps
  getting retried."*
- #2187 `:1251` — `procStatus != "0"` means the *inquiry* didn't complete, not that the payment
  failed: *"this stamps `Failure` on exactly the charged-but-response-lost payment PSync exists to recover."*
- #2221 `:1475` — an explicit refund decline and "accepted but not visible yet" collapse into the same
  `Pending` arm, so the refund *"stays Pending forever and the merchant waits on money that never moves."*
- #2227 — @gopikrishna000: UCS has no previous-status concept; on an unknown connector status emit
  `UNSPECIFIED` and let Hyperswitch apply previous-status handling — don't invent `Pending`.

```rust
// anti-pattern — in ConnectorCommon::build_error_response
attempt_status: Some(FlowStatus::Payment(AttemptStatus::Failure)),

// correct — flow-agnostic builder stays neutral (cf. ilixium.rs:190)
attempt_status: None,
// ...and the refund transformer sets its own, because types.rs:9600 has no fallback
error_response.attempt_status = Some(FlowStatus::Refund(RefundStatus::Failure));
```

---

### 4. Errors with no diagnostics — `IntegrationErrorContext::default()`, dropped codes, wrong `field_name`
**Rule:** Fill `IntegrationErrorContext` (`suggested_action`, `doc_url`, `additional_context`) instead
of `::default()`. Propagate the connector's machine-readable `code`/`reason` on the path that
actually fires. `field_name` must be the **caller-facing request path**, never the connector's
internal JSON key. Use `attach_printable` on every fallible conversion.

**6 distinct PRs:** #1812, #1910, #2183, #2187, #2221, #2223

- #1812 `airwallex/transformers.rs:138` — @JeevaRamu0104 [S1]: renaming `field_name` from the request
  path to the connector's own key broke the field-probe solver *and* the merchant contract:
  *"you get `Missing required field: shopper_name`, which is not a field in the request contract at
  all; it exists only inside Airwallex's JSON body."* It regressed Blik and Trustly from `supported`
  to `error` in `data/field_probe/airwallex.json`.

  ```rust
  // anti-pattern
  field_name: "shopper_name",
  field_name: "country_code",
  // correct — machine-readable request path; put the prose in additional_context
  field_name: "billing.address.first_name",
  field_name: "billing.address.country",
  field_name: "payment_method_data.wallet.google_pay.tokenization_data",
  ```
- #1910 `tesouro/transformers.rs:1219` — in-band GraphQL errors always report `NO_ERROR_CODE` because
  `TesouroApiErrorData` deserializes only `message`; the `extensions.code`/`reason` parser only fires
  on non-2xx, *"and GraphQL APIs conventionally return errors with HTTP 200"*.
- #2187 `:782` — `IntegrationErrorContext::default()` *"collapses three causes — negative amount, over
  12 characters, and a 3-decimal currency … into one opaque `InvalidDataFormat`."*
- #2223 `:1670` — a 200 carrying `status: "failed"` returns `Ok` and drops the parsed
  `failure_code`/`failure_message`: *"a merchant gets a failed refund with no reason."*
- #2221 `:994` — *"a 401 signature rejection or a PayNearMe 5xx surfaces to the merchant as
  'Payment declined by Paynearme'."*
- #1812 `:942` / #2183 `:92` / #1910 `:296`,`:393` — @iemyashasvi: *"add more details in context"*,
  *"also use attach_printable"*; a `get_unimplemented_payment_method_error_message("airwallex")`
  reused for an unsupported *bank* points at the wrong thing entirely.

---

### 5. Unreachable guards and dead branches
**Rule:** After adding a guard or a match arm, prove it can actually fire. Check that the field it
reads is populated on that request path, and that no earlier arm shadows it.

**6 distinct PRs:** #1812, #1910, #2124, #2183, #2187, #2212

- #2124 `citigate/transformers.rs:987` — `minor_amount_authorized` is `None` at *every* construction
  site in `types.rs` (incl. Capture at `types.rs:11072`), so the partial-capture guard never fires:
  *"A partial capture then serializes with no `Amount`, Citigate settles the full authorisation, and
  it maps to `Charged`."* Reviewer supplied the fail-closed pattern from `forte/transformers.rs:647`:

  ```rust
  let authorized = router_data.resource_common_data.minor_amount_authorized
      .ok_or_else(|| error_stack::report!(IntegrationError::MissingRequiredField {
          field_name: "minor_amount_authorized",
          context: IntegrationErrorContext::default(),
      }))?;
  if authorized != request.minor_amount_to_capture {
      return Err(not_supported("Partial capture".to_string()));
  }
  ```
- #2212 `:412` — a RUT length check runs on *every* document type, so the `Cpf`/`Cnpj`/`Psn` arms are
  unreachable: *"A Chilean payer with a passport gets `document_type: PSN` and is then rejected here."*
- #2183 `:479` — `RefundSyncData::refund_status` is hardcoded `Pending`, so `PATH_CAPTURE` always wins
  and `Inquire` is unreachable; every later sync re-POSTs Capture and 402s.
- #2187 `:543` — `trans_type_for` rejects `CaptureMethod::Manual`, making `success_status`'s non-`AC`
  branch unreachable — and contradicting the PR description's own captured run.
- #1910 `:274` — `is_auto_capture` and `is_auto_capture_request` encode the same match with different
  answers: *"Unreachable today … but the pair will drift."*
- #1812 `:543` — stale comment claiming PayLater is unimplemented, directly above the Klarna/Atome impl.

---

### 6. Capture-method classification
**Rule:** `SequentialAutomatic` groups with `Automatic`, always. If `Capture`/`Void` are
`not_implemented`, reject manual capture methods in the request transformer, and don't emit a status
that tells the caller to capture.

**4 distinct PRs:** #1812, #1910, #2124, #2221

- #1910 `:274` [S1] — @JeevaRamu0104: `SequentialAutomatic` fell to the manual branch and emitted
  `automaticCapture: NEVER` + `authorizationIntent: PRE_AUTHORIZATION`. *"Every other connector in this
  repo groups it with `Automatic`"* — citing `nexinets.rs:326`, `noon.rs:264`,
  `worldpay/transformers.rs:388`, `authorizedotnet/transformers.rs:324`, `fiuu/transformers.rs:92`,
  `nexixpay/transformers.rs:409`.

  ```rust
  // anti-pattern
  matches!(capture_method, Some(CaptureMethod::Automatic) | None)
  // correct
  matches!(capture_method,
      Some(CaptureMethod::Automatic) | Some(CaptureMethod::SequentialAutomatic) | None)
  ```
- #1910 `tesouro.rs:407` [S1] — manual capture accepted while `Capture` and `Void` are
  `not_implemented`: *"funds held on the cardholder's card with no settle path and no release path."*
  Precedent given: `xendit/transformers.rs:262-268` returns
  `IntegrationError::CaptureMethodNotSupported`.
- #2221 `:771` — `authorized -> AttemptStatus::Authorized` with Capture `not_implemented` and
  `supported_capture_methods = [Automatic]`: *"A payment landing here can't be advanced."*
- #1812 — the `None | SequentialAutomatic => auto-capture` fix was correct but shipped undeclared;
  reviewer asked for a release note because it silently flips existing callers.

---

### 7. Unmasked secrets and PII
**Rule:** Any token, credential, PAN-adjacent value, cardholder name, email, phone, or address field
in a request/response struct is `Secret<String>` / `masking::Secret`. Reviewers ask this on
essentially every connector PR.

**4 distinct PRs:** #1986, #2183, #2212, #2223

- #2183 `saferpay/transformers.rs:250`,`:252` — @swangi-kumari: *"this should be secret"* (twice);
  `:315` *"What is this token used for? Should this be treated as a secret/sensitive value?"*
- #1986 `paysafe/requests.rs:344` — @deepanshu-iiitu: *"Lets make this a secret"*.
- #2223 `paydotcom/transformers.rs:245` — @deepanshu-iiitu: *"Should this field be a secret?"*
- #2212 `d24/transformers.rs:630` — @deepanshu-iiitu: *"Make PII fields secret and use amount structs
  for amount related fields"*.
- Related, #2050: the whole `masked_connector_response` PR turned on multiple leak paths flagged by
  @JeevaRamu0104 (keyless top-level JSON, arrays under an allowlisted key, XML comments/DOCTYPE,
  anything falling through to `Format::Form`) — the invariant reviewers apply is *default to masked*.

---

### 8. Stringly-typed amounts and currencies
**Rule:** Amounts use the repo's amount wrappers (`StringMajorUnit`, `FloatMajorUnit`, `MinorUnit`) —
never `String`/`f64`/`i64`. Currency uses `common_enums::Currency`, never `String`. Be consistent
within a single file.

**4 distinct PRs:** #2187, #2212, #2221, #2223

- #2221 — @deepanshu-iiitu, 3× *"Can we use StringMajorUnit for … amount field?"* (`:705`, `:722`,
  `:725`) and 4× *"Can we use currency enum here?"* (`:234`, `:318`, `:577`, `:705`).
- #2223 — @deepanshu-iiitu, 5× *"Can we use currency enum here?"* (`:141`, `:1109`, `:1130`, `:1589`).
- #2212 `:918` — *"use floatmajorunit for amount and currency enum for currency field"*;
  @JeevaRamu0104 `:1043` *"nit: every other amount field in this file is `FloatMajorUnit`."*
- #2187 `:409` — @swangi-kumari: *"amount should not be string."*

---

### 9. Magic strings instead of enums
**Rule:** Any closed set of connector-defined tokens (document types, payment types, status strings,
transaction types) gets a `#[derive(Serialize, Deserialize)]` enum with `rename_all`, not a `String`.

**4 distinct PRs:** #1910, #2183, #2212, #2221

- #2212 `:211` — @deepanshu-iiitu: *"Can we make document_type an enum?"*
- #2221 `:727` — *"Can we create an enum for payment_type field?"*
- #2183 `:47` — @AmitsinghTanwar007: *"can't we have a enum for this"*
- #1910 `:428` — @iemyashasvi: *"avoid magic strings"*

---

### 10. Unexplained or unnecessary fields, impls and comments
**Rule:** If a reviewer would ask "why is this here?", either delete it or comment it. Every
non-obvious transform gets a doc comment; stale comments get deleted, not maintained.

**5 distinct PRs for "why is this needed":** #1910, #1986, #2013, #2187, #2212
**4 distinct PRs for "add/fix the comment":** #1812, #1910, #2221, #2223

- @swangi-kumari #2187 `:196`,`:218`: *"why is this required?"* ×2.
- @deepanshu-iiitu #2212 `:1018`: *"Why is this impl needed?"*; #1986 `:135`: *"Why do we need this?"*
- @AmitsinghTanwar007 #2013: *"if not necessary you can remove this as we dont have any pipeline or
  workflow to test this"*.
- @iemyashasvi #1910 `:58`: *"what are these ?"*
- @deepanshu-iiitu #2221 `:956`: *"Please add a detailed comment about the last_payment fn?"*;
  #2223 `:1393`: *"Can we refactor this line and add an explanation on whats happening here"*.
- @JeevaRamu0104 #2223 `:604`: the doc comment *contradicted* the guard directly below it;
  #1812 `:543`: stale comment listing PayLater as unimplemented; #1812 `payment_methods.proto`:
  dead commented-out `// Atome` block left behind after the real message landed.

---

### 11. Silent fallbacks that swallow errors or fabricate data
**Rule:** Don't `.ok()` a fallible conversion. Don't substitute "now" or a synthetic value for
missing data. Fail locally with a named field rather than sending a request you know is wrong.

**3 distinct PRs:** #1910, #2050, #2212

- #1910 `:1125` — *"`.ok()` swallows the month-derivation error … stores a mandate without expiry and
  defers the failure to a later MIT rejected by Tesouro's schema — far from the actual cause."*
- #1910 `:935` — `originalPurchaseDate` falls back to `now().date()`: *"That is inaccurate
  stored-credential data submitted to the network."* @iemyashasvi, same line: *"why are we falling
  back to current, if this is needed and critical throw err"*.
- #2212 `:438` — `router_return_url` optional, so all three of `success_url`/`back_url`/`error_url`
  are dropped: *"WebPay is a mandatory redirect — the customer then has nowhere to return.
  `request.get_router_return_url()?` fails locally instead."*
- #2212 `d24.rs:350` — a non-numeric `connector_transaction_id` *"goes into the URL raw, so … 404s on
  every poll instead of failing locally."*
- #1910 `:1212` — mandate metadata built only inside `activity_date.map(...)`, so the credential
  expiry is silently dropped whenever `activityDate` is absent. *"Build the metadata when **any** of
  the three values is present."*

---

### 12. Sync flows that can't find their own payment
**Rule:** PSync/RSync must key off something persisted by the create call
(`connector_metadata` / `encoded_data` / `refund_connector_metadata`), never a value re-derived from
caller-supplied input. And PSync must return the *same* `resource_id` Authorize returned.

**3 distinct PRs:** #2183, #2187, #2221

- #2187 `:945` — *"`inquiryRetryNumber` is re-derived from `merchant_transaction_id`, so PSync only
  finds the payment if the sync caller resends the identical value. A different one makes `/inquiry`
  return no record, which maps to `Failure` — a charged payment reported as failed, on the exact path
  meant to prevent a double charge. Authorize already persists `retry_trace` into
  `connector_metadata`; can PSync read it back off `encoded_data`?"*
- #2221 `:1289` — *"PSync returns `site_payment_identifier` here while Authorize returns the order id,
  so the same payment reports two different reference ids."*
- #2183 `:479` — asked to key the refund-sync stage off a marker in `refund_connector_metadata`,
  *"the way the 3DS token round-trips"*.

---

### 13. serde hygiene
**Rule:** `#[serde(skip_serializing_if = "Option::is_none")]` on every optional wire field — a `None`
that serializes as `null` is not an omitted field. Closed enums deserialized from a connector need
`#[serde(other)] Unknown`. Prefer an explicit tag over `#[serde(untagged)]`. Delete `alias`
attributes that `rename_all` already produces.

**2 distinct PRs:** #1812, #1910

- #1812 `:302` — *"Parity fix #1 is not achieved at the wire level."* `payment_consent`/`customer_id`
  got the attribute; `payment_method_options` and `device_data` did not, and the committed probe body
  shows `"payment_method_options":null,"device_data":null` still going on the wire.
- #1910 `:1664` — a closed `__typename` enum with no fallback: an unmapped union member falls through
  an `#[serde(untagged)]` response enum and surfaces as *"data did not match any variant of untagged
  enum TesouroApiResponse"*. *"So a perfectly successful payment would sync as a deserialization
  failure with nothing in the message pointing at the cause."*

  ```rust
  #[serde(other)]
  Unknown,   // then map Unknown => previous_status, so it fails soft
  ```
- #1910 `:1266` — dead `alias = "authorizeCustomerInitiatedTransaction"`; `rename_all = "camelCase"`
  already produces that exact name.

---

### 14. Idempotency / request-id stability
**Rule:** Don't mint a fresh UUID per call for a field the connector dedupes on. Use
`resource_common_data.get_merchant_request_id()` with `connector_request_reference_id` as fallback.

**2 distinct PRs:** #2183, #2221

- #2183 `:205` — *"Saferpay dedupes on `RequestId` + `RetryIndicator`, and a fresh UUID per call opts
  out of it — a retried `AuthorizeDirect` after a client timeout authorizes twice."* Precedent:
  `twoc_twop_paco/transformers.rs:889`.
- #2221 `:268` — *"nit: this fallback is unique per order, so every payment without a customer mints a
  fresh PayNearMe customer record."*

---

### 15. Required fields that quietly fall back to a different field
**Rule:** Don't paper over a missing required field with a neighbouring one. Make it required and
fail with a named `field_name`.

**2 distinct PRs:** #2221, #2223

- #2221 `:421` — @deepanshu-iiitu: *"Why are we using cardholdername as backup? Lets try to make
  billing_full_name as mandatory field?"*
- #2223 `:628` — *"Lets make card holder name a required field and lets not fallback to
  billing_full_name"*.

---

### 16. `config/production.toml` base URLs
**Rule:** Never ship a sandbox host or a guessed host in `production.toml`. If the live host is
unconfirmed, ship it commented out — a config-load failure is a better place to find out than a
payment.

**3 distinct PRs:** #1910, #2050, #2187 (the third for unsafe config defaults)

- #1910 — @XyneSpaces: *"`config/production.toml` adds `tesouro.base_url =
  "https://api.sandbox.tesouro.com"` … production traffic will hit Tesouro's sandbox environment."*
- #2187 `config/production.toml:75` — *"This hostname is a guess, so the first real production payment
  routes somewhere nobody has confirmed. Can it ship commented out until JPM Merchant Services
  confirms it?"*
- #2050 `config/production.toml:187` — a new feature defaulted `enabled = true` in production and
  sandbox, *"which makes the bypasses I've flagged … live rather than latent. Can we default it
  `false` until those are closed and covered?"*

---

### 17. Proto enum / field-number collisions between concurrent PRs
**Rule:** Before adding a `ConnectorEnum` value or a `*Config` oneof field, check every other open PR
touching `payment.proto` for the same number.

**2 distinct PRs:** #2221, #2223 (they collided with each other)

- @XyneSpaces on both: *"`payment.proto:912` adds `PAYNEARME = 143`, which collides with
  `PAYDOTCOM = 143` in PR #2223"* and *"`PaynearmeConfig paynearme = 154` … collides with
  `PaydotcomConfig paydotcom = 154`"*. *"Coordinate … before either PR merges."*

---

### 18. Generated artifacts must be regenerated — and must not regress
**Rule:** Run `make docs` and diff `docs-generated/all_connector.md`, `docs-generated/llms.txt`, and
`data/field_probe/<connector>.json`. CI does **not** gate on probe status, so the auto-fix job will
happily commit a regression green.

**2 distinct PRs:** #1812, #2183

- #1812 — @JeevaRamu0104 [S1]: Blik and Trustly went `supported -> error` in the probe file, flipped to
  `?` in `all_connector.md`, and were dropped from the `payment_methods:` line in `llms.txt`.
  *"this PR ships generated docs stating that two working payment methods no longer work … Worth
  flagging that CI does not gate on probe status, so the 'Auto-fix (format + docs + generate)' job
  committed the regression and everything still went green."*
- #2183 — @XyneSpaces: the Saferpay table marked non-card methods `⚠` while the connector returns
  `NotImplemented("Only card payments are supported by saferpay")`. *"Please regenerate the connector
  docs so unsupported methods render as unsupported."*

---

### 19. PR description must match the diff; split out-of-scope core/proto changes
**Rule:** Anything touching `proto/`, `domain_types`, `common/`, or another connector is core-level
and must be either split out or explicitly declared in-scope. Behavioural changes to already-shipped
flows need a line in the description.

**2 distinct PRs:** #1812, #2187

- #1812 — @JeevaRamu0104 [S1 scope]: `PaymentCreateOrderData.order_details` and the `Atome` proto
  ingress were both listed as out-of-scope follow-ups and both shipped. *"I reviewed it and the wiring
  is correct … The problem is purely that a reviewer sizing blast radius from the description will
  treat this PR as connector-local and skip the core review."* Same PR: an undeclared
  `payment_attempt_id -> payment_intent_id` rename on the refund request, and an undeclared
  `auto_capture` default change that *"will silently switch [callers] to auto-capture on the next
  deploy."*
- #2187 — *"The PR description says `retryTrace` is FNV-1a/64 folded; this is SHA-256."* and the
  description's captured run used `capture_method: MANUAL`, which the code now rejects. Also:
  a generator fix regenerated docs for `absasanlam`, `pinelabsonline` and `tsystransit` —
  *"7 files of unrelated output riding along with a new connector. Worth splitting out."*

---

### 20. Style: `match` over `if/else`, `TryFrom` for status mapping, refactor long inline blocks
**Rule:** Status/enum mapping goes in a `TryFrom`/`From` impl, not inline `if/else` chains.

**2 distinct PRs:** #1910, #2223

- @iemyashasvi #1910: *"can we make a tryfrom for status mapping"* (`:1545`), *"can we use match"*
  (`:307`), *"refactor this"* (`:383`), *"nit : can we aoid if else"* (`:673`), *"nit : can we make
  this cleaner"* (`:730`).
- @deepanshu-iiitu #2223 `:1393`: *"Can we refactor this line and add an explanation"*.

---

## PRE-COMMIT CHECKLIST

Check the diff against every line below before opening the PR.

**Reuse**
- [ ] Grepped `connector-integration/src/utils.rs`, `common_utils`, `domain_types` for every helper I wrote; nothing generic lives in `<connector>/transformers.rs`.
- [ ] Used `is_manual_capture` / setup-mandate / expiry-padding / prefix-strip helpers instead of hand-rolling them.
- [ ] Any helper that is not connector-specific was moved to the shared util file in this same PR.

**Types**
- [ ] No amount is `String`, `f64` or bare `i64` — all are `StringMajorUnit` / `FloatMajorUnit` / `MinorUnit`, consistent within the file.
- [ ] No currency is `String` — all are `common_enums::Currency`.
- [ ] Every closed set of connector tokens is an enum, not a magic string.
- [ ] Every token / credential / PAN-adjacent / cardholder-name / email / phone / address field is `Secret<_>`.

**serde**
- [ ] Every `Option` wire field has `#[serde(skip_serializing_if = "Option::is_none")]` — verified against a captured request body, not by reading the struct.
- [ ] Every enum deserialized from the connector has `#[serde(other)] Unknown` (or an explicit fallback arm).
- [ ] No `#[serde(untagged)]` on a response envelope where an explicit tag would work.
- [ ] No `alias` that `rename_all` already produces.

**Status & errors**
- [ ] `ConnectorCommon::build_error_response` passes `attempt_status: None` — no hardcoded `Payment(AttemptStatus::Failure)` (it also routes Refund/RSync and maps to `RefundFailure`).
- [ ] Refund error paths explicitly set `Some(FlowStatus::Refund(RefundStatus::Failure))` — `types.rs:9600` has no fallback.
- [ ] Every terminal connector status maps to a terminal `AttemptStatus`/`RefundStatus` (no `CANCELED -> Pending`).
- [ ] A sync/inquiry that fails to *find* a record maps to `Unresolved`/`Pending`, never `Failure`.
- [ ] An explicit connector decline and "not visible yet" are in different match arms.
- [ ] No `IntegrationErrorContext::default()` on a new error — `suggested_action` / `doc_url` / `additional_context` filled in.
- [ ] `field_name` is the caller-facing request path (`billing.address.first_name`), never the connector's JSON key (`shopper_name`).
- [ ] Connector `code`/`reason` propagated on the path that actually fires (incl. errors returned with HTTP 200).
- [ ] Parsed `failure_code`/`failure_message` are actually read, not dropped.
- [ ] Every fallible conversion has `.attach_printable(...)` — no bare `.ok()`.

**Flow correctness**
- [ ] `SequentialAutomatic` is grouped with `Automatic` in every capture-method predicate.
- [ ] If `Capture`/`Void` are `not_implemented`, manual capture methods are rejected in the request transformer and no status maps to `Authorized`.
- [ ] Only one source of truth for capture-method classification (no near-duplicate predicates).
- [ ] 3DS/auth-type guards exist on **every** flow that builds the same request (Authorize *and* SetupMandate *and* RepeatPayment) — no asymmetry.
- [ ] PSync/RSync key off `connector_metadata` / `encoded_data`, not a value re-derived from caller input.
- [ ] PSync returns the same `resource_id` Authorize returned.
- [ ] Dedupe/idempotency fields use `get_merchant_request_id()` with `connector_request_reference_id` fallback — no per-call UUID.
- [ ] Required redirect URLs fail locally (`get_router_return_url()?`) rather than being silently omitted.
- [ ] No fabricated data (`now()` for an original-purchase date, synthetic customer ids per order).
- [ ] Mandate/credential metadata is built when **any** of its values is present, not gated on one optional field.
- [ ] Every new guard and match arm was traced to a request path where it actually fires.

**Tests & evidence**
- [ ] `crates/internal/integration-tests/src/connector_specs/<connector>/specs.json` present and lists every supported suite.
- [ ] `override.json` present with connector-real test data (the global scenario fixture is connector-agnostic).
- [ ] `browser_automation_spec.json` present if the PR claims a 3DS/redirect flow.
- [ ] At least one `make test-scenario connector=<c> suite=<s> scenario=<n>` PASS against the real sandbox, pasted into the PR.
- [ ] `#[cfg(test)]` covers pure logic: signature preimage, untagged-enum round-trip, amount/status/error mapping.
- [ ] The grpcurl/evidence in the description was captured against the code **as merged**, not an earlier revision.

**Config & proto**
- [ ] `config/production.toml` has a confirmed live host — not a sandbox URL, not a guess (comment it out if unconfirmed).
- [ ] New feature flags default `false` in `production.toml`/`sandbox.toml` until covered.
- [ ] New `ConnectorEnum` value and `*Config` oneof field numbers checked against every other open PR touching `payment.proto`.

**Generated output & description**
- [ ] Ran `make docs`; diffed `docs-generated/all_connector.md`, `docs-generated/llms.txt`, `data/field_probe/<connector>.json` — no method moved from `supported` to `error`, nothing dropped from the methods list. (CI does not gate on this.)
- [ ] No unrelated regenerated files from other connectors riding along.
- [ ] Description matches the diff: every proto / `domain_types` / `common/` / other-connector change is declared in-scope or split out.
- [ ] Behavioural changes to already-shipped flows (field renames, default changes) called out explicitly.
- [ ] Every non-obvious transform has a doc comment; no stale or contradicting comments; no dead commented-out blocks.
