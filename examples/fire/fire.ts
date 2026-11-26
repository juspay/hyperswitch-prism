// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py fire
//
// Fire — all integration scenarios and flows in one file.
// Run a scenario:  npx tsx fire.ts checkout_autocapture

import { MerchantAuthenticationClient, EventClient, PaymentClient, types } from 'hyperswitch-prism';
const { Environment, HttpMethod } = types;
export const SUPPORTED_FLOWS = ["create_server_authentication_token", "parse_event"];

const _defaultConfig: types.IConnectorConfig = {
    options: {
        environment: Environment.SANDBOX,
    },
    connectorConfig: {
        fire: {
            clientId: { value: 'YOUR_CLIENT_ID' },
            clientKey: { value: 'YOUR_CLIENT_KEY' },
            refreshToken: { value: 'YOUR_REFRESH_TOKEN' },
            icanTo: { value: 'YOUR_ICAN_TO' },
            baseUrl: 'YOUR_BASE_URL',
            webhookSecret: { value: 'YOUR_WEBHOOK_SECRET' },
        }
    },
};


function _buildCreateServerAuthenticationTokenRequest(): types.IMerchantAuthenticationServiceCreateServerAuthenticationTokenRequest {
    return {
    };
}

function _buildHandleEventRequest(): types.IEventServiceHandleRequest {
    return {
        "merchantEventId": "probe_event_001",
        "requestDetails": {
            "method": HttpMethod.HTTP_METHOD_POST,  // HTTP method of the request (e.g., GET, POST).
            "uri": "https://example.com/webhook",  // URI of the request.
            "headers": {  // Headers of the HTTP request.
            },
            "body": new Uint8Array(Buffer.from("eyJraWQiOiI2NjAzZWQzYi1iYzMwLTQ3ZmItYjVlMy1iMjFjOWFiYmNlYzkiLCJhbGciOiJIUzI1NiJ9.eyJ0eXBlIjoiT1RIRVIiLCJzdGF0dXMiOiJBVVRIT1JJU0VEIiwidHJhbnNhY3Rpb25UeXBlIjoiUEFZTUVOVCIsInBheW1lbnRVdWlkIjoiM2YyZDBhMjAtZTg0Yi00MWMxLTlkM2QtYTFiMmMzZDRlNWY2In0.c2lnbmF0dXJl", "utf-8"))  // Body of the HTTP request.
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
            "body": new Uint8Array(Buffer.from("eyJraWQiOiI2NjAzZWQzYi1iYzMwLTQ3ZmItYjVlMy1iMjFjOWFiYmNlYzkiLCJhbGciOiJIUzI1NiJ9.eyJ0eXBlIjoiT1RIRVIiLCJzdGF0dXMiOiJBVVRIT1JJU0VEIiwidHJhbnNhY3Rpb25UeXBlIjoiUEFZTUVOVCIsInBheW1lbnRVdWlkIjoiM2YyZDBhMjAtZTg0Yi00MWMxLTlkM2QtYTFiMmMzZDRlNWY2In0.c2lnbmF0dXJl", "utf-8"))  // Body of the HTTP request.
        }
    };
}

function _buildVerifyRedirectRequest(): types.IPaymentServiceVerifyRedirectResponseRequest {
    return {
    };
}


// ANCHOR: scenario_functions
// Flow: MerchantAuthenticationService.CreateServerAuthenticationToken
async function createServerAuthenticationToken(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const merchantAuthenticationClient = new MerchantAuthenticationClient(config);

    const createResponse = await merchantAuthenticationClient.createServerAuthenticationToken(_buildCreateServerAuthenticationTokenRequest());

    return createResponse;
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

// Flow: PaymentService.VerifyRedirectResponse
async function verifyRedirect(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const verifyResponse = await paymentClient.verifyRedirectResponse(_buildVerifyRedirectRequest());

    return verifyResponse;
}


// Export all process* functions for the smoke test
export {
    createServerAuthenticationToken, handleEvent, parseEvent, verifyRedirect, _buildCreateServerAuthenticationTokenRequest, _buildHandleEventRequest, _buildParseEventRequest, _buildVerifyRedirectRequest
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
