# PayLater — Technical Specification

> Source: PayLater merchant-portal API docs (verified via WebFetch on 2026-09-02).
> PayLater is a Qatar-hosted BNPL gateway. All payments are initiated from a server-side
> "generate payment link" call (hosted web checkout); the shopper completes the
> installment plan on PayLater's hosted page and is sent back to the merchant's
> success/fail redirect URL. There is no separate Capture/Void — the payment is
> auto-settled by the hosted checkout and refund is synchronous-accepted.

## Connector identity

| Field | Value |
|---|---|
| Connector name | `paylater` |
| Display name | PayLater |
| Country focus | Qatar |
| Currency in scope | `QAR` **only** |
| Base URL — Sandbox | `https://connect.uat.paylaterapp.com` |
| Base URL — Production | `https://connect.paylaterapp.com` |
| Amount format | JSON **double** (major / base units), range `300.00` – `25000.00` QAR. **Use base unit** — do NOT scale to minor units. |
| Content-Type (API) | `application/json` |
| Content-Type (Auth token) | `application/x-www-form-urlencoded` |
| Auth header (post-token) | `Authorization: Bearer <access_token>` |
| Error envelope | `{ "error": "<string>", "message": "<string?>" }` — `message` only present on some endpoints |

## Payment methods in scope

**PayLater BNPL only** — hosted web-checkout redirect. Shopper is redirected to PayLater's hosted page where they pick instalment plan, authenticate, and complete payment.

Mapped to `PaymentMethodData::PayLater(PayLaterData::PayLaterRedirect {})` — a new
variant must be added to `crates/types-traits/domain_types/src/payment_method_data.rs`
`PayLaterData` enum (alongside `KlarnaRedirect`, `AffirmRedirect`, `AfterpayClearpayRedirect`,
`PayBrightRedirect`, `WalleyRedirect`, `AlmaRedirect`, `TamaraRedirect`, `AtomeRedirect`).

Out of scope for this PR:
- Cards (not a card gateway)
- Wallets, bank transfers, direct debit — none documented
- Tokenization / RepeatPayment / Mandates — not in docs

## Authentication

Two-step OAuth 2.0 `client_credentials` flow. UCS models this as
**ServerAuthenticationToken** — `should_do_access_token = true`.

### Step 1 — Obtain access token

`POST /auth/realms/api/protocol/openid-connect/token`

**Request — `application/x-www-form-urlencoded`:**
```
grant_type=client_credentials
client_id=<client_id>
client_secret=<client_secret>
```

**Response 200:**
```json
{
  "access_token":       "<jwt>",
  "expires_in":         300,
  "token_type":         "Bearer",
  "refresh_token":      "<jwt>",
  "refresh_expires_in": 1800,
  "scope":              "email profile"
}
```

| Field | Type | Notes |
|---|---|---|
| `access_token` | string | Sent as `Authorization: Bearer <access_token>` on every subsequent API call. |
| `expires_in` | int | Lifetime in seconds. Currently **300** (5 min). UCS must cache and refresh pre-expiry. |
| `token_type` | string | Always `Bearer`. |
| `refresh_token` | string | May be used with `grant_type=refresh_token` to extend session; simpler to re-run `client_credentials`. |
| `refresh_expires_in` | int | Refresh-token lifetime in seconds. |
| `scope` | string | Echo of granted scopes. Informational. |

**Error responses:**
- `401` / `400` with `{"error": "invalid_client", "error_description": "..."}` — wrong `client_id` / `client_secret`.
- `400` with `{"error": "unsupported_grant_type"}` — wrong `grant_type` value.

### Step 2 — Use token

Every API call carries:
```
Authorization: Bearer <access_token>
Content-Type: application/json
```

## Connector credentials (`ConnectorSpecificConfig::Paylater`)

| Field | Type | Notes |
|---|---|---|
| `client_id` | `Secret<String>` | OAuth client_id issued by PayLater. |
| `client_secret` | `Secret<String>` | OAuth client_secret issued by PayLater. |
| `outlet_id` | `i64` | Issued to merchant at onboarding. Sandbox value: `1000000061`. Sent as `outlet_id` (long) in the Authorize body. |
| `webhook_secret` | `Option<Secret<String>>` | HMAC-SHA256 key for inbound webhook signature verification. Provisioned out-of-band from PayLater account manager. Optional in sandbox, **required in production**. |
| `base_url` | `Option<String>` | Override sandbox vs production. |

