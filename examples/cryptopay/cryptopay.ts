// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py cryptopay
//
// Cryptopay — all integration scenarios and flows in one file.
// Run a scenario:  npx tsx cryptopay.ts checkout_autocapture

import { PaymentClient, EventClient, types } from 'hyperswitch-prism';
const { Environment, Currency, HttpMethod } = types;
export const SUPPORTED_FLOWS = ["get", "parse_event"];

const _defaultConfig: types.IConnectorConfig = {
    options: {
        environment: Environment.SANDBOX,
    },
    connectorConfig: {
        cryptopay: {
            apiKey: { value: 'YOUR_API_KEY' },
            apiSecret: { value: 'YOUR_API_SECRET' },
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
            "body": {"value": new Uint8Array([123, 34, 116, 121, 112, 101, 34, 58, 34, 73, 110, 118, 111, 105, 99, 101, 34, 44, 34, 101, 118, 101, 110, 116, 34, 58, 34, 115, 116, 97, 116, 117, 115, 95, 99, 104, 97, 110, 103, 101, 100, 34, 44, 34, 100, 97, 116, 97, 34, 58, 123, 34, 105, 100, 34, 58, 34, 112, 114, 111, 98, 101, 95, 105, 110, 118, 111, 105, 99, 101, 95, 48, 48, 49, 34, 44, 34, 115, 116, 97, 116, 117, 115, 34, 58, 34, 99, 111, 109, 112, 108, 101, 116, 101, 100, 34, 44, 34, 112, 114, 105, 99, 101, 95, 97, 109, 111, 117, 110, 116, 34, 58, 34, 49, 48, 46, 48, 48, 34, 44, 34, 112, 114, 105, 99, 101, 95, 99, 117, 114, 114, 101, 110, 99, 121, 34, 58, 34, 85, 83, 68, 34, 44, 34, 110, 97, 109, 101, 34, 58, 34, 112, 114, 111, 98, 101, 95, 99, 104, 97, 114, 103, 101, 34, 125, 125])},  // Secret-typed replacements. Presence takes precedence over the legacy field, including an explicitly empty body, map, URI or query string.
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
            "body": {"value": new Uint8Array([123, 34, 116, 121, 112, 101, 34, 58, 34, 73, 110, 118, 111, 105, 99, 101, 34, 44, 34, 101, 118, 101, 110, 116, 34, 58, 34, 115, 116, 97, 116, 117, 115, 95, 99, 104, 97, 110, 103, 101, 100, 34, 44, 34, 100, 97, 116, 97, 34, 58, 123, 34, 105, 100, 34, 58, 34, 112, 114, 111, 98, 101, 95, 105, 110, 118, 111, 105, 99, 101, 95, 48, 48, 49, 34, 44, 34, 115, 116, 97, 116, 117, 115, 34, 58, 34, 99, 111, 109, 112, 108, 101, 116, 101, 100, 34, 44, 34, 112, 114, 105, 99, 101, 95, 97, 109, 111, 117, 110, 116, 34, 58, 34, 49, 48, 46, 48, 48, 34, 44, 34, 112, 114, 105, 99, 101, 95, 99, 117, 114, 114, 101, 110, 99, 121, 34, 58, 34, 85, 83, 68, 34, 44, 34, 110, 97, 109, 101, 34, 58, 34, 112, 114, 111, 98, 101, 95, 99, 104, 97, 114, 103, 101, 34, 125, 125])},  // Secret-typed replacements. Presence takes precedence over the legacy field, including an explicitly empty body, map, URI or query string.
            "uri": {"value": "https://example.com/webhook"}
        }
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


// Export all process* functions for the smoke test
export {
    get, handleEvent, parseEvent, _buildGetRequest, _buildHandleEventRequest, _buildParseEventRequest
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
