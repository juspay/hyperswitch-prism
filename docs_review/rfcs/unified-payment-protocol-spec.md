# RFC: Unified Payment Protocol (UPP)

| Field       | Value                                    |
|-------------|------------------------------------------|
| **Status**  | Draft                                    |
| **Created** | 2026-05-15                               |
| **Authors** | Natarajan Kannan, Arun Raj               |

---

## 1. Abstract

The Unified Payment Protocol (UPP) defines a standardized, connector-agnostic interface for processing payments across 110+ payment processors and 100+ payment method types. It provides a single gRPC-based API surface that normalizes the lifecycle of payments — from authorization through capture, refund, dispute, and recurring billing — while abstracting away connector-specific behaviors behind a consistent set of services, messages, and state machines.

This document specifies the protocol's architecture, service contracts, message schemas, state models, and behavioral semantics.

---

## 2. Motivation

Payment processing is fragmented. Every connector (Stripe, Adyen, PayPal, Razorpay, etc.) exposes a different API shape, uses different terminology, returns different error formats, and implements different payment flows. Integrating with even a handful of connectors requires significant per-connector engineering effort.

UPP solves this by defining:

- **A single service interface** that works identically regardless of which connector processes the payment.
- **A unified payment method taxonomy** covering cards, digital wallets, bank transfers, direct debits, BNPL, vouchers, crypto, and more.
- **A canonical state machine** for payments, refunds, disputes, and mandates.
- **Structured error reporting** with unified, issuer, and connector-level detail.
- **Secure-by-default data handling** with `SecretString` wrappers for all sensitive fields.

---

## 3. Terminology

| Term | Definition |
|------|-----------|
| **Connector** | A third-party payment processor (e.g., Stripe, Adyen, Braintree). |
| **Payment Method** | A specific instrument used to pay (e.g., a Visa card, UPI, Apple Pay). |
| **Authorization** | A hold on funds without transferring them. |
| **Capture** | The finalization of an authorized transaction, initiating fund transfer. |
| **Void** | Cancellation of an authorization before capture. |
| **Reversal** | Reversal of a captured payment before settlement. |
| **Mandate** | A standing instruction authorizing future charges (recurring payments). |
| **3DS** | 3-D Secure — a cardholder authentication protocol for online transactions. |
| **Webhook** | An inbound HTTP callback from a connector notifying of a state change. |
| **Minor Units** | The smallest currency unit (e.g., cents for USD, paise for INR). All monetary amounts in UPP are expressed in minor units. |

---

## 4. Architecture Overview

UPP is structured as a set of gRPC services that collectively manage the full payment lifecycle:

```
┌──────────────────────────────────────────────────────────────────────┐
│                        Unified Payment Protocol                      │
│                                                                      │
│  ┌─────────────┐  ┌──────────────┐  ┌────────────────────────────┐   │
│  │  Payment    │  │  Refund      │  │  Dispute                   │   │
│  │  Service    │  │  Service     │  │  Service                   │   │
│  ├─────────────┤  ├──────────────┤  ├────────────────────────────┤   │
│  │ Authorize   │  │ Get          │  │ SubmitEvidence             │   │
│  │ Capture     │  │ HandleEvent  │  │ Get                        │   │
│  │ Void        │  │              │  │ Defend                     │   │
│  │ Reverse     │  │              │  │ Accept                     │   │
│  │ Get         │  │              │  │ HandleEvent                │   │
│  │ Refund      │  │              │  │                            │   │
│  │ CreateOrder │  │              │  │                            │   │
│  │ IncrAuth    │  │              │  │                            │   │
│  │ SetupRecur  │  │              │  │                            │   │
│  │ VerifyRedir │  │              │  │                            │   │
│  │ HandleEvent │  │              │  │                            │   │
│  └─────────────┘  └──────────────┘  └────────────────────────────┘   │
│                                                                      │
│  ┌─────────────┐  ┌──────────────┐  ┌────────────────────────────┐   │
│  │  Recurring  │  │  Payment     │  │  Merchant                  │   │
│  │  Payment    │  │  Method      │  │  Authentication            │   │
│  │  Service    │  │  Auth Svc    │  │  Service                   │   │
│  ├─────────────┤  ├──────────────┤  ├────────────────────────────┤   │
│  │ Charge      │  │ PreAuth      │  │ CreateAccessToken          │   │
│  │ Revoke      │  │ Authenticate │  │ CreateSessionToken         │   │
│  │             │  │ PostAuth     │  │ CreateSdkSessionToken      │   │
│  └─────────────┘  └──────────────┘  └────────────────────────────┘   │
│                                                                      │
│  ┌─────────────┐  ┌──────────────┐  ┌────────────────────────────┐   │
│  │  Customer   │  │  PayMethod   │  │  Composite Payment         │   │
│  │  Service    │  │  Service     │  │  Service                   │   │
│  ├─────────────┤  ├──────────────┤  ├────────────────────────────┤   │
│  │ Create      │  │ Tokenize     │  │ CompositeAuthorize         │   │
│  │             │  │              │  │ CompositeGet               │   │
│  └─────────────┘  └──────────────┘  └────────────────────────────┘   │
│                                                                      │
│  ┌─────────────┐   ┌──────────────┐                                  │
│  │  Event      │   │  Health      │                                  │
│  │  Service    │   │  Service     │                                  │
│  ├─────────────┤   ├──────────────┤                                  │
│  │ HandleEvent │   │ Check        │                                  │
│  └─────────────┘   └──────────────┘                                  │
└──────────────────────────────────────────────────────────────────────┘
                              │
                    ┌─────────┴──────────┐
                    │  Connector Layer   │
                    │  (110+ connectors) │
                    └────────────────────┘
```

