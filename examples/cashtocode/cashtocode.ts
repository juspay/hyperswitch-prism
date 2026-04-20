// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py cashtocode
//
// Cashtocode — all integration scenarios and flows in one file.
// Run a scenario:  npx tsx cashtocode.ts checkout_autocapture

import { EventClient, types } from 'hyperswitch-prism';
const { Environment } = types;
export const SUPPORTED_FLOWS = [];

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
    };
}


// ANCHOR: scenario_functions
// Flow: EventService.HandleEvent
async function handleEvent(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const eventClient = new EventClient(config);

    const handleResponse = await eventClient.handleEvent(_buildHandleEventRequest());

    return handleResponse;
}


// Export all process* functions for the smoke test
export {
    handleEvent, _buildHandleEventRequest
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
