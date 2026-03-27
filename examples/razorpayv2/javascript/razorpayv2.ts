// This file is auto-generated. Do not edit manually.
// Replace placeholder credentials with real values.
// Regenerate: python3 scripts/generate-connector-docs.py razorpayv2
//
// Razorpayv2 — all integration scenarios and flows in one file.
// Run a scenario:  npx ts-node razorpayv2.ts checkout_autocapture

import { PaymentClient, TokenizedPaymentClient } from 'hyperswitch-prism';
import { types } from 'hyperswitch-prism';

const { ConnectorConfig, ConnectorSpecificConfig, SdkOptions, Environment } = types;

const _defaultConfig: types.IConnectorConfig = ConnectorConfig.create({
    options: SdkOptions.create({ environment: Environment.SANDBOX }),
    // connectorConfig: ConnectorSpecificConfig.create({
    //     razorpayv2: { apiKey: { value: 'YOUR_API_KEY' } }
    // }),
});


function _buildTokenizedAuthorizeRequest() {
    return {
        "merchantTransactionId": "probe_tokenized_txn_001",
        "amount": {
            "minorAmount": 1000,  // Amount in minor units (e.g., 1000 = $10.00)
            "currency": "USD"  // ISO 4217 currency code (e.g., "USD", "EUR")
        },
        "connectorToken": {"value": "pm_1AbcXyzStripeTestToken"},  // Connector-issued token. Replaces PaymentMethod entirely. Examples: Stripe pm_xxx, Adyen recurringDetailReference, Braintree nonce.
        "address": {
            "billingAddress": {
            }
        },
        "captureMethod": "AUTOMATIC",
        "returnUrl": "https://example.com/return",
        "merchantOrderId": "probe_order_001"
    };
}


// ANCHOR: scenario_functions
// Card Payment (Automatic Capture)
// Flow: authorize
// Authorize and capture in one call using `capture_method=AUTOMATIC`. Use for digital goods or immediate fulfillment.
async function processCheckoutAutocapture(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig): Promise<unknown> {
    const paymentClient = new PaymentClient(config);

    // Step 1: Authorize — reserve funds on the payment method
    const authorizeResponse = await paymentClient.authorize({
        "merchantTransactionId": "probe_txn_001",
        "amount": {
            "minorAmount": 1000,
            "currency": "USD"
        },
        "paymentMethod": {
            "card": {
                "cardNumber": "4111111111111111",
                "cardExpMonth": "03",
                "cardExpYear": "2030",
                "cardCvc": "737",
                "cardHolderName": "John Doe"
            }
        },
        "captureMethod": "AUTOMATIC",
        "address": {
            "billingAddress": {
            }
        },
        "authType": "NO_THREE_DS",
        "returnUrl": "https://example.com/return",
        "merchantOrderId": "probe_order_001"
    });

    if (authorizeResponse.status === 'FAILED') {
        throw new Error(`Payment failed: ${authorizeResponse.error?.message}`);
    }
    if (authorizeResponse.status === 'PENDING') {
        // Awaiting async confirmation — handle via webhook
        return { status: 'pending', transactionId: authorizeResponse.connectorTransactionId };
    }

    return { status: authorizeResponse.status, transactionId: authorizeResponse.connectorTransactionId, error: authorizeResponse.error };
}

// Wallet Payment (Google Pay / Apple Pay)
// Flow: authorize
// Wallet payments pass an encrypted token from the browser/device SDK. Pass the token blob directly — do not decrypt client-side.
async function processCheckoutWallet(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig): Promise<unknown> {
    const paymentClient = new PaymentClient(config);

    // Step 1: Authorize — reserve funds on the payment method
    const authorizeResponse = await paymentClient.authorize({
        "merchantTransactionId": "probe_txn_001",
        "amount": {
            "minorAmount": 1000,
            "currency": "USD"
        },
        "paymentMethod": {
            "googlePay": {
                "type": "CARD",
                "description": "Visa 1111",
                "info": {
                    "cardNetwork": "VISA",
                    "cardDetails": "1111"
                },
                "tokenizationData": {
                    "encryptedData": {
                        "tokenType": "PAYMENT_GATEWAY",
                        "token": "{\"id\":\"tok_probe_gpay\",\"object\":\"token\",\"type\":\"card\"}"
                    }
                }
            }
        },
        "captureMethod": "AUTOMATIC",
        "address": {
            "billingAddress": {
            }
        },
        "authType": "NO_THREE_DS",
        "returnUrl": "https://example.com/return",
        "merchantOrderId": "probe_order_001"
    });

    if (authorizeResponse.status === 'FAILED') {
        throw new Error(`Payment failed: ${authorizeResponse.error?.message}`);
    }
    if (authorizeResponse.status === 'PENDING') {
        // Awaiting async confirmation — handle via webhook
        return { status: 'pending', transactionId: authorizeResponse.connectorTransactionId };
    }

    return { status: authorizeResponse.status, transactionId: authorizeResponse.connectorTransactionId, error: authorizeResponse.error };
}