---

## 5. Core Data Types

### 5.1 Money

All monetary values in UPP are represented using the `Money` message, which uses integer minor units to avoid floating-point precision issues.

```protobuf
message Money {
  int64 minor_amount = 1;   // Amount in smallest currency unit (e.g., 1000 = $10.00 USD)
  Currency currency = 2;     // ISO 4217 currency code
}
```

UPP supports 160+ currencies following the ISO 4217 standard (AED through ZWL).

### 5.2 SecretString

Sensitive data is wrapped in `SecretString` to enable automatic masking in logs, traces, and error messages. All PII, credentials, card numbers, and tokens use this wrapper.

```protobuf
message SecretString {
  string value = 1;
}
```

Fields wrapped in `SecretString` include: email addresses, card numbers, CVVs, expiry dates, API keys, tokens, address lines, phone numbers, and authentication credentials.

### 5.3 Customer

```protobuf
message Customer {
  optional string name = 1;
  optional SecretString email = 2;
  optional string id = 3;
  optional string connector_customer_id = 4;
  optional string phone_number = 5;
  optional string phone_country_code = 6;
}
```

### 5.4 Address

```protobuf
message Address {
  optional SecretString first_name = 1;
  optional SecretString last_name = 2;
  optional SecretString line1 = 3;
  optional SecretString line2 = 4;
  optional SecretString line3 = 5;
  optional SecretString city = 6;
  optional SecretString state = 7;
  optional SecretString zip_code = 8;
  optional CountryAlpha2 country_alpha2_code = 9;
  optional SecretString email = 10;
  optional SecretString phone_number = 11;
}

message PaymentAddress {
  Address shipping_address = 1;
  Address billing_address = 2;
}
```

### 5.5 Connector State

Connector state carries session-scoped data that must be round-tripped between requests in multi-step flows (e.g., access tokens, connector-scoped customer IDs).

```protobuf
message ConnectorState {
  optional AccessToken access_token = 1;
  optional string connector_customer_id = 2;
}

message AccessToken {
  SecretString token = 1;
  optional int64 expires_in_seconds = 2;
  optional string token_type = 3;
}
```

---

## 6. Payment Method Taxonomy

UPP defines a comprehensive payment method taxonomy organized into categories. Each payment method is represented as a variant in a `oneof` block within the `PaymentMethod` message.

### 6.1 Categories

| Range | Category | Examples |
|-------|----------|----------|
| 1–9 | Cards | Credit/debit cards, card redirects, tokenized cards |
| 10–29 | Digital Wallets | Apple Pay, Google Pay, Samsung Pay, PayPal, Amazon Pay, Revolut Pay, WeChat Pay, Alipay |
| 30–39 | UPI | UPI Collect, UPI Intent, UPI QR |
| 40–59 | Online Banking / Redirects | iDEAL, Sofort, Giropay, EPS, Przelewy24, BLIK, Trustly, Open Banking, Interac, Bizum |
| 60–69 | Mobile Payments | DuitNow |
| 70–79 | Cryptocurrency | Configurable crypto networks |
| 80–89 | Rewards & E-Vouchers | Classic Reward, E-Voucher |
| 90–99 | Bank Transfers | ACH, SEPA, BACS, Multibanco, PIX, instant transfers |
| 100–104 | Direct Debit | ACH, SEPA, BACS, BECS, SEPA Guaranteed |
| 110–112 | Buy Now, Pay Later | Affirm, Afterpay/Clearpay, Klarna |
| 120–121 | Network Transactions | Network Transaction ID, Network Token |
| 130–155 | Regional Methods | Indonesian VA transfers, local bank transfers, vouchers (Boleto, OXXO, 7-Eleven, etc.) |

