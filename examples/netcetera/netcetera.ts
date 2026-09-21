// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py netcetera
//
// Netcetera — all integration scenarios and flows in one file.
// Run a scenario:  npx tsx netcetera.ts checkout_autocapture

import { PaymentMethodAuthenticationClient, types } from 'hyperswitch-prism';
const { Environment, Currency } = types;
export const SUPPORTED_FLOWS = ["authenticate", "post_authenticate", "pre_authenticate"];

const _defaultConfig: types.IConnectorConfig = {
    options: {
        environment: Environment.SANDBOX,
    },
    connectorConfig: {
        netcetera: {
            certificate: { value: 'YOUR_CERTIFICATE' },
            privateKey: { value: 'YOUR_PRIVATE_KEY' },
            threeDsRequestorId: 'YOUR_THREE_DS_REQUESTOR_ID',
            threeDsRequestorName: 'YOUR_THREE_DS_REQUESTOR_NAME',
            merchantConfigurationId: 'YOUR_MERCHANT_CONFIGURATION_ID',
            baseUrl: 'YOUR_BASE_URL',
        }
    },
};


function _buildAuthenticateRequest(): types.IPaymentMethodAuthenticationServiceAuthenticateRequest {
    return {
        "amount": {  // Amount Information.
            "minorAmount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            "currency": Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        },
        "paymentMethod": {  // Payment Method.
            "card": {  // Generic card payment.
                "cardNumber": {"value": "4111111111111111"},  // Card Identification.
                "cardExpMonth": {"value": "03"},
                "cardExpYear": {"value": "2030"},
                "cardCvc": {"value": "737"},
                "cardHolderName": {"value": "John Doe"}  // Cardholder Information.
            }
        },
        "address": {  // Address Information.
            "billingAddress": {
            }
        },
        "returnUrl": "https://example.com/3ds-return"  // URLs for Redirection. For 3DS this is the browser challenge return URL (EMVCo notificationURL / threeDSRequestorURL).
    };
}

function _buildPostAuthenticateRequest(): types.IPaymentMethodAuthenticationServicePostAuthenticateRequest {
    return {
        "amount": {  // Amount Information.
            "minorAmount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            "currency": Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        },
        "paymentMethod": {  // Payment Method.
            "card": {  // Generic card payment.
                "cardNumber": {"value": "4111111111111111"},  // Card Identification.
                "cardExpMonth": {"value": "03"},
                "cardExpYear": {"value": "2030"},
                "cardCvc": {"value": "737"},
                "cardHolderName": {"value": "John Doe"}  // Cardholder Information.
            }
        },
        "address": {  // Address Information.
            "billingAddress": {
            }
        }
    };
}

function _buildPreAuthenticateRequest(): types.IPaymentMethodAuthenticationServicePreAuthenticateRequest {
    return {
        "amount": {  // Amount Information.
            "minorAmount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            "currency": Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        },
        "paymentMethod": {  // Payment Method.
            "card": {  // Generic card payment.
                "cardNumber": {"value": "4111111111111111"},  // Card Identification.
                "cardExpMonth": {"value": "03"},
                "cardExpYear": {"value": "2030"},
                "cardCvc": {"value": "737"},
                "cardHolderName": {"value": "John Doe"}  // Cardholder Information.
            }
        },
        "address": {  // Address Information.
            "billingAddress": {
            }
        },
        "enrolledFor_3ds": false,  // Authentication Details.
        "returnUrl": "https://example.com/3ds-return"  // URLs for Redirection.
    };
}


// ANCHOR: scenario_functions
// Flow: PaymentMethodAuthenticationService.Authenticate
async function authenticate(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentMethodAuthenticationClient = new PaymentMethodAuthenticationClient(config);

    const authenticateResponse = await paymentMethodAuthenticationClient.authenticate(_buildAuthenticateRequest());

    return authenticateResponse;
}

// Flow: PaymentMethodAuthenticationService.PostAuthenticate
async function postAuthenticate(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentMethodAuthenticationClient = new PaymentMethodAuthenticationClient(config);

    const postResponse = await paymentMethodAuthenticationClient.postAuthenticate(_buildPostAuthenticateRequest());

    return postResponse;
}

// Flow: PaymentMethodAuthenticationService.PreAuthenticate
async function preAuthenticate(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentMethodAuthenticationClient = new PaymentMethodAuthenticationClient(config);

    const preResponse = await paymentMethodAuthenticationClient.preAuthenticate(_buildPreAuthenticateRequest());

    return preResponse;
}


// Export all process* functions for the smoke test
export {
    authenticate, postAuthenticate, preAuthenticate, _buildAuthenticateRequest, _buildPostAuthenticateRequest, _buildPreAuthenticateRequest
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
