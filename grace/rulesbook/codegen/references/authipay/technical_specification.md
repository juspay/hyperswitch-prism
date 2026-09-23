# Authipay (Fiserv IPG) API — Technical Specification

Connector name (crate): `authipay`
Module: `crates/integrations/connector-integration/src/connectors/authipay.rs` (+ `authipay/transformers.rs`)
Generation mode: REGENERATE (spec regenerated from 18 fetched sources; see Amendment Log)
Scope: extend the existing connector with ThreeDS/Card and IncomingWebhook (plus adjacent
fields documented here where they affect those units: $0 card verification, CIT/MIT recurring
payment tokens, L2/L3 purchase cards, soft descriptor, billing/shipping, AVS/CVV response
mapping, issuer decline codes, merchantTransactionId passthrough).

## Overview

**Connector Name:** Authipay
**API Version:** Payments API v2 (OpenAPI `version: 23.2.0`) (Source: `source_11.md`)
**Protocol:** REST over HTTPS
**Data Format:** JSON (`Content-Type: application/json`)
**Architecture:** Fiserv International Payment Gateway (IPG) unified payments REST API.
Authipay is the AIB Merchant Services (EMEA) deployment of this API; a single gateway
exposes primary transactions (sale/preauth/credit), secondary transactions
(postAuth/void/return), GET-based payment/order status, payment tokenization,
card/account verification, and server-to-server (S2S) web notifications.

### Base URLs

| Environment | Endpoint URL |
|-------------|--------------|
| Sandbox | `https://prod.emea.api.fiservapps.com/sandbox/ipp/payments-gateway/v2` |
| Production | `https://prod.emea.api.fiservapps.com/ipp/payments-gateway/v2` |

(Source: `source_11.md` servers block; also `source_13.md`.)

### Additional Resources

- Payments API Reference (OpenAPI): https://docs.fiserv.dev/public/openapi/6582b0d2483992005c5c56f0 (Source: `source_11.md`)
- Submit Primary Transaction reference: https://docs.fiserv.dev/public/reference/submitprimarytransaction (Source: `source_13.md`)
- Submit Secondary Transaction reference: https://docs.fiserv.dev/public/reference/submitsecondarytransaction (Source: `source_14.md`)
- Order inquiry reference: https://docs.fiserv.dev/public/reference/orderinquiry (Source: `source_12.md`)
- Message Signature guide: https://docs.fiserv.dev/public/docs/message-signature (Source: `source_4.md`)
- Errors guide: https://docs.fiserv.dev/public/docs/errors (Source: `source_3.md`)
- Server-to-Server Notifications guide: https://docs.fiserv.dev/public/docs/s2s-notifications (Source: `source_8.md`)
- Webhooks and status updates (Checkout): https://docs.fiserv.dev/public/docs/webhooks-and-status-updates-checkout (Source: `source_10.md`)
- Payment methods general concepts: https://docs.fiserv.dev/public/docs/payments-general-concepts (Source: `source_6.md`)
- Test cards: https://docs.fiserv.dev/public/docs/payments-test-cards (Source: `source_7.md`)
- Sandbox usage: https://docs.fiserv.dev/public/docs/sandbox-usage (Source: `source_9.md`)
- AIBMS Authipay API (Web Service / XML — legacy) Integration Guide (PDF): Source: `source_17.md`
- AIBMS Authipay Connect (hosted payment page) Integration Guide (PDF): Source: `source_18.md`

<!-- CONFLICT: source_17.md documents the legacy SOAP/XML Web Service API (test.ipg-online.com),
and source_18.md documents the Authipay Connect hosted-payment-page (form POST) product.
The connector integrates to the REST payments-gateway v2 API (source_11). The XML guide is
used here only for response-code semantics (AVS codes, approval-code structure,
recurringType FIRST/REPEAT) that are common to the platform; all endpoints and JSON schemas
come from source_11/source_13/source_14. -->
---

## Authentication

### Method

HMAC-SHA256 request signing of every API call. The merchant is issued an **API Key** and a
**Secret Key** by Fiserv/Authipay (retrieved from the Developer Portal Apps screen; both must
come from the same application). Four headers carry authentication on every request:
`Api-Key`, `Client-Request-Id`, `Timestamp`, `Message-Signature`.
(Source: https://docs.fiserv.dev/public/docs/message-signature — `source_4.md`)

### Creating the Authentication Header

Step by step (Source: `source_4.md`):

1. `apiKey` — the API Key from the developer portal.
2. `Client-Request-Id` — client-generated unique ID per request (128-bit UUID recommended);
   it is echoed back in the response and also used for idempotency control
   (Source: `source_13.md` headers section).
3. `Timestamp` — `new Date().getTime()`; epoch time in **milliseconds**. Also used by the
   gateway for a 5-minute time-limit check (Source: `source_13.md` Timestamp header).
4. Request body — for non-GET requests the JSON body **stringified exactly as sent**;
   for GET requests the body component is the empty string `''` (Source: `source_4.md` demo code).
5. `rawSignature = apiKey + Client-Request-Id + Timestamp + requestBody` (string concatenation,
   no separators).
6. `computedHash = HMAC-SHA256(rawSignature, key=secret)`.
7. `Message-Signature = Base64(computedHash)`.

This is exactly what the existing module implements in `Authipay::build_headers_with_signature`
(`crates/integrations/connector-integration/src/connectors/authipay.rs`), signing `""` for GET
flows (PSync/RSync). **Do not change this.**

### Credentials (live merchant account)

| Credential | Source | Used as |
|------------|--------|---------|
| `api_key` | Developer Portal / Authipay boarding letter | `Api-Key` header + HMAC raw-string prefix |
| `api_secret` | Developer Portal / Authipay boarding letter | HMAC-SHA256 key for `Message-Signature` |

The connector's `AuthipayAuthType` holds exactly these two secrets (live creds, no Key1/
Signature1 split, no AdditionalSecretData).

### Sandbox credentials / test cards

Public test cards are listed in https://docs.fiserv.dev/public/docs/payments-test-cards
(`source_7.md`). A demo app exists at
https://github.com/Fiserv-Developer/fiserv-payments-demo (`source_15.md`).

### API Key Permission Levels

Not specified in the source documentation. Keys are issued per application/store; S2S
notification signing uses a separate **shared secret** (`sharedsecret`; for recurring
notifications a different `rcpSharedSecret`), configured in the Virtual Terminal /
store settings — NOT the API secret (Source: `source_18.md` §15.2-15.3; `source_8.md`).
---

## Common Headers

### Request Headers

| Header | Value | Required | Description |
|--------|-------|----------|-------------|
| `Content-Type` | `application/json` | Yes | Body format. (Source: `source_13.md`, `source_4.md`) |
| `Api-Key` | merchant API key | Yes | "Key given to merchant after boarding associating their requests with the appropriate app in Apigee." (Source: `source_13.md`) |
| `Client-Request-Id` | UUID / unique string | Yes | "A client-generated ID for request tracking and signature creation, unique per request. This is also used for idempotency control. We recommend 128-bit UUID format." (Source: `source_13.md`) |
| `Timestamp` | int64 epoch milliseconds | Yes | "Epoch timestamp in milliseconds ... Used for Message Signature generation and time limit (5 mins)." (Source: `source_13.md`) |
| `Message-Signature` | Base64 HMAC-SHA256 | Yes | "The Message-Signature is the Base64 encoded HMAC hash (SHA256 algorithm with the API Secret as the key.)" (Source: `source_13.md`) |
| `Message-Authentication-Value` | card-present MAC | No | Only for card-present/terminal transactions (DUKPT/AES). Not used by the wizard/ECOM integration. (Source: `source_13.md`) |

### Response Headers/body fields

| Field | Description |
|-------|-------------|
| `clientRequestId` | Echo in every response body (`BasicResponse`). (Source: `source_11.md` BasicResponse) |
| `apiTraceId` | Gateway trace identifier for support requests, e.g. `rrt-0bd552c12342d3448-b-ea-1142-12938318-7`. (Source: `source_11.md`) |
| `type` | Response discriminator, e.g. `transactionResponse`, `errorResponse`, `orderResponse`. (Source: `source_11.md`) |
| `responseType` | `BasicResponse.responseType`. (Source: `source_11.md`) |

### cURL Example

```bash
# Signature inputs
API_KEY="..."
SECRET="..."
CLIENT_REQUEST_ID=$(uuidgen)
TIMESTAMP=$(date +%s%3N)
BODY='{"requestType":"PaymentCardSaleTransaction","transactionAmount":{"total":13,"currency":"GBP"},"paymentMethod":{"paymentCard":{"number":"4012000000000001","securityCode":"123","expiryDate":{"month":"01","year":"29"}}}}'

RAW="${API_KEY}${CLIENT_REQUEST_ID}${TIMESTAMP}${BODY}"
SIG=$(printf '%s' "$RAW" | openssl dgst -sha256 -hmac "$SECRET" -binary | base64)

curl -X POST \
  https://prod.emea.api.fiservapps.com/sandbox/ipp/payments-gateway/v2/payments \
  -H "Content-Type: application/json" \
  -H "Api-Key: ${API_KEY}" \
  -H "Client-Request-Id: ${CLIENT_REQUEST_ID}" \
  -H "Timestamp: ${TIMESTAMP}" \
  -H "Message-Signature: ${SIG}" \
  -d "${BODY}"
```

