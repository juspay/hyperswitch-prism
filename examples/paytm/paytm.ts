// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py paytm
//
// Paytm — all integration scenarios and flows in one file.
// Run a scenario:  npx tsx paytm.ts checkout_autocapture

import { PaymentClient, types } from 'hyperswitch-prism';
const { Environment, AuthenticationType, CaptureMethod, Currency } = types;
export const SUPPORTED_FLOWS = ["authorize"];

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
            "minorAmount": 1000,
            "currency": Currency.USD
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


// ANCHOR: scenario_functions
// Flow: PaymentService.Authorize (UpiCollect)
async function authorize(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const authorizeResponse = await paymentClient.authorize(_buildAuthorizeRequest(CaptureMethod.AUTOMATIC));

    return authorizeResponse;
}


// Export all process* functions for the smoke test
export {
    authorize, _buildAuthorizeRequest
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
