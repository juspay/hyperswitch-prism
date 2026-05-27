// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py twoctwoppaco
//
// Twoctwoppaco — all integration scenarios and flows in one file.
// Run a scenario:  npx tsx twoctwoppaco.ts checkout_autocapture

import { PaymentClient, types } from 'hyperswitch-prism';
const { Environment } = types;
export const SUPPORTED_FLOWS = [];

const _defaultConfig: types.IConnectorConfig = {
    options: {
        environment: Environment.SANDBOX,
    },
    // connectorConfig: { twoctwoppaco: { apiKey: { value: 'YOUR_API_KEY' } } },
};


function _buildVerifyRedirectRequest(): types.IPaymentServiceVerifyRedirectResponseRequest {
    return {
    };
}


// ANCHOR: scenario_functions
// Flow: PaymentService.VerifyRedirectResponse
async function verifyRedirect(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const verifyResponse = await paymentClient.verifyRedirectResponse(_buildVerifyRedirectRequest());

    return verifyResponse;
}


// Export all process* functions for the smoke test
export {
    verifyRedirect, _buildVerifyRedirectRequest
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
