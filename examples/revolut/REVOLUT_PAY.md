# Revolut Pay handoff

The existing hosted checkout remains the default when Revolut Pay is not explicitly selected. Dedicated Revolut Pay uses the same order creation and subsequent GET/capture/refund/webhook lifecycle.

## Merchant configuration

Configure `revolut.public_key` alongside `secret_api_key`, using keys from the same merchant account and environment. Regenerate clients from payment.proto to expose the new optional field. Never send the secret API key to checkout.

## Web redirection

Send `connector_feature_data` JSON with `checkout_mode: "redirection"` and the normal `return_url`. Prism validates the public key and return URL before creating the order, then returns a dedicated Revolut Pay URL. Success, failure and cancellation return to the existing handler; GET/webhooks remain authoritative.

Prism's Revolut connector selects dedicated redirection from the incoming Revolut Pay payment method. Euler's shared request builder contains no Revolut-specific routing logic. PM registration/merchant enablement must still be provisioned through the owning configuration service; the adapter cannot create that registration.

## Native universal link / merchant SDK

A direct Prism Authorize caller can use `checkout_mode: "sdk"`. It returns no hosted redirect. Response connector metadata contains `order_id`, `order_token`, `public_key`, and `checkout_base_url`. Pass only these public handoff fields to the authorised checkout client. Keep the backend order ID associated with the transaction attempt. Do not call ClientAuthenticationToken after Authorize: both create orders.

`pay-links.mjs` retrieves the universal link from Revolut using this session. For native same-device checkout pass `appReturnUrl`; for cross-device/QR pass an `INPERSON_` source. Open/display the returned URL, not the JSON retrieval endpoint. Mobile browsers should use the redirection flow. A merchant SDK can instead use the returned order token with the Revolut SDK.

The native application and Euler merchant-facing SDK response mapping are not wired by this example. Prism defaults selected REVOLUTPAY to web redirection; do not switch its intent/QR path to sdk mode until its response adapter handles token sessions.

## Validation and rollout

Unit tests exercise URL encoding and explicit mode parsing; the Node helper tests use mocked fetch responses. Sandbox validation is still required with merchant keys, callback URLs, app installed/absent and expiry/cancellation races. Customer storage, location settings and expiry orchestration are separate capabilities. No consent is enabled automatically.