### 6.2 Card Details

```protobuf
message CardDetails {
  CardNumberType card_number = 1;
  SecretString card_exp_month = 2;
  SecretString card_exp_year = 3;
  SecretString card_cvc = 4;
  optional SecretString card_holder_name = 5;
  optional string card_issuer = 6;
  optional CardNetwork card_network = 7;
  optional string card_type = 8;
  optional string card_issuing_country_alpha2 = 9;
  optional string bank_code = 10;
  optional string nick_name = 11;
}
```

**Supported Card Networks**: Visa, Mastercard, American Express, Discover, JCB, Diners Club, UnionPay, Maestro, RuPay, Interac, CartesBancaires, Elo, and more.

### 6.3 Digital Wallets

Each wallet type has a dedicated message with wallet-specific fields:

- **Apple Pay** (`AppleWallet`): Encrypted payment data from Apple's payment sheet.
- **Google Pay** (`GoogleWallet`): Tokenized payment data from Google's payment API.
- **Samsung Pay** (`SamsungWallet`): Samsung wallet payment token.
- **PayPal** (`PaypalSdkWallet`, `PaypalRedirectWallet`): SDK-based or redirect-based PayPal flows.
- **Amazon Pay** (`AmazonPayRedirectWallet`): Redirect-based Amazon Pay.
- **WeChat Pay** (`WeChatPayQrWallet`): QR code-based payment.
- **Alipay** (`AliPayRedirectWallet`): Redirect-based Alipay.

### 6.4 Payment Experiences

UPP supports multiple payment experiences per method:

| Experience | Description |
|-----------|-------------|
| `REDIRECT_TO_URL` | Customer is redirected to an external URL for payment completion |
| `INVOKE_SDK_CLIENT` | Payment is completed via a client-side SDK |
| `DISPLAY_QR_CODE` | A QR code is displayed for scanning |
| `ONE_CLICK` | Payment completes with a single click (stored credentials) |
| `LINK_WALLET` | Customer links a wallet account |
| `INVOKE_PAYMENT_APP` | A native payment app is invoked |
| `DISPLAY_WAIT_SCREEN` | A waiting screen is shown during async processing |
| `COLLECT_OTP` | An OTP is collected from the customer |

---

## 7. Service Specifications

### 7.1 PaymentService

The core service for payment lifecycle management.

#### 7.1.1 Authorize

Initiates a payment authorization — reserving funds on the customer's payment method without capturing them.

**Request fields:**
- `payment_method` — The payment instrument (card, wallet, bank transfer, etc.)
- `money` — Amount and currency
- `customer` — Customer information
- `address` — Billing and shipping addresses
- `authentication_data` — 3DS authentication results (if applicable)
- `connector_state` — State from prior steps (access tokens, customer IDs)
- `order_details` — Line-item order data (L2/L3)
- `capture_method` — `AUTOMATIC`, `MANUAL`, `MANUAL_MULTIPLE`, `SCHEDULED`, or `SEQUENTIAL_AUTOMATIC`
- `setup_mandate_details` — For setting up recurring payment mandates during authorization
- `browser_info` — Browser metadata for fraud/3DS decisions
- `metadata` — Arbitrary key-value pairs
- `connector_mandate_reference_id` — Reference for mandate-based payments
- `statement_descriptor` — Text appearing on the customer's statement
- `webhook_url`, `return_url` — URLs for async notification and redirect
- `is_extended_authorization`, `is_incremental_authorization`, `is_partial_authorization` — Authorization variant flags

**Response fields:**
- `status` — `PaymentStatus` (see Section 8.1)
- `connector_transaction_id` — The connector's transaction identifier
- `redirect_form` — Redirect data if the flow requires customer redirection
- `connector_state` — Updated state for subsequent requests
- `mandate_reference` — Mandate identifier for recurring flows
- `error` — Structured error information
- `authentication_data` — Updated 3DS data
- `network_transaction_id` — Network-level transaction identifier
- `raw_connector_request` — Raw request sent to connector (for debugging)

#### 7.1.2 Capture

Finalizes an authorized transaction, initiating the actual transfer of funds.

**Request fields:**
- `connector_transaction_id` — The transaction to capture
- `money` — Amount to capture (may differ from authorized amount for partial captures)
- `connector_state` — Session state
- `multiple_capture_data` — For `MANUAL_MULTIPLE` capture flows

