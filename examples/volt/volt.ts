// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py volt
//
// Volt — all integration scenarios and flows in one file.
// Run a scenario:  npx tsx volt.ts checkout_autocapture

import { MerchantAuthenticationClient, FraudClient, PaymentClient, types } from 'hyperswitch-prism';
const { ConnectorConfig, ConnectorSpecificConfig, SdkOptions, Environment, Currency } = types;

const _defaultConfig: ConnectorConfig = {
    options: {
        environment: Environment.SANDBOX,
    },
};
// Standalone credentials (field names depend on connector auth type):
// _defaultConfig.connectorConfig = {
//     volt: { apiKey: { value: 'YOUR_API_KEY' } }
// };


function _buildCreateServerAuthenticationTokenRequest(): MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest {
    return {
    };
}

function _buildGetRequest(connectorTransactionId): FraudServiceGetRequest {
    return {
        "merchantTransactionId": "probe_merchant_txn_001",
        "connectorTransactionId": connectorTransactionId,
        "amount": {
            "minorAmount": 1000,
            "currency": "USD"
        },
        "state": {
            "token": "probe_access_token",
            "expiresInSeconds": 3600,
            "tokenType": "Bearer"
        }
    };
}

function _buildRefundRequest(connectorTransactionId: string): PaymentServiceRefundRequest {
    return {
        "merchantRefundId": "probe_refund_001",  // Identification.
        "connectorTransactionId": connectorTransactionId,
        "paymentAmount": 1000,  // Amount Information.
        "refundAmount": {
            "minorAmount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            "currency": Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        },
        "reason": "customer_request",  // Reason for the refund.
        "state": {  // State data for access token storage and.
            "accessToken": {  // Access token obtained from connector.
                "token": {"value": "probe_access_token"},  // The token string.
                "expiresInSeconds": 3600,  // Expiration timestamp (seconds since epoch).
                "tokenType": "Bearer"  // Token type (e.g., "Bearer", "Basic").
            }
        }
    };
}


// ANCHOR: scenario_functions
// Flow: MerchantAuthenticationService.CreateServerAuthenticationToken
async function createServerAuthenticationToken(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse> {
    const merchantAuthenticationClient = new MerchantAuthenticationClient(config);

    const createResponse = await merchantAuthenticationClient.createServerAuthenticationToken(_buildCreateServerAuthenticationTokenRequest());

    return { status: createResponse.status };
}

// Flow: FraudService.Get
async function get(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<FraudServiceGetResponse> {
    const fraudClient = new FraudClient(config);

    const getResponse = await fraudClient.get(_buildGetRequest('probe_connector_txn_001'));

    return { status: getResponse.status };
}

// Flow: PaymentService.Refund
async function refund(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<RefundResponse> {
    const paymentClient = new PaymentClient(config);

    const refundResponse = await paymentClient.refund(_buildRefundRequest('probe_connector_txn_001'));

    return { status: refundResponse.status };
}


// Export all process* functions for the smoke test
export {
    createServerAuthenticationToken, get, refund, _buildCreateServerAuthenticationTokenRequest, _buildGetRequest, _buildRefundRequest
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
