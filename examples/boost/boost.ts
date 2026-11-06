// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py boost
//
// Boost — all integration scenarios and flows in one file.
// Run a scenario:  npx tsx boost.ts checkout_autocapture

import { PaymentClient, EventClient, RefundClient, types } from 'hyperswitch-prism';
const { Environment, Currency, HttpMethod } = types;
export const SUPPORTED_FLOWS = ["get", "parse_event", "refund_get"];

const _defaultConfig: types.IConnectorConfig = {
    options: {
        environment: Environment.SANDBOX,
    },
    connectorConfig: {
        boost: {
            clientId: { value: 'YOUR_CLIENT_ID' },
            merchantSecret: { value: 'YOUR_MERCHANT_SECRET' },
            publicKey: { value: 'YOUR_PUBLIC_KEY' },
            baseUrl: 'YOUR_BASE_URL',
        }
    },
};


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
            "body": {"value": new Uint8Array([123, 34, 116, 105, 109, 101, 115, 116, 97, 109, 112, 34, 58, 34, 50, 48, 50, 53, 45, 48, 57, 45, 48, 57, 84, 48, 49, 58, 50, 50, 58, 53, 48, 46, 54, 53, 48, 51, 57, 56, 50, 50, 57, 90, 34, 44, 34, 117, 117, 105, 100, 34, 58, 34, 53, 101, 51, 99, 99, 49, 49, 51, 45, 48, 52, 50, 54, 45, 52, 52, 101, 50, 45, 57, 56, 53, 98, 45, 102, 57, 48, 98, 100, 100, 100, 100, 48, 54, 101, 56, 34, 44, 34, 97, 109, 111, 117, 110, 116, 34, 58, 49, 44, 34, 99, 117, 114, 114, 101, 110, 99, 121, 34, 58, 34, 77, 89, 82, 34, 44, 34, 99, 114, 101, 97, 116, 101, 100, 34, 58, 34, 50, 48, 50, 53, 45, 48, 57, 45, 48, 57, 84, 48, 49, 58, 50, 50, 58, 48, 54, 46, 55, 51, 57, 90, 34, 44, 34, 100, 101, 115, 99, 114, 105, 112, 116, 105, 111, 110, 34, 58, 34, 80, 97, 121, 109, 101, 110, 116, 34, 44, 34, 115, 116, 97, 116, 117, 115, 34, 58, 34, 115, 117, 99, 99, 101, 101, 100, 101, 100, 34, 44, 34, 112, 97, 121, 109, 101, 110, 116, 77, 101, 116, 104, 111, 100, 34, 58, 34, 99, 97, 114, 100, 34, 44, 34, 114, 101, 102, 101, 114, 101, 110, 99, 101, 73, 100, 34, 58, 34, 76, 108, 111, 102, 50, 97, 54, 73, 48, 69, 100, 86, 109, 85, 82, 76, 55, 89, 111, 76, 34, 125])},  // Secret-typed replacements. Presence takes precedence over the legacy field, including an explicitly empty body, map, URI or query string.
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
            "body": {"value": new Uint8Array([123, 34, 116, 105, 109, 101, 115, 116, 97, 109, 112, 34, 58, 34, 50, 48, 50, 53, 45, 48, 57, 45, 48, 57, 84, 48, 49, 58, 50, 50, 58, 53, 48, 46, 54, 53, 48, 51, 57, 56, 50, 50, 57, 90, 34, 44, 34, 117, 117, 105, 100, 34, 58, 34, 53, 101, 51, 99, 99, 49, 49, 51, 45, 48, 52, 50, 54, 45, 52, 52, 101, 50, 45, 57, 56, 53, 98, 45, 102, 57, 48, 98, 100, 100, 100, 100, 48, 54, 101, 56, 34, 44, 34, 97, 109, 111, 117, 110, 116, 34, 58, 49, 44, 34, 99, 117, 114, 114, 101, 110, 99, 121, 34, 58, 34, 77, 89, 82, 34, 44, 34, 99, 114, 101, 97, 116, 101, 100, 34, 58, 34, 50, 48, 50, 53, 45, 48, 57, 45, 48, 57, 84, 48, 49, 58, 50, 50, 58, 48, 54, 46, 55, 51, 57, 90, 34, 44, 34, 100, 101, 115, 99, 114, 105, 112, 116, 105, 111, 110, 34, 58, 34, 80, 97, 121, 109, 101, 110, 116, 34, 44, 34, 115, 116, 97, 116, 117, 115, 34, 58, 34, 115, 117, 99, 99, 101, 101, 100, 101, 100, 34, 44, 34, 112, 97, 121, 109, 101, 110, 116, 77, 101, 116, 104, 111, 100, 34, 58, 34, 99, 97, 114, 100, 34, 44, 34, 114, 101, 102, 101, 114, 101, 110, 99, 101, 73, 100, 34, 58, 34, 76, 108, 111, 102, 50, 97, 54, 73, 48, 69, 100, 86, 109, 85, 82, 76, 55, 89, 111, 76, 34, 125])},  // Secret-typed replacements. Presence takes precedence over the legacy field, including an explicitly empty body, map, URI or query string.
            "uri": {"value": "https://example.com/webhook"}
        }
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

// Flow: RefundService.Get
async function refundGet(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const refundClient = new RefundClient(config);

    const refundResponse = await refundClient.refundGet(_buildRefundGetRequest());

    return refundResponse;
}


// Export all process* functions for the smoke test
export {
    get, handleEvent, parseEvent, refundGet, _buildGetRequest, _buildHandleEventRequest, _buildParseEventRequest, _buildRefundGetRequest
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