**Response fields:**
- `status` — Updated payment status
- `connector_transaction_id` — Capture transaction ID
- `error` — Error details if capture failed

#### 7.1.3 Void

Cancels an authorization before capture, releasing held funds.

**Request fields:**
- `connector_transaction_id` — The authorization to void
- `connector_state` — Session state
- `void_reason` — Reason for voiding

**Response fields:**
- `status` — Updated payment status (`VOIDED` on success)
- `error` — Error details if void failed

#### 7.1.4 Reverse

Reverses a captured payment before settlement.

**Request/Response**: Similar structure to Void, operating on captured transactions.

#### 7.1.5 Refund

Initiates a full or partial refund to the customer's original payment method.

**Request fields:**
- `connector_transaction_id` — The payment to refund
- `payment_amount`, `refund_amount` — Original and refund amounts
- `connector_state` — Session state
- `webhook_url` — Notification URL for async refund updates

**Response fields:**
- `connector_refund_id` — Connector's refund identifier
- `refund_status` — `RefundStatus` (see Section 8.2)
- `error` — Error details

#### 7.1.6 Get (Sync)

Retrieves the current payment status from the connector, synchronizing local state.

**Request fields:**
- `connector_transaction_id` — The transaction to query
- `connector_state` — Session state
- `multiple_capture_sync_data` — For syncing multiple captures

**Response fields:**
- Full payment state including `status`, `connector_transaction_id`, `error`, `authentication_data`, and `mandate_reference`.

#### 7.1.7 CreateOrder

Initializes an order context in the payment processor before the customer provides payment details. Used by connectors that require order pre-creation (e.g., PayPal, Razorpay).

#### 7.1.8 IncrementalAuthorization

Increases the authorized amount on an existing authorization. Common in hospitality (hotel stays, car rentals) where the final amount is not known at authorization time.

#### 7.1.9 SetupRecurring

Establishes a recurring payment mandate — a standing instruction authorizing the merchant to charge the customer's payment method on a recurring basis.

**Response fields:**
- `mandate_reference` — The mandate identifier for future charges
- `status` — Mandate status

#### 7.1.10 VerifyRedirectResponse

Validates redirect-based payment responses to confirm authenticity and prevent replay attacks.

### 7.2 RecurringPaymentService

Manages merchant-initiated transactions (MIT) using established mandates.

#### 7.2.1 Charge

Processes a payment using a previously established mandate, without requiring the customer to re-enter payment details.

**Request fields:**
- `mandate_reference` — Reference to the established mandate
- `money` — Amount to charge
- `connector_state` — Session state
- `payment_method` — Payment method details (may be minimal for MIT)

**Response fields:**
- `status` — Payment status
- `connector_transaction_id` — Transaction identifier

#### 7.2.2 Revoke

Cancels a recurring payment mandate, stopping all future automatic charges.

### 7.3 RefundService

#### 7.3.1 Get

Retrieves the current status of a refund from the connector.

#### 7.3.2 HandleEvent

Processes refund-related webhook notifications.

### 7.4 DisputeService

Manages chargebacks and payment disputes.

#### 7.4.1 SubmitEvidence

Uploads evidence documents to contest a dispute.

**Evidence Types:**
- `CANCELLATION_POLICY` — Cancellation policy documentation
- `CUSTOMER_COMMUNICATION` — Communication records with the customer
- `CUSTOMER_SIGNATURE` — Customer signature proof
- `RECEIPT` — Transaction receipt
- `REFUND_POLICY` — Refund policy documentation
- `SERVICE_DOCUMENTATION` — Service delivery documentation
- `SHIPPING_DOCUMENTATION` — Shipping/delivery proof
- `INVOICE_SHOWING_DISTINCT_TRANSACTIONS` — Invoice clarification
- `RECURRING_TRANSACTION_AGREEMENT` — Recurring payment agreement
- `UNCATEGORIZED_FILE` — Other evidence

Evidence can be provided as binary file content (`file_content` + `file_mime_type`), as a connector-hosted file reference (`provider_file_id`), or as text (`text_content`).

#### 7.4.2 Defend

Submits a formal defense against a dispute with a reason code.

#### 7.4.3 Accept

Concedes a dispute and accepts the chargeback.

#### 7.4.4 Get

Retrieves the current status and details of a dispute.

### 7.5 PaymentMethodAuthenticationService (3DS)

Implements the 3-D Secure authentication protocol as a three-phase flow.

#### 7.5.1 PreAuthenticate

Initiates the 3DS flow. Collects device fingerprint data and prepares the authentication context.

#### 7.5.2 Authenticate