## Flows

### 1. Authorize — Generate Payment Link (hosted web checkout)

`POST /api/paylater/merchant-portal/v2/web-checkout`

**Headers:**
```
Authorization: Bearer <access_token>
Content-Type: application/json
```

**Request body (all fields required):**
```json
{
  "outlet_id":            1000000061,
  "currency":             "QAR",
  "amount":               500.00,
  "order_id":             "ORD-<merchant-unique-id>",
  "success_redirect_url": "https://merchant.example/success",
  "fail_redirect_url":    "https://merchant.example/fail",
  "expiry_duration":      60
}
```

| Field | Type | Constraints | Notes |
|---|---|---|---|
| `outlet_id` | long (i64) | issued at onboarding | From connector config. |
| `currency` | string | **must be `"QAR"`** | Hard-coded; reject anything else. |
| `amount` | double | `300.00` – `25000.00` | **Base unit** (major). Pass through `req.amount` after base-unit conversion. Do NOT multiply by 100. |
| `order_id` | string | unique per merchant | **Acts as idempotency key.** Use UCS `payment_id` (or `merchant_order_id`); duplicate submit returns `Order ID must be unique` error. |
| `success_redirect_url` | string (URL) | — | From `req.router_return_url` or connector metadata. |
| `fail_redirect_url` | string (URL) | — | Same source as success URL. |
| `expiry_duration` | int (i64) | `1` – `1440` | Link validity in **minutes**. Default to a sane merchant-configurable value (e.g. 60). |

**Response 200:**
```json
{
  "paymentLinkUrl": "https://payments.uat.paylaterapp.com/paylink/uuid?token=..."
}
```

**Response → UCS mapping (`PaymentsResponseData`):**
- Status: `AttemptStatus::AuthenticationPending` — shopper must visit `paymentLinkUrl` to complete payment.
- `redirection_data`: `Some(Box::new(RedirectForm::Uri { uri: paymentLinkUrl }))` — simple GET redirect, no form fields.
- `connector_transaction_id`: `None` — no gateway-side transaction id is issued at this point. (The gateway only emits `payLaterOrderId` **after** the shopper initiates on the hosted page; see PSync.)
- `connector_response_reference_id`: `Some(order_id)` — our order_id is the only reference that exists.

**Error responses (HTTP 4xx, body `{ "error": "<string>" }`):**

| Error string | Trigger | UCS mapping |
|---|---|---|
| `Merchant ID cannot be null` | missing/invalid `outlet_id` | `ConnectorError::InvalidConnectorConfig` |
| `Order ID must be unique` | duplicate `order_id` | `ConnectorError::ProcessingStepFailed` (idempotency violation — return existing payment link if cached) |
| `Amount must be between 300 and 25000` | amount out of range | `ConnectorError::ProcessingStepFailed` |
| (anything else) | generic | `ConnectorError::ProcessingStepFailed` |

### 2. PSync — Check Payment Status

`GET /api/paylater/merchant-portal/v2/web-checkout/status?order_id=<order_id>`

**Headers:**
```
Authorization: Bearer <access_token>
```

**Query params:**
- `order_id` (required) — the merchant order id from Authorize.

**Response 200 (body always JSON, **status is an integer**, `message` is a human hint):**
```json
{
  "status":  2,
  "message": "success",
  "payLaterOrderId":    "PL-ORD-XXXXXXXX",
  "merchantReference":  "ORD-<merchant-unique-id>"
}
```

| Field | Type | Always present | Notes |
|---|---|---|---|
| `status` | integer | yes | See status mapping table below. |
| `message` | string | yes | One of `pending`, `success`, `failed`, `Order not initiated`. |
| `payLaterOrderId` | string | no | Present once shopper has initiated on hosted page. |
| `merchantReference` | string | no | Present once initiated; echoes our `order_id`. |

**Error response:**
```json
{ "error": "Order ID is required" }
```