(Source: constructed from `source_4.md` example code and `source_13.md` header spec.)
---

## HTTP Codes and Errors

### HTTP Status Codes

| Code | Definition | Explanation |
|------|------------|-------------|
| 200 | Success | Transaction processed; inspect `transactionState` / `transactionResult` / `processor.responseCode` for business outcome. (Source: `source_13.md`) |
| 400 | Bad Request | "The request cannot be validated." No payment attempted. (Source: `source_13.md` / `source_3.md`) |
| 401 | Unauthorised | "The request cannot be authenticated or was submitted with the wrong credentials." Check API key / signature. (Source: `source_13.md`) |
| 403 | Forbidden | "The request was unauthorized." Insufficient privileges. (Source: `source_13.md`) |
| 404 | Not Found | "The requested resource doesn't exist." (Source: `source_13.md`) |
| 409 | Conflict | "The attempted action is not valid according to gateway rules. For example, the merchant is not set-up or the order already exists." (Source: `source_13.md`) |
| 415 | Unsupported Media Type | "Format that is not supported by the server for the HTTP method." (Source: `source_13.md`) |
| 422 | Unprocessable Entity | **"The processor declined the transaction."** — gateway-level decline surfaced as HTTP 422 on primary transaction calls. (Source: `source_13.md`) |
| 429 | Too Many Requests | Rate limit; wait and retry. (Source: `source_3.md`) |
| 500 | Internal Server Error | "An unexpected internal server error occurred." (Source: `source_13.md`) |
| 502 | Bad Gateway | "There was a problem communicating with the endpoint." (Source: `source_13.md`) |

### Error Response Body Format

Two shapes are documented:

**(a) Gateway error model** — array form returned by edge/Apigee validation (Source: `source_3.md`):

```json
{
  "errors": [
    { "title": "Missing 'name'",
      "detail": "The 'name' field is required and must be specified",
      "source": "Apigee" },
    { "title": "Invalid 'phoneNumber'",
      "detail": "The 'phoneNumber' field must be of type 'number'",
      "source": "Backend" }
  ]
}
```

**(b) API `ErrorResponse`** — single error object (Source: `source_11.md` ErrorResponse → Error → ErrorDetails):

```json
{
  "clientRequestId": "30dd879c-ee2f-11db-8314-0800200c9a66",
  "apiTraceId": "rrt-0c80a3403e2c2def0-d-ea-28805-6810951-2",
  "responseType": "ERROR",
  "type": "errorResponse",
  "error": {
    "code": "2303",
    "message": "Invalid credit card number",
    "details": [
      { "field": "PaymentCard.number", "message": "may not be null" }
    ],
    "declineReasonCode": "Do not try again"
  }
}
```

The existing connector parses shape (b): `AuthipayErrorResponse { code, message, api_trace_id (reason) }`
(authipay.rs `build_error_response`). Shape (a) appears on edge validation failures; treat it
as a secondary parse fallback.

### Processor / issuer codes (success-path decline info)

On HTTP 200 (or 422) financial declines, the outcome lives in `TransactionResponse`:
(Source: `source_11.md`)

- `transactionResult`: `CREATED|APPROVED|DECLINED|FAILED|WAITING|PARTIAL|FRAUD`
- `transactionStatus` (deprecated, mirrors above): `APPROVED|WAITING|PARTIAL|VALIDATION_FAILED|PROCESSING_FAILED|DECLINED`
- `transactionState`: `AUTHORIZED|CAPTURED|DECLINED|CHECKED|COMPLETED_GET|INITIALIZED|PENDING|READY|TEMPLATE|SETTLED|VOIDED|WAITING`
- `processor.responseCode` — processor endpoint response code (e.g. `00`)
- `processor.responseMessage` — human-readable processor message (e.g. `APPROVED`)
- `processor.associationResponseCode` — **raw issuer (association) response code**; use for issuer decline-code mapping
- `processor.associationResponseMessage` — text for the association code
- `processor.authorizationCode`, `processor.referenceNumber`
- `errorMessage` — gateway error text, e.g. `"000100: Tx was processed but response was not stored correctly"`
- `approvalCode` — structured `"Y:<authcode>:<ref>:<sub>:<txnum>"` on success, or `"N:-30031:No terminal setup"` on failure (first char: `Y` = approved, `N` = not approved, `?` = initialized/waiting; Source: `source_18.md` §15.1, `source_17.md` approval-code examples)

The legacy Web Service API guide (`source_17.md`) uses XML `ProcessorResponseCode` with the
same `00` = approval convention.

### Error Codes

Specific numeric `error.code` values (e.g. `2303` "Invalid credit card number") are not
enumerated in the fetched sources beyond the OpenAPI examples. Map by HTTP code +
`error.code` string + `processor.associationResponseCode`; see Status Mappings below for
the concrete decline-code table the connector should implement.
---

## Configuration Parameters

### Idempotent Requests

`Client-Request-Id` is the idempotency key: "unique per request ... also used for
idempotency control". (Source: `source_13.md` headers.) Retry-safe replays must reuse the
same `Client-Request-Id`; new attempts must generate a fresh UUID.

Timestamp validity window: 5 minutes (Source: `source_13.md` Timestamp header,
"time limit (5 mins)").

### Rate Limits

429 is returned when "at your rate limit" (Source: `source_3.md`). No numeric limits are
documented in the sources.

---

## Complete Endpoint Inventory

All endpoints are relative to `<base>/ipp/payments-gateway/v2` (sandbox: prefix
`https://prod.emea.api.fiservapps.com/sandbox`). Full path list (Source: `source_11.md` paths):

| Method | Path | Purpose |
|--------|------|---------|
| POST | `/payments` | Primary transactions: sale, preauth, credit, forced ticket, wallet, token, SEPA, APM, payer-auth |
| GET | `/payments/{transaction-id}` | Retrieve transaction state by ipgTransactionId (payment sync / refund sync) |
| POST | `/payments/{transaction-id}` | Secondary transactions: PostAuth, Void, Return (Refund) |
| PATCH | `/payments/{transaction-id}` | Update/continue a payment — **3DS authentication continue (post-ACS)** |
| PATCH | `/payments/action/{transaction-id}` | Perform an update action on an existing transaction |
| GET | `/orders/{order-id}` | Retrieve order state (order incl. transactions) |
| POST | `/orders/{order-id}` | Secondary PostAuth/Return on an order |
| POST | `/payment-tokens` | Create payment token from a payment card (standalone tokenization) |
| GET | `/payment-tokens/{token-id}` | Look up card details for a token |
| DELETE | `/payment-tokens/{token-id}` | Delete a payment token |
| PATCH | `/payment-tokens` | Update one or more payment tokens |
| POST | `/card-verification` | Verify a payment card ($0/AVS card verification) |
| POST | `/account-verification` | Verify card or token (PaymentCardVerificationRequest etc.) |
| POST | `/payment-schedules` | Create gateway-side recurring schedule |
| GET/PATCH/DELETE | `/payment-schedules/{order-id}` | View / update / cancel schedule |
| POST | `/payment-url` | Create a hosted payment URL |
| POST | `/exchange-rates` | DCC rate inquiry |
| POST | `/card-information`, `/account-information` | Card/account lookup |

Only the endpoints used by the connector's flows are detailed below.

### Primary Transactions (POST /payments)

#### 1. Card Sale — `PaymentCardSaleTransaction`

**Endpoint:** `POST /payments`
**Purpose:** Authorise and capture in one call (auto-capture). Implemented by the existing
Authorize flow.

**Request Body:**

```json
{
  "requestType": "PaymentCardSaleTransaction",
  "transactionAmount": { "total": 13, "currency": "GBP" },
  "paymentMethod": {
    "paymentCard": {
      "number": "4012000000000001",
      "securityCode": "123",
      "expiryDate": { "month": "01", "year": "29" }
    }
  },
  "transactionOrigin": "ECOM",
  "merchantTransactionId": "lsk23532djljff3",
  "order": {
    "orderId": "ABC12345",
    "billing": { "name": "John Doe", "address": { "address1": "123 Main St.", "city": "Sandy Springs", "region": "Georgia", "postalCode": "30303", "country": "USA" } },
    "shipping": { "name": "John Doe", "address": { "address1": "123 Main St.", "city": "Sandy Springs", "region": "Georgia", "postalCode": "30303", "country": "USA" } }
  }
}
```

(Source: `source_11.md` PaymentCardSaleTransaction/Order/Billing; `source_13.md`.)