Executes the 3DS challenge (or frictionless verification). The customer may be presented with a challenge by their issuing bank.

#### 7.5.3 PostAuthenticate

Validates the authentication results with the issuing bank. Returns the final `AuthenticationData` including:
- `eci` — Electronic Commerce Indicator
- `cavv` — Cardholder Authentication Verification Value
- `threeds_server_transaction_id` — 3DS Server transaction ID
- `ds_transaction_id` — Directory Server transaction ID
- `trans_status` — Transaction status (success, failure, challenge required, etc.)
- `exemption_indicator` — SCA exemption type if applicable

**SCA Exemption Types:**
- `LOW_VALUE` — Transaction below exemption threshold
- `SECURE_CORPORATE_PAYMENT` — Corporate card payment
- `TRUSTED_LISTING` — Merchant on customer's trusted list
- `TRANSACTION_RISK_ASSESSMENT` — Low-risk per TRA
- `RECURRING_OPERATION` — Subsequent recurring payment
- `THREE_DS_OUTAGE` — 3DS service unavailable
- `SCA_DELEGATION` — SCA delegated to another party
- `LOW_RISK_PROGRAM` — Card network low-risk program

### 7.6 MerchantAuthenticationService

Manages connector-level authentication and session tokens.

#### 7.6.1 CreateAccessToken

Generates a short-lived access token for connector API authentication. The token is returned in `ConnectorState` and must be passed to subsequent requests.

#### 7.6.2 CreateSessionToken

Creates a session token that maintains state across multiple payment operations.

#### 7.6.3 CreateSdkSessionToken

Initializes wallet payment sessions for Apple Pay, Google Pay, and PayPal. Returns wallet-specific session data:

**Google Pay Session:**
- `merchant_info` — Merchant name and ID
- `allowed_payment_methods` — Supported card networks and auth methods
- `transaction_info` — Currency, amount, price status
- `shipping_address_required`, `email_required` — Data collection flags

**Apple Pay Session:**
- `payment_request_data` — Payment request with merchant capabilities, supported networks, line items
- `session_token_data` — Apple Pay session for domain validation
- `connector_merchant_id` — Connector's merchant ID for Apple Pay

**PayPal Session:**
- `session_token` — PayPal session identifier
- `client_token` — Client-side token for PayPal SDK

### 7.7 CustomerService

#### 7.7.1 Create

Creates a customer record in the payment processor, returning a `connector_customer_id` for use in subsequent operations.

### 7.8 PaymentMethodService

#### 7.8.1 Tokenize

Tokenizes a payment method for secure storage and future use. Enables one-click payments and recurring billing without re-collecting sensitive card data.

### 7.9 CompositePaymentService

Provides composite operations that combine multiple service calls into a single request for common workflows.

#### 7.9.1 CompositeAuthorize

Combines access token creation, customer creation, and payment authorization into a single atomic operation. Each sub-operation's response includes a status code, response headers, and the typed result.

```
CompositeAuthorizeRequest:
  ├── create_access_token_request
  ├── create_customer_request
  └── authorize_request

CompositeAuthorizeResponse:
  ├── create_access_token_response (with status_code, response_headers)
  ├── create_customer_response (with status_code, response_headers)
  └── authorize_response (with status_code, response_headers)
```

#### 7.9.2 CompositeGet

Combines access token refresh with payment status retrieval.

### 7.10 EventService

Processes inbound webhooks from connectors and transforms them into typed, normalized events.

**Request fields:**
- `webhook_body` — Raw webhook payload
- `webhook_headers` — HTTP headers from the webhook request
- `webhook_secrets` — Connector secrets for signature verification

**Response fields:**
- `event_type` — Normalized event type (see Section 8.5)
- `event_response` — Typed response (payment, refund, dispute, or incomplete)
- `source_verified` — Whether the webhook signature was verified
- `merchant_event_id` — Connector-assigned event identifier
- `event_status` — `COMPLETE` or `INCOMPLETE`

### 7.11 HealthService

Standard gRPC health check returning `SERVING`, `NOT_SERVING`, or `SERVICE_UNKNOWN`.

---

## 8. State Machines

### 8.1 Payment Status

Payments follow a state machine with 26 distinct states:

```
                          ┌──────────────────┐
                          │     STARTED      │
                          └────────┬─────────┘
                                   │
                    ┌──────────────┼───────────────┐
                    ▼              ▼               ▼
          ┌─────────────┐  ┌────────────┐  ┌──────────────┐
          │ PM_AWAITED  │  │CONFIRMATION│  │  DDC_PENDING │
          │             │  │  _AWAITED  │  │              │
          └──────┬──────┘  └─────┬──────┘  └──────┬───────┘
                 │               │                │
                 └────────┬──────┘                │
                          ▼                       │
                 ┌────────────────┐               │
                 │ AUTH_PENDING   │◄─────────────┘
                 └───────┬────────┘
                    ┌────┴────┐
                    ▼         ▼
           ┌────────────┐ ┌──────────┐
           │AUTH_SUCCESS│ │AUTH_FAIL │
           └─────┬──────┘ └──────────┘
                 │
                 ▼
            ┌───────────┐
            │AUTHORIZING│
            └─────┬─────┘
            ┌─────┴──────────────────────┐
            ▼                            ▼
     ┌────────────┐              ┌──────────────┐
     │ AUTHORIZED │              │ AUTH_FAILED  │
     └──┬───┬───┬─┘              └──────────────┘
        │   │   │
   ┌────┘   │   └─────────┐
   ▼        ▼             ▼
┌───────┐ ┌───────┐  ┌──────────┐
│CAPTURE│ │ VOID  │  │ INCR_AUTH│
│_INIT  │ │_INIT  │  │          │
└──┬────┘ └──┬────┘  └──────────┘
   │         │
   ▼         ▼
┌───────┐ ┌──────┐
│CHARGED│ │VOIDED│
└──┬────┘ └──────┘
   │
   ├──────────────┐
   ▼              ▼
┌──────────┐  ┌──────────────┐
│PARTIAL_  │  │AUTO_REFUNDED │
│CHARGED   │  │              │
└──────────┘  └──────────────┘
```

**Terminal states:** `CHARGED`, `VOIDED`, `AUTO_REFUNDED`, `AUTHORIZATION_FAILED`, `AUTHENTICATION_FAILED`, `CAPTURE_FAILED`, `VOID_FAILED`, `ROUTER_DECLINED`, `FAILURE`, `EXPIRED`.

**Recoverable states:** `PENDING`, `UNRESOLVED`.

### 8.2 Refund Status

```
REFUND_PENDING ──► REFUND_SUCCESS
       │
       ├──► REFUND_FAILURE
       │
       ├──► REFUND_TRANSACTION_FAILURE
       │
       └──► REFUND_MANUAL_REVIEW
```

### 8.3 Dispute Status

```
DISPUTE_OPENED ──► DISPUTE_CHALLENGED ──► DISPUTE_WON
       │                    │
       ├──► DISPUTE_ACCEPTED │──► DISPUTE_LOST
       │
       ├──► DISPUTE_EXPIRED
       │
       └──► DISPUTE_CANCELLED
```

### 8.4 Mandate Status

```
MANDATE_PENDING ──► ACTIVE ──► REVOKED
                       │
                       └──► MANDATE_INACTIVE
                                  │
                                  └──► MANDATE_REVOKE_FAILED
```

### 8.5 Webhook Event Types

UPP normalizes 40+ connector-specific webhook events into a canonical set:

**Payment events:**
- `PAYMENT_INTENT_SUCCESS`, `PAYMENT_INTENT_FAILURE`, `PAYMENT_INTENT_PROCESSING`
- `PAYMENT_INTENT_PARTIALLY_FUNDED`
- `PAYMENT_INTENT_REQUIRES_CUSTOMER_ACTION`, `PAYMENT_INTENT_REQUIRES_MERCHANT_ACTION`
- `PAYMENT_INTENT_CANCELLED`, `PAYMENT_INTENT_EXPIRED`
- `PAYMENT_INTENT_AUTHORIZED`, `PAYMENT_INTENT_CAPTURED`
- `PAYMENT_INTENT_CAPTURE_FAILED`, `PAYMENT_INTENT_AUTHENTICATION_FAILED`
- `PAYMENT_ACTION_REQUIRED`, `PAYMENT_INTENT_PARTIALLY_CAPTURED_AND_CAPTURABLE`
- `PAYMENT_INTENT_VOIDED`, `PAYMENT_INTENT_VOID_FAILED`
- `PAYMENT_INTENT_AUTHENTICATION_SUCCESS`

**Refund events:**
- `WEBHOOK_REFUND_SUCCESS`, `WEBHOOK_REFUND_FAILURE`

**Dispute events:**
- `WEBHOOK_DISPUTE_OPENED`, `WEBHOOK_DISPUTE_EXPIRED`
- `WEBHOOK_DISPUTE_ACCEPTED`, `WEBHOOK_DISPUTE_CANCELLED`
- `WEBHOOK_DISPUTE_CHALLENGED`, `WEBHOOK_DISPUTE_WON`, `WEBHOOK_DISPUTE_LOST`

