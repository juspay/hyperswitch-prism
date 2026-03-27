// This file is auto-generated. Do not edit manually.
// Replace placeholder credentials with real values.
// Regenerate: python3 scripts/generate-connector-docs.py forte
//
// Forte — all integration scenarios and flows in one file.
// Run a scenario:  npx ts-node forte.ts checkout_autocapture

import { PaymentClient } from 'hyperswitch-prism';
import { types } from 'hyperswitch-prism';

const { ConnectorConfig, ConnectorSpecificConfig, SdkOptions, Environment } = types;

const _defaultConfig: types.IConnectorConfig = ConnectorConfig.create({
    options: SdkOptions.create({ environment: Environment.SANDBOX }),
    connectorConfig: ConnectorSpecificConfig.create({
        forte: {
            apiAccessId: { value: 'YOUR_API_ACCESS_ID' },
            organizationId: { value: 'YOUR_ORGANIZATION_ID' },
            locationId: { value: 'YOUR_LOCATION_ID' },
            apiSecretKey: { value: 'YOUR_API_SECRET_KEY' },
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
        "address": {
            "billingAddress": {
                "firstName": "John"
            }
        },
        "authType": "NO_THREE_DS",
        "returnUrl": "https://example.com/return"
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
// Direct bank debit (Ach). Bank transfers typically use `capture_method=AUTOMATIC`.
async function processCheckoutBank(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig): Promise<unknown> {
    const paymentClient = new PaymentClient(config);

    // Step 1: Authorize — reserve funds on the payment method
    const authorizeResponse = await paymentClient.authorize({
        "merchantTransactionId": "probe_txn_001",
        "amount": {
            "minorAmount": 1000,
            "currency": "USD"
        },
        "paymentMethod": {
            "ach": {
                "accountNumber": "000123456789",
                "routingNumber": "110000000",
                "bankAccountHolderName": "John Doe"
            }
        },
        "captureMethod": "AUTOMATIC",
        "address": {
            "billingAddress": {
                "firstName": "John"
            }
        },
        "authType": "NO_THREE_DS",
        "returnUrl": "https://example.com/return"
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

// Void a Payment
// Flow: authorize → void
// Authorize funds with a manual capture flag, then cancel the authorization with Void before any capture occurs. Releases the hold on the customer's funds.
async function processVoidPayment(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig): Promise<unknown> {
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
                "firstName": "John"
            }
        },
        "authType": "NO_THREE_DS",
        "returnUrl": "https://example.com/return"
    });

    if (authorizeResponse.status === 'FAILED') {
        throw new Error(`Payment failed: ${authorizeResponse.error?.message}`);
    }
    if (authorizeResponse.status === 'PENDING') {
        // Awaiting async confirmation — handle via webhook
        return { status: 'pending', transactionId: authorizeResponse.connectorTransactionId };
    }

    // Step 2: Void — release reserved funds (cancel authorization)
    const voidResponse = await paymentClient.void({
        "merchantVoidId": "probe_void_001",
        "connectorTransactionId": authorizeResponse.connectorTransactionId,  // from authorize response
    });

    return { status: voidResponse.status, transactionId: authorizeResponse.connectorTransactionId, error: voidResponse.error };
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
                "firstName": "John"
            }
        },
        "authType": "NO_THREE_DS",
        "returnUrl": "https://example.com/return"
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
                "firstName": "John"
            }
        },
        "authType": "NO_THREE_DS",
        "returnUrl": "https://example.com/return"
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

// Flow: PaymentService.void
async function voidPayment(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig): Promise<unknown> {
    // Step 1: Void — release reserved funds (cancel authorization)
    const voidResponse = await paymentClient.void({
        "merchantVoidId": "probe_void_001",
        "connectorTransactionId": "probe_connector_txn_001"
    });

    return { status: voidResponse.status };
}


export { processCheckoutAutocapture, processCheckoutBank, processVoidPayment, processGetPayment, authorize, get, voidPayment };

const _scenarioMap: Record<string, (id: string) => Promise<unknown>> = {
    processCheckoutAutocapture,
    processCheckoutBank,
    processVoidPayment,
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
