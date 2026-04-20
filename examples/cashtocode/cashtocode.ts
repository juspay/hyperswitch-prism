// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py cashtocode
//
// Cashtocode — all integration scenarios and flows in one file.
// Run a scenario:  npx tsx cashtocode.ts checkout_autocapture

import { PaymentClient, types } from 'hyperswitch-prism';
const { Environment } = types;
export const SUPPORTED_FLOWS = ["parse_event"];

const _defaultConfig: types.IConnectorConfig = {
    options: {
        environment: Environment.SANDBOX,
    },
    // connectorConfig: { cashtocode: { apiKey: { value: 'YOUR_API_KEY' } } },
};


// ANCHOR: scenario_functions
// Flow: PaymentService.handle_event
async function handleEvent(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    // Step 1: handle_event
    const handleResponse = await paymentClient.handleEvent({
        "merchantEventId": "probe_event_001",
        "requestDetails": {
        }
    });

    return handleResponse;
}

// Flow: PaymentService.parse_event
async function parseEvent(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    // Step 1: parse_event
    const parseResponse = await paymentClient.parseEvent({
        "requestDetails": {
        }
    });

    return parseResponse;
}


// Export all process* functions for the smoke test
export {
    handleEvent, parseEvent
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
