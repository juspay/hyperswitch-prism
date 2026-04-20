// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py redsys
//
// Redsys — all integration scenarios and flows in one file.
// Run a scenario:  npx tsx redsys.ts checkout_autocapture

import { PaymentClient, types } from 'hyperswitch-prism';
const { Environment } = types;
export const SUPPORTED_FLOWS = ["authenticate", "capture", "get", "pre_authenticate", "refund", "refund_get", "void"];

const _defaultConfig: types.IConnectorConfig = {
    options: {
        environment: Environment.SANDBOX,
    },
    // connectorConfig: { redsys: { apiKey: { value: 'YOUR_API_KEY' } } },
};


// ANCHOR: scenario_functions
// Flow: PaymentService.authenticate
async function authenticate(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    // Step 1: Authenticate — execute 3DS challenge or frictionless verification
    const authenticateResponse = await paymentClient.authenticate({
        "amount": {
        },
        "paymentMethod": {
        },
        "address": {
        },
        "authenticationData": {
        },
        "returnUrl": "https://example.com/3ds-return",
        "continueRedirectionUrl": "https://example.com/3ds-continue",
        "browserInfo": {
        }
    });

    return authenticateResponse;
}

// Flow: PaymentService.capture
async function capture(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    // Step 1: Capture — settle the reserved funds
    const captureResponse = await paymentClient.capture({
        "merchantCaptureId": "probe_capture_001",
        "connectorTransactionId": "probe_connector_txn_001",
        "amountToCapture": {
        }
    });

    if (captureResponse.status === types.PaymentStatus.FAILURE) {
        throw new Error(`Capture failed: ${JSON.stringify(captureResponse.error)}`);
    }

    return captureResponse;
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

// Flow: PaymentService.pre_authenticate
async function preAuthenticate(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    // Step 1: Pre-Authenticate — initiate 3DS flow (collect device/browser data)
    const preAuthenticateresponse = await paymentClient.preAuthenticate({
        "amount": {
        },
        "paymentMethod": {
        },
        "address": {
        },
        "enrolledFor_3ds": false,
        "returnUrl": "https://example.com/3ds-return"
    });

    return preResponse;
}

// Flow: PaymentService.refund
async function refund(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    // Step 1: Refund — return funds to the customer
    const refundResponse = await paymentClient.refund({
        "merchantRefundId": "probe_refund_001",
        "connectorTransactionId": "probe_connector_txn_001",
        "paymentAmount": 1000,
        "refundAmount": {
        },
        "reason": "customer_request"
    });

    if (refundResponse.status === types.RefundStatus.REFUND_FAILURE) {
        throw new Error(`Refund failed: ${JSON.stringify(refundResponse.error)}`);
    }

    return refundResponse;
}

// Flow: PaymentService.refund_get
async function refundGet(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    // Step 1: refund_get
    const refundResponse = await paymentClient.refundGet({
        "merchantRefundId": "probe_refund_001",
        "connectorTransactionId": "probe_connector_txn_001",
        "refundId": "probe_refund_id_001"
    });

    return refundResponse;
}

// Flow: PaymentService.void
async function voidPayment(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    // Step 1: Void — release reserved funds (cancel authorization)
    const voidResponse = await paymentClient.void({
        "merchantVoidId": "probe_void_001",
        "connectorTransactionId": "probe_connector_txn_001",
        "amount": {
        }
    });

    return voidResponse;
}


// Export all process* functions for the smoke test
export {
    authenticate, capture, get, preAuthenticate, refund, refundGet, voidPayment
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
