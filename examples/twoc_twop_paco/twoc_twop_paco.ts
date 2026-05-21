// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py twoc_twop_paco
//
// Twoc_Twop_Paco — all integration scenarios and flows in one file.
// Run a scenario:  npx tsx twoc_twop_paco.ts checkout_autocapture

import { PaymentClient, RefundClient, PaymentMethodAuthenticationClient, types } from 'hyperswitch-prism';
const { Environment, AuthenticationType, CaptureMethod, CountryAlpha2, Currency } = types;
export const SUPPORTED_FLOWS = ["authorize", "get", "capture", "void", "reverse", "refund", "refund_get", "post_authenticate"];

const _defaultConfig: types.IConnectorConfig = {
    options: {
        environment: Environment.SANDBOX,
    },
    connectorConfig: {
        twocTwopPaco: {
            accessToken: { value: 'YOUR_ACCESS_TOKEN' },
            officeId: { value: 'YOUR_OFFICE_ID' },
            pacoKid: { value: 'YOUR_PACO_KID' },
            merchantSigningPrivateKey: { value: 'YOUR_MERCHANT_SIGNING_PRIVATE_KEY' },
            merchantEncryptionPrivateKey: { value: 'YOUR_MERCHANT_ENCRYPTION_PRIVATE_KEY' },
            pacoSigningPublicKey: { value: 'YOUR_PACO_SIGNING_PUBLIC_KEY' },
            pacoEncryptionPublicKey: { value: 'YOUR_PACO_ENCRYPTION_PUBLIC_KEY' },
            responseAudience: { value: 'YOUR_RESPONSE_AUDIENCE' },
            baseUrl: 'YOUR_BASE_URL',
        }
    },
};


function _buildAuthorizeRequest(captureMethod: types.CaptureMethod): types.IPaymentServiceAuthorizeRequest {
    return {
        "merchantTransactionId": "probe_txn_001",  // Identification.
        "amount": {  // The amount for the payment.
            "minorAmount": 10000,  // Amount in minor units (e.g., 1000 = $10.00).
            "currency": Currency.PHP  // ISO 4217 currency code (e.g., "USD", "EUR").
        },
        "paymentMethod": {  // Payment method to be used.
            "card": {  // Generic card payment.
                "cardNumber": {  // Card Identification.
                    "value": "4111111111111111"
                },
                "cardExpMonth": {
                    "value": "12"
                },
                "cardExpYear": {
                    "value": "2027"
                },
                "cardCvc": {
                    "value": "123"
                },
                "cardHolderName": {  // Cardholder Information.
                    "value": "Test Customer"
                },
                "cardType": "credit"
            }
        },
        "address": {  // Address Information.
            "billingAddress": {
                "countryAlpha2Code": CountryAlpha2.PH
            }
        },
        "authType": AuthenticationType.NO_THREE_DS,  // Authentication Details.
        "returnUrl": "https://example.com/return",  // URLs for Redirection and Webhooks.
        "webhookUrl": "https://example.com/webhook"
    };
}

function _buildGetRequest(connectorTransactionId: string): types.IPaymentServiceGetRequest {
    return {
    };
}

function _buildCaptureRequest(connectorTransactionId: string): types.IPaymentServiceCaptureRequest {
    return {
    };
}

function _buildVoidRequest(connectorTransactionId: string): types.IPaymentServiceVoidRequest {
    return {
    };
}

function _buildReverseRequest(connectorTransactionId: string): types.IPaymentServiceReverseRequest {
    return {
    };
}

function _buildRefundRequest(connectorTransactionId: string): types.IPaymentServiceRefundRequest {
    return {
        "merchantRefundId": "probe_refund_001",  // Identification.
        "connectorTransactionId": connectorTransactionId,
        "paymentAmount": 10000,  // Amount Information.
        "refundAmount": {
            "minorAmount": 10000,  // Amount in minor units (e.g., 1000 = $10.00).
            "currency": Currency.PHP  // ISO 4217 currency code (e.g., "USD", "EUR").
        },
        "reason": "customer request",  // Reason for the refund.
        "refundMetadata": {  // Metadata specific to the refund.
            "value": "{\"original_order_no\":\"probe_txn_001\"}"
        }
    };
}

