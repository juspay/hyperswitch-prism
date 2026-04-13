// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py paytm
//
// Paytm — all integration scenarios and flows in one file.
// Run a scenario:  npx tsx paytm.ts checkout_autocapture

import { PaymentClient, MerchantAuthenticationClient, types } from 'hyperswitch-prism';
const { ConnectorConfig, ConnectorSpecificConfig, SdkOptions, Environment, AuthenticationType, CaptureMethod, Currency } = types;

const _defaultConfig: ConnectorConfig = {
    options: {
        environment: Environment.SANDBOX,
    },
};
// Standalone credentials (field names depend on connector auth type):
// _defaultConfig.connectorConfig = {
//     paytm: { apiKey: { value: 'YOUR_API_KEY' } }
// };


function _buildAuthorizeRequest(captureMethod: CaptureMethod): PaymentServiceAuthorizeRequest {
    return {
        "merchantTransactionId": "probe_txn_001",  // Identification.
        "amount": {  // The amount for the payment.
            "minorAmount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            "currency": Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        },
        "paymentMethod": {  // Payment method to be used.
            "upiCollect": {  // UPI Collect.
                "vpaId": {"value": "test@upi"}  // Virtual Payment Address.
            }
        },
        "captureMethod": captureMethod,  // Method for capturing the payment.
        "address": {  // Address Information.
            "billingAddress": {
            }
        },
        "authType": AuthenticationType.NO_THREE_DS,  // Authentication Details.
        "returnUrl": "https://example.com/return",  // URLs for Redirection and Webhooks.
        "sessionToken": "probe_session_token"  // Session and Token Information.
    };
}

function _buildCreateServerSessionAuthenticationTokenRequest(): MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenRequest {
    return {
        "domainContext": {
            "minorAmount": 1000,
            "currency": "USD"
        }
    };
}

function _buildGetRequest(connectorTransactionId: string): PaymentServiceGetRequest {
    return {
        "merchantTransactionId": "probe_merchant_txn_001",  // Identification.
        "connectorTransactionId": connectorTransactionId,
        "amount": {  // Amount Information.
            "minorAmount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            "currency": Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        }
    };
}


// ANCHOR: scenario_functions
// Flow: PaymentService.Authorize (UpiCollect)
async function authorize(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<PaymentServiceAuthorizeResponse> {
    const paymentClient = new PaymentClient(config);

    const authorizeResponse = await paymentClient.authorize(_buildAuthorizeRequest(CaptureMethod.AUTOMATIC));

    return { status: authorizeResponse.status, transactionId: authorizeResponse.connectorTransactionId };
}

// Flow: MerchantAuthenticationService.CreateServerSessionAuthenticationToken
async function createServerSessionAuthenticationToken(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenResponse> {
    const merchantAuthenticationClient = new MerchantAuthenticationClient(config);

    const createResponse = await merchantAuthenticationClient.createServerSessionAuthenticationToken(_buildCreateServerSessionAuthenticationTokenRequest());

    return { status: createResponse.status };
}

// Flow: PaymentService.Get
async function get(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<PaymentServiceGetResponse> {
    const paymentClient = new PaymentClient(config);

    const getResponse = await paymentClient.get(_buildGetRequest('probe_connector_txn_001'));

    return { status: getResponse.status };
}


// Export all process* functions for the smoke test
export {
    authorize, createServerSessionAuthenticationToken, get, _buildAuthorizeRequest, _buildCreateServerSessionAuthenticationTokenRequest, _buildGetRequest
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
