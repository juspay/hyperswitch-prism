// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py loonio
//
// Loonio — all integration scenarios and flows in one file.
// Run a scenario:  npx tsx loonio.ts checkout_autocapture

import { FraudClient, types } from 'hyperswitch-prism';
const { ConnectorConfig, ConnectorSpecificConfig, SdkOptions, Environment } = types;

const _defaultConfig: ConnectorConfig = {
    options: {
        environment: Environment.SANDBOX,
    },
};
// Standalone credentials (field names depend on connector auth type):
// _defaultConfig.connectorConfig = {
//     loonio: { apiKey: { value: 'YOUR_API_KEY' } }
// };


function _buildGetRequest(connectorTransactionId): FraudServiceGetRequest {
    return {
        "merchantTransactionId": "probe_merchant_txn_001",
        "connectorTransactionId": connectorTransactionId,
        "amount": {
            "minorAmount": 1000,
            "currency": "USD"
        }
    };
}


// ANCHOR: scenario_functions
// Flow: FraudService.Get
async function get(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<FraudServiceGetResponse> {
    const fraudClient = new FraudClient(config);

    const getResponse = await fraudClient.get(_buildGetRequest('probe_connector_txn_001'));

    return { status: getResponse.status };
}


// Export all process* functions for the smoke test
export {
    get, _buildGetRequest
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
