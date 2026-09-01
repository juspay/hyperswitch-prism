// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py grabpay
//
// Grabpay — all integration scenarios and flows in one file.
// Run a scenario:  npx tsx grabpay.ts checkout_autocapture

import { PaymentClient, EventClient, types } from 'hyperswitch-prism';
const { Environment, AuthenticationType, CaptureMethod, Currency, HttpMethod } = types;
export const SUPPORTED_FLOWS = ["authorize", "get", "parse_event"];

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
            }
        },
        "authType": AuthenticationType.NO_THREE_DS,  // Authentication Details.
        "returnUrl": "https://example.com/return",  // URLs for Redirection and Webhooks.
        "sessionToken": "probe_session_token"  // Session and Token Information.
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

function _buildHandleEventRequest(): types.IEventServiceHandleRequest {
    return {
        "merchantEventId": "probe_event_001",  // Caller-supplied correlation key, echoed in the response. Not used by UCS for processing.
        "requestDetails": {
            "method": HttpMethod.HTTP_METHOD_POST,  // HTTP method of the request (e.g., GET, POST).
            "uri": "https://example.com/webhook",  // URI of the request.
            "headers": {  // Headers of the HTTP request.
            },
            "body": new Uint8Array(Buffer.from("{\"txType\":\"payment\",\"txStatus\":\"success\",\"partnerID\":\"partner_123\",\"partnerTxID\":\"txn_123\",\"txID\":\"grab_txn_123\",\"amount\":100,\"currency\":\"SGD\",\"payload\":{\"newStatus\":\"success\",\"paymentMethod\":\"GRABPAY\"}}", "utf-8"))  // Body of the HTTP request.
        }
    };
}

function _buildParseEventRequest(): types.IEventServiceParseRequest {
    return {
        "requestDetails": {
            "method": HttpMethod.HTTP_METHOD_POST,  // HTTP method of the request (e.g., GET, POST).
            "uri": "https://example.com/webhook",  // URI of the request.
            "headers": {  // Headers of the HTTP request.
            },
            "body": new Uint8Array(Buffer.from("{\"txType\":\"payment\",\"txStatus\":\"success\",\"partnerID\":\"partner_123\",\"partnerTxID\":\"txn_123\",\"txID\":\"grab_txn_123\",\"amount\":100,\"currency\":\"SGD\",\"payload\":{\"newStatus\":\"success\",\"paymentMethod\":\"GRABPAY\"}}", "utf-8"))  // Body of the HTTP request.
        }
    };
}

function _buildVerifyRedirectRequest(): types.IPaymentServiceVerifyRedirectResponseRequest {
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

// Get Payment Status
// Retrieve current payment status from the connector.
async function processGetPayment(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
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

    // Step 2: Get — retrieve current payment status from the connector
    const getResponse = await paymentClient.get(_buildGetRequest(authorizeResponse.connectorTransactionId!));

    return { status: getResponse.status, transactionId: getResponse.connectorTransactionId!, error: getResponse.error } as any;
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

// Flow: EventService.HandleEvent
async function handleEvent(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const eventClient = new EventClient(config);

    const handleResponse = await eventClient.handleEvent(_buildHandleEventRequest());

    return handleResponse;
}

// Flow: EventService.ParseEvent
async function parseEvent(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const eventClient = new EventClient(config);

    const parseResponse = await eventClient.parseEvent(_buildParseEventRequest());

    return parseResponse;
}

// Flow: PaymentService.VerifyRedirectResponse
async function verifyRedirect(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const verifyResponse = await paymentClient.verifyRedirectResponse(_buildVerifyRedirectRequest());

    return verifyResponse;
}


// Export all process* functions for the smoke test
export {
    processCheckoutAutocapture, processGetPayment, authorize, get, handleEvent, parseEvent, verifyRedirect, _buildAuthorizeRequest, _buildGetRequest, _buildHandleEventRequest, _buildParseEventRequest, _buildVerifyRedirectRequest
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
