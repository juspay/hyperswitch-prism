# Domain Schema

<!--
---
title: Domain Schema
description: Complete reference for all domain types, enums, and data structures used across Connector Service services
last_updated: 2026-03-11
generated_from: crates/types-traits/grpc-api-types/proto/
auto_generated: false
reviewed_by: engineering
reviewed_at: 2026-03-05
approved: true
---
-->

## Overview

Domain types are the foundational data structures and enumerations defined in the Protocol Buffer (protobuf) files that form the core data model of the Universal Connector Service (Connector Service) API. These types represent the building blocks used across all services to model payments, refunds, disputes, payment methods, and related entities.

### Relationship to Services

The domain schema are used by the gRPC services defined in [services.proto](../services/) as:

- **Request/Response Messages**: Service methods accept and return structured messages documented in the [API Reference](../services/) section for each service
- **Enums**: Status codes, payment methods, and categorical values ensure type safety (e.g., `PaymentStatus`, `Currency`)
- **Nested Types**: Complex types like `Money`, `Address`, and `Customer` are reused across multiple service operations

### What's Included

This Domain Schema includes:
- **Core data structures**: `Money`, `Customer`, `Address`, `PaymentMethod` - reusable types that appear as fields across multiple services
- **Enumerations**: `PaymentStatus`, `Currency`, `Connector`, `CardNetwork` - categorical values for type safety
- **Connector responses**: `ConnectorResponseData`, `CardConnectorResponse` - additional data returned by connectors

### What's Not Included

Service request and response types (e.g., `PaymentServiceAuthorizeRequest`, `RefundResponse`) are documented in the [API Reference](../services/) section for each service. These types are specific to individual RPC operations and are covered alongside their corresponding operation documentation.

### Key Design Principles

The design principles translate directly to reliable, extensible and compliant integrations for developers.

1. **Type Safety**: Type safety prevents rounding errors and currency confusion that can cause financial discrepancies. Example: All monetary amounts use the `Money` message with amounts in minor units (e.g., 1000 = $10.00)
2. **Secret Handling**: Secret handling ensures PCI compliance by automatically masking sensitive data in your logs, reducing audit risk. Sensitive data uses `SecretString` types that are masked in logs and traces
3. **Extensibility**: Extensibility means your integration won't break when new payment methods are added—your existing code handles new variants through the same interfaces. `oneof` fields allow polymorphic data structures (e.g., `PaymentMethod` can be card, wallet, or bank transfer)
4. **Versioning**: Stable versioning guarantees that updating your API client won't require rewrites; new fields are additive, preserving backward compatibility. Field numbers are stable; enum zero values follow the `*_UNSPECIFIED` convention


---

## Index of Domain Types

### Core Data Types

Basic building blocks used across all Connector Service services. These fundamental types handle monetary amounts, error information, customer data, identifiers, and metadata that form the foundation of payment processing.

| Domain Type | Description | Example | Related Types |
|-------------|-------------|---------|---------------|
| `Money` | Monetary amount with currency. Amounts are in minor units (e.g., 1000 = $10.00). | `{"minor_amount": 1000, "currency": "USD"}` | `Currency` |
| `ErrorInfo` | Structured error information for API responses. Includes error code and human-readable message. | `{"code": "CARD_DECLINED", "message": "Card was declined", "reason": "INSUFFICIENT_FUNDS"}` | `PaymentStatus` |
| `Customer` | Customer information including name, email, and unique identifier. | `{"id": "cus_123", "name": "John Doe", "email": "john@example.com", "phone": "+1-555-0123"}` | `Address`, `PaymentMethod` |
| `Metadata` | Key-value metadata for connectors. Stores additional context about transactions. | `{"order_id": "ORD-123", "source": "mobile_app", "campaign": "spring_sale"}` | `SecretString` |
| `SecretString` | Sensitive data masked in logs and traces for PCI compliance. | `"***MASKED***"` | `Metadata`, `CardDetails` |

### Address and Contact Types

Location and contact information for billing, shipping, and customer records. These types standardize address formats across different regions and payment connectors.

| Domain Type | Description | Example | Related Types |
|-------------|-------------|---------|---------------|
| `Address` | Physical address with street, city, country, and postal code. | `{"line1": "123 Main St", "city": "San Francisco", "state": "CA", "country": "US", "zip": "94105", "phone": "+1-555-0123"}` | `PaymentAddress`, `Customer` |
| `PaymentAddress` | Container for billing and shipping addresses in a single payment context. | `{"billing": {"line1": "123 Main St", ...}, "shipping": {"line1": "456 Oak Ave", ...}}` | `Address`, `ShippingDetails` |
| `CustomerInfo` | Simplified customer information for specific payment scenarios. | `{"name": "John Smith", "email": "john@example.com"}` | `PaymentMethod`, `Upi` |