// Bank Transfer (SEPA / ACH / BACS)
// Flow: authorize
// Direct bank debit (Sepa). Bank transfers typically use `capture_method=AUTOMATIC`.
async function processCheckoutBank(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig): Promise<unknown> {
    const paymentClient = new PaymentClient(config);

    // Step 1: Authorize — reserve funds on the payment method
    const authorizeResponse = await paymentClient.authorize({
        "merchantTransactionId": "probe_txn_001",
        "amount": {
            "minorAmount": 1000,
            "currency": "EUR"
        },
        "paymentMethod": {
            "sepa": {
                "iban": "DE89370400440532013000",
                "bankAccountHolderName": "John Doe"
            }
        },
        "captureMethod": "AUTOMATIC",
        "address": {
            "billingAddress": {
            }
        },
        "authType": "NO_THREE_DS",
        "returnUrl": "https://example.com/return",
        "merchantOrderId": "probe_order_001"
    });

    if (authorizeResponse.status === 'FAILED') {
        throw new Error(`Payment failed: ${authorizeResponse.error?.message}`);
    }
    if (authorizeResponse.status === 'PENDING') {
        // Awaiting async confirmation — handle via webhook
        return { status: 'pending', transactionId: authorizeResponse.connectorTransactionId };
    }

    return { status: authorizeResponse.status, transactionId: authorizeResponse.connectorTransactionId, error: authorizeResponse.error };
}

// Refund a Payment
// Flow: authorize → refund
// Authorize with automatic capture, then refund the captured amount. `connector_transaction_id` from the Authorize response is reused for the Refund call.
async function processRefund(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig): Promise<unknown> {
    const paymentClient = new PaymentClient(config);

    // Step 1: Authorize — reserve funds on the payment method
    const authorizeResponse = await paymentClient.authorize({
        "merchantTransactionId": "probe_txn_001",
        "amount": {
            "minorAmount": 1000,
            "currency": "USD"
        },
        "paymentMethod": {
            "card": {
                "cardNumber": "4111111111111111",
                "cardExpMonth": "03",
                "cardExpYear": "2030",
                "cardCvc": "737",
                "cardHolderName": "John Doe"
            }
        },
        "captureMethod": "AUTOMATIC",
        "address": {
            "billingAddress": {
            }
        },
        "authType": "NO_THREE_DS",
        "returnUrl": "https://example.com/return",
        "merchantOrderId": "probe_order_001"
    });

    if (authorizeResponse.status === 'FAILED') {
        throw new Error(`Payment failed: ${authorizeResponse.error?.message}`);
    }
    if (authorizeResponse.status === 'PENDING') {
        // Awaiting async confirmation — handle via webhook
        return { status: 'pending', transactionId: authorizeResponse.connectorTransactionId };
    }

    // Step 2: Refund — return funds to the customer
    const refundResponse = await paymentClient.refund({
        "merchantRefundId": "probe_refund_001",
        "connectorTransactionId": authorizeResponse.connectorTransactionId,  // from authorize response
        "paymentAmount": 1000,
        "refundAmount": {
            "minorAmount": 1000,
            "currency": "USD"
        },
        "reason": "customer_request"
    });

    if (refundResponse.status === 'FAILED') {
        throw new Error(`Refund failed: ${refundResponse.error?.message}`);
    }

    return { status: refundResponse.status, error: refundResponse.error };
}

// Get Payment Status
// Flow: authorize → get
// Authorize a payment, then poll the connector for its current status using Get. Use this to sync payment state when webhooks are unavailable or delayed.
async function processGetPayment(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig): Promise<unknown> {
    const paymentClient = new PaymentClient(config);

    // Step 1: Authorize — reserve funds on the payment method
    const authorizeResponse = await paymentClient.authorize({
        "merchantTransactionId": "probe_txn_001",
        "amount": {
            "minorAmount": 1000,
            "currency": "USD"
        },
        "paymentMethod": {
            "card": {
                "cardNumber": "4111111111111111",
                "cardExpMonth": "03",
                "cardExpYear": "2030",
                "cardCvc": "737",
                "cardHolderName": "John Doe"
            }
        },
        "captureMethod": "MANUAL",
        "address": {
            "billingAddress": {
            }
        },
        "authType": "NO_THREE_DS",
        "returnUrl": "https://example.com/return",
        "merchantOrderId": "probe_order_001"
    });

    if (authorizeResponse.status === 'FAILED') {
        throw new Error(`Payment failed: ${authorizeResponse.error?.message}`);
    }
    if (authorizeResponse.status === 'PENDING') {
        // Awaiting async confirmation — handle via webhook
        return { status: 'pending', transactionId: authorizeResponse.connectorTransactionId };
    }

    // Step 2: Get — retrieve current payment status from the connector
    const getResponse = await paymentClient.get({
        "merchantTransactionId": "probe_merchant_txn_001",
        "connectorTransactionId": authorizeResponse.connectorTransactionId,  // from authorize response
        "amount": {
            "minorAmount": 1000,
            "currency": "USD"
        }
    });

    return { status: getResponse.status, transactionId: getResponse.connectorTransactionId, error: getResponse.error };
}

