# Extending to More Flows

You have implemented the basic plumbing for routing payment processor agnostic APIs. All methods work the same way with the single interface regardless of which payment processor you use. That's the power you get with the library.

Beyond the basic authorization and capture, the library handles complex payment scenarios in a processor agnostic manner. This includes recurring payments, incremental authorization, void, reverse, refund and more.

## Payment Flows Overview

Below are some sample real world scenarios to try out quickly.

| Flow | Use Case | Key Operations |
|------|----------|----------------|
| **Authorize + Capture** | Standard e-commerce | [`authorize`](../../api-reference/services/payment-service/authorize.md), [`capture`](../../api-reference/services/payment-service/capture.md) |
| **Authorize + Void** | Cancel pending order | [`authorize`](../../api-reference/services/payment-service/authorize.md), [`void`](../../api-reference/services/payment-service/void.md) |
| **Automatic Capture** | Digital goods, immediate charge | [`authorize`](../../api-reference/services/payment-service/authorize.md) with `AUTOMATIC` |
| **Incremental Authorization** | Hotel check-in, car rental | [`authorize`](../../api-reference/services/payment-service/authorize.md), [`incrementalAuthorization`](../../api-reference/services/payment-service/incremental-authorization.md) |
| **Partial Capture** | Multi-shipment orders | [`capture`](../../api-reference/services/payment-service/capture.md) with partial amount |
| **Refunds** | Customer returns | [`refund`](../../api-reference/services/payment-service/refund.md) |
| **Recurring Payments** | SaaS billing and more | [`setupRecurring`](../../api-reference/services/payment-service/setup-recurring.md), [`charge`](../../api-reference/services/recurring-payment-service/charge.md) |


## Incremental Authorization

Let's take hotels and car rentals. Such businesses will need to make an initial charge (like a security deposit) and then need to increase authorization amounts after the initial charge. When you use hyperswitch-prism the flow will work like this.

```javascript
const { PaymentClient } = require('hyperswitch-prism');
const types = require('hyperswitch-prism').types;

const config = {
    connectorConfig: {
        stripe: { apiKey: { value: process.env.STRIPE_API_KEY } }
    }
};
const paymentClient = new PaymentClient(config);

// 1. Initial authorization: $100 hold
const auth = await paymentClient.authorize({
    merchantTransactionId: 'hotel-reservation-001',
    amount: { minorAmount: 10000, currency: types.Currency.USD },
    paymentMethod: { card: { /* card details */ } },
    captureMethod: types.CaptureMethod.MANUAL,
    address: { billingAddress: {} },
    authType: types.AuthenticationType.NO_THREE_DS,
    returnUrl: "https://example.com/return"
});

// 2. Customer adds room service: increase hold to $150
const incremental = await paymentClient.incrementalAuthorization({
    connectorTransactionId: auth.connectorTransactionId,
    additionalAmount: { minorAmount: 5000, currency: types.Currency.USD }
});

// 3. At checkout: capture final amount
await paymentClient.capture({
    merchantCaptureId: 'capture-001',
    connectorTransactionId: auth.connectorTransactionId,
    amountToCapture: { minorAmount: 14750, currency: types.Currency.USD }  // Actual amount
});
```

See: [`incrementalAuthorization` API Reference](../../api-reference/services/payment-service/incremental-authorization.md)

## Subscription / Recurring Payments

Let's take subscription businesses like an email subscription or an AI subscription. Such businesses would want to store a payment method of a customer against a particular subscription plan, and charge it later:

```javascript
const { PaymentClient, RecurringPaymentClient } = require('hyperswitch-prism');
const types = require('hyperswitch-prism').types;

const config = {
    connectorConfig: {
        stripe: { apiKey: { value: process.env.STRIPE_API_KEY } }
    }
};
const paymentClient = new PaymentClient(config);
const recurringPaymentClient = new RecurringPaymentClient(config);

// 1. Set up recurring (store payment method)
const recurring = await paymentClient.setupRecurring({
    merchantRecurringPaymentId: 'mandate-001',
    amount: { minorAmount: 0, currency: types.Currency.USD },
    paymentMethod: { 
        card: {
            cardNumber: { value: '4111111111111111' },
            cardExpMonth: { value: '03' },
            cardExpYear: { value: '2030' },
            cardCvc: { value: '737' },
            cardHolderName: { value: 'John Doe' }
        }
    },
    address: { billingAddress: {} },
    authType: types.AuthenticationType.NO_THREE_DS,
    returnUrl: "https://example.com/mandate-return",
    setupFutureUsage: "OFF_SESSION"
});

// 2. Charge the stored method monthly
const charge = await recurringPaymentClient.charge({
    connectorRecurringPaymentId: {
        mandateIdType: {
            connectorMandateId: {
                connectorMandateId: recurring.mandateReference?.connectorMandateId?.connectorMandateId
            }
        }
    },
    amount: { minorAmount: 2900, currency: types.Currency.USD },
    returnUrl: "https://example.com/recurring-return",
    offSession: true
});
```

See: [`setupRecurring`](../../api-reference/services/payment-service/setup-recurring.md), [`charge`](../../api-reference/services/recurring-payment-service/charge.md), [`revoke`](../../api-reference/services/recurring-payment-service/revoke.md)


## Partial Capture

Let's take e-commerce businesses with multi-shipment orders. Such businesses may need to capture partial amounts as each shipment is fulfilled, rather than capturing the full authorized amount at once. When you use hyperswitch-prism the flow will work like this.