**Mandate events:**
- `MANDATE_ACTIVE`, `MANDATE_REVOKED`

**Payout events:**
- `PAYOUT_SUCCESS`, `PAYOUT_FAILURE`, `PAYOUT_PROCESSING`
- `PAYOUT_CANCELLED`, `PAYOUT_INITIATED`, `PAYOUT_EXPIRED`, `PAYOUT_REVERSED`

---

## 9. Error Model

UPP provides structured error reporting at three levels:

```protobuf
message ErrorInfo {
  optional UnifiedErrorDetails unified_details = 1;
  optional IssuerErrorDetails issuer_details = 2;
  optional ConnectorErrorDetails connector_details = 3;
}
```

### 9.1 Unified Error Details

Connector-agnostic error information with a human-readable guidance message:

```protobuf
message UnifiedErrorDetails {
  optional string code = 1;
  optional string message = 2;
  optional string description = 3;
  optional string user_guidance_message = 4;
}
```

### 9.2 Issuer Error Details

Error information from the card-issuing bank, including network-level decline codes:

```protobuf
message IssuerErrorDetails {
  optional string code = 1;
  optional string message = 2;
  optional NetworkErrorDetails network_details = 3;
}

message NetworkErrorDetails {
  optional string advice_code = 1;
  optional string decline_code = 2;
  optional string error_message = 3;
}
```

### 9.3 Connector Error Details

Raw error information from the connector:

```protobuf
message ConnectorErrorDetails {
  optional string code = 1;
  optional string message = 2;
}
```

---

## 10. Redirect and SDK Flows

Many payment methods require customer interaction outside the direct API flow. UPP models these as `RedirectForm` variants:

| Variant | Use Case |
|---------|----------|
| `FormData` | HTTP form POST redirect (endpoint, method, form fields) |
| `HtmlData` | Full HTML page to render (e.g., 3DS challenge iframe) |
| `UriData` | Simple URL redirect |
| `BraintreeData` | Braintree-specific 3DS flow (client token, card token, ACS URL) |
| `MifinityData` | Mifinity initialization token |

After redirect completion, the connector response is validated via `VerifyRedirectResponse` using `RedirectResponseSecrets` for signature verification.

---

## 11. Connector Configuration

Each connector is configured through a typed `ConnectorSpecificConfig` message that provides compile-time safety for credential schemas. Configurations range from single-field (API key only) to multi-field (API key, merchant account, secret, additional credentials).

```protobuf
message ConnectorConfig {
  ConnectorSpecificConfig connector_config = 1;
  SdkOptions options = 2;
}

message SdkOptions {
  Environment environment = 1;  // SANDBOX or PRODUCTION
}
```

### 11.1 HTTP Configuration

```protobuf
message HttpConfig {
  optional uint32 total_timeout_ms = 1;       // Default: 45000ms
  optional uint32 connect_timeout_ms = 2;     // Default: 10000ms
  optional uint32 response_timeout_ms = 3;    // Default: 30000ms
  optional uint32 keep_alive_timeout_ms = 4;  // Default: 60000ms
  optional ProxyOptions proxy = 5;
  optional CaCert ca_cert = 6;
}
```

---

## 12. Capture Methods

UPP supports five capture strategies:

| Method | Behavior |
|--------|----------|
| `AUTOMATIC` | Funds are captured immediately upon authorization |
| `MANUAL` | Merchant explicitly triggers a single capture after authorization |
| `MANUAL_MULTIPLE` | Merchant triggers multiple partial captures against a single authorization |
| `SCHEDULED` | Capture occurs at a scheduled time |
| `SEQUENTIAL_AUTOMATIC` | Captures occur automatically in sequence |

---

## 13. Level 2 / Level 3 Data

UPP supports enhanced transaction data (L2/L3) for reduced interchange rates on commercial card transactions:

```protobuf
message L2L3Data {
  optional OrderInfo order_info = 1;
  optional TaxInfo tax_info = 2;
}
```

**Order Info**: Order date, line-item details (product name, quantity, amount, tax rate, product type), discount amount, shipping cost, duty amount.

**Tax Info**: Tax status, customer/merchant tax registration IDs, shipping tax, order tax amount.

---

## 14. Supported Connectors

UPP supports 110+ payment connectors including:

**Global processors:** Stripe, Adyen, Braintree, PayPal, Square, Worldpay, CyberSource, Fiserv, JP Morgan, Wells Fargo, Bank of America, TSYS, Elavon, Barclaycard

