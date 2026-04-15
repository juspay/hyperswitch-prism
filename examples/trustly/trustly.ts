// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py trustly
//
// Trustly — all integration scenarios and flows in one file.
// Run a scenario:  npx tsx trustly.ts checkout_autocapture

import { EventClient, PaymentClient, types } from 'hyperswitch-prism';
const { ConnectorConfig, ConnectorSpecificConfig, SdkOptions, Environment } = types;

const _defaultConfig: ConnectorConfig = {
    options: {
        environment: Environment.SANDBOX,
    },
};
// Standalone credentials (field names depend on connector auth type):
// _defaultConfig.connectorConfig = {
//     trustly: { apiKey: { value: 'YOUR_API_KEY' } }
// };


function _buildHandleEventRequest(): EventServiceHandleRequest {
    return {
    };
}


// ANCHOR: scenario_functions
// Flow: EventService.HandleEvent
async function handleEvent(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<EventServiceHandleResponse> {
    const eventClient = new EventClient(config);

    const handleResponse = await eventClient.handleEvent(_buildHandleEventRequest());

    return { status: handleResponse.status };
}

// Flow: PaymentService.parse_event
async function parseEvent(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<any> {
    // Step 1: parse_event
    const parseResponse = await paymentClient.parseEvent({
        // No required fields
    });

    return { status: parseResponse.status };
}


// Export all process* functions for the smoke test
export {
    handleEvent, parseEvent, _buildHandleEventRequest
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
