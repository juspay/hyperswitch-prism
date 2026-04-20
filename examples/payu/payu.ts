// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py payu
//
// Payu — all integration scenarios and flows in one file.
// Run a scenario:  npx tsx payu.ts checkout_autocapture

import { PaymentClient, types } from 'hyperswitch-prism';
const { Environment } = types;
export const SUPPORTED_FLOWS = ["authorize", "get"];

const _defaultConfig: types.IConnectorConfig = {
    options: {
        environment: Environment.SANDBOX,
    },
    // connectorConfig: { payu: { apiKey: { value: 'YOUR_API_KEY' } } },
};


// ANCHOR: scenario_functions
// Flow: PaymentService.authorize (UpiCollect)
async function authorize(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    // Step 1: Authorize — reserve funds on the payment method
    const authorizeResponse = await paymentClient.authorize({
        "merchantTransactionId": "probe_txn_001",
        "amount": {
        },
        "paymentMethod": {
        },
        "captureMethod": "AUTOMATIC",
        "address": {
        },
        "authType": "NO_THREE_DS",
        "returnUrl": "https://example.com/return",
        "browserInfo": {
        }
    });

    if (authorizeResponse.status === types.PaymentStatus.FAILURE) {
        throw new Error(`Payment failed: ${JSON.stringify(authorizeResponse.error)}`);
    }
    if (authorizeResponse.status === types.PaymentStatus.PENDING) {
        // Awaiting async confirmation — handle via webhook
        return { status: 'pending', transactionId: authorizeResponse.connectorTransactionId } as any;
    }

    return authorizeResponse;
}

// Flow: PaymentService.get
async function get(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    // Step 1: Get — retrieve current payment status from the connector
    const getResponse = await paymentClient.get({
        "merchantTransactionId": "probe_merchant_txn_001",
        "connectorTransactionId": "probe_connector_txn_001",
        "amount": {
        }
    });

    return getResponse;
}


// Export all process* functions for the smoke test
export {
    authorize, get
};

// CLI runner
if (require.main === module) {
    const scenario = process.argv[2] || 'checkout_autocapture';
    const key = 'process' + scenario.replace(/_([a-z])/g, (_, l) => l.toUpperCase()).replace(/^(.)/, c => c.toUpperCase());
    const fn = (globalThis as any)[key] || (exports as any)[key];
    if (!fn) {
        const available = Object.keys(exports).map(k =>
            k.replace(/^process/, '').replace(/([A-Z])/g, '_$1').toLowerCase().replace(/^_/, '')
        );
        console.error(`Unknown scenario: ${scenario}. Available: ${available.join(', ')}`);
        process.exit(1);
    }
    fn('order_001').catch(console.error);
}
