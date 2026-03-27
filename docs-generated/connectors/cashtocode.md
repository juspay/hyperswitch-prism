# CashtoCode

<!--
This file is auto-generated. Do not edit by hand.
Regenerate: python3 scripts/generate-connector-docs.py cashtocode
-->

## Implemented Flows

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Capture](#paymentservicecapture) | Payments | `PaymentServiceCaptureRequest` |
| [MerchantAuthenticationService.CreateAccessToken](#merchantauthenticationservicecreateaccesstoken) | Authentication | `MerchantAuthenticationServiceCreateAccessTokenRequest` |
| [CustomerService.Create](#customerservicecreate) | Customers | `CustomerServiceCreateRequest` |
| [PaymentService.CreateOrder](#paymentservicecreateorder) | Payments | `PaymentServiceCreateOrderRequest` |
| [MerchantAuthenticationService.CreateSessionToken](#merchantauthenticationservicecreatesessiontoken) | Authentication | `MerchantAuthenticationServiceCreateSessionTokenRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [PaymentService.Refund](#paymentservicerefund) | Payments | `PaymentServiceRefundRequest` |
| [PaymentService.Reverse](#paymentservicereverse) | Payments | `PaymentServiceReverseRequest` |
| [PaymentService.Void](#paymentservicevoid) | Payments | `PaymentServiceVoidRequest` |

## Flow Details

### Payments

#### PaymentService.Capture

Finalize an authorized payment transaction. Transfers reserved funds from customer to merchant account, completing the payment lifecycle.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Minimum Request**

```json
{
  "merchant_capture_id": "probe_capture_001",
  "connector_transaction_id": "probe_connector_txn_001",
  "amount_to_capture": {
    "minor_amount": 1000,
    "currency": "USD"
  }
}
```

#### PaymentService.CreateOrder

Initialize an order in the payment processor system. Sets up payment context before customer enters card details for improved authorization rates.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCreateOrderRequest` |
| **Response** | `PaymentServiceCreateOrderResponse` |

**Minimum Request**

```json
{
  "merchant_order_id": "probe_order_001",
  "amount": {
    "minor_amount": 1000,
    "currency": "USD"
  }
}
```

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Minimum Request**

```json
{
  "connector_transaction_id": "probe_connector_txn_001",
  "amount": {
    "minor_amount": 1000,
    "currency": "USD"
  }
}
```

#### PaymentService.Refund

Initiate a refund to customer's payment method. Returns funds for returns, cancellations, or service adjustments after original payment.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Minimum Request**

```json
{
  "merchant_refund_id": "probe_refund_001",
  "connector_transaction_id": "probe_connector_txn_001",
  "payment_amount": 1000,
  "refund_amount": {
    "minor_amount": 1000,
    "currency": "USD"
  },
  "reason": "customer_request"
}
```

#### PaymentService.Reverse

Reverse a captured payment before settlement. Recovers funds after capture but before bank settlement, used for corrections or cancellations.

| | Message |
|---|---------|
| **Request** | `PaymentServiceReverseRequest` |
| **Response** | `PaymentServiceReverseResponse` |

**Minimum Request**

```json
{
  "merchant_reverse_id": "probe_reverse_001",
  "connector_transaction_id": "probe_connector_txn_001"
}
```

#### PaymentService.Void

Cancel an authorized payment before capture. Releases held funds back to customer, typically used when orders are cancelled or abandoned.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Minimum Request**

```json
{
  "merchant_void_id": "probe_void_001",
  "connector_transaction_id": "probe_connector_txn_001"
}
```

### Customers

#### CustomerService.Create

Create customer record in the payment processor system. Stores customer details for future payment operations without re-sending personal information.

| | Message |
|---|---------|
| **Request** | `CustomerServiceCreateRequest` |
| **Response** | `CustomerServiceCreateResponse` |

**Minimum Request**

```json
{
  "customer_name": "John Doe",
  "email": "test@example.com",
  "phone_number": "4155552671",
  "address": {
    "billing_address": {
      "first_name": "John",
      "last_name": "Doe",
      "line1": "123 Main St",
      "city": "Seattle",
      "state": "WA",
      "zip_code": "98101",
      "country_alpha2_code": "US",
      "email": "test@example.com",
      "phone_number": "4155552671",
      "phone_country_code": "+1"
    }
  }
}
```

### Authentication

#### MerchantAuthenticationService.CreateAccessToken

Generate short-lived connector authentication token. Provides secure credentials for connector API access without storing secrets client-side.

| | Message |
|---|---------|
| **Request** | `MerchantAuthenticationServiceCreateAccessTokenRequest` |
| **Response** | `MerchantAuthenticationServiceCreateAccessTokenResponse` |

**Minimum Request**

```json
{}
```

#### MerchantAuthenticationService.CreateSessionToken

Create session token for payment processing. Maintains session state across multiple payment operations for improved security and tracking.

| | Message |
|---|---------|
| **Request** | `MerchantAuthenticationServiceCreateSessionTokenRequest` |
| **Response** | `MerchantAuthenticationServiceCreateSessionTokenResponse` |

**Minimum Request**

```json
{
  "amount": {
    "minor_amount": 1000,
    "currency": "USD"
  }
}
```
