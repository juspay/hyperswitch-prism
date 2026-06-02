// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py mercadopago
//
// Mercadopago — all integration scenarios and flows in one file.
// Run a scenario:  npx tsx mercadopago.ts checkout_autocapture

import { MerchantAuthenticationClient, PaymentClient, types } from 'hyperswitch-prism';
const { Environment, CaptureMethod, Currency } = types;
export const SUPPORTED_FLOWS = ["create_server_session_authentication_token", "token_authorize"];

const _defaultConfig: types.IConnectorConfig = {
    options: {
        environment: Environment.SANDBOX,
    },
    connectorConfig: {
        mercadopago: {
            apiKey: { value: 'YOUR_API_KEY' },
            baseUrl: 'YOUR_BASE_URL',
        }
    },
};


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

function _buildTokenAuthorizeRequest(): types.IPaymentServiceTokenAuthorizeRequest {
    return {
        "merchantTransactionId": "probe_tokenized_txn_001",
        "amount": {
            "minorAmount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            "currency": Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        },
        "connectorToken": {"value": "pm_1AbcXyzStripeTestToken"},  // Connector-issued token. Replaces PaymentMethod entirely. Examples: Stripe pm_xxx, Adyen recurringDetailReference, Braintree nonce.
        "address": {
            "billingAddress": {
            }
        },
        "captureMethod": CaptureMethod.AUTOMATIC,
        "returnUrl": "https://example.com/return"
    };
}


// ANCHOR: scenario_functions
// Flow: MerchantAuthenticationService.CreateServerSessionAuthenticationToken
async function createServerSessionAuthenticationToken(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const merchantAuthenticationClient = new MerchantAuthenticationClient(config);

    const createResponse = await merchantAuthenticationClient.createServerSessionAuthenticationToken(_buildCreateServerSessionAuthenticationTokenRequest());

    return createResponse;
}

// Flow: PaymentService.TokenAuthorize
async function tokenAuthorize(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const tokenResponse = await paymentClient.tokenAuthorize(_buildTokenAuthorizeRequest());

    return tokenResponse;
}


// Export all process* functions for the smoke test
export {
    createServerSessionAuthenticationToken, tokenAuthorize, _buildCreateServerSessionAuthenticationTokenRequest, _buildTokenAuthorizeRequest
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
