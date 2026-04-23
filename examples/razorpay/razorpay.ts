// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py razorpay
//
// Razorpay — all integration scenarios and flows in one file.
// Run a scenario:  npx tsx razorpay.ts checkout_autocapture

import { PaymentClient, EventClient, RefundClient, types } from 'hyperswitch-prism';
const { Environment, Currency, HttpMethod } = types;
export const SUPPORTED_FLOWS = ["capture", "create_order", "get", "parse_event", "refund", "refund_get"];

const _defaultConfig: types.IConnectorConfig = {
    options: {
        environment: Environment.SANDBOX,
    },
    // connectorConfig: { razorpay: { apiKey: { value: 'YOUR_API_KEY' } } },
};


function _buildCaptureRequest(connectorTransactionId: string): types.IPaymentServiceCaptureRequest {
    return {
        "merchantCaptureId": "probe_capture_001",  // Identification.
        "connectorTransactionId": connectorTransactionId,
        "amountToCapture": {  // Capture Details.
            "minorAmount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            "currency": Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        }
    };
}

function _buildCreateOrderRequest(): types.IPaymentServiceCreateOrderRequest {
    return {
        "merchantOrderId": "probe_order_001",  // Identification.
        "amount": {  // Amount Information.
            "minorAmount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            "currency": Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        }
    };
}

function _buildGetRequest(connectorTransactionId: string): types.IPaymentServiceGetRequest {
    return {
        "merchantTransactionId": "probe_merchant_txn_001",  // Identification.
        "connectorTransactionId": connectorTransactionId,
        "amount": {  // Amount Information.
            "minorAmount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            "currency": Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        }
    };
}

function _buildHandleEventRequest(): types.IEventServiceHandleRequest {
    return {
        "merchantEventId": "probe_event_001",  // Caller-supplied correlation key, echoed in the response. Not used by UCS for processing.
        "requestDetails": {
            "method": HttpMethod.HTTP_METHOD_POST,  // HTTP method of the request (e.g., GET, POST).
            "uri": "https://example.com/webhook",  // URI of the request.
            "headers": {  // Headers of the HTTP request.
            },
            "body": new Uint8Array(Buffer.from("{\"account_id\":\"probe_acct\",\"contains\":[\"payment\"],\"entity\":\"event\",\"event\":\"payment.captured\",\"payload\":{\"payment\":{\"entity\":{\"id\":\"pay_probe001\",\"entity\":\"payment\",\"amount\":1000,\"currency\":\"USD\",\"status\":\"captured\",\"order_id\":\"order_probe001\"}}}}", "utf-8"))  // Body of the HTTP request.
        }
    };
}

function _buildParseEventRequest(): types.IEventServiceParseRequest {
    return {
        "requestDetails": {
            "method": HttpMethod.HTTP_METHOD_POST,  // HTTP method of the request (e.g., GET, POST).
            "uri": "https://example.com/webhook",  // URI of the request.
            "headers": {  // Headers of the HTTP request.
            },
            "body": new Uint8Array(Buffer.from("{\"account_id\":\"probe_acct\",\"contains\":[\"payment\"],\"entity\":\"event\",\"event\":\"payment.captured\",\"payload\":{\"payment\":{\"entity\":{\"id\":\"pay_probe001\",\"entity\":\"payment\",\"amount\":1000,\"currency\":\"USD\",\"status\":\"captured\",\"order_id\":\"order_probe001\"}}}}", "utf-8"))  // Body of the HTTP request.
        }
    };
}

function _buildRefundRequest(connectorTransactionId: string): types.IPaymentServiceRefundRequest {
    return {
        "merchantRefundId": "probe_refund_001",  // Identification.
        "connectorTransactionId": connectorTransactionId,
        "paymentAmount": 1000,  // Amount Information.
        "refundAmount": {
            "minorAmount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            "currency": Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        },
        "reason": "customer_request"  // Reason for the refund.
    };
}

function _buildRefundGetRequest(): types.IRefundServiceGetRequest {
    return {
        "merchantRefundId": "probe_refund_001",  // Identification.
        "connectorTransactionId": "probe_connector_txn_001",
        "refundId": "probe_refund_id_001"  // Deprecated.
    };
}


// ANCHOR: scenario_functions
// Flow: PaymentService.Capture
async function capture(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const captureResponse = await paymentClient.capture(_buildCaptureRequest('probe_connector_txn_001'));

    return captureResponse;
}

// Flow: PaymentService.CreateOrder
async function createOrder(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const createResponse = await paymentClient.createOrder(_buildCreateOrderRequest());

    return createResponse;
}

// Flow: PaymentService.Get
async function get(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const getResponse = await paymentClient.get(_buildGetRequest('probe_connector_txn_001'));

    return getResponse;
}

// Flow: EventService.HandleEvent
async function handleEvent(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const eventClient = new EventClient(config);

    const handleResponse = await eventClient.handleEvent(_buildHandleEventRequest());

    return handleResponse;
}

// Flow: EventService.ParseEvent
async function parseEvent(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const eventClient = new EventClient(config);

    const parseResponse = await eventClient.parseEvent(_buildParseEventRequest());

    return parseResponse;
}

// Flow: PaymentService.Refund
async function refund(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const refundResponse = await paymentClient.refund(_buildRefundRequest('probe_connector_txn_001'));

    return refundResponse;
}

// Flow: RefundService.Get
async function refundGet(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const refundClient = new RefundClient(config);

    const refundResponse = await refundClient.refundGet(_buildRefundGetRequest());

    return refundResponse;
}


// Export all process* functions for the smoke test
export {
    capture, createOrder, get, handleEvent, parseEvent, refund, refundGet, _buildCaptureRequest, _buildCreateOrderRequest, _buildGetRequest, _buildHandleEventRequest, _buildParseEventRequest, _buildRefundRequest, _buildRefundGetRequest
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