function _buildRefundGetRequest(): types.IRefundServiceGetRequest {
    return {
    };
}

function _buildPostAuthenticateRequest(): types.IPaymentMethodAuthenticationServicePostAuthenticateRequest {
    return {
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

// Card Payment (Authorize + Capture)
// Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.
async function processCheckoutCard(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    // Step 1: Authorize — reserve funds on the payment method
    const authorizeResponse = await paymentClient.authorize(_buildAuthorizeRequest(CaptureMethod.MANUAL));

    if (authorizeResponse.status === types.PaymentStatus.FAILURE) {
        throw new Error(`Payment failed: ${JSON.stringify(authorizeResponse.error)}`);
    }
    if (authorizeResponse.status === types.PaymentStatus.PENDING) {
        // Awaiting async confirmation — handle via webhook
        return { status: 'pending', connectorTransactionId: authorizeResponse.connectorTransactionId };
    }

    // Step 2: Capture — settle the reserved funds
    const captureResponse = await paymentClient.capture(_buildCaptureRequest(authorizeResponse.connectorTransactionId!));

    if (captureResponse.status === types.PaymentStatus.FAILURE) {
        throw new Error(`Capture failed: ${JSON.stringify(captureResponse.error)}`);
    }

    return { status: captureResponse.status, transactionId: authorizeResponse.connectorTransactionId!, error: authorizeResponse.error } as any;
}

// Refund
// Return funds to the customer for a completed payment.
async function processRefund(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
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

    // Step 2: Refund — return funds to the customer
    const refundResponse = await paymentClient.refund(_buildRefundRequest(authorizeResponse.connectorTransactionId!));

    if (refundResponse.status === types.RefundStatus.REFUND_FAILURE) {
        throw new Error(`Refund failed: ${JSON.stringify(refundResponse.error)}`);
    }

    return { status: refundResponse.status, error: refundResponse.error } as any;
}

// Flow: PaymentService.Authorize (Card)
async function authorize(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const authorizeResponse = await paymentClient.authorize(_buildAuthorizeRequest(CaptureMethod.AUTOMATIC));

    return authorizeResponse;
}

// Flow: PaymentService.Get
async function get(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const getResponse = await paymentClient.get(_buildGetRequest('probe_connector_txn_001'));

    return getResponse;
}

// Flow: PaymentService.Capture
async function capture(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const captureResponse = await paymentClient.capture(_buildCaptureRequest('probe_connector_txn_001'));

    return captureResponse;
}

// Flow: PaymentService.Void
async function voidPayment(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const voidResponse = await paymentClient.void(_buildVoidRequest('probe_connector_txn_001'));

    return voidResponse;
}

// Flow: PaymentService.Reverse
async function reverse(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const reverseResponse = await paymentClient.reverse(_buildReverseRequest('probe_connector_txn_001'));

    return reverseResponse;
}

// Flow: PaymentService.Refund
async function refund(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const refundResponse = await paymentClient.refund(_buildRefundRequest('CEBU0000000000000'));

    return refundResponse;
}

// Flow: RefundService.Get
async function refundGet(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const refundClient = new RefundClient(config);

    const refundResponse = await refundClient.refundGet(_buildRefundGetRequest());

    return refundResponse;
}

// Flow: PaymentMethodAuthenticationService.PostAuthenticate
async function postAuthenticate(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentMethodAuthenticationClient = new PaymentMethodAuthenticationClient(config);

    const postResponse = await paymentMethodAuthenticationClient.postAuthenticate(_buildPostAuthenticateRequest());

    return postResponse;
}


// Export all process* functions for the smoke test
export {
    processCheckoutAutocapture, processCheckoutCard, processRefund, authorize, get, capture, voidPayment, reverse, refund, refundGet, postAuthenticate, _buildAuthorizeRequest, _buildCaptureRequest, _buildGetRequest, _buildPostAuthenticateRequest, _buildRefundRequest, _buildRefundGetRequest, _buildReverseRequest, _buildVoidRequest
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
