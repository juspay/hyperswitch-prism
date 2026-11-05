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
            "minorAmount": 1000,
            "currency": Currency.USD
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
            "minorAmount": 1000,
            "currency": Currency.USD
        }
    };
}

function _buildHandleEventRequest(): types.IEventServiceHandleRequest {
    return {
        "merchantEventId": "probe_event_001",
        "requestDetails": {
            "method": HttpMethod.HTTP_METHOD_POST,  // HTTP method of the request (e.g., GET, POST).
            "legacyHeaders": {  // Headers of the HTTP request.
            },
            "body": {"value": new Uint8Array([123, 34, 116, 120, 84, 121, 112, 101, 34, 58, 34, 112, 97, 121, 109, 101, 110, 116, 34, 44, 34, 116, 120, 83, 116, 97, 116, 117, 115, 34, 58, 34, 115, 117, 99, 99, 101, 115, 115, 34, 44, 34, 112, 97, 114, 116, 110, 101, 114, 73, 68, 34, 58, 34, 112, 97, 114, 116, 110, 101, 114, 95, 49, 50, 51, 34, 44, 34, 112, 97, 114, 116, 110, 101, 114, 84, 120, 73, 68, 34, 58, 34, 116, 120, 110, 95, 49, 50, 51, 34, 44, 34, 116, 120, 73, 68, 34, 58, 34, 103, 114, 97, 98, 95, 116, 120, 110, 95, 49, 50, 51, 34, 44, 34, 97, 109, 111, 117, 110, 116, 34, 58, 49, 48, 48, 44, 34, 99, 117, 114, 114, 101, 110, 99, 121, 34, 58, 34, 83, 71, 68, 34, 44, 34, 112, 97, 121, 108, 111, 97, 100, 34, 58, 123, 34, 110, 101, 119, 83, 116, 97, 116, 117, 115, 34, 58, 34, 115, 117, 99, 99, 101, 115, 115, 34, 44, 34, 112, 97, 121, 109, 101, 110, 116, 77, 101, 116, 104, 111, 100, 34, 58, 34, 71, 82, 65, 66, 80, 65, 89, 34, 125, 125])},  // Secret-typed replacements. Presence takes precedence over the legacy field, including an explicitly empty body, map, URI or query string.
            "uri": {"value": "https://example.com/webhook"}
        }
    };
}

function _buildParseEventRequest(): types.IEventServiceParseRequest {
    return {
        "requestDetails": {
            "method": HttpMethod.HTTP_METHOD_POST,  // HTTP method of the request (e.g., GET, POST).
            "legacyHeaders": {  // Headers of the HTTP request.
            },
            "body": {"value": new Uint8Array([123, 34, 116, 120, 84, 121, 112, 101, 34, 58, 34, 112, 97, 121, 109, 101, 110, 116, 34, 44, 34, 116, 120, 83, 116, 97, 116, 117, 115, 34, 58, 34, 115, 117, 99, 99, 101, 115, 115, 34, 44, 34, 112, 97, 114, 116, 110, 101, 114, 73, 68, 34, 58, 34, 112, 97, 114, 116, 110, 101, 114, 95, 49, 50, 51, 34, 44, 34, 112, 97, 114, 116, 110, 101, 114, 84, 120, 73, 68, 34, 58, 34, 116, 120, 110, 95, 49, 50, 51, 34, 44, 34, 116, 120, 73, 68, 34, 58, 34, 103, 114, 97, 98, 95, 116, 120, 110, 95, 49, 50, 51, 34, 44, 34, 97, 109, 111, 117, 110, 116, 34, 58, 49, 48, 48, 44, 34, 99, 117, 114, 114, 101, 110, 99, 121, 34, 58, 34, 83, 71, 68, 34, 44, 34, 112, 97, 121, 108, 111, 97, 100, 34, 58, 123, 34, 110, 101, 119, 83, 116, 97, 116, 117, 115, 34, 58, 34, 115, 117, 99, 99, 101, 115, 115, 34, 44, 34, 112, 97, 121, 109, 101, 110, 116, 77, 101, 116, 104, 111, 100, 34, 58, 34, 71, 82, 65, 66, 80, 65, 89, 34, 125, 125])},  // Secret-typed replacements. Presence takes precedence over the legacy field, including an explicitly empty body, map, URI or query string.
            "uri": {"value": "https://example.com/webhook"}
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