### Payment Method Types

Payment instruments supported by Connector Service including cards, wallets, bank transfers, and local payment methods. The `PaymentMethod` type is polymorphic, supporting various payment instruments through a `oneof` field.

| Domain Type | Description | Example | Related Types |
|-------------|-------------|---------|---------------|
| `PaymentMethod` | Polymorphic payment instrument. Supports card, wallet, or bank transfer via `oneof`. | `{"card": {"card_number": "4242424242424242", "expiry_month": "12", "expiry_year": "2027", "cvv": "123"}}` | `CardDetails`, `AppleWallet`, `GooglePay`, `Paypal`, `BankDebit`, `BankRedirect`, `BankTransfer`, `Upi`, `Wallet` |
| `CardDetails` | Complete card payment details including number, expiry, and optional CVV. | `{"card_number": "4242424242424242", "expiry_month": "12", "expiry_year": "2027", "card_holder_name": "John Doe", "card_network": "VISA"}` | `PaymentMethod`, `CardNetwork` |
| `AppleWallet` | Apple Pay payment method with encrypted payment token. | `{"pk_payment_token": {"payment_data": "encrypted_data", "transaction_id": "txn_123"}}` | `PaymentMethod`, `SessionToken` |
| `GooglePay` | Google Pay payment method with tokenized card data. | `{"gp_payment_token": {"signature": "sig_123", "protocol_version": "ECv2"}}` | `PaymentMethod`, `GpaySessionTokenResponse` |
| `Paypal` | PayPal wallet payment using email or PayPal ID. | `{"email": "user@example.com", "paypal_id": "PAYER_123"}` | `PaymentMethod`, `Wallet` |
| `BankDebit` | Bank debit payment via ACH (US) or SEPA (Europe) using IBAN/account numbers. | `{"iban": "DE89370400440532013000", "bank_name": "Deutsche Bank", "country": "DE"}` | `PaymentMethod`, `MandateReference` |
| `BankRedirect` | Bank redirect methods like Sofort, iDEAL requiring customer redirection. | `{"bank_name": "Deutsche Bank", "bank_redirect_type": "SOFORT", "country": "DE"}` | `PaymentMethod`, `RedirectForm` |
| `BankTransfer` | Direct bank transfer using account and routing details. | `{"account_number": "123456789", "routing_number": "011000015", "bank_name": "Bank of America"}` | `PaymentMethod` |
| `Upi` | UPI (Unified Payments Interface) payment for India using VPA. | `{"vpa": "john@upi", "customer_info": {"name": "John", "email": "john@example.com"}}` | `PaymentMethod`, `CustomerInfo` |
| `Wallet` | Generic wallet container for wallet-type payments (PayPal, Venmo, etc.). | `{"wallet_type": "PAYPAL", "wallet_details": {...}}` | `PaymentMethod`, `Paypal` |

### Card Network Enums

Card networks supported for card payments. This enum identifies the card scheme when processing card transactions and handling network-specific logic.

| Domain Type | Values | Description |
|-------------|--------|-------------|
| `CardNetwork` | `VISA`, `MASTERCARD`, `AMEX`, `DISCOVER`, `JCB`, `DINERS`, `UNIONPAY`, `MAESTRO`, `CARTES_BANCAIRES`, `RUPAY`, `INTERAC_CARD` | Card networks supported for card payments. Used to identify the card scheme and apply network-specific logic. |

### Authentication Types

3D Secure and Strong Customer Authentication data structures. These types support frictionless authentication, challenge flows, and SCA exemption handling for regulatory compliance.

| Domain Type | Description | Example | Related Types |
|-------------|-------------|---------|---------------|
| `AuthenticationData` | 3DS authentication data including ECI, CAVV, and transaction status for verified payments. | `{"eci": "05", "cavv": "AAABBIIFmAAAAAAAAAAAAAAAAAA=", "transaction_status": "SUCCESS", "authentication_type": "THREE_DS"}` | `CardConnectorResponse`, `CardNetwork` |
| `BrowserInformation` | Browser details required for 3DS authentication and SCA compliance. | `{"accept_header": "text/html", "user_agent": "Mozilla/5.0", "ip_address": "192.168.1.1", "language": "en-US"}` | `ThreeDsCompletionIndicator`, `ExemptionIndicator` |
| `CustomerAcceptance` | Mandate acceptance details for recurring payment authorization. | `{"acceptance_type": "ONLINE", "accepted_at": "2024-01-15T10:30:00Z", "online": {"ip_address": "192.168.1.1", "user_agent": "Mozilla/5.0"}}` | `MandateReference`, `SetupMandateDetails` |

