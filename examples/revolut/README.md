# Revolut checkout lifecycle

## Returning customers, stores, and expiry

On `PaymentService.Authorize`, pass the previously returned Revolut ID in
`customer.connector_customer_id`. This is the **Revolut customer ID**, not your
application's customer ID. It takes precedence over the optional
`connector_feature_data.customer_id` fallback. The outbound request then contains
only `customer: {"id": "..."}`; an email is not required for returning customers.
For a new customer, email and optional name retain the existing behavior.

Both Authorize and CreateClientAuthenticationToken accept this JSON in
`connector_feature_data` (serialize it into the API's SecretString field):

```json
{
  "customer_id": "PREVIOUSLY_RETURNED_REVOLUT_CUSTOMER_ID",
  "location_id": "REVOLUT_LOCATION_UUID",
  "expire_pending_after": "PT30M"
}
```

For CreateClientAuthenticationToken, supply the existing customer ID through this
feature-data field. Its generic CustomerInfo currently does not retain the
connector-specific customer ID. New customers can supply email/name through the
payment context's customer object.

All three settings are optional. Location IDs must be configured in Revolut;
expiry uses Revolut's ISO 8601 duration format. Omitting expiry preserves the
provider default; this change does not impose a new timeout on existing callers.
Malformed field types fail request construction rather than being silently ignored.

Persist the customer ID returned in Authorize/Get/Capture's
`connector_feature_data.customer_id`. SDK initialization returns it as the optional
`customer_id` in `RevolutClientAuthenticationResponse`; regenerate protobuf-based
clients to consume the new field (older clients ignore it). Store IDs per merchant
account/environment and application customer, and send them on later requests.
Prism does not maintain a customer database. Reusing a customer enables association
with saved methods but does not replace SDK consent/saved-method configuration.

## Replacing a pending order

1. Keep the previous Revolut **order ID** associated with the checkout attempt.
2. Call `PaymentService.Void` with that ID and wait for success.
3. Only then create the replacement order via Authorize or SDK initialization.
4. If cancellation fails, retrieve/reconcile the old order before proceeding;
   it may already have been paid.

Void sends `POST /api/orders/{order_id}/cancel` and handles its empty HTTP 200
response. It does not refund completed orders. Prism is a stateless connector:
retry sequencing and locating the previous order remain the caller's responsibility.

## Webhooks and refunds

Configure and verify the webhook subscription in the merchant account. The code
change does not create or update subscriptions. Include `ORDER_COMPLETED`,
`ORDER_AUTHORISED`, `ORDER_FAILED`, `ORDER_CANCELLED`, `ORDER_PAYMENT_AUTHENTICATED`,
`ORDER_PAYMENT_DECLINED`, and `ORDER_PAYMENT_FAILED`. In particular, pending expiry
is delivered as `ORDER_FAILED`.

Revolut's payment and refund callbacks share event names and only identify the
order. For new refunds, Prism prefixes the outbound `merchant_order_data.reference`
with `prism:revolut:refund:`. Revolut echoes it as `merchant_order_ext_ref`, allowing
EventService.Parse/Handle to identify the refund without a network lookup. The
prefix is stripped from normalized refund references. Payment references beginning
with this reserved prefix are rejected to avoid misclassification. Preserve the
reference when updating refund orders outside Prism.

Completed refunds map to success; cancelled/failed/declined refunds map to failure.
The webhook `order_id` is the **refund order ID**, not the original payment ID.
Persist the refund ID returned by the refund API and make callback processing
idempotent. Signature verification continues to use the original, unmodified body.

**Migration:** callbacks for refunds created before this change, refunds created
outside Prism, or callbacks without the tagged reference cannot be automatically
distinguished from payments. Correlate their order IDs with your stored refund IDs
and use RefundService.Get for reconciliation. Continue polling when necessary;
do not interpret an untagged refund callback as payment completion in your caller.

## Payment-method reporting

Authorize/Get/Capture return available customer IDs and payment-attempt metadata in
`connector_feature_data`, for example:

```json
{
  "customer_id": "REVOLUT_CUSTOMER_ID",
  "payments": [
    {"id": "PAYMENT_ID", "state": "completed", "payment_method_type": "apple_pay"}
  ]
}
```

Recognized types are `card`, `apple_pay`, `google_pay`, `revolut_pay_card`, and
`revolut_pay_account`. Call PaymentService.Get after completion to obtain the
populated payment details. Each attempt has its own ID and state so failed attempts
are not counted as completed payments. Persist/aggregate these fields in your
analytics layer; Prism does not provide analytics storage. Card details are not
copied into this metadata.

## References

- [Revolut Merchant API](https://developer.revolut.com/docs/api/merchant)
- [Refund lifecycle and webhook events](https://developer.revolut.com/docs/guides/merchant/operations/refunds)
- [Hosted checkout expiry](https://developer.revolut.com/docs/guides/merchant/accept-payments/online-payments/hosted-checkout-page/api)