```javascript
const { PaymentClient } = require('hyperswitch-prism');
const types = require('hyperswitch-prism').types;

const config = {
    connectorConfig: {
        stripe: { apiKey: { value: process.env.STRIPE_API_KEY } }
    }
};
const paymentClient = new PaymentClient(config);

// Authorized $100
const auth = await paymentClient.authorize({
    merchantTransactionId: 'multi-ship-order-001',
    amount: { minorAmount: 10000, currency: types.Currency.USD },
    paymentMethod: { card: { /* card details */ } },
    captureMethod: types.CaptureMethod.MANUAL,
    address: { billingAddress: {} },
    authType: types.AuthenticationType.NO_THREE_DS,
    returnUrl: "https://example.com/return"
});

// First shipment: capture $40
await paymentClient.capture({
    merchantCaptureId: 'capture-001',
    connectorTransactionId: auth.connectorTransactionId,
    amountToCapture: { minorAmount: 4000, currency: types.Currency.USD }  // Partial
});

// Second shipment: capture remaining $60
await paymentClient.capture({
    merchantCaptureId: 'capture-002',
    connectorTransactionId: auth.connectorTransactionId,
    amountToCapture: { minorAmount: 6000, currency: types.Currency.USD }
});
```

See: [`capture`](../../api-reference/services/payment-service/capture.md)

## Void (Cancel Authorization)

Let's take scenarios where a customer cancels an order before it ships, or inventory issues prevent fulfillment. Such businesses need to release the held funds without charging the customer. When you use hyperswitch-prism the flow will work like this.

```javascript
const { PaymentClient } = require('hyperswitch-prism');
const types = require('hyperswitch-prism').types;

const config = {
    connectorConfig: {
        stripe: { apiKey: { value: process.env.STRIPE_API_KEY } }
    }
};
const paymentClient = new PaymentClient(config);

// Customer cancels order before shipment
await paymentClient.void({
    merchantVoidId: 'void-001',
    connectorTransactionId: auth.connectorTransactionId
});
// Funds released immediately
```

See: [`void`](../../api-reference/services/payment-service/void.md)

## Reverse (Refund Without Reference)

Let's take scenarios where you need to refund a payment but don't have the original payment reference stored in your system. Such businesses may only have the connector transaction ID from a webhook or external system. When you use hyperswitch-prism the flow will work like this.

```javascript
await paymentClient.reverse({
    connectorTransactionId: 'pi_3MqSCR2eZvKYlo2C1',
    amount: { minorAmount: 10000, currency: types.Currency.USD }
});
```

See: [`reverse`](../../api-reference/services/payment-service/reverse.md)

## Webhook Handling

Let's take businesses that need to process asynchronous payment events from multiple processors. Such businesses need a unified way to handle webhooks for payment status updates, refunds, disputes and more. When you use hyperswitch-prism the flow will work like this.

```javascript
const { EventClient } = require('hyperswitch-prism');
const types = require('hyperswitch-prism').types;

const config = {
    connectorConfig: {
        stripe: { apiKey: { value: process.env.STRIPE_API_KEY } }
    }
};
const eventClient = new EventClient(config);

// Express route for webhooks
app.post('/webhooks', async (req, res) => {
    try {
        const result = await eventClient.handleEvent({
            merchantEventId: `evt_${Date.now()}`,
            requestDetails: {
                method: 'POST',
                url: `${req.protocol}://${req.get('host')}${req.originalUrl}`,
                headers: req.headers,
                body: req.body
            },
            webhookSecrets: {
                secret: { value: process.env.STRIPE_WEBHOOK_SECRET }
            }
        });

        // Use normalized WebhookEventType enum
        switch (result.eventType) {
            case types.WebhookEventType.PAYMENT_INTENT_SUCCESS:
                await fulfillOrder(result.eventResponse?.paymentsResponse?.connectorTransactionId);
                break;
            case types.WebhookEventType.PAYMENT_INTENT_FAILURE:
                await notifyCustomer(result.eventResponse?.paymentsResponse?.connectorTransactionId);
                break;
            case types.WebhookEventType.PAYMENT_INTENT_CAPTURE_SUCCESS:
                await updateOrder(result.eventResponse?.paymentsResponse?.connectorTransactionId);
                break;
        }

        res.sendStatus(200);
    } catch (error) {
        console.error('Webhook error:', error.message);
        res.sendStatus(400);
    }
});
```

See: [`handleEvent`](../../api-reference/services/event-service/handle.md)

## Dispute Handling

Let's take scenarios where a customer disputes a charge with their bank or credit card company. Such businesses need to either accept the dispute and issue a refund, or defend it by providing evidence. When you use hyperswitch-prism the flow will work like this.

```javascript
const { DisputeClient } = require('hyperswitch-prism');

const config = {
    connectorConfig: {
        stripe: { apiKey: { value: process.env.STRIPE_API_KEY } }
    }
};
const disputeClient = new DisputeClient(config);

// Accept the dispute (refund immediately)
await disputeClient.accept({
    disputeId: 'dp_xyz789'
});

// Or defend with evidence
await disputeClient.defend({
    disputeId: 'dp_xyz789'
});

await disputeClient.submitEvidence({
    disputeId: 'dp_xyz789',
    evidence: {
        productDescription: 'Premium Widget',
        customerCommunication: emailThread,
        shippingReceipt: trackingNumber
    }
});
```

See: [`accept`](../../api-reference/services/dispute-service/accept.md), [`defend`](../../api-reference/services/dispute-service/defend.md), [`submitEvidence`](../../api-reference/services/dispute-service/submit-evidence.md)


## Next Steps

- Browse the full [API Reference](../../api-reference/)
- Jump to [SDK-specific guides](../../sdks/)