**Request Parameters (PrimaryTransaction base + payment-card extras):**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `requestType` | string | Yes | `PaymentCardSaleTransaction` (sale) or `PaymentCardPreAuthTransaction` (manual capture). Other accepted types: `PaymentCardCreditTransaction`, `PaymentCardPayerAuthTransaction`, `PaymentToken{Sale,PreAuth,Credit,Payerauth}Transaction`, wallet/terminal/SEPA/APM variants. (Source: `source_13.md`) |
| `transactionAmount.total` | number | Yes | Amount in major/minor per currency convention; number type. |
| `transactionAmount.currency` | string | Yes | ISO 4217 alpha-3 or numeric. |
| `merchantTransactionId` | string ≤40 | No | "The unique merchant transaction ID from the request [body], if supplied." **Echoed back in response as-is — passthrough semantics.** (Source: `source_11.md`) |
| `storeId` | string ≤20 | No | Outlet ID for multi-store apps. |
| `userId` | string ≤128 | No | Store's user ID. |
| `transactionOrigin` | enum | No | `ECOM|MOTO|MAIL|PHONE|RETAIL`. |
| `parentUri` | uri | No | For embedding HP pages in an iFrame. |
| `allowPartialApproval` | boolean | No | Allow partial approval. |
| `ipgTransactionId` | int64/null | No | Reference a previous payer-auth transaction. |
| `paymentMethod.paymentCard` | object | Yes | `number` (13-34 chars), `securityCode` (3-4), `expiryDate{month,year}`, optional `cardholderName`, `brand`, `cardFunction`, `cardAccountType`. (Source: `source_11.md` PaymentCard) |
| `storedCredentials` | object | No | CIT/MIT flags — see Recurring section. |
| `createToken` | object | No | DataVault token creation — see Tokens section. |
| `authenticationRequest` | object (Secure3D*) | No | Request 3DS — mutually exclusive with `authenticationResult`. |
| `authenticationResult` | object (Secure3D*Result) | No | Externally managed 3DS results (CAVV/XID/ECI). |
| `order` | Order object | No | `orderId`, `billing`, `shipping`, `purchaseCard` (L2/L3), `softDescriptor`, `additionalDetails`. |
| `currencyConversion` | object | No | DCC / DynamicPricing. |
| `settlementSplit` | object | No | Settlement splits. |

**Response 200 — TransactionResponse:**

```json
{
  "clientRequestId": "30dd879c-ee2f-11db-8314-0800200c9a66",
  "apiTraceId": "rrt-0c80a3403e2c2def0-d-ea-28805-6810951-2",
  "type": "transactionResponse",
  "ipgTransactionId": "838916029301",
  "orderId": "123456",
  "userId": "1001",
  "transactionType": "SALE",
  "transactionOrigin": "ECOM",
  "paymentMethodDetails": {
    "paymentCard": { "number": "************4977", "expiryDate": {"month": "12", "year": "25"}, "brand": "Visa" },
    "paymentMethodType": "PAYMENT_CARD"
  },
  "country": "USA",
  "terminalId": "123456",
  "merchantId": "199950008",
  "merchantTransactionId": "lsk23532djljff3",
  "transactionTime": 1518811817,
  "approvedAmount": { "total": 10.24, "currency": "EUR" },
  "transactionAmount": { "total": 10.24, "currency": "EUR", "components": {"subtotal": 8, "localTax": 1, "shipping": 1.24} },
  "transactionStatus": "APPROVED",
  "transactionResult": "APPROVED",
  "transactionState": "CAPTURED",
  "approvalCode": "Y:OK7118:811720726601:YYYM:441809",
  "schemeResponseCode": "00",
  "secure3dResponse": { "responseCode3dSecure": "3", "authenticationValue": "AAAA...", "directoryServerTransactionId": "123e4567-..." },
  "schemeTransactionId": "019078743804756",
  "transactionLinkIdentifier": "01236548543965",
  "processor": {
    "referenceNumber": "811720726601",
    "authorizationCode": "OK7118",
    "responseCode": "00",
    "responseMessage": "APPROVED",
    "associationResponseCode": "000",
    "avsResponse": { "streetMatch": "Y", "postalCodeMatch": "N", "associationAvsResponse": "Y" },
    "securityCodeResponse": "MATCHED"
  }
}
```

(Source: `source_11.md` TransactionResponse example fields; `source_13.md` 200.)

Note: `merchantTransactionId` is returned only when supplied (passthrough). `transactionState`
for a straight sale is `CAPTURED`; for pre-auth it is `AUTHORIZED`; `DECLINED` on decline;
`WAITING` when 3DS/redirect is pending.
#### 2. Card PreAuth — `PaymentCardPreAuthTransaction`

**Endpoint:** `POST /payments`
**Purpose:** Authorise only (manual capture). Same shape as Sale with
`requestType: "PaymentCardPreAuthTransaction"`; response `transactionState: "AUTHORIZED"`.
Implemented by the existing Authorize flow (manual capture branch). Must be followed by a
`PostAuthTransaction` to move funds. (Source: `source_13.md`; `source_2.md` — the
PreAuthorization vs Sale doc page was empty in the scrape, so rely on the OpenAPI.)

#### 3. Capture (PostAuth) — `PostAuthTransaction`

**Endpoint:** `POST /payments/{transaction-id}` where `{transaction-id}` is the
pre-auth's `ipgTransactionId`.
**Purpose:** Capture a previously-authorized transaction (partial or full amount).

```json
{
  "requestType": "PostAuthTransaction",
  "transactionAmount": { "total": 10.24, "currency": "EUR" },
  "merchantTransactionId": "capture-0001",
  "comments": "This is a comment"
}
```

Response: `TransactionResponse` (as above) with `transactionType: "POSTAUTH"`.
(Source: `source_14.md`, `source_11.md` SecondaryTransaction + PostAuthTransaction.)
The existing Capture flow implements this.

#### 4. Refund (Return) — `ReturnTransaction`