// Flow: PaymentService.authorize (Card)
async function authorize(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig): Promise<unknown> {
    // Step 1: Authorize — reserve funds on the payment method
    const authorizeResponse = await paymentClient.authorize({
        "merchantTransactionId": "probe_txn_001",
        "amount": {
            "minorAmount": 1000,
            "currency": "USD"
        },
        "paymentMethod": {
            "card": {
                "cardNumber": "4111111111111111",
                "cardExpMonth": "03",
                "cardExpYear": "2030",
                "cardCvc": "737",
                "cardHolderName": "John Doe"
            }
        },
        "captureMethod": "AUTOMATIC",
        "address": {
            "billingAddress": {
            }
        },
        "authType": "NO_THREE_DS",
        "returnUrl": "https://example.com/return",
        "merchantOrderId": "probe_order_001"
    });

    if (authorizeResponse.status === 'FAILED') {
        throw new Error(`Payment failed: ${authorizeResponse.error?.message}`);
    }
    if (authorizeResponse.status === 'PENDING') {
        // Awaiting async confirmation — handle via webhook
        return { status: 'pending', transactionId: authorizeResponse.connectorTransactionId };
    }

    return { status: authorizeResponse.status, transactionId: authorizeResponse.connectorTransactionId };
}

// Flow: PaymentService.create_order
async function createOrder(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig): Promise<unknown> {
    // Step 1: create_order
    const createResponse = await paymentClient.createOrder({
        "merchantOrderId": "probe_order_001",
        "amount": {
            "minorAmount": 1000,
            "currency": "USD"
        }
    });

    return { status: createResponse.status };
}

// Flow: PaymentService.get
async function get(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig): Promise<unknown> {
    // Step 1: Get — retrieve current payment status from the connector
    const getResponse = await paymentClient.get({
        "merchantTransactionId": "probe_merchant_txn_001",
        "connectorTransactionId": "probe_connector_txn_001",
        "amount": {
            "minorAmount": 1000,
            "currency": "USD"
        }
    });

    return { status: getResponse.status };
}

// Flow: PaymentService.refund
async function refund(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig): Promise<unknown> {
    // Step 1: Refund — return funds to the customer
    const refundResponse = await paymentClient.refund({
        "merchantRefundId": "probe_refund_001",
        "connectorTransactionId": "probe_connector_txn_001",
        "paymentAmount": 1000,
        "refundAmount": {
            "minorAmount": 1000,
            "currency": "USD"
        },
        "reason": "customer_request"
    });

    if (refundResponse.status === 'FAILED') {
        throw new Error(`Refund failed: ${refundResponse.error?.message}`);
    }

    return { status: refundResponse.status };
}

// Flow: TokenizedPaymentService.Authorize
async function tokenizedAuthorize(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig): Promise<unknown> {
    const tokenizedPaymentClient = new TokenizedPaymentClient(config);

    const tokenizedResponse = await tokenizedPaymentClient.tokenizedAuthorize(_buildTokenizedAuthorizeRequest());

    return { status: tokenizedResponse.status };
}


export { processCheckoutAutocapture, processCheckoutWallet, processCheckoutBank, processRefund, processGetPayment, authorize, createOrder, get, refund, tokenizedAuthorize, _buildTokenizedAuthorizeRequest };

const _scenarioMap: Record<string, (id: string) => Promise<unknown>> = {
    processCheckoutAutocapture,
    processCheckoutWallet,
    processCheckoutBank,
    processRefund,
    processGetPayment,
};

if (require.main === module) {
    const scenario = process.argv[2] || 'checkout_autocapture';
    const key = 'process' + scenario.replace(/_([a-z])/g, (_: string, l: string) => l.toUpperCase()).replace(/^(.)/, (c: string) => c.toUpperCase());
    const fn = _scenarioMap[key];
    if (!fn) {
        const available = Object.keys(_scenarioMap).map(k =>
            k.replace(/^process/, '').replace(/([A-Z])/g, '_$1').toLowerCase().replace(/^_/, '')
        );
        console.error(`Unknown scenario: ${scenario}. Available: ${available.join(', ')}`);
        process.exit(1);
    }
    fn('order_001').catch(console.error);
}