**Status → UCS mapping:**

| PayLater `status` (int) | `message` | UCS `AttemptStatus` | Notes |
|---|---|---|---|
| `0` | `Order not initiated` | `AuthenticationPending` | Shopper hasn't proceeded on the hosted page yet. Payment link still valid (or expired — distinguish by age since link creation). |
| `1` | `pending` | `Pending` (a.k.a. `Authorizing`) | Payment has been initiated on the hosted page but not yet finalized. |
| `2` | `success` | `Charged` | Payment captured (auto-settled by hosted checkout — no separate Capture). |
| `3` | `failed` | `Failure` | Payment failed/declined. |

On `status == 2` (`Charged`): also populate
- `connector_transaction_id = payLaterOrderId` (if present in response)
- `connector_response_reference_id = merchantReference` (echoes our `order_id`)

### 3. Refund — Full or partial

`POST /api/paylater/merchant-portal/v2/web-checkout/refund`

**Headers:**
```
Authorization: Bearer <access_token>
Content-Type: application/json
```

**Request body:**
```json
{
  "order_id": "ORD-<merchant-order-id>"
}
```

- The docs title the endpoint "Full refund" but the endpoint description and rules support **full AND partial** refunds; the request body only carries `order_id`, so any partial amount must be passed out-of-band or the endpoint auto-refunds the full transaction. **Treat PayLater refunds as full-amount refunds** driven only by `order_id`. If UCS receives a partial refund request, document and pass through; surface any gateway-rejected partial amount as a connector error.
- Rules (from docs): refundable within **29 days** of transaction and **no sooner than 10 minutes** after the transaction.
- The refund is synchronous-accepted — no status endpoint exists for refunds (RSync is `not_supported`).

**Response 200:**
```json
{
  "message": "Refund request accepted for reference Id: ORD-TEST-1"
}
```

**Response → UCS mapping:**
- `RefundStatus::Success` (refund is accepted synchronously; finality is implied by the 200). No `connector_refund_id` is issued by the gateway — echo back the UCS `refund_id` as `connector_refund_id` so RSync-by-us has something to key on if ever called.
- `connector_refund_id = req.refund_id` (since gateway does not return its own refund reference).

**Error responses — body is either `{ "error": "Refund Error", "message": "<detail>" }` or plain `{ "error": "<string>" }`:**

| Error body | Trigger | UCS mapping |
|---|---|---|
| `{"error": "Transaction Reference is required"}` | missing/empty `order_id` | `ConnectorError::ProcessingStepFailed` |
| `{"error": "Invalid API Key"}` | bad/expired token | `ConnectorError::FailedToObtainAuthType` |
| `"Order cannot be refunded as it happened more than 29 days ago."` | >29d | `ConnectorError::ProcessingStepFailed` |
| `"Order contains transactions other than down payment and cannot be refunded."` | installment state | `ConnectorError::ProcessingStepFailed` |
| `"Transaction happened less than 10 minutes ago. Please try again later."` | <10min | `ConnectorError::ProcessingStepFailed` |
| `{"error": "Refund Error", "message": "Invalid Transaction Reference."}` | unknown `order_id` | `ConnectorError::ResourceNotFound` |

### 4. RSync — Refund status sync

**Not supported by gateway.** PayLater exposes no refund-retrieve endpoint. The connector must declare `RSync` as `not_supported` in the macro table.

### 5. Webhook — Incoming status notification

PayLater POSTs JSON events to the merchant webhook endpoint configured on the merchant account (or forwarded from UCS).

**Request shape (PayLater → merchant):**
- Method: `POST`
- Content-Type: `application/json`

**Payload fields:**

| Field | Type | Required | Notes |
|---|---|---|---|
| `merchantId` | string | yes | PayLater's id for the merchant. |
| `orderId` | string | yes | Echoes our `order_id`. Use to join back to `payments`. |
| `paylaterRef` | string | yes | PayLater's internal reference; equals `payLaterOrderId` from PSync. |
| `status` | string | yes | One of `success`, `failed`, `pending`. |
| `timestamp` | long (i64) | yes | Unix epoch (seconds). Used in txHash. |
| `txHash` | string (hex) | yes | MD5 checksum — see verification. |
| `signature` | string (hex) | yes | HMAC-SHA256 — see verification. |
| `comments` | string | **optional** | Free-form. Used in txHash **if present**. Empty-string when absent. |

