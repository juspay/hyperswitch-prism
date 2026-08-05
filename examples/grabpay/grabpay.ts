// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py grabpay
//
// Grabpay — all integration scenarios and flows in one file.
// Run a scenario:  npx tsx grabpay.ts checkout_autocapture

import { PaymentClient, types } from 'hyperswitch-prism';
const { Environment, Currency } = types;
export const SUPPORTED_FLOWS = ["create_order"];

const _defaultConfig: types.IConnectorConfig = {
    options: {
        environment: Environment.SANDBOX,
    },
    connectorConfig: {
        grabpay: {
            partnerId: { value: 'YOUR_PARTNER_ID' },
            partnerSecret: { value: 'YOUR_PARTNER_SECRET' },
            clientId: { value: 'YOUR_CLIENT_ID' },
            clientSecret: { value: 'YOUR_CLIENT_SECRET' },
            merchantId: { value: 'YOUR_MERCHANT_ID' },
            baseUrl: 'YOUR_BASE_URL',
        }
    },
};


function _buildCreateOrderRequest(): types.IPaymentServiceCreateOrderRequest {
    return {
        "merchantOrderId": "probe_order_001",  // Identification.
        "amount": {  // Amount Information.
            "minorAmount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            "currency": Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        }
    };
}

function _buildVerifyRedirectRequest(): types.IPaymentServiceVerifyRedirectResponseRequest {
    return {
    };
}


// ANCHOR: scenario_functions
// Flow: PaymentService.CreateOrder
async function createOrder(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const createResponse = await paymentClient.createOrder(_buildCreateOrderRequest());

    return createResponse;
}

// Flow: PaymentService.VerifyRedirectResponse
async function verifyRedirect(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const verifyResponse = await paymentClient.verifyRedirectResponse(_buildVerifyRedirectRequest());

    return verifyResponse;
}


// Export all process* functions for the smoke test
export {
    createOrder, verifyRedirect, _buildCreateOrderRequest, _buildVerifyRedirectRequest
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
