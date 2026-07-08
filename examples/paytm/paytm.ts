// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py paytm
//
// Paytm — all integration scenarios and flows in one file.
// Run a scenario:  npx tsx paytm.ts checkout_autocapture

import { PaymentClient, MerchantAuthenticationClient, types } from 'hyperswitch-prism';
const { Environment, AuthenticationType, CaptureMethod, Currency } = types;
export const SUPPORTED_FLOWS = ["authorize", "create_server_session_authentication_token", "get"];

const _defaultConfig: types.IConnectorConfig = {
    options: {
        environment: Environment.SANDBOX,
    },
    connectorConfig: {
        paytm: {
            merchantId: { value: 'YOUR_MERCHANT_ID' },
            merchantKey: { value: 'YOUR_MERCHANT_KEY' },
            website: { value: 'YOUR_WEBSITE' },
            clientId: { value: 'YOUR_CLIENT_ID' },
            baseUrl: 'YOUR_BASE_URL',
        }
    },
};


function _buildAuthorizeRequest(captureMethod: types.CaptureMethod): types.IPaymentServiceAuthorizeRequest {
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

function _buildCreateServerSessionAuthenticationTokenRequest(): types.IMerchantAuthenticationServiceCreateServerSessionAuthenticationTokenRequest {
    return {
        "payment": {  // PayoutSessionContext payout = 6; // future FrmSessionContext frm = 7; // future.
            "amount": {
                "minorAmount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
                "currency": Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
            }
        }
    };
}

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


// ANCHOR: scenario_functions
// Flow: PaymentService.Authorize (UpiCollect)
async function authorize(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const authorizeResponse = await paymentClient.authorize(_buildAuthorizeRequest(CaptureMethod.AUTOMATIC));

    return authorizeResponse;
}

// Flow: MerchantAuthenticationService.CreateServerSessionAuthenticationToken
async function createServerSessionAuthenticationToken(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const merchantAuthenticationClient = new MerchantAuthenticationClient(config);

    const createResponse = await merchantAuthenticationClient.createServerSessionAuthenticationToken(_buildCreateServerSessionAuthenticationTokenRequest());

    return createResponse;
}

// Flow: PaymentService.Get
async function get(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const getResponse = await paymentClient.get(_buildGetRequest('probe_connector_txn_001'));

    return getResponse;
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
