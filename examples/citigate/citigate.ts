// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py citigate
//
// Citigate — all integration scenarios and flows in one file.
// Run a scenario:  npx tsx citigate.ts checkout_autocapture

import { PaymentClient, types } from 'hyperswitch-prism';
const { Environment, AuthenticationType, CaptureMethod, CardNetwork, CountryAlpha2, Currency } = types;
export const SUPPORTED_FLOWS = ["authorize", "proxy_authorize"];

const _defaultConfig: types.IConnectorConfig = {
    options: {
        environment: Environment.SANDBOX,
    },
    connectorConfig: {
        citigate: {
            apiKey: { value: 'YOUR_API_KEY' },
            key1: { value: 'YOUR_KEY1' },
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
            "card": {  // Generic card payment.
                "cardNumber": {"value": "4111111111111111"},  // Card Identification.
                "cardExpMonth": {"value": "03"},
                "cardExpYear": {"value": "2030"},
                "cardCvc": {"value": "737"},
                "cardHolderName": {"value": "John Doe"}  // Cardholder Information.
            }
        },
        "captureMethod": captureMethod,  // Method for capturing the payment.
        "address": {  // Address Information.
            "billingAddress": {
                "firstName": {"value": "John"},  // Personal Information.
                "lastName": {"value": "Doe"},
                "line1": {"value": "123 Main St"},  // Address Details.
                "city": {"value": "Seattle"},
                "zipCode": {"value": "98101"},
                "countryAlpha2Code": CountryAlpha2.US,
                "email": {"value": "test@example.com"}  // Contact Information.
            }
        },
        "authType": AuthenticationType.NO_THREE_DS,  // Authentication Details.
        "returnUrl": "https://example.com/return",  // URLs for Redirection and Webhooks.
        "browserInfo": {
            "ipAddress": "1.2.3.4"  // Device Information.
        }
    };
}

function _buildProxyAuthorizeRequest(): types.IPaymentServiceProxyAuthorizeRequest {
    return {
        "merchantTransactionId": "probe_proxy_txn_001",
        "amount": {
            "minorAmount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            "currency": Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        },
        "cardProxy": {  // Card proxy for vault-aliased payments (VGS, Basis Theory, Spreedly). Real card values are substituted by the proxy before reaching the connector.
            "cardNumber": {"value": "4111111111111111"},  // Card Identification.
            "cardExpMonth": {"value": "03"},
            "cardExpYear": {"value": "2030"},
            "cardCvc": {"value": "123"},
            "cardHolderName": {"value": "John Doe"},  // Cardholder Information.
            "cardNetwork": CardNetwork.VISA
        },
        "address": {
            "billingAddress": {
                "firstName": {"value": "John"},  // Personal Information.
                "lastName": {"value": "Doe"},
                "line1": {"value": "123 Main St"},  // Address Details.
                "city": {"value": "Seattle"},
                "zipCode": {"value": "98101"},
                "countryAlpha2Code": CountryAlpha2.US,
                "email": {"value": "test@example.com"}  // Contact Information.
            }
        },
        "captureMethod": CaptureMethod.AUTOMATIC,
        "authType": AuthenticationType.NO_THREE_DS,
        "returnUrl": "https://example.com/return",
        "browserInfo": {
            "ipAddress": "1.2.3.4"  // Device Information.
        }
    };
}


// ANCHOR: scenario_functions
// One-step Payment (Authorize + Capture)
// Simple payment that authorizes and captures in one call. Use for immediate charges.
async function processCheckoutAutocapture(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    // Step 1: Authorize — reserve funds on the payment method
    const authorizeResponse = await paymentClient.authorize(_buildAuthorizeRequest(CaptureMethod.AUTOMATIC));

    if (authorizeResponse.status === types.PaymentStatus.FAILURE) {
        throw new Error(`Payment failed: ${JSON.stringify(authorizeResponse.error)}`);
    }
    if (authorizeResponse.status === types.PaymentStatus.PENDING) {
        // Awaiting async confirmation — handle via webhook
        return { status: 'pending', connectorTransactionId: authorizeResponse.connectorTransactionId };
    }

    return { status: authorizeResponse.status, transactionId: authorizeResponse.connectorTransactionId!, error: authorizeResponse.error } as any;
}

// Flow: PaymentService.Authorize (Card)
async function authorize(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const authorizeResponse = await paymentClient.authorize(_buildAuthorizeRequest(CaptureMethod.AUTOMATIC));

    return authorizeResponse;
}

// Flow: PaymentService.ProxyAuthorize
async function proxyAuthorize(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const proxyResponse = await paymentClient.proxyAuthorize(_buildProxyAuthorizeRequest());

    return proxyResponse;
}


// Export all process* functions for the smoke test
export {
    processCheckoutAutocapture, authorize, proxyAuthorize, _buildAuthorizeRequest, _buildProxyAuthorizeRequest
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
