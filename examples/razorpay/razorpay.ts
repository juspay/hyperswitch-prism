// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py razorpay
//
// Razorpay — all integration scenarios and flows in one file.
// Run a scenario:  npx tsx razorpay.ts checkout_autocapture

import { PaymentClient, MerchantAuthenticationClient, EventClient, RefundClient, types } from 'hyperswitch-prism';
const { Environment, Currency, HttpMethod } = types;
export const SUPPORTED_FLOWS = ["capture", "create_order", "create_server_session_authentication_token", "get", "parse_event", "refund", "refund_get"];

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
            "minorAmount": 1000,
            "currency": Currency.USD
        }
    };
}

function _buildCreateOrderRequest(): types.IPaymentServiceCreateOrderRequest {
    return {
        "merchantOrderId": "probe_order_001",  // Identification.
        "amount": {  // Amount Information.
            "minorAmount": 1000,
            "currency": Currency.USD
        }
    };
}

function _buildCreateServerSessionAuthenticationTokenRequest(): types.IMerchantAuthenticationServiceCreateServerSessionAuthenticationTokenRequest {
    return {
        "payment": {  // PayoutSessionContext payout = 6; // future FrmSessionContext frm = 7; // future.
            "amount": {
                "minorAmount": 1000,
                "currency": Currency.USD
            }
        }
    };
}

function _buildGetRequest(connectorTransactionId: string): types.IPaymentServiceGetRequest {
    return {
        "merchantTransactionId": "probe_merchant_txn_001",  // Identification.
        "connectorTransactionId": connectorTransactionId,
        "amount": {  // Amount Information.
            "minorAmount": 1000,
            "currency": Currency.USD
        }
    };
}

function _buildHandleEventRequest(): types.IEventServiceHandleRequest {
    return {
        "merchantEventId": "probe_event_001",
        "requestDetails": {
            "method": HttpMethod.HTTP_METHOD_POST,  // HTTP method of the request (e.g., GET, POST).
            "legacyHeaders": {  // Headers of the HTTP request.
            },
            "body": {"value": new Uint8Array([123, 34, 97, 99, 99, 111, 117, 110, 116, 95, 105, 100, 34, 58, 34, 112, 114, 111, 98, 101, 95, 97, 99, 99, 116, 34, 44, 34, 99, 111, 110, 116, 97, 105, 110, 115, 34, 58, 91, 34, 112, 97, 121, 109, 101, 110, 116, 34, 93, 44, 34, 101, 110, 116, 105, 116, 121, 34, 58, 34, 101, 118, 101, 110, 116, 34, 44, 34, 101, 118, 101, 110, 116, 34, 58, 34, 112, 97, 121, 109, 101, 110, 116, 46, 99, 97, 112, 116, 117, 114, 101, 100, 34, 44, 34, 112, 97, 121, 108, 111, 97, 100, 34, 58, 123, 34, 112, 97, 121, 109, 101, 110, 116, 34, 58, 123, 34, 101, 110, 116, 105, 116, 121, 34, 58, 123, 34, 105, 100, 34, 58, 34, 112, 97, 121, 95, 112, 114, 111, 98, 101, 48, 48, 49, 34, 44, 34, 101, 110, 116, 105, 116, 121, 34, 58, 34, 112, 97, 121, 109, 101, 110, 116, 34, 44, 34, 97, 109, 111, 117, 110, 116, 34, 58, 49, 48, 48, 48, 44, 34, 99, 117, 114, 114, 101, 110, 99, 121, 34, 58, 34, 85, 83, 68, 34, 44, 34, 115, 116, 97, 116, 117, 115, 34, 58, 34, 99, 97, 112, 116, 117, 114, 101, 100, 34, 44, 34, 111, 114, 100, 101, 114, 95, 105, 100, 34, 58, 34, 111, 114, 100, 101, 114, 95, 112, 114, 111, 98, 101, 48, 48, 49, 34, 125, 125, 125, 125])},  // Secret-typed replacements. Presence takes precedence over the legacy field, including an explicitly empty body, map, URI or query string.
            "uri": {"value": "https://example.com/webhook"}
        }
    };
}

function _buildParseEventRequest(): types.IEventServiceParseRequest {
    return {
        "requestDetails": {
            "method": HttpMethod.HTTP_METHOD_POST,  // HTTP method of the request (e.g., GET, POST).
            "legacyHeaders": {  // Headers of the HTTP request.
            },
            "body": {"value": new Uint8Array([123, 34, 97, 99, 99, 111, 117, 110, 116, 95, 105, 100, 34, 58, 34, 112, 114, 111, 98, 101, 95, 97, 99, 99, 116, 34, 44, 34, 99, 111, 110, 116, 97, 105, 110, 115, 34, 58, 91, 34, 112, 97, 121, 109, 101, 110, 116, 34, 93, 44, 34, 101, 110, 116, 105, 116, 121, 34, 58, 34, 101, 118, 101, 110, 116, 34, 44, 34, 101, 118, 101, 110, 116, 34, 58, 34, 112, 97, 121, 109, 101, 110, 116, 46, 99, 97, 112, 116, 117, 114, 101, 100, 34, 44, 34, 112, 97, 121, 108, 111, 97, 100, 34, 58, 123, 34, 112, 97, 121, 109, 101, 110, 116, 34, 58, 123, 34, 101, 110, 116, 105, 116, 121, 34, 58, 123, 34, 105, 100, 34, 58, 34, 112, 97, 121, 95, 112, 114, 111, 98, 101, 48, 48, 49, 34, 44, 34, 101, 110, 116, 105, 116, 121, 34, 58, 34, 112, 97, 121, 109, 101, 110, 116, 34, 44, 34, 97, 109, 111, 117, 110, 116, 34, 58, 49, 48, 48, 48, 44, 34, 99, 117, 114, 114, 101, 110, 99, 121, 34, 58, 34, 85, 83, 68, 34, 44, 34, 115, 116, 97, 116, 117, 115, 34, 58, 34, 99, 97, 112, 116, 117, 114, 101, 100, 34, 44, 34, 111, 114, 100, 101, 114, 95, 105, 100, 34, 58, 34, 111, 114, 100, 101, 114, 95, 112, 114, 111, 98, 101, 48, 48, 49, 34, 125, 125, 125, 125])},  // Secret-typed replacements. Presence takes precedence over the legacy field, including an explicitly empty body, map, URI or query string.
            "uri": {"value": "https://example.com/webhook"}
        }
    };
}

function _buildRefundRequest(connectorTransactionId: string): types.IPaymentServiceRefundRequest {
    return {
        "merchantRefundId": "probe_refund_001",  // Identification.
        "connectorTransactionId": connectorTransactionId,
        "paymentAmount": 1000,  // Amount Information.
        "refundAmount": {
            "minorAmount": 1000,
            "currency": Currency.USD
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

// Flow: MerchantAuthenticationService.CreateServerSessionAuthenticationToken
async function createServerSessionAuthenticationToken(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const merchantAuthenticationClient = new MerchantAuthenticationClient(config);

    const createResponse = await merchantAuthenticationClient.createServerSessionAuthenticationToken(_buildCreateServerSessionAuthenticationTokenRequest());

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
    capture, createOrder, createServerSessionAuthenticationToken, get, handleEvent, parseEvent, refund, refundGet, _buildCaptureRequest, _buildCreateOrderRequest, _buildCreateServerSessionAuthenticationTokenRequest, _buildGetRequest, _buildHandleEventRequest, _buildParseEventRequest, _buildRefundRequest, _buildRefundGetRequest
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