#### Signature verification — TWO steps, BOTH must pass

1. **txHash check.** Compute:
   ```
   txHash_expected = MD5( UPPERCASE( merchantId + orderId + status + timestamp + comments ) )
   ```
   Compare against `payload.txHash`. On mismatch → reject webhook.
   - Concatenation is raw string concat, **no separators**.
   - `timestamp` rendered as decimal string (no padding).
   - `comments` = empty string when the JSON field is missing/null.

2. **signature check.** Compute:
   ```
   signature_expected = HMAC_SHA256( message = txHash_expected, key = merchantWebhookSecret )
   ```
   Compare against `payload.signature`. On mismatch → reject webhook.

**Merchant endpoint must return HTTP 200 quickly.** PayLater retries on non-2xx.

**Event-type mapping (`IncomingWebhookEvent`):**

| Webhook `status` | UCS `IncomingWebhookEvent` |
|---|---|
| `success` | `PaymentIntentSuccess` |
| `failed` | `PaymentIntentFailure` |
| `pending` | `PaymentIntentProcessing` |

**No refund events are documented** — refunds must be reconciled via the synchronous Refund response only.

If `webhook_secret` is absent: log warning, skip HMAC step in sandbox, **fail closed in production**.

## Pre-auth detection (Phase 1 of GRACE)

| Step / flow | Required? | Notes |
|---|---|---|
| `ServerAuthenticationToken` (OAuth client_credentials) | **YES** | `should_do_access_token = true`. Token lifetime 300s — cache and refresh. |
| `PreAuthenticate` | NO | No 3DS S2S; auth happens inside hosted page. |
| `Authenticate` | NO | Hosted checkout handles it. |
| `PostAuthenticate` | NO | Hosted checkout handles it. |
| `CreateOrder` | NO | No separate order-creation step; the Authorize call both creates the order and generates the link. |
| `CreateConnectorCustomer` | NO | Not in docs. |
| `PaymentMethodToken` | NO | Not in docs. |
| `ServerSessionAuthenticationToken` | NO | Not in docs. |

**Access-token handling details for codegen:**
- The OAuth token response does not include a `Correlation-Id` / trace header honoured by the gateway. Surface `error_description` in connector error logs verbatim.
- Token cache key: `client_id`. Refresh proactively when less than 60 s of `expires_in` remains.

## Flow status — `macro_connector_flow_status_impls`

| Flow | Status |
|---|---|
| `Authorize` | **implemented** |
| `PSync` | **implemented** |
| `Refund` | **implemented** |
| `Capture` | `not_supported` — auto-settled by hosted checkout; no capture endpoint exists |
| `Void` | `not_supported` — no void endpoint exists |
| `RSync` | `not_supported` — no refund-status endpoint exists |
| `PreAuthenticate`, `Authenticate`, `PostAuthenticate` | `not_supported` — hosted checkout handles auth |
| `CreateOrder` | `not_supported` |
| `CreateConnectorCustomer` | `not_supported` |
| `PaymentMethodToken` | `not_supported` |
| `ServerSessionAuthenticationToken` | `not_supported` |
| `SetupMandate` | `not_supported` |
| `RepeatPayment` | `not_supported` |
| `IncrementalAuthorization` | `not_supported` |
| `SubmitEvidence`, `DefendDispute`, `Accept`, all dispute flows | `not_supported` |

No empty trait stubs — the macro carries the not_supported declarations.

## Amount handling — important

- PayLater takes amounts as JSON **double** (major / base unit). UCS internally uses minor units (i64). The transformer MUST divide by the currency's exponent (for QAR, 100) and serialize as a Rust `f64`.
- Use the `get_amount_as_f64`-style helper (or `convert_amount` with `AmountConvertor::JsonMajorUnit` / base-unit strategy that other major-unit connectors use — see `stax`, `payu`, `loonio` transformers for prior art).
- Validate `300.00 <= amount <= 25000.00` and reject out-of-range **before** sending to the gateway. Surface as `ConnectorError::ProcessingStepFailed` with a human-readable message.

