// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py itaubank
//
// Itaubank — all integration scenarios and flows in one file.
// Run a scenario:  npx tsx itaubank.ts checkout_autocapture

import { MerchantAuthenticationClient, types } from 'hyperswitch-prism';
const { Environment } = types;
export const SUPPORTED_FLOWS = ["create_server_authentication_token"];

const _defaultConfig: types.IConnectorConfig = {
    options: {
        environment: Environment.SANDBOX,
    },
    connectorConfig: {
        itaubank: {
            clientSecret: { value: 'YOUR_CLIENT_SECRET' },
            clientId: { value: 'YOUR_CLIENT_ID' },
            baseUrl: 'YOUR_BASE_URL',
        }
    },
};


function _buildCreateServerAuthenticationTokenRequest(): types.IMerchantAuthenticationServiceCreateServerAuthenticationTokenRequest {
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


// Export all process* functions for the smoke test
export {
    createServerAuthenticationToken, _buildCreateServerAuthenticationTokenRequest
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