**Endpoint:** `POST /payments/{transaction-id}` (parent = the captured sale/postAuth's `ipgTransactionId`).
**Purpose:** Return funds to the cardholder. Partial and multiple returns are supported; a
return can only reference a transaction in a settleable state.

```json
{
  "requestType": "ReturnTransaction",
  "transactionAmount": { "total": 12.04, "currency": "EUR" },
  "merchantTransactionId": "refund-0001",
  "comments": "This is a comment"
}
```

Optional extras on ReturnTransaction (Source: `source_11.md` ReturnTransaction):
`softDescriptor`, `storedCredentials`, `currencyConversion`, `walletDetails`, `paymentMethod`.
Response `transactionType: "RETURN"`. The existing Refund flow implements this.

#### 5. Void — `VoidTransaction` / `VoidPreAuthTransactions`

**Endpoint:** `POST /payments/{transaction-id}`.
**Purpose:** Cancel a same-day authorization/capture before settlement; no amount for a full
void (`transactionAmount` optional; `reversalReason`, `storedCredentials`,
`walletDetails` optional). The module implements:

- **VoidPC** (`requestType: "VoidTransaction"`) — generic void (e.g. void a returned/captured txn same day).
- **Void** (`requestType: "VoidPreAuthTransactions"`) — void a pre-auth specifically.

<!-- Leonard note: the connectors's Void marker maps to VoidPreAuthTransactions and VoidPC to VoidTransaction
in transformers.rs — both are accepted secondary request types. -->

```json
{ "requestType": "VoidPreAuthTransactions", "comments": "cancel pre-auth" }
{ "requestType": "VoidTransaction", "transactionAmount": {"total": 12.04, "currency": "EUR"}, "reversalReason": "CARDHOLDER_REQUEST" }
```

Legacy XML equivalent (`source_17.md` §Voids): a void can be referenced by OrderId/Tdate or
by `ReferencedMerchantTransactionId` when the original payment method is unknown to the
caller. In REST the path parameter `transaction-id` is the ipgTransactionId; order-level
voids can use `POST /orders/{order-id}`. (Source: `source_17.md`, `source_11.md`.)

#### 6. Payment Sync (PSync)

**Endpoint:** `GET /payments/{transaction-id}`
**Purpose:** Retrieve the state of a single transaction by `ipgTransactionId`.
No body; HMAC raw-string uses empty body (Source: `source_4.md` GET handling).
Response: `TransactionResponse` with current `transactionState` / `transactionResult` /
`transactionType`, `approvedAmount`, `processor` etc. The existing PSync flow implements this.

Order-wide equivalent: `GET /orders/{order-id}` returns `OrderResponse` containing a
`transactions[]` array of TransactionResponse items (Source: `source_12.md`, `source_11.md`).

#### 7. Refund Sync (RSync)

**Endpoint:** `GET /payments/{transaction-id}`
**Purpose:** Same retrieval endpoint applied to the ReturnTransaction's `ipgTransactionId`.
RSync must NOT terminally fail when the return is `WAITING`/processing — treat
`transactionResult: WAITING` as pending. The existing RSync flow implements this.
#### 8. 3-D Secure — authentication on a primary transaction

The gateway-managed 3DS 2.x flow uses three request types working together (Source:
`source_11.md` Secure3D* schemas; `source_13.md`):

1. **Initiate:** POST `/payments` with sale/preauth plus an `authenticationRequest` block
   (`authenticationType: "Secure3DAuthenticationRequest"`). **Do not** send
   `authenticationResult` in the same request (Source: `source_11.md` AuthenticationRequest).
2. **Respond:** response `transactionState: "WAITING"` with an `authenticationResponse`
   object carrying the redirect payload (see below). The merchant closes the response to
   the shopper and redirects to the ACS.
3. **Continue:** after the shopper returns from the ACS (termURL POST), submit
   `PATCH /payments/{transaction-id}` with `Secure3DAuthenticationUpdateRequest` to finish.
   The financial outcome is then returned: `transactionState` moves to `AUTHORIZED`/`CAPTURED`
   or `DECLINED`.

There is also a split variant: `requestType: "PaymentCardPayerAuthTransaction"` performs
only authentication (PAYER_AUTH), then the subsequent sale/preauth references it via
`ipgTransactionId` in the sale/preauth body (Source: `source_11.md` PrimaryTransaction:
"The IPG transactionId to reference a payerauth for example"; legacy equivalent
source_18.md §8.1 3DSecure Split Authentication). The default integrated flow (1)-(3)
is the one the connector's ThreeDS/Card unit should implement; `PaymentCardPayerAuthTransaction`
maps to the `PreAuthenticate` leg of the 3-marker layout.

##### `authenticationRequest` (Secure3DAuthenticationRequest)

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `authenticationType` | string | Yes | `Secure3DAuthenticationRequest` (current). `Secure3D21AuthenticationRequest` is deprecated. (Source: `source_11.md`) |
| `termURL` | string ≤2048 | Yes | "The result of the authentication will be sent to this URL." ACS posts results here. |
| `methodNotificationURL` | string ≤2048 | Yes | Optional 3DS-method iframe callback; the 3DS method form and transaction ID are posted here. |
| `challengeIndicator` | enum | No | `01` no preference (default), `02` no challenge requested, `03` challenge requested (preference), `04` challenge requested (mandate); 3DS 2.2 adds `05..09`. (Source: `source_11.md`) |
| `challengeWindowSize` | enum | No | `01` 250x400 … `05` full screen. |
| `messageCategory` | enum | No | `01` Payment Authentication (default), `02` Non-Payment Authentication — **use 02 with `transactionAmount.total: 0` for $0 card verification**; `80` Mastercard Data Only. (Source: `source_11.md` Secure3DAuthenticationRequest) |
| `transactionMode` | enum | No | `M`(oto)/`P`(ayment)/`I`(nstallment)… (per enum in OpenAPI) |
| `scaExemptionType` | enum | No | SCA exemption request flag. |
| `customerDecoupledRequested` / `decoupledMaxRetry` | — | No | Decoupled authentication options. |
| `billingAddress` / related address objects | object | No | Address passed to the ACS. |
| `emailAddress` etc. | — | No | Cardholder contact for the ACS. |

The deprecated `Secure3D21AuthenticationRequest` retains `termURL`,
`methodNotificationURL`, `challengeIndicator`, `challengeWindowSize` and is what the demo
code in `source_4.md` uses (`challengeIndicator: "04"`); prefer the non-versioned type for
new work per the OpenAPI deprecation note.

##### `authenticationResponse` (Secure3DAuthenticationResponse) — WAITING payload

Returned on the initiate response when redirect is needed (Source: `source_11.md`
Secure3DAuthenticationResponse):

| Field | Description |
|-------|-------------|
| `type` | `"3D_SECURE"` |
| `version` | `"1.0" | "2.1" | "2.2"` |
| `params.payerAuthenticationRequest` | PaReq (3DS1) message to POST to the ACS |
| `params.acsURL` | ACS redirect endpoint |
| `params.termURL` | Echoed term URL |
| `params.merchantData` | MD for 3DS1 round-tripping |
| `params.cReq` | Base64 CReq for 3DS2 challenge |
| `params.sessionData` | Browser session data (pair with redirect) |
| `secure3dMethod.methodForm` | Hidden iframe HTML form for the 3DS method call |
| `otpVerificationResponse` | OTP subflow if configured |

Example WAITING response shape:

```json
{
  "type": "transactionResponse",
  "ipgTransactionId": "838916029301",
  "transactionState": "WAITING",
  "transactionResult": "WAITING",
  "authenticationResponse": {
    "type": "3D_SECURE",
    "version": "2.1",
    "params": {
      "acsURL": "https://3ds-acs.test.modirum.com/mdpayacs/pareq",
      "cReq": "ewogICAiYWNzVHJhbnNJRCIgOiAi...",
      "sessionData": "50F2156E03083CA665BCB4.."
    },
    "secure3dMethod": { "methodForm": "<form ...>" }
  }
}
```

##### `Secure3DAuthenticationUpdateRequest` — `PATCH /payments/{transaction-id}`

| Field | Type | Description |
|-------|------|-------------|
| `authenticationType` | string | `Secure3DAuthenticationUpdateRequest` |
| `storeId` | string ≤20 | Optional outlet |
| `billingAddress` | Address | Optional |
| `methodNotificationStatus` | enum | `RECEIVED|EXPECTED_BUT_NOT_RECEIVED|NOT_EXPECTED` — report the 3DS-method reception |
| `acsResponse.cRes` | string | Base64 CRes returned by the ACS browser POST to termURL |
| `securityCode` | string | CVV if required by merchant config |
| `tokenCryptogram` / `paymentAccountReferenceNumber` | — | Optional |
| `additionalStep` | enum | `COMPLETE_DECOUPLED_AUTHENTICATION` for decoupled flows |

(Source: `source_11.md` Secure3DAuthenticationUpdateRequest + AuthenticationUpdateRequest.)

The PATCH response is the completed `TransactionResponse` with `secure3dResponse`
(`responseCode3dSecure`, `authenticationValue` = CAVV, `directoryServerTransactionId`,
`transactionStatusReason`) and final `transactionState`.

Legacy XML ECI mapping (`source_18.md` §15 responses, `response_code_3dsecure`):
`1` = fully authenticated (VISA ECI 05 / MC ECI 02), `2` = authenticated without AVV,
`3` = authentication failed (decline), `4` = authentication attempt (ECI 06/01),
`5`/`6` = unable to authenticate (ECI 07), `7` = cardholder not enrolled (ECI 06),
`8` = invalid 3DS values. `Secure3DAuthenticationResult.authenticationResponse`
(`A|N|U|Y|C|R`) is the modern ARes equivalent and `transactionStatus` (CRes)
adds `I`. (Source: `source_11.md`.)

##### Externally-managed 3DS (`authenticationResult`)

When 3DS was done outside the gateway, send on the sale/preauth body instead of
`authenticationRequest` (never both). Fields (Source: `source_11.md`
Secure3DAuthenticationResult):

| Field | Type | Description |
|-------|------|-------------|
| `authenticationType` | string | `Secure3DAuthenticationResult` (or `Secure3D10AuthenticationResult`) |
| `cavv` | string 20-32 | Cardholder Authentication Verification Value |
| `xid` | string 20-32 | Transaction identifier |
| `dsTransactionId` | string | Directory-server transaction UUID |
| `acsTransactionId` | string ≤40 | ACS transaction ID |
| `authenticationResponse` | enum | ARes result `A|N|U|Y|C|R` |
| `transactionStatus` | enum | CRes result `A|N|U|Y|C|R|I` |
| `messageCategory` | enum | `01|02|80` |
| `secure3DProtocolVersion` | string | per Secure3DProtocolVersion |


#### 9. Card Verification ($0)

**Endpoint:** `POST /account-verification` (preferred) or `POST /card-verification`

`PaymentCardVerificationRequest` (Source: `source_11.md`):

```json
{
  "requestType": "PaymentCardVerificationRequest",
  "paymentCard": { "number": "4035874000424977", "expiryDate": {"month": "12", "year": "25"}, "securityCode": "977" },
  "billingAddress": { "address1": "5565 Glenridge Conn", "city": "Atlanta", "postalCode": "30342", "country": "USA" },
  "merchantTransactionId": "verify-0001"
}
```

Response: `TransactionResponse` with `accountVerificationResponse` (see AccountVerificationResponse);
no capture occurs.

3DS-flavoured $0 verification: submit `PaymentCardSaleTransaction` (or PreAuth) with
`transactionAmount.total: 0` and
`authenticationRequest.authenticationType: "Secure3DAuthenticationRequest",
messageCategory: "02"` (Non-Payment Authentication / `ThreeDSEmvCoMessageCategory=02`).
(Source: `source_11.md` Secure3DAuthenticationRequest messageCategory + the Authorize shape.)

Optional `verifyCard` behavior is also embedded in token creation (`createToken` with
`reusable: true` runs account-verification first; see PaymentTokenDetails.accountVerification).

#### 10. Payment Tokens (Data Vault)

Token lifecycle (Source: `source_11.md` paths and schemas):

- Create standalone: `POST /payment-tokens` with `PaymentCardPaymentTokenizationRequest`.
- Create as a side effect of a sale/preauth: send `createToken` (`CreatePaymentToken`) with
  the transaction; response has `paymentToken.value` (+ `last4`, `brand`, `type`,
  `accountVerification`, `networkTokenProvisionStatus`).
- `CreatePaymentToken`: `{ value?, reusable=true by default, declineDuplicates=false by default, customWalletRegistration? }`.
- Use: `PaymentToken*Transaction` request types with `paymentMethod.paymentToken = { value, tokenOriginStoreId?, function?, securityCode?, expiryDate? }` (`UsePaymentToken`).
- Lookup/delete: `GET|DELETE /payment-tokens/{token-id}`; update `PATCH /payment-tokens`.

`declineDuplicates: true` rejects duplicate payment info on client-supplied token values.

#### 11. Gateway payment schedules (recurring plans)

`POST /payment-schedules` creates a gateway-managed recurring plan keyed by order
(`RecurringPaymentDetails`: `creationDate`, `startDate`, `nextAttemptDate`, `frequency`,
`numberOfPayments`, `runCount`, `state`, `paymentMethodDetails`, `transactionAmount`).
Lifecycle: `GET|PATCH|DELETE /payment-schedules/{order-id}`. (Source: `source_11.md` paths + schema.)
This is an alternative to merchant-driven MIT (Option B below); the connector's
SetupMandate/RepeatPayment units should use **merchant-driven CIT/MIT with storedCredentials** —
no scheduling at the gateway — unless a requirement says otherwise.
---

## Webhook Events (Server-to-Server Notifications)

Authipay/IPG's asynchronous notification is an **HTTP POST with
`Content-Type: application/x-www-form-urlencoded` — form data, not JSON** — sent to a
merchant-configured notification URL. (Sources: `source_8.md` S2S notifications;
`source_10.md` webhooks-and-status-updates-checkout; `source_18.md` §15.3.)

### Delivery

- URL configured per store ("transactionNotificationUrl" / Connect's `transactionNotificationURL`
  request parameter) or per recurring run ("recurringTransactionNotificationUrl" /
  `rcpTransactionNotificationURL`). Must listen on 443/HTTPS.
- The notification carries the same result parameters as the browser redirect — it is NOT
  proof of payment by itself; also de-duplicate by `ipgTransactionId` and accept
  field-set variance per transaction type. (Source: `source_8.md`; `source_18.md` §15.3.)
- Some APMs first report `WAITING` and later send an updated final result; the endpoint
  must tolerate duplicates/retries (idempotent). (Source: `source_8.md`.)

### Payload fields

Representative notification (Source: `source_8.md`):

```
ipgTransactionId=1234567890&oid=ORDER-100045&chargetotal=49.99&currency=978&
txndatetime=2026:09:10-15:30:45&storename=12345678901&approval_code=Y:123456:...&
status=APPROVED&hash_algorithm=SHA256&notification_hash=...
```

| Field | Description |
|-------|-------------|
| `ipgTransactionId` | IPG transaction identifier — primary idempotency key |
| `oid` | Merchant order ID when supplied in the request |
| `orderId` | Included for some asynchronous flows |
| `chargetotal` | Processed amount |
| `currency` | Currency (ISO numeric for Connect, e.g. `978` EUR) |
| `txndatetime` | Format `yyyy:MM:dd-HH:mm:ss` |
| `storename` | Store ID — used to pick the shared secret |
| `approval_code` | Approval/result code (`Y:...` first char = approved; `N:...` not approved; `?:...` waiting) |
| `status` | `APPROVED|DECLINED|FAILED|WAITING` |
| `processor_response_code` | Backend response code (e.g. `00` for card approvals; `4000` for giropay) |
| `fail_reason` | Reason for failure (from the response field set) |
| `tdate` | Transaction identification number |
| `refnumber` | Reference number |
| `response_code_3dsecure` | 3DS classification (1-8, see 3DS section) — 3DS transactions only |
| `cardnumber`, `ccbin`, `cccountry`, `ccbrand`, `expmonth`, `expyear` | Masked card data when card payment |
| `schemeTransactionId` | Returned for stored-credentials transactions; reference value for later MIT |
| `hash_algorithm` | e.g. `HMACSHA256` |
| `notification_hash` | Integrity value to verify with the store shared secret |

(Source: `source_8.md` field table; `source_18.md` §15.1 response-field table including
`approval_code`, `oid`, `refnumber`, `status`, `txndate_processed`, `ipgTransactionId`,
`tdate`, `fail_reason`, `response_hash`, `processor_response_code`, `fail_rc`.)

### `notification_hash` verification

Current gateway contract (Source: `source_8.md`):

1. Concatenate, in this order, `chargetotal | currency | txndatetime | storename | approval_code`
   (**pipe-separated**; do not add spaces; `storename` = IPG store ID).
2. Compute `HMAC-<hash_algorithm>(message, key=<shared secret>)`, Base64 if configured
   that way, and compare to `notification_hash` with constant-time compare.
3. Reject the notification on missing/mismatched hash (HTTP 4xx); accept with 2xx only
   after durable persistence; return 5xx when you want a retry.

Connect guide legacy contract (Source: `source_18.md` §15.3): the same pipe-joined
concatenation with shared secret as the HMAC key. For **recurring** transactions the
shared secret is instead part of the signed string:
`chargetotal+rcpSharedSecret+currency+txndatetime+storename+approval_code` hashed with
SHA-256 (default).

Browser redirect responses (to `responseSuccessURL`/`responseFailURL`) carry the same
fields plus `response_hash`, computed over
`approval_code|chargetotal|currency|txndatetime|storename` (Source: `source_18.md` §15.2).

### Event types

Single async "transaction result" event per transaction; key lifecycle values via `status`:

| `status` | Meaning |
|----------|---------|
| `APPROVED` | Final; authorisation/capture succeeded |
| `DECLINED` / `FAILED` | Final; unsuccessful (declined by issuer/fraud gates, or malformed/invalid content) |
| `WAITING` | Interim for async APMs; a later notification or PSync/RSYNC will resolve it |

Authipay does not expose a type-tagged event catalogue in the fetched sources; treat
"transaction result notification" as one event (there is no separate refund/void event name
in the documents — match by `ipgTransactionId` and by the presence of amount/status fields).
---

## Status Mappings

### Transaction state → hyperswitch-style AttemptStatus

| Connector `transactionState` | `transactionResult` | Meaning → mapping |
|------------------------------|---------------------|-------------------|
| `AUTHORIZED` | `APPROVED` | Pre-auth approved → **Authorized** |
| `CAPTURED` / `SETTLED` / `COMPLETED_GET` | `APPROVED` | Captured → **Charged** |
| `DECLINED` | `DECLINED` | → **Failure** |
| `CHECKED` | — | Card verification passed → **Authorized** (verification) |
| `VOIDED` | — | Void succeeded → **Voided** |
| `WAITING` | `WAITING` | In-flight (3DS redirect pending, or APM) → **AuthenticationPending** (3DS) / **Pending** |
| `INITIALIZED` / `READY` / `TEMPLATE` / `PENDING` | — | → **Pending** (non-terminal; poll) |

For returns: a ReturnTransaction response maps to refund status: `transactionType=RETURN`
+ `APPROVED` → succeeded; `DECLINED/FAILED` → failure; `WAITING` → pending. Do not treat
initial RSync retrieval errors as terminal (see repo convention
"RSync must not terminally fail a refund").

### AVS response (`processor.avsResponse`, Source: `source_11.md` AVSResponse)

| Field | Values | Meaning |
|-------|--------|---------|
| `streetMatch` | `Y | N | NO_INPUT_DATA | NOT_CHECKED` | Street matches file |
| `postalCodeMatch` | `Y | N | NO_INPUT_DATA | NOT_CHECKED` | Postal code matches file |
| `associationAvsResponse` | raw issuer code, e.g. `Y` | Full raw code; legacy XML guide shows 3-char AVS strings like `YYY`/`PPX` (Source: `source_17.md`) |

### CVV response (`processor.securityCodeResponse`)

Enum: `MATCHED | NOT_MATCHED | NOT_PROCESSED | NOT_PRESENT | NOT_CERTIFIED | NOT_CHECKED`.
(Source: `source_11.md` ProcessorData.securityCodeResponse.)

### Issuer / association decline codes (`processor.associationResponseCode`)

The sources do not provide an exhaustive table; the OpenAPI documents the field and
examples (`"000"`, `"Requested function not supported"`). Standard association codes the
connector should surface via network decline codes:

| association code family | Meaning → suggested standard decline mapping |
|-------------------------|----------------------------------------------|
| `00` / `000` | Approved |
| `05` | Do not honor → `do_not_honor` |
| `51` | Insufficient funds → `insufficient_funds` |
| `54` / `33` | Expired card → `expired_card` |
| `41` / `43` | Lost / stolen card → `lost_or_stolen_card` |
| `57` | Transaction not permitted → `transaction_not_permitted` |
| `61` / `65` | Activity/amount limit exceeded → `card_velocity_exceeded` |
| `82` | Incorrect CVV → `incorrect_cvc` |
| `85` | No reason to decline / account verification OK (verification request) |
| `91` / `96` | Issuer unavailable / system error → `issuer_unavailable` / `generic_decline` |
| `1A`/`1B` (authentication required class) | SCA required → `authentication_required` |

NOTE: map from `processor.associationResponseCode` when present, else
`processor.responseCode`. `processor.merchantAdviceCodeIndicator` (pattern `[0-9]{2}`)
with `merchantAdviceMessage` may add scheme advice (e.g. `01` accounts). (Source:
`source_11.md` ProcessorData.) **Unverified** — exact strings per Authipay should be
reconciled with the Operations response-code handbook when available; the OpenAPI itself
only gives examples.
---

## Extension Feature Reference (request-building blocks)

### A. CIT / MIT recurring and card-on-file (`storedCredentials`)

`StoredCredential` on sale/preauth/credit/void/return bodies (Source: `source_11.md`):

```json
{
  "requestType": "PaymentCardSaleTransaction",
  "transactionAmount": { "total": 12.04, "currency": "EUR" },
  "paymentMethod": { "paymentCard": { "number": "…", "expiryDate": {"month":"12","year":"25"} } },
  "storedCredentials": {
    "sequence": "FIRST",
    "scheduled": true,
    "initiator": "CARDHOLDER"
  }
}
```

| Field | Type | Values |
|-------|------|--------|
| `sequence` | enum, required | `FIRST` (setup / CIT initial) or `SUBSEQUENT` (repeat) |
| `scheduled` | boolean, required | true = scheduled/installment; false = unscheduled |
| `initiator` | enum | `MERCHANT` (MIT) or `CARDHOLDER` (CIT) — default inferred from sequence when absent |
| `referencedSchemeTransactionId` | string ≤50 | scheme transaction ID from the FIRST response (`schemeTransactionId`); required/expected on SUBSEQUENT |
| `referencedTransactionLinkIdentifier` | string ≤36 | scheme TLID from FIRST response (`transactionLinkIdentifier`) |
| `indicatorSubcategory` | enum | CARDHOLDER: `CREDENTIAL_ON_FILE_FIRST`, `CREDENTIAL_ON_FILE_SUBSEQUENT`, `STANDING_ORDER`, `SUBSCRIPTION`, `INSTALLMENT`; MERCHANT: `UNSCHEDULED_CREDENTIAL_ON_FILE`, `STANDING_ORDER`, `SUBSCRIPTION`, `INSTALLMENT`, `PARTIAL_SHIPMENT`, `DELAYED_CHARGE`, `NO_SHOW_CHARGE`, `RESUBMISSION` |

The FIRST response returns `schemeTransactionId` and `transactionLinkIdentifier`
(Source: `source_11.md` TransactionResponse); persist them for the SUBSEQUENT calls
(the **NTID** = `schemeTransactionId`).

Legacy XML equivalents for the same flags: `recurringType: FIRST|REPEAT` (+ `STANDIN`),
`unscheduledCredentialOnFileType: FIRST|CARDHOLDER_INITIATED|MERCHANT_INITIATED`,
`ReferencedSchemeTransactionId` (Source: `source_17.md` §13.1.5/13.1.6 and §5.1.9;
`source_18.md` fields `referencedSchemeTransactionId`, `unscheduledCredentialOnFileType`).
<!-- Note: REST API uses storedCredentials.sequence/initiator instead of recurringType. -->

Mapping for the connector:

- **SetupMandate (CIT, sequence=FIRST):** sale/preauth or $0-verification with
  `storedCredentials { sequence: FIRST, scheduled: <per mandate type>, initiator: CARDHOLDER }`,
  and typically `createToken { reusable: true, declineDuplicates: <config> }` to get
  `paymentToken.value` back; capture the returned `schemeTransactionId` as the mandate/NTID.
- **RepeatPayment (MIT, sequence=SUBSEQUENT):** sale/preauth with
  `storedCredentials { sequence: SUBSEQUENT, scheduled: false, initiator: MERCHANT,
  referencedSchemeTransactionId: <NTID from FIRST> }`; card instrument can be the
  gateway payment token (`paymentMethod.paymentToken.value`, optional `securityCode`).
  CVV is not sent on MIT (issuer decline risk); expiryDate is not required for
  transactions with recurring flags (Source: `source_11.md` Expiration: "Required for
  normal transactions except for payment with 'RECURRING' flags").

### B. $0 Card Verification

Two documented routes (Source: `source_11.md`):

1. `POST /account-verification` with `PaymentCardVerificationRequest` (no amount; returns
   `accountVerificationResponse`).
2. `POST /payments` sale/preauth with `transactionAmount.total: 0` and
   `authenticationRequest: { authenticationType: "Secure3DAuthenticationRequest", messageCategory: "02" }`
   (`ThreeDSEmvCoMessageCategory=02` = Non-Payment Authentication) when SCA on the
   verification is required.

Combined with `createToken { reusable: true }` this is also how a PCI-free vaulted
card is registered with verification:
`POST /payments` sale, `$0`, `createToken.reusable: true` → `paymentToken.value` +
`accountVerification: true` in response.
(Source: `source_11.md` PaymentTokenDetails.accountVerification.)

### C. Billing / Shipping address blocks

`order.billing` (Billing; Source: `source_11.md`):

```json
"billing": {
  "name": "John Doe", "firstName": "John", "lastName": "Doe",  // first/last only for AMEX
  "customerId": "1234567890",
  "birthDate": "1980-01-31",
  "contact": { "phone": "…", "mobilePhone": "…", "fax": "…", "email": "john@test.com" },
  "address": { "company": "…", "address1": "123 Main St.", "address2": "Suite 123",
               "city": "Sandy Springs", "region": "Georgia", "postalCode": "30303",
               "country": "USA" }
}
```

`order.shipping` (Shipping): `name`, `contact`, same `address` model. Country accepts
ISO-3166-1 alpha-2/alpha-3/numeric or full name (Source: `source_11.md` Address).
Billing/Shipping field details also independently documented at
https://developer.fiserv.com/product/IPGNA/docs/additionalInfo/BillingShippingFields.md
(`source_1.md` — the scraped body was effectively empty; rely on the OpenAPI).

### D. L2/L3 purchase cards (`order.purchaseCard`)

`PurchaseCards` (Source: `source_11.md`):

- `Level2`: `customerReferenceID` (≤17), `supplierInvoiceNumber` (≤30),
  `supplierVATRegistrationNumber` (≤30), `vatDocumentationIndicator` (`1|2`),
  `totalDiscountAmountAndRate`, `vatShippingAmountAndRate`, `dutyAmountAndRate`
  (each `{ amount, rate }`).
- `Level3.lineItems[]` (required on Level3; ≤100 items): `commodityCode` (≤4),
  `productCode` (≤20), `description` (≤30), `quantity` (int ≥1), `unitMeasure` (≤3),
  `unitPrice` (number, 3-decimal), `vatAmountAndRate`, `discountAmountAndRate`,
  `lineItemTotal` (= unitPrice*quantity − discount; see `source_18.md` §14 calculation note).

### E. Statement narrative / soft descriptor (`order.softDescriptor` or on ReturnTransaction)

`SoftDescriptor` (Source: `source_11.md`):

```json
"softDescriptor": {
  "dynamicMerchantName": "Merchant XYZ",
  "url": "https://www.firstdata.com",
  "customerServiceNumber": "8045018787",   // digits only, ≤10
  "mcc": "7311",                            // 4-digit, overrides merchant MCC for this txn
  "dynamicAddress": { /* Address */ }
}
```

The legacy Connect form field equivalents such as `bname`/statement narrative text are
superseded by this object in REST (Source: `source_18.md` for form names).

### F. merchantTransactionId passthrough semantics

`merchantTransactionId` (≤40 chars) is merchant-supplied per transaction and is echoed
verbatim in the response. It is the caller's correlation ID but is **not** the idempotency
key (`Client-Request-Id` is). The legacy API exposes it as
`MerchantTransactionId` and allows referencing a previous transaction in secondary
operations via `ReferencedMerchantTransactionId` in place of `IpgTransactionId` (Source:
`source_17.md` §5.1.9 area; `source_18.md` `merchantTransactionId` /
`referencedMerchantTransactionID` fields). In REST, reference the parent by path
`{transaction-id}` = `ipgTransactionId`; `merchantTransactionId` rides along as an
annotated passthrough, not as a lookup key (no documented REST "find by merchantTransactionId").
---

## API Call Sequences

### Authorize — Card (existing; auto & manual capture)

```
Step 1: POST /payments
        Input: requestType=PaymentCardSaleTransaction (auto) or PaymentCardPreAuthTransaction (manual) — USER_PROVIDED
               transactionAmount/currency — USER_PROVIDED
               paymentMethod.paymentCard — USER_PROVIDED
               merchantTransactionId — USER_PROVIDED
               order.billing/shipping — USER_PROVIDED
        Returns: ipgTransactionId, transactionState (CAPTURED|AUTHORIZED|DECLINED|WAITING),
                 orderId, processor.*, authenticationResponse (if 3DS requested)
```

### Capture (existing)

```
Step 1: POST /payments/{ipgTransactionId}
        Input: requestType=PostAuthTransaction — from prior Authorize response.transactionType=PREAUTH
               transactionAmount — USER_PROVIDED (or full pre-auth amount)
        Returns: transactionState → CAPTURED step → Charged
```

### PSync (existing)

```
Step 1: GET /payments/{ipgTransactionId}
        Input: path id = from Authorize response.ipgTransactionId
        Returns: TransactionResponse with current transactionState/transactionResult
```

### Void / VoidPC (existing)

```
Step 1: POST /payments/{ipgTransactionId}
        Input: requestType=VoidPreAuthTransactions (pre-auth) or VoidTransaction (captured same-day)
        Returns: transactionState → VOIDED
```

### Refund (existing) + RSync

```
Step 1: POST /payments/{ipgTransactionId}   requestType=ReturnTransaction, transactionAmount
        Returns: ipgTransactionId of the RETURN, transactionResult APPROVED/WAITING
Step 2: GET /payments/{returnIpgTransactionId}   (for async settlement of the return)
        Returns: current transactionResult for the RETURN
```

### ThreeDS / Card (NEW)

Integrated gateway-managed 3DS 2.x (also covers `PreAuthenticate/Authenticate/PostAuthenticate`
marker wiring):

```
Flow: ThreeDS / Card

Step 1: POST /payments
        Input: requestType=PaymentCardSaleTransaction (or PreAuth) — USER_PROVIDED
               paymentMethod.paymentCard — USER_PROVIDED
               transactionAmount — USER_PROVIDED
               authenticationRequest = { authenticationType: "Secure3DAuthenticationRequest",
                                         termURL, methodNotificationURL, challengeIndicator?,
                                         messageCategory=01 } — termURL = CONFIGURATION/USER_PROVIDED
        Returns: ipgTransactionId; transactionState=WAITING; authenticationResponse.params{acsURL, cReq (or payerAuthenticationRequest), sessionData}
        Charges: NO — "The result of the authentication will be sent to this URL" implies the request only starts authentication when authenticationRequest is present; money moves only after PATCH (Step 3) completes
        Redirect: YES — authenticationResponse.params.acsURL (with cReq/sessionData or PaReq)

Step 2: (browser) POST to ACS at authenticationResponse.params.acsURL; ACS posts back to termURL
        Input: CReq / PaReq from Step 1 — PREVIOUS_API
        Returns: (browser POST to our termURL) cRes (and for 3DS1: PaRes + MD) — USER_PROVIDED by redirection
        Charges: NO — purely the ACS challenge round-trip out of band
        Redirect: YES — this step IS the redirect; result arrives at termURL

Step 3: PATCH /payments/{ipgTransactionId}
        Input: authenticationType=Secure3DAuthenticationUpdateRequest — static
               acsResponse.cRes = browser POST body from ACS — PREVIOUS_API (Step 2)
               methodNotificationStatus — USER_PROVIDED/system
        Returns: final TransactionResponse: transactionState AUTHORIZED|CAPTURED|DECLINED,
                 secure3dResponse {responseCode3dSecure, authenticationValue(CAVV), directoryServerTransactionId}
        Charges: YES — this is the call that completes the authorization/sale: final state becomes AUTHORIZED (preauth) or CAPTURED (sale)
        Redirect: YES(on-initiate)/NO — the redirect happened in Step 1/2 via authenticationResponse.params.acsURL
```

Split variant (maps to the 3-marker PreAuthenticate/Authenticate/PostAuthenticate layout where
needed):

```
Leg A1 (PreAuthenticate — authentication-request leg; Charges: NO; Redirect: YES via
        authenticationResponse.params.acsURL):
        POST /payments   requestType=PaymentCardPayerAuthTransaction
        + authenticationRequest { Secure3DAuthenticationRequest, termURL, methodNotificationURL }
        Returns: ipgTransactionId; authenticationResponse.params (redirect payload); state WAITING
Leg A2 (Authenticate — out-of-band ACS round trip; Charges: NO; Redirect: browser → ACS → termURL
        POST with CRes/PaRes)
Leg A3 (Authenticate continue; Charges: NO; Redirect: NO):
        PATCH /payments/{payer-auth-ipgTransactionId}
        { authenticationType: "Secure3DAuthenticationUpdateRequest", acsResponse.cRes }
        Returns: completed authentication, stored for reference
Leg B (PostAuthenticate = charging leg; Charges: YES; Redirect: NO):
        POST /payments   requestType=PaymentCardSaleTransaction
        With ipgTransactionId referencing the payer-auth id (PrimaryTransaction.ipgTransactionId)
        OR with authenticationResult { Secure3DAuthenticationResult, cavv, xid, dsTransactionId, ... }
        when 3DS was externally managed.
```

Note: in the split variant the CHARGING call is Leg B (the sale/preauth POST); the
payer-auth legs A1–A3 are authentication markers and move no money.

(Sources: `source_11.md` Secure3DAuthenticationRequest/Response/UpdateRequest/Result +
PaymentCardPayerAuthTransaction; `source_18.md` §8.1.)

### $0 Card Verification

```
Step 1: POST /account-verification   requestType=PaymentCardVerificationRequest,
        paymentCard + billingAddress + merchantTransactionId
        Returns: TransactionResponse with accountVerificationResponse (verified)
— OR —
Step 1': POST /payments   requestType=PaymentCardSaleTransaction,
        transactionAmount.total=0, authenticationRequest{authenticationType=Secure3DAuthenticationRequest, messageCategory="02", termURL, methodNotificationURL}
        then Steps exactly as "ThreeDS / Card" (Redirect + PATCH continue).
```

### SetupMandate / RepeatPayment (CIT/MIT recurring)

```
SetupMandate (first, CIT):
Step 1: POST /payments   requestType=PaymentCardSaleTransaction (or $0 variant),
        storedCredentials{sequence=FIRST, scheduled, initiator=CARDHOLDER},
        createToken{reusable=true, declineDuplicates=<opt>}
        Returns: ipgTransactionId, schemeTransactionId (NTID — persist), transactionLinkIdentifier,
                 paymentToken.value (persist)

RepeatPayment (subsequent, MIT):
Step 1: POST /payments   requestType=PaymentCardSaleTransaction or PaymentTokenSaleTransaction,
        storedCredentials{sequence=SUBSEQUENT, scheduled=false, initiator=MERCHANT,
        referencedSchemeTransactionId=<NTID>}, paymentMethod.paymentToken{value=<token>}
        (securityCode omitted, expiryDate optional on recurring flags)
        Returns: transactionState CAPTURED|DECLINED
```

### IncomingWebhook (NEW — per-transaction S2S notification)

```
Step 1: IPG → merchant endpoint: POST (application/x-www-form-urlencoded) with
        ipgTransactionId, oid, chargetotal, currency, txndatetime, storename,
        approval_code, status, hash_algorithm, notification_hash, (fail_reason,
        processor_response_code, tdate, refnumber, ...)
        — Parse: EventService.ParseEvent = stateless field parse + extract ipgTransactionId
Step 2: Merchant verifies: HMAC(shared secret) over
        chargetotal|currency|txndatetime|storename|approval_code (hash from notification_hash;
        algorithm per hash_algorithm)
        — Verify: EventService.HandleEvent verify_webhook_source
Step 3: Merchant processes: map status → payment status, de-duplicate by ipgTransactionId,
        apply state transitions (APPROVED>never overwrite with older state)
        — Process: EventService.HandleEvent process_*_webhook
```

(Sources: `source_8.md`, `source_10.md`, `source_18.md` §15.3.)
---

## Field Dependency Analysis

### Authorize (PaymentCardSaleTransaction / PaymentCardPreAuthTransaction)

| Field | Category | Reasoning | Source API | Source Response Field |
|-------|----------|-----------|------------|-----------------------|
| `requestType` | CONFIGURATION-per-flow | Chosen by flow+capture method | - | - |
| `transactionAmount.total/.currency` | USER_PROVIDED | Amount & currency of payment | - | - |
| `paymentMethod.paymentCard.number/.securityCode/.expiryDate` | USER_PROVIDED | Card data | - | - |
| `merchantTransactionId` | USER_PROVIDED | Merchant correlation ID | - | - |
| `transactionOrigin` | CONFIGURATION | `ECOM` for e-commerce | - | - |
| `storeId`, `userId` | CONFIGURATION | Store config | - | - |
| `order.billing`, `order.shipping` | USER_PROVIDED | From shopper/checkout data | - | - |
| `order.purchaseCard` (L2/L3) | USER_PROVIDED | B2B card data | - | - |
| `order.softDescriptor` | CONFIGURATION/USER_PROVIDED | Per-merchant or per-transaction narrative | - | - |
| `storedCredentials` | USER_PROVIDED + PREVIOUS_API | sequence/initiator chosen by flow; `referencedSchemeTransactionId` = FIRST response.schemeTransactionId | POST /payments (FIRST) | `schemeTransactionId` |
| `createToken` | CONFIGURATION | Token-on-file request | - | - |
| `authenticationRequest.termURL` | CONFIGURATION | Merchant's 3DS result endpoint (URL where ACS posts results) | merchant config | - |
| `authenticationRequest.methodNotificationURL` | CONFIGURATION | 3DS method iframe callback | merchant config | - |
| HMAC auth headers (`Api-Key`, `Client-Request-Id`, `Timestamp`, `Message-Signature`) | CONFIGURATION | Creds + per-request derived | - | - |

### Capture / Void / Refund

| Field | Category | Reasoning | Source API | Source Response Field |
|-------|----------|-----------|------------|-----------------------|
| `{transaction-id}` path param | PREVIOUS_API | Parent transaction to act on | POST /payments | `ipgTransactionId` |
| `requestType` | CONFIGURATION | flow fixed: PostAuthTransaction / VoidPreAuthTransactions / VoidTransaction / ReturnTransaction | - | - |
| `transactionAmount` | USER_PROVIDED | Partial capture/refund amount (absent for full void) | - | - |

### PSync / RSync

| Field | Category | Reasoning | Source API | Source Response Field |
|-------|----------|-----------|------------|-----------------------|
| path transaction id | PREVIOUS_API | ID of the txn or return to query | POST /payments (or POST secondary) | `ipgTransactionId` |

### ThreeDS / Card

| Field | Category | Reasoning | Source API | Source Response Field |
|-------|----------|-----------|------------|-----------------------|
| `authenticationRequest.authenticationType` | CONFIGURATION | static `Secure3DAuthenticationRequest` | - | - |
| `authenticationRequest.termURL` | CONFIGURATION | Merchant callback URL for ACS redirect result | merchant config | - |
| `authenticationRequest.messageCategory` | CONFIGURATION | `01` normal payment, `02` non-payment/$0 verification | - | - |
| `authenticationResponse.params.acsURL/.cReq/.sessionData` (response) | PREVIOUS_API | Drives shopper redirect | POST /payments (Step 1) | `authenticationResponse` |
| `acsResponse.cRes` (PATCH input) | PREVIOUS_API | CRes posted by ACS to termURL | termURL redirect callback | browser POST body |
| `paymentMethod.paymentCard` in initiate | USER_PROVIDED | card data | - | - |
| `merchantTransactionId` | USER_PROVIDED | passthrough | - | - |

### $0 Card Verification

| Field | Category | Reasoning | Source API | Source Response Field |
|-------|----------|-----------|------------|-----------------------|
| `requestType` | CONFIGURATION | `PaymentCardVerificationRequest` or ZERO-amount sale + messageCategory=02 | - | - |
| `paymentCard` | USER_PROVIDED | - | - | - |
| `billingAddress` | USER_PROVIDED | - | - | - |

### SetupMandate / RepeatPayment

| Field | Category | Reasoning | Source API | Source Response Field |
|-------|----------|-----------|------------|-----------------------|
| `storedCredentials.sequence` | CONFIGURATION | FIRST vs SUBSEQUENT by flow | - | - |
| `storedCredentials.initiator` | CONFIGURATION | CARDHOLDER (CIT) / MERCHANT (MIT) | - | - |
| `storedCredentials.referencedSchemeTransactionId` | PREVIOUS_API | NTID from FIRST | SetupMandate's POST /payments | `schemeTransactionId` |
| `paymentMethod.paymentToken.value` | PREVIOUS_API | Token from FIRST (or standalone tokenize) | POST /payments or /payment-tokens | `paymentToken.value` |
| expiryDate | USER_PROVIDED but often optional | docs: not required with 'RECURRING' flags | - | - |

### IncomingWebhook

| Field | Category | Reasoning | Source API | Source Response Field |
|-------|----------|-----------|------------|-----------------------|
| `notification_hash` (verify) | CONFIGURATION | shared secret per store from Authipay config | - | - |
| `ipgTransactionId` | — | event key; dedupe with idempotent persistence | IPG POST | body field |
| `status` / `approval_code` | — | event outcome | IPG POST | body field |
| `chargetotal`, `currency`, `txndatetime`, `storename` | — | hash inputs, status context | IPG POST | body fields |

---

## UNDECIDED Fields

### 1. `termURL` and `methodNotificationURL` (ThreeDS / Card / POST /payments)
**Specification says:** termURL = "The result of the authentication will be sent to this URL. If not provided, a term URL will be dynamically generated."; methodNotificationURL = "The 3DS method iframe and transaction ID will be sent here." (Source: `source_11.md`.)
**Question:** Which URLs should the connector send?
  a) Router-provided `return_url` (use the browser redirect URL supplied by the platform; the term URL is where the ACS/browser returns)
  b) Store-configured static webhook/return endpoint
  **Proposed:** use the flow's redirect/return URL for `termURL`; for `methodNotificationURL` the flow's webhook or redirect URL (simulation: same host+path family as termURL) — the docs allow the same URL.