## Reuse — explicit references for code generation

| Concept | Source |
|---|---|
| Overall connector layout (main `.rs` + `transformers.rs` + macros) | `crates/integrations/connector-integration/src/connectors/adyen.rs` + `adyen/transformers.rs` |
| OAuth client_credentials token flow + cache | `crates/integrations/connector-integration/src/connectors/truelayer.rs` (PS256 / token pattern), `crates/integrations/connector-integration/src/connectors/fiuu/` (client_credentials) |
| Major-unit (f64) amount conversion | `crates/integrations/connector-integration/src/connectors/stax/transformers.rs`, `crates/integrations/connector-integration/src/connectors/payu/transformers.rs` |
| `PayLaterData` variant pattern | `crates/types-traits/domain_types/src/payment_method_data.rs:808` |
| `ConnectorSpecificConfig::Paylater` strongly-typed pattern | `crates/types-traits/domain_types/src/router_data.rs` (Adyen + Truelayer variants) |
| `TryFrom<&ConnectorSpecificConfig>` for connector auth | `adyen/transformers.rs:1372-1393`, `truelayer/transformers.rs:83-100` |
| Simple `RedirectForm::Uri` construction | `flywire/transformers.rs` — uses the same "return a redirect URL in `redirection_data`" pattern |
| HMAC-SHA256 util for webhook signature | shared crypto helpers — see `crates/common_utils` / `crates/integrations/connector-integration/src/utils.rs` |
| MD5 util | `crypto` crate (md5 feature) or `md-5` — already used by Fiuu / Airwallex |

## Test values (Sandbox)

- `outlet_id`: `1000000061`
- Currency: `QAR`
- Amount: any double in `[300, 25000]`. Use `500.00` for happy-path.
- `expiry_duration`: `60` (minutes) — sensible default for local tests.
- Successful completion requires a real shopper flow (PayLater hosted page); UCS tests should mock the gateway or use `paymentLinkUrl` extraction only.

## Known limitations / notes

1. **Auto-settlement.** The hosted checkout auto-settles. No Capture endpoint exists; after `Charged` is reached the payment is final at the gateway level.
2. **No refund status endpoint.** Refund success/failure must be inferred from the synchronous Refund response only — no RSync polling, no refund webhook event.
3. **Refund idempotency.** The Refund endpoint is keyed by `order_id` only — not by `refund_id`. A refund retry on the same `order_id` will likely re-attempt the refund; treat Refund as **not idempotent** at the gateway level. UCS-level idempotency must be enforced upstream (deduplicate by UCS `refund_id`).
4. **10-minute refund window.** Refunds submitted within 10 minutes of transaction completion are rejected. Surface the gateway error verbatim. Consider UX guidance: temporary-failure → retry-after.
5. **29-day window.** Refunds older than 29 days are rejected. Surface the error verbatim.
6. **`connector_transaction_id` is empty after Authorize.** It only appears (as `payLaterOrderId`) after the shopper initiates on the hosted page. Persist it on first PSync observation; use it for join-back in subsequent webhook events.
7. **Idempotency key is `order_id`.** A second Authorize with the same `order_id` fails with `Order ID must be unique`. UCS must either (a) cache the payment link by its `payment_id` and return the cached link on retry, or (b) accept the gateway error as the idempotent-rejection signal.
8. **Currency is QAR-only.** Reject non-QAR currencies at the transformer with `ConnectorError::InvalidConnectorConfig` or `ConnectorError::NotImplemented`.

## Documentation references

- Auth: PayLater developer docs — OAuth client_credentials (`/auth/realms/api/protocol/openid-connect/token`)
- Generate Payment Link: `POST /api/paylater/merchant-portal/v2/web-checkout`
- Check Status: `GET /api/paylater/merchant-portal/v2/web-checkout/status?order_id=...`
- Refund: `POST /api/paylater/merchant-portal/v2/web-checkout/refund`
- Webhooks: txHash (MD5) + HMAC-SHA256 signature scheme; secret via account manager
