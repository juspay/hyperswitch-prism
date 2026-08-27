// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py payhound
//
// Payhound — all integration scenarios and flows in one file.
// Run a scenario:  npx tsx payhound.ts checkout_autocapture

import { PaymentClient, EventClient, types } from 'hyperswitch-prism';
const { Environment, Currency, HttpMethod } = types;
export const SUPPORTED_FLOWS = ["get", "parse_event"];

const _defaultConfig: types.IConnectorConfig = {
    options: {
        environment: Environment.SANDBOX,
    },
    connectorConfig: {
        payhound: {
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
            "body": new Uint8Array(Buffer.from("{\"id\":\"378d8ec6e305f469b009cb4e2deedf93\",\"status\":\"completed\",\"address\":\"lAeMbkpHia8FVuKczQKUrv9uMzv7uClHZi\",\"merchant_currency\":\"EUR\",\"merchant_amount\":\"266.45\",\"invoice_currency\":\"BTC\",\"invoice_amount\":\"0.88613839\",\"paid_currency\":\"BTC\",\"paid_amount\":\"0.88613839\",\"reference\":\"probe_ref\",\"invoice_url\":\"/invoices/378d8ec6e305f469b009cb4e2deedf93\",\"create_time\":1398871897.0,\"valid_until_time\":1398872497.0}", "utf-8"))  // Body of the HTTP request.
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
            "body": new Uint8Array(Buffer.from("{\"id\":\"378d8ec6e305f469b009cb4e2deedf93\",\"status\":\"completed\",\"address\":\"lAeMbkpHia8FVuKczQKUrv9uMzv7uClHZi\",\"merchant_currency\":\"EUR\",\"merchant_amount\":\"266.45\",\"invoice_currency\":\"BTC\",\"invoice_amount\":\"0.88613839\",\"paid_currency\":\"BTC\",\"paid_amount\":\"0.88613839\",\"reference\":\"probe_ref\",\"invoice_url\":\"/invoices/378d8ec6e305f469b009cb4e2deedf93\",\"create_time\":1398871897.0,\"valid_until_time\":1398872497.0}", "utf-8"))  // Body of the HTTP request.
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