### Authentication Enums

Enumerations for authentication flows and 3DS status tracking. These define the authentication methods, transaction outcomes, and exemption categories for SCA compliance.

| Domain Type | Values | Description |
|-------------|--------|-------------|
| `AuthenticationType` | `THREE_DS`, `NO_THREE_DS` | Whether 3D Secure authentication is performed. `THREE_DS` for authenticated, `NO_THREE_DS` for frictionless flow. |
| `TransactionStatus` | `SUCCESS`, `FAILURE`, `CHALLENGE_REQUIRED`, `REJECTED` | Result of 3DS authentication. `CHALLENGE_REQUIRED` means additional customer verification needed. |
| `ExemptionIndicator` | `LOW_VALUE`, `SECURE_CORPORATE_PAYMENT`, `TRUSTED_LISTING` | SCA exemption categories for skipping 3DS. Must meet specific regulatory criteria. |
| `CavvAlgorithm` | `CAVV_ALGORITHM_ZERO` through `CAVV_ALGORITHM_A` | Algorithm used to generate CAVV cryptogram. Indicates authentication method strength. |
| `ThreeDsCompletionIndicator` | `SUCCESS`, `FAILURE`, `NOT_AVAILABLE` | Whether 3DS challenge was completed. Used in subsequent authorization requests. |

### Mandate Types

Recurring payment mandate data structures. These types manage stored payment credentials for subscription billing, allowing merchants to charge customers on a recurring basis with prior consent.

| Domain Type | Description | Example | Related Types |
|-------------|-------------|---------|---------------|
| `MandateReference` | Reference to stored mandate for recurring payments. Contains connector-specific mandate ID. | `{"connector_mandate_id": "mandate_123", "payment_method_id": "pm_456", "mandate_metadata": {...}}` | `BankDebit`, `CustomerAcceptance` |
| `SetupMandateDetails` | Mandate setup details including type (single/multi-use) and amount constraints. | `{"mandate_type": {"multi_use": {"amount": 5000, "currency": "USD"}}, "customer_acceptance": {...}}` | `CustomerAcceptance`, `MandateType` |
| `MandateAmountData` | Mandate amount constraints with currency, frequency, and validity period. | `{"amount": 1000, "currency": "USD", "start_date": "2024-01-01", "end_date": "2025-01-01", "frequency": "MONTHLY"}` | `SetupMandateDetails`, `Money` |

### Mandate Enums

Status and type enumerations for recurring payment mandates. These track mandate lifecycle states and define whether a mandate can be used once or multiple times.

| Domain Type | Values | Description |
|-------------|--------|-------------|
| `MandateType` | `single_use`, `multi_use` | Whether mandate can be used once (`single_use`) or multiple times (`multi_use`) for recurring charges. |
| `MandateStatus` | `PENDING`, `ACTIVE`, `INACTIVE`, `REVOKED` | Lifecycle state of mandate. `ACTIVE` means ready for recurring charges, `REVOKED` means cancelled by customer. |

### Payment Status Enums

State machines for payment lifecycle tracking. These enums represent the complete payment journey from initiation through completion, failure, or refund.

| Domain Type | Values | Description |
|-------------|--------|-------------|
| `PaymentStatus` | `STARTED`, `AUTHORIZED`, `CAPTURED`, `VOIDED`, `REFUNDED`, `FAILED`, `EXPIRED` | Complete payment lifecycle states. `AUTHORIZED` = funds held, `CAPTURED` = funds transferred, `VOIDED` = authorization cancelled. |
| `AuthorizationStatus` | `SUCCESS`, `FAILURE`, `PROCESSING`, `UNRESOLVED` | Result of authorization attempt. `UNRESOLVED` typically indicates 3DS challenge or async processing. |
| `CaptureMethod` | `AUTOMATIC`, `MANUAL`, `MANUAL_MULTIPLE`, `SCHEDULED` | When to capture funds. `AUTOMATIC` = immediate, `MANUAL` = merchant-initiated, `SCHEDULED` = delayed capture. |
| `RefundStatus` | `PENDING`, `SUCCESS`, `FAILURE`, `MANUAL_REVIEW` | State of refund processing. `MANUAL_REVIEW` requires merchant intervention for compliance or risk reasons. |
| `DisputeStatus` | `OPENED`, `EXPIRED`, `ACCEPTED`, `CHALLENGED`, `WON`, `LOST` | Chargeback lifecycle. `CHALLENGED` = merchant submitted evidence, `WON`/`LOST` = final resolution. |

