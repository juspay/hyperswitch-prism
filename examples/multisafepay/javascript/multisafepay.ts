// This file is auto-generated. Do not edit manually.
// Replace placeholder credentials with real values.
// Regenerate: python3 scripts/generate-connector-docs.py multisafepay
//
// Multisafepay — all integration scenarios and flows in one file.
// Run a scenario:  npx ts-node multisafepay.ts checkout_autocapture

import { PaymentClient } from 'hyperswitch-prism';
import { types } from 'hyperswitch-prism';

const { ConnectorConfig, ConnectorSpecificConfig, SdkOptions, Environment } = types;

const _defaultConfig: types.IConnectorConfig = ConnectorConfig.create({
    options: SdkOptions.create({ environment: Environment.SANDBOX }),
    connectorConfig: ConnectorSpecificConfig.create({
        multisafepay: {
            apiKey: { value: 'YOUR_API_KEY' },
        },
    }),
});


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
        "customer": {
            "email": "test@example.com"
        },
        "address": {
            "billingAddress": {
            }
        },
        "authType": "NO_THREE_DS",
        "returnUrl": "https://example.com/return",
        "description": "Probe payment"
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
        "customer": {
            "email": "test@example.com"
        },
        "address": {
            "billingAddress": {
            }
        },
        "authType": "NO_THREE_DS",
        "returnUrl": "https://example.com/return",
        "description": "Probe payment"
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
        "customer": {
            "email": "test@example.com"
        },
        "address": {
            "billingAddress": {
            }
        },
        "authType": "NO_THREE_DS",
        "returnUrl": "https://example.com/return",
        "description": "Probe payment"
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
        "customer": {
            "email": "test@example.com"
        },
        "address": {
            "billingAddress": {
            }
        },
        "authType": "NO_THREE_DS",
        "returnUrl": "https://example.com/return",
        "description": "Probe payment"
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
        "customer": {
            "email": "test@example.com"
        },
        "address": {
            "billingAddress": {
            }
        },
        "authType": "NO_THREE_DS",
        "returnUrl": "https://example.com/return",
        "description": "Probe payment"
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


export { processCheckoutAutocapture, processCheckoutWallet, processRefund, processGetPayment, authorize, get, refund };

const _scenarioMap: Record<string, (id: string) => Promise<unknown>> = {
    processCheckoutAutocapture,
    processCheckoutWallet,
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