### 2. `messageCategory` for $0 verification (SetupMandate + $0 verification / POST /payments)
**Specification says:** `messageCategory` enum `01` payment authentication, `02` non-payment authentication, `80` MC data only. (**Source: `source_11.md`.**)
**Question:** $0 verification should use which message category?
  a) `02` Non-Payment — matches "ThreeDSEmvCoMessageCategory=02" the integration asked for
  b) N/A — use `POST /account-verification` instead and don't send `authenticationRequest`
  **Proposed:** (a) when the mandate setup flow is asked to run both verification and 3DS; (b) when it's a bare card check.

### 3. `Client-Request-Id` reuse on retries
**Specification says:** Client-Request-Id is the idempotency key; unique per request. (Source: `source_13.md`.)
**Question:** Should retry of the same logical operation reuse the same Client-Request-Id?
  a) Yes — exact retries reuse (idempotency semantics)
  b) No — fresh UUID per try; idempotency handled elsewhere (transaction result lookup)
  **Proposed:** (a) for literal HTTP retries of the same operation; (b) when the client retries a different operation attempt.

### 4. `orderId` generation
**Specification says:** "If not supplied by client, IPG will generate." (Source: `source_11.md`.)
**Question:** Should the connector always generate `order.orderId`?
  a) Yes — generate from merchant_reference_id / resource_id to make downstream order queries work
  b) No — leave to IPG
  **Proposed:** (a) when the platform supplies a reference id; otherwise leave empty.