### Payment Configuration Types

Configuration enums for payment processing. These define how payments should be captured, when payment methods can be reused, and the customer experience preferences.

| Domain Type | Values | Description |
|-------------|--------|-------------|
| `FutureUsage` | `OFF_SESSION`, `ON_SESSION` | Intent for storing payment method. `OFF_SESSION` allows recurring charges without customer present. |
| `PaymentExperience` | `REDIRECT_TO_URL`, `INVOKE_SDK_CLIENT`, `DISPLAY_QR_CODE`, `ONE_CLICK` | Customer experience for payment. `REDIRECT_TO_URL` for 3DS/bank auth, `INVOKE_SDK_CLIENT` for Apple/Google Pay. |
| `PaymentChannel` | `ECOMMERCE`, `MAIL_ORDER`, `TELEPHONE_ORDER` | How the payment was initiated. Affects fraud scoring and SCA requirements. |
| `PaymentMethodType` | `CARD`, `APPLE_PAY`, `GOOGLE_PAY`, `SEPA`, `ACH` | Specific payment instrument category. Used for routing and processing logic. |
| `MitCategory` | `INSTALLMENT_MIT`, `UNSCHEDULED_MIT`, `RECURRING_MIT` | Merchant-initiated transaction type for exemption requests. Required for off-session recurring charges. |
| `Tokenization` | `TOKENIZE_AT_PSP`, `TOKENIZE_SKIP_PSP` | Whether to tokenize card at payment service provider. `SKIP` when using existing network tokens. |

### Currency and Connector Enums

Currency codes and supported payment processor connectors. These enums standardize currency representation across 170+ currencies and identify which connector is handling a transaction.

| Domain Type | Values | Description |
|-------------|--------|-------------|
| `Currency` | `USD`, `EUR`, `GBP`, `INR`, `JPY` (ISO 4217) | Three-letter currency codes. Used with `Money` type to specify transaction currency. Supports 170+ currencies. |
| `Connector` | `ADYEN`, `STRIPE`, `PAYPAL`, `BRAINTREE`, `CHECKOUT` | Payment processor handling the transaction. Determines API endpoint and authentication method. |
| `CountryAlpha2` | `US`, `GB`, `DE`, `IN`, `JP` (ISO 3166-1) | Two-letter country codes for billing/shipping addresses. Required for fraud detection and regulatory compliance. |

### Order and Billing Types

Order details and billing descriptor configurations. These types support itemized order information and statement descriptors that appear on customer card statements.

| Domain Type | Description | Example | Related Types |
|-------------|-------------|---------|---------------|
| `OrderDetailsWithAmount` | Product line item details with name, quantity, and amount for itemized orders. | `{"product_name": "Premium Widget", "product_id": "SKU-12345", "quantity": 2, "amount": 5000, "currency": "USD"}` | `Money`, `ShippingDetails` |
| `BillingDescriptor` | Statement descriptor shown on customer's card statement. Helps reduce chargebacks. | `{"name": "ACME INC*", "city": "SAN FRANCISCO", "state": "CA", "phone": "1-800-555-0123"}` | `Address` |
| `ShippingDetails` | Shipping address, carrier, and tracking information for physical goods. | `{"address": {"line1": "123 Main St", ...}, "carrier": "FedEx", "tracking_number": "1234567890", "method": "EXPRESS"}` | `PaymentAddress`, `OrderDetailsWithAmount` |

### State and Access Types

Token and state management types. These handle authentication tokens for API access and connector-specific state for multi-step payment flows.

| Domain Type | Description | Example | Related Types |
|-------------|-------------|---------|---------------|
| `ConnectorState` | Connector state for multi-step flows. Pass between API calls to maintain context. | `{"access_token": "tok_123", "connector_customer_id": "cus_456", "connector_metadata": {...}, "redirect_response": {...}}` | `AccessToken`, `Connector` |
| `AccessToken` | API access token with expiration for connector authentication. | `{"token": "tok_123456", "expires_at": 1704067200, "token_type": "BEARER", "scope": "payments"}` | `ConnectorState` |

