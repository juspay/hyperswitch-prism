// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py finix
//
// Finix — all integration scenarios and flows in one file.
// Run a scenario:  npx tsx finix.ts checkout_autocapture

import { PaymentClient, types } from 'hyperswitch-prism';
const { Environment } = types;
export const SUPPORTED_FLOWS = ["capture", "create_customer", "get", "recurring_charge", "refund", "refund_get", "token_authorize", "tokenize", "void"];

const _defaultConfig: types.IConnectorConfig = {
    options: {
        environment: Environment.SANDBOX,
    },
    // connectorConfig: { finix: { apiKey: { value: 'YOUR_API_KEY' } } },
};


// ANCHOR: scenario_functions
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

// Flow: PaymentService.create_customer
async function createCustomer(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    // Step 1: Create Customer — register customer record in the connector
    const createResponse = await paymentClient.create({
        "merchantCustomerId": "cust_probe_123",
        "customerName": "John Doe",
        "email": "test@example.com",
        "phoneNumber": "4155552671"
    });

    return createResponse;
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

// Flow: PaymentService.recurring_charge
async function recurringCharge(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    // Step 1: Recurring Charge — charge against the stored mandate
    const recurringResponse = await paymentClient.charge({
        "connectorRecurringPaymentId": {
        },
        "amount": {
        },
        "paymentMethod": {
        },
        "returnUrl": "https://example.com/recurring-return",
        "connectorCustomerId": "cust_probe_123",
        "paymentMethodType": "PAY_PAL",
        "offSession": true
    });

    if (recurringResponse.status === types.PaymentStatus.FAILURE) {
        throw new Error(`Recurring Charge failed: ${JSON.stringify(recurringResponse.error)}`);
    }

    return recurringResponse;
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

// Flow: PaymentService.token_authorize
async function tokenAuthorize(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    // Step 1: token_authorize
    const tokenResponse = await paymentClient.tokenAuthorize({
        "merchantTransactionId": "probe_tokenized_txn_001",
        "amount": {
        },
        "connectorToken": "pm_1AbcXyzStripeTestToken",
        "address": {
        },
        "captureMethod": "AUTOMATIC",
        "returnUrl": "https://example.com/return"
    });

    return tokenResponse;
}

// Flow: PaymentService.tokenize
async function tokenize(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    // Step 1: Tokenize — store card details and return a reusable token
    const tokenizeResponse = await paymentClient.tokenize({
        "amount": {
        },
        "paymentMethod": {
        },
        "customer": {
        },
        "address": {
        }
    });

    return tokenizeResponse;
}

// Flow: PaymentService.void
async function voidPayment(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    // Step 1: Void — release reserved funds (cancel authorization)
    const voidResponse = await paymentClient.void({
        "merchantVoidId": "probe_void_001",
        "connectorTransactionId": "probe_connector_txn_001"
    });

    return voidResponse;
}


// Export all process* functions for the smoke test
export {
    capture, createCustomer, get, recurringCharge, refund, refundGet, tokenAuthorize, tokenize, voidPayment
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
