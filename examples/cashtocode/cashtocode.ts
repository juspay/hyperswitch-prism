// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py cashtocode
//
// Cashtocode — all integration scenarios and flows in one file.
// Run a scenario:  npx tsx cashtocode.ts checkout_autocapture

import { EventClient, types } from 'hyperswitch-prism';
const { Environment, HttpMethod } = types;
export const SUPPORTED_FLOWS = ["parse_event"];

const _defaultConfig: types.IConnectorConfig = {
    options: {
        environment: Environment.SANDBOX,
    },
    connectorConfig: {
        cashtocode: {
            baseUrl: 'YOUR_BASE_URL',
        }
    },
};


function _buildHandleEventRequest(): types.IEventServiceHandleRequest {
    return {
        "merchantEventId": "probe_event_001",  // Caller-supplied correlation key, echoed in the response. Not used by UCS for processing.
        "requestDetails": {
            "method": HttpMethod.HTTP_METHOD_POST,  // HTTP method of the request (e.g., GET, POST).
            "uri": "https://example.com/webhook",  // URI of the request.
            "headers": {  // Headers of the HTTP request.
            },
            "body": new Uint8Array(Buffer.from("{\"amount\":10.0,\"currency\":\"EUR\",\"foreignTransactionId\":\"probe_foreign_001\",\"type\":\"payment\",\"transactionId\":\"probe_txn_001\"}", "utf-8"))  // Body of the HTTP request.
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
            "body": new Uint8Array(Buffer.from("{\"amount\":10.0,\"currency\":\"EUR\",\"foreignTransactionId\":\"probe_foreign_001\",\"type\":\"payment\",\"transactionId\":\"probe_txn_001\"}", "utf-8"))  // Body of the HTTP request.
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