### Redirection Types

Handling redirect flows for 3DS and bank authentication. These types manage the various redirection mechanisms required for authentication flows, including HTML forms, deep links, and SDK redirects.

| Domain Type | Description | Example | Related Types |
|-------------|-------------|---------|---------------|
| `RedirectForm` | Container for redirect flow data including URL or form for 3DS/bank authentication. | `{"form": {"form_method": "POST", "form_data": {"PaReq": "abc123", "MD": "merchant_data"}}}` | `FormData`, `PaymentExperience` |
| `FormData` | HTML form details for POST-based redirection in 3DS flows. | `{"form_method": "POST", "form_url": "https://bank.com/3ds", "form_fields": {"PaReq": "abc123", "TermUrl": "https://merchant.com/callback"}}` | `RedirectForm`, `PaymentExperience` |
| `RedirectionResponse` | Data returned after customer completes redirect (3DS or bank auth). | `{"params": {"PaRes": "xyz789", "MD": "merchant_data"}, "payload": "base64_encoded_data", "redirect_url": "https://..."}` | `RedirectForm`, `BrowserInformation` |

### Webhook and Event Types

Handling asynchronous events from connectors. These types define webhook event categories and structures for processing connector callbacks when payment states change.

| Domain Type | Description | Example | Related Types |
|-------------|-------------|---------|---------------|
| `WebhookEventType` | Type of webhook event received from connector (payment success, dispute opened, etc.). | `PAYMENT_INTENT_SUCCESS` | `PaymentStatus`, `DisputeStatus` |
| `WebhookSecrets` | Secrets for verifying webhook authenticity from payment connectors. | `{"secret": "whsec_1234567890abcdef", "additional_secret": "whsec_second"}` | `RequestDetails` |
| `RequestDetails` | HTTP request details from webhook including headers, body, and method for verification. | `{"method": "POST", "url": "https://merchant.com/webhook", "headers": {"Stripe-Signature": "t=123,v1=abc"}, "body": "{...}"}` | `WebhookSecrets` |

### SDK and Session Types

Wallet SDK integration types for Apple Pay, Google Pay, and PayPal. These support client-side tokenization and native payment experiences in mobile and web applications.

| Domain Type | Description | Example | Related Types |
|-------------|-------------|---------|---------------|
| `SessionToken` | Wallet session token for Apple Pay or Google Pay SDK initialization. | `{"google_pay": {"merchant_info": {...}, "payment_methods": [...]}}` or `{"apple_pay": {"session_data": "...", "display_message": "..."}}` | `GooglePay`, `AppleWallet` |
| `GpaySessionTokenResponse` | Google Pay session configuration returned from session initialization API. | `{"merchant_info": {"merchant_name": "Example Store", "merchant_id": "123"}, "payment_methods": [{"type": "CARD"}], "transaction_info": {...}}` | `SessionToken`, `GooglePay` |
| `ApplepaySessionTokenResponse` | Apple Pay session data including session object and display message. | `{"session_data": {"epochTimestamp": 1234567890, "merchantIdentifier": "merchant.com.example"}, "display_message": "Pay Example Store"}` | `SessionToken`, `AppleWallet` |
| `SdkNextAction` | Instructions for client-side SDK action (invoke Apple Pay, Google Pay, etc.). | `{"next_action": "INVOKE_SDK_CLIENT", "wallet_name": "APPLE_PAY", "sdk_data": {...}}` | `SessionToken`, `PaymentExperience` |

### Connector Response Types

Additional data returned by connectors. These types provide connector-specific response details for different payment methods and extended authorization information.

| Domain Type | Description | Example | Related Types |
|-------------|-------------|---------|---------------|
| [`ConnectorResponseData`](./connector-response-data.md) | Connector response container. | `{"card": {...}, "extended_authorization": {...}}` | [`CardConnectorResponse`](./card-connector-response.md) |
| [`CardConnectorResponse`](./card-connector-response.md) | Card-specific response. | `{"authentication_data": {...}, "card_network": "VISA"}` | [`ConnectorResponseData`](./connector-response-data.md), [`AuthenticationData`](./authentication-data.md), [`CardNetwork`](./card-network.md) |

## Next Steps

- [Payment Service](../services/payment-service/README.md) - Core payment operations
- [Refund Service](../services/refund-service/README.md) - Refund processing
- [Dispute Service](../services/dispute-service/README.md) - Dispute management
- [Recurring Payment Service](../services/recurring-payment-service/README.md) - Recurring billing operations
