// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py trustly
//
// Trustly — all integration scenarios and flows in one file.
// Run a scenario:  npx tsx trustly.ts checkout_autocapture

import { EventClient, types } from 'hyperswitch-prism';
const { Environment, HttpMethod } = types;
export const SUPPORTED_FLOWS = ["parse_event"];

const _defaultConfig: types.IConnectorConfig = {
    options: {
        environment: Environment.SANDBOX,
    },
    connectorConfig: {
        trustly: {
            username: { value: 'YOUR_USERNAME' },
            password: { value: 'YOUR_PASSWORD' },
            privateKey: { value: 'YOUR_PRIVATE_KEY' },
            baseUrl: 'YOUR_BASE_URL',
        }
    },
};


function _buildHandleEventRequest(): types.IEventServiceHandleRequest {
    return {
        "merchantEventId": "probe_event_001",
        "requestDetails": {
            "method": HttpMethod.HTTP_METHOD_POST,  // HTTP method of the request (e.g., GET, POST).
            "legacyHeaders": {  // Headers of the HTTP request.
            },
            "body": {"value": new Uint8Array([123, 34, 109, 101, 116, 104, 111, 100, 34, 58, 34, 99, 104, 97, 114, 103, 101, 34, 44, 34, 112, 97, 114, 97, 109, 115, 34, 58, 123, 34, 100, 97, 116, 97, 34, 58, 123, 34, 111, 114, 100, 101, 114, 105, 100, 34, 58, 34, 112, 114, 111, 98, 101, 95, 111, 114, 100, 101, 114, 95, 48, 48, 49, 34, 44, 34, 97, 109, 111, 117, 110, 116, 34, 58, 34, 49, 48, 46, 48, 48, 34, 44, 34, 99, 117, 114, 114, 101, 110, 99, 121, 34, 58, 34, 69, 85, 82, 34, 44, 34, 101, 110, 100, 117, 115, 101, 114, 105, 100, 34, 58, 34, 112, 114, 111, 98, 101, 95, 117, 115, 101, 114, 34, 125, 125, 125])},  // Secret-typed replacements. Presence takes precedence over the legacy field, including an explicitly empty body, map, URI or query string.
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
            "body": {"value": new Uint8Array([123, 34, 109, 101, 116, 104, 111, 100, 34, 58, 34, 99, 104, 97, 114, 103, 101, 34, 44, 34, 112, 97, 114, 97, 109, 115, 34, 58, 123, 34, 100, 97, 116, 97, 34, 58, 123, 34, 111, 114, 100, 101, 114, 105, 100, 34, 58, 34, 112, 114, 111, 98, 101, 95, 111, 114, 100, 101, 114, 95, 48, 48, 49, 34, 44, 34, 97, 109, 111, 117, 110, 116, 34, 58, 34, 49, 48, 46, 48, 48, 34, 44, 34, 99, 117, 114, 114, 101, 110, 99, 121, 34, 58, 34, 69, 85, 82, 34, 44, 34, 101, 110, 100, 117, 115, 101, 114, 105, 100, 34, 58, 34, 112, 114, 111, 98, 101, 95, 117, 115, 101, 114, 34, 125, 125, 125])},  // Secret-typed replacements. Presence takes precedence over the legacy field, including an explicitly empty body, map, URI or query string.
            "uri": {"value": "https://example.com/webhook"}
        }
    };
}


// ANCHOR: scenario_functions
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
    handleEvent, parseEvent, _buildHandleEventRequest, _buildParseEventRequest
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