### 5. Response-path choice between `transactionState` and `transactionResult`
**Specification says:** `transactionStatus` is deprecated → favour `transactionState` (financial state) and `transactionResult` (operation status). (Source: `source_11.md`.)
**Question:** Status derivation precedence?
  a) transactionResult first, transactionState as tiebreaker
  b) transactionState primary; transactionResult only for multi-queue logic
  **Proposed:** (b) — "unless transaction status that represent financial status of the transaction".

---

## Status Mappings — Error → ErrorResponse mapping (recommended)

| HTTP | Body signals | recommended UCS mapping |
|------|--------------|-------------------------|
| 400  | error.code=4xxx validation | attempt_status=Failure |
| 401/403 | auth failures | attempt_status=Failure with technical details preserved |
| 404 | unknown transaction id | Failure |
| 409 | gateway rules conflict (e.g. order/issue state) | Failure |
| 422 | processor declined | Failure unless `processor.associationResponseCode` indicates retryable |
| 429 | rate limit | Failure with retry hint |
| 5xx | endpoint/comms | Failure (retryable) |

The existing `build_error_response` deserialises shape (b) — keep and, when the body is shape
(a) (`{"errors": [...]}`), map first error title/detail into message/reason instead of failing
deserialisation. (Source: `source_3.md` + authipay.rs.)

---

## Amendment Log

| Who | When (UTC) | Brief | Sections changed | What changed |
|-----|------------|-------|------------------|--------------|
| techspec | 2026-09-23 | REGENERATE (authipay-d68a8d), FLOWS=ThreeDS/Card,IncomingWebhook | Full document | Regenerated from 18 fetched sources (`source_1.md`..`source_18.md`) with extension-focused coverage of 3DS, webhooks, $0 verification, CIT/MIT, L2/L3, soft descriptor, billing/shipping, AVS/CVV, decline codes, merchantTransactionId. Signs supplement the existing core specs for Authorize/Capture/PSync/Void/VoidPC/Refund/RSync. |