**Regional processors:** Razorpay, Cashfree, PhonePe (India); Mercado Pago, dLocal (Latin America); Mollie, Multisafepay (Europe); Xendit (Southeast Asia); EbanX (Brazil)

**Alternative payment:** Klarna, Affirm, Afterpay; Coinbase, Coingate (crypto); Wise, Revolut (transfers)

**Specialized:** Airwallex, Checkout.com, Rapyd, Nuvei, GlobalPay, Payoneer, Shift4, Stax, Helcim, and many more.

Each connector implements a subset of UPP services based on its capabilities. The protocol handles capability differences transparently.

---

## 15. Security Considerations

### 15.1 Data Protection

- All sensitive fields use `SecretString` wrappers for automatic log/trace masking.
- Card numbers use a dedicated `CardNumberType` with controlled serialization.
- API keys and tokens are never exposed in error messages or debug output.

### 15.2 Webhook Verification

- Webhooks carry `WebhookSecrets` for signature verification.
- The `source_verified` field on event responses indicates whether the webhook signature was successfully validated.
- Redirect responses are verified via `RedirectResponseSecrets`.

### 15.3 Authentication

- Connector authentication uses short-lived access tokens with configurable expiry.
- Session tokens maintain state isolation between concurrent payment flows.

### 15.4 3DS / SCA Compliance

- Full 3-D Secure 2.x support through the three-phase authentication flow.
- SCA exemption handling for PSD2 compliance (low value, TRA, trusted listing, etc.).
- Browser information collection for risk-based authentication decisions.

---

## 16. Design Principles

1. **Connector Agnosticism**: The protocol abstracts away all connector-specific behavior. Callers interact with a single, uniform API regardless of the underlying processor.

2. **Minor-Unit Precision**: All monetary amounts use integer minor units (cents, paise, etc.) to eliminate floating-point errors.

3. **State Round-Tripping**: `ConnectorState` enables multi-step flows by carrying session-scoped data (tokens, IDs) between requests without server-side session storage.

4. **Exhaustive Payment Coverage**: 100+ payment method types organized into a structured taxonomy, from traditional cards to crypto and regional vouchers.

5. **Secure by Default**: Sensitive data is structurally protected via `SecretString`, not by convention.

6. **Webhook Normalization**: 40+ event types are normalized from connector-specific formats into a canonical event taxonomy.

7. **Composability**: Composite services allow common multi-step workflows (token + customer + authorize) to be executed as a single request.

8. **Extensibility**: New connectors and payment methods can be added by extending the respective `oneof` blocks without breaking existing integrations.

---

## Appendix A: Connector List

<details>
<summary>Full connector enum (110+ connectors)</summary>

ACH, ADYEN, AIRWALLEX, AUTHIPAY, AUTHORIZED_DOT_NET, BAMBORA, BANK_OF_AMERICA, BARCLAYCARD, BILLWERK, BLUESNAP, BOA, BRAINTREE, CALIDA, CASHFREE, CASHTOCODE, CELERO, CHECKOUT, COINBASE, COINGATE, CRYPTOPAY, CYBERSOURCE, DATATRANS, DLOCAL, EBANX, ELAVON, EWAY, FISERV, FISERV_EMEA, FORTE, GETNET, GIGADAT, GLOBALPAY, GLOBEPAY, GOCARDLESS, HELCIM, HIPAY, IATAPAY, ITAU_BANK, JPMORGAN, KLARNA, LOONIO, MERCADOPAGO, MIFINITY, MOLLIE, MULTISAFEPAY, NEXIXPAY, NOMUPAY, NOON, NOVALNET, NUVEI, PAYBOX, PAYEEZY, PAYME, PAYONEER, PAYPAL, PAYSAFE, PAYU, PHONEPE, PLAID, POWERTRANZ, PROPHETPAY, RAPYD, RAZORPAY, REDSYS, REVOLV3, REVOLUT, SCREENSTREAM, SHIFT4, SILVERFLOW, SQUARE, STAX, STRIPE, TAXJAR, THREEDSECUREIO, TRUSTPAY, TRUSTPAYMENTS, TSYS, UNIFIED_AUTHENTICATION, VOLT, WELLSFARGO, WELLSFARGO_VANTIV, WISE, WORLDPAY, WORLDPAY_XML, XENDIT, ZIFT, and more.

</details>

---

## Appendix B: Currency Support

UPP supports 160+ currencies following ISO 4217, including all major currencies (USD, EUR, GBP, JPY, CNY, INR) and regional currencies across Africa, Asia, the Americas, Europe, and Oceania.

---

## Appendix C: Country Support

Full ISO 3166-1 alpha-2 country code support for billing/shipping address specification.
