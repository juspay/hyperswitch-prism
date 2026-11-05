// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py braintree
//
// Braintree — all integration scenarios and flows in one file.
// Run a scenario:  npx tsx braintree.ts checkout_autocapture

import { PaymentClient, MerchantAuthenticationClient, EventClient, types } from 'hyperswitch-prism';
const { Environment, AcceptanceType, AuthenticationType, CardNetwork, Currency, FutureUsage, HttpMethod } = types;
export const SUPPORTED_FLOWS = ["capture", "create_client_authentication_token", "get", "parse_event", "proxy_setup_recurring", "refund", "reverse", "setup_recurring", "void"];

const _defaultConfig: types.IConnectorConfig = {
    options: {
        environment: Environment.SANDBOX,
    },
    connectorConfig: {
        braintree: {
            publicKey: { value: 'YOUR_PUBLIC_KEY' },
            privateKey: { value: 'YOUR_PRIVATE_KEY' },
            baseUrl: 'YOUR_BASE_URL',
            merchantAccountId: { value: 'YOUR_MERCHANT_ACCOUNT_ID' },
            merchantConfigCurrency: 'YOUR_MERCHANT_CONFIG_CURRENCY',
            applePaySupportedNetworks: ['YOUR_APPLE_PAY_SUPPORTED_NETWORKS'],
            applePayMerchantCapabilities: ['YOUR_APPLE_PAY_MERCHANT_CAPABILITIES'],
            applePayLabel: 'YOUR_APPLE_PAY_LABEL',
            gpayMerchantName: 'YOUR_GPAY_MERCHANT_NAME',
            gpayMerchantId: 'YOUR_GPAY_MERCHANT_ID',
            gpayAllowedAuthMethods: ['YOUR_GPAY_ALLOWED_AUTH_METHODS'],
            gpayAllowedCardNetworks: ['YOUR_GPAY_ALLOWED_CARD_NETWORKS'],
            paypalClientId: 'YOUR_PAYPAL_CLIENT_ID',
            gpayGatewayMerchantId: 'YOUR_GPAY_GATEWAY_MERCHANT_ID',
        }
    },
};


function _buildCaptureRequest(connectorTransactionId: string): types.IPaymentServiceCaptureRequest {
    return {
        "merchantCaptureId": "probe_capture_001",  // Identification.
        "connectorTransactionId": connectorTransactionId,
        "amountToCapture": {  // Capture Details.
            "minorAmount": 1000,
            "currency": Currency.USD
        }
    };
}

function _buildCreateClientAuthenticationTokenRequest(): types.IMerchantAuthenticationServiceCreateClientAuthenticationTokenRequest {
    return {
        "merchantClientSessionId": "probe_sdk_session_001",  // Infrastructure.
        "payment": {
            "amount": {
                "minorAmount": 1000,
                "currency": Currency.USD
            }
        }
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
            "body": {"value": new Uint8Array([98, 116, 95, 115, 105, 103, 110, 97, 116, 117, 114, 101, 61, 100, 117, 109, 109, 121, 95, 112, 117, 98, 108, 105, 99, 95, 107, 101, 121, 37, 55, 67, 100, 117, 109, 109, 121, 95, 115, 105, 103, 110, 97, 116, 117, 114, 101, 38, 98, 116, 95, 112, 97, 121, 108, 111, 97, 100, 61, 80, 71, 53, 118, 100, 71, 108, 109, 97, 87, 78, 104, 100, 71, 108, 118, 98, 106, 52, 56, 97, 50, 108, 117, 90, 68, 53, 107, 97, 88, 78, 119, 100, 88, 82, 108, 88, 50, 57, 119, 90, 87, 53, 108, 90, 68, 119, 118, 97, 50, 108, 117, 90, 68, 52, 56, 100, 71, 108, 116, 90, 88, 78, 48, 89, 87, 49, 119, 80, 106, 73, 119, 77, 106, 81, 116, 77, 68, 69, 116, 77, 68, 70, 85, 77, 68, 65, 54, 77, 68, 65, 54, 77, 68, 66, 97, 80, 67, 57, 48, 97, 87, 49, 108, 99, 51, 82, 104, 98, 88, 65, 37, 50, 66, 80, 71, 82, 112, 99, 51, 66, 49, 100, 71, 85, 37, 50, 66, 80, 71, 70, 116, 98, 51, 86, 117, 100, 70, 57, 107, 97, 88, 78, 119, 100, 88, 82, 108, 90, 68, 52, 120, 77, 68, 65, 119, 80, 67, 57, 104, 98, 87, 57, 49, 98, 110, 82, 102, 90, 71, 108, 122, 99, 72, 86, 48, 90, 87, 81, 37, 50, 66, 80, 71, 78, 49, 99, 110, 74, 108, 98, 109, 78, 53, 88, 50, 108, 122, 98, 49, 57, 106, 98, 50, 82, 108, 80, 108, 86, 84, 82, 68, 119, 118, 89, 51, 86, 121, 99, 109, 86, 117, 89, 51, 108, 102, 97, 88, 78, 118, 88, 50, 78, 118, 90, 71, 85, 37, 50, 66, 80, 71, 108, 107, 80, 109, 82, 49, 98, 87, 49, 53, 88, 50, 82, 112, 99, 51, 66, 49, 100, 71, 86, 102, 97, 87, 82, 102, 77, 68, 65, 120, 80, 67, 57, 112, 90, 68, 52, 56, 97, 50, 108, 117, 90, 68, 53, 68, 83, 69, 70, 83, 82, 48, 86, 67, 81, 85, 78, 76, 80, 67, 57, 114, 97, 87, 53, 107, 80, 106, 120, 122, 100, 71, 70, 48, 100, 88, 77, 37, 50, 66, 98, 51, 66, 108, 98, 106, 119, 118, 99, 51, 82, 104, 100, 72, 86, 122, 80, 106, 120, 121, 90, 87, 70, 122, 98, 50, 52, 37, 50, 66, 90, 110, 74, 104, 100, 87, 81, 56, 76, 51, 74, 108, 89, 88, 78, 118, 98, 106, 52, 56, 99, 109, 86, 104, 99, 50, 57, 117, 88, 50, 78, 118, 90, 71, 85, 37, 50, 66, 79, 68, 77, 56, 76, 51, 74, 108, 89, 88, 78, 118, 98, 108, 57, 106, 98, 50, 82, 108, 80, 106, 120, 48, 99, 109, 70, 117, 99, 50, 70, 106, 100, 71, 108, 118, 98, 106, 52, 56, 89, 87, 49, 118, 100, 87, 53, 48, 80, 106, 69, 119, 76, 106, 65, 119, 80, 67, 57, 104, 98, 87, 57, 49, 98, 110, 81, 37, 50, 66, 80, 71, 108, 107, 80, 109, 82, 49, 98, 87, 49, 53, 88, 51, 82, 52, 98, 108, 57, 112, 90, 70, 56, 119, 77, 68, 69, 56, 76, 50, 108, 107, 80, 106, 119, 118, 100, 72, 74, 104, 98, 110, 78, 104, 89, 51, 82, 112, 98, 50, 52, 37, 50, 66, 80, 67, 57, 107, 97, 88, 78, 119, 100, 88, 82, 108, 80, 106, 119, 118, 98, 109, 57, 48, 97, 87, 90, 112, 89, 50, 70, 48, 97, 87, 57, 117, 80, 103, 37, 51, 68, 37, 51, 68])},  // Secret-typed replacements. Presence takes precedence over the legacy field, including an explicitly empty body, map, URI or query string.
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
            "body": {"value": new Uint8Array([98, 116, 95, 115, 105, 103, 110, 97, 116, 117, 114, 101, 61, 100, 117, 109, 109, 121, 95, 112, 117, 98, 108, 105, 99, 95, 107, 101, 121, 37, 55, 67, 100, 117, 109, 109, 121, 95, 115, 105, 103, 110, 97, 116, 117, 114, 101, 38, 98, 116, 95, 112, 97, 121, 108, 111, 97, 100, 61, 80, 71, 53, 118, 100, 71, 108, 109, 97, 87, 78, 104, 100, 71, 108, 118, 98, 106, 52, 56, 97, 50, 108, 117, 90, 68, 53, 107, 97, 88, 78, 119, 100, 88, 82, 108, 88, 50, 57, 119, 90, 87, 53, 108, 90, 68, 119, 118, 97, 50, 108, 117, 90, 68, 52, 56, 100, 71, 108, 116, 90, 88, 78, 48, 89, 87, 49, 119, 80, 106, 73, 119, 77, 106, 81, 116, 77, 68, 69, 116, 77, 68, 70, 85, 77, 68, 65, 54, 77, 68, 65, 54, 77, 68, 66, 97, 80, 67, 57, 48, 97, 87, 49, 108, 99, 51, 82, 104, 98, 88, 65, 37, 50, 66, 80, 71, 82, 112, 99, 51, 66, 49, 100, 71, 85, 37, 50, 66, 80, 71, 70, 116, 98, 51, 86, 117, 100, 70, 57, 107, 97, 88, 78, 119, 100, 88, 82, 108, 90, 68, 52, 120, 77, 68, 65, 119, 80, 67, 57, 104, 98, 87, 57, 49, 98, 110, 82, 102, 90, 71, 108, 122, 99, 72, 86, 48, 90, 87, 81, 37, 50, 66, 80, 71, 78, 49, 99, 110, 74, 108, 98, 109, 78, 53, 88, 50, 108, 122, 98, 49, 57, 106, 98, 50, 82, 108, 80, 108, 86, 84, 82, 68, 119, 118, 89, 51, 86, 121, 99, 109, 86, 117, 89, 51, 108, 102, 97, 88, 78, 118, 88, 50, 78, 118, 90, 71, 85, 37, 50, 66, 80, 71, 108, 107, 80, 109, 82, 49, 98, 87, 49, 53, 88, 50, 82, 112, 99, 51, 66, 49, 100, 71, 86, 102, 97, 87, 82, 102, 77, 68, 65, 120, 80, 67, 57, 112, 90, 68, 52, 56, 97, 50, 108, 117, 90, 68, 53, 68, 83, 69, 70, 83, 82, 48, 86, 67, 81, 85, 78, 76, 80, 67, 57, 114, 97, 87, 53, 107, 80, 106, 120, 122, 100, 71, 70, 48, 100, 88, 77, 37, 50, 66, 98, 51, 66, 108, 98, 106, 119, 118, 99, 51, 82, 104, 100, 72, 86, 122, 80, 106, 120, 121, 90, 87, 70, 122, 98, 50, 52, 37, 50, 66, 90, 110, 74, 104, 100, 87, 81, 56, 76, 51, 74, 108, 89, 88, 78, 118, 98, 106, 52, 56, 99, 109, 86, 104, 99, 50, 57, 117, 88, 50, 78, 118, 90, 71, 85, 37, 50, 66, 79, 68, 77, 56, 76, 51, 74, 108, 89, 88, 78, 118, 98, 108, 57, 106, 98, 50, 82, 108, 80, 106, 120, 48, 99, 109, 70, 117, 99, 50, 70, 106, 100, 71, 108, 118, 98, 106, 52, 56, 89, 87, 49, 118, 100, 87, 53, 48, 80, 106, 69, 119, 76, 106, 65, 119, 80, 67, 57, 104, 98, 87, 57, 49, 98, 110, 81, 37, 50, 66, 80, 71, 108, 107, 80, 109, 82, 49, 98, 87, 49, 53, 88, 51, 82, 52, 98, 108, 57, 112, 90, 70, 56, 119, 77, 68, 69, 56, 76, 50, 108, 107, 80, 106, 119, 118, 100, 72, 74, 104, 98, 110, 78, 104, 89, 51, 82, 112, 98, 50, 52, 37, 50, 66, 80, 67, 57, 107, 97, 88, 78, 119, 100, 88, 82, 108, 80, 106, 119, 118, 98, 109, 57, 48, 97, 87, 90, 112, 89, 50, 70, 48, 97, 87, 57, 117, 80, 103, 37, 51, 68, 37, 51, 68])},  // Secret-typed replacements. Presence takes precedence over the legacy field, including an explicitly empty body, map, URI or query string.
            "uri": {"value": "https://example.com/webhook"}
        }
    };
}

function _buildProxySetupRecurringRequest(): types.IPaymentServiceProxySetupRecurringRequest {
    return {
        "merchantRecurringPaymentId": "probe_proxy_mandate_001",
        "amount": {
            "minorAmount": 0,
            "currency": Currency.USD
        },
        "cardProxy": {  // Card proxy for vault-aliased payments.
            "cardNumber": {"value": "4111111111111111"},  // Card Identification.
            "cardExpMonth": {"value": "03"},
            "cardExpYear": {"value": "2030"},
            "cardCvc": {"value": "123"},
            "cardHolderName": {"value": "John Doe"},  // Cardholder Information.
            "cardNetwork": CardNetwork.VISA
        },
        "address": {
            "billingAddress": {
            }
        },
        "customerAcceptance": {
            "acceptanceType": AcceptanceType.OFFLINE,  // Type of acceptance (e.g., online, offline).
            "acceptedAt": 0  // Timestamp when the acceptance was made (Unix timestamp, seconds since epoch).
        },
        "authType": AuthenticationType.NO_THREE_DS,
        "setupFutureUsage": FutureUsage.OFF_SESSION
    };
}

function _buildRefundRequest(connectorTransactionId: string): types.IPaymentServiceRefundRequest {
    return {
        "merchantRefundId": "probe_refund_001",  // Identification.
        "connectorTransactionId": connectorTransactionId,
        "paymentAmount": 1000,  // Amount Information.
        "refundAmount": {
            "minorAmount": 1000,
            "currency": Currency.USD
        },
        "reason": "customer_request"  // Reason for the refund.
    };
}

function _buildReverseRequest(connectorTransactionId: string): types.IPaymentServiceReverseRequest {
    return {
        "merchantReverseId": "probe_reverse_001",  // Identification.
        "connectorTransactionId": connectorTransactionId
    };
}

function _buildSetupRecurringRequest(): types.IPaymentServiceSetupRecurringRequest {
    return {
        "merchantRecurringPaymentId": "probe_mandate_001",  // Identification.
        "amount": {  // Mandate Details.
            "minorAmount": 0,
            "currency": Currency.USD
        },
        "paymentMethod": {
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
        "authType": AuthenticationType.NO_THREE_DS,  // Type of authentication to be used.
        "enrolledFor_3ds": false,  // Indicates if the customer is enrolled for 3D Secure.
        "returnUrl": "https://example.com/mandate-return",  // URL to redirect after setup.
        "setupFutureUsage": FutureUsage.OFF_SESSION,  // Indicates future usage intention.
        "requestIncrementalAuthorization": false,  // Indicates if incremental authorization is requested.
        "customerAcceptance": {  // Details of customer acceptance.
            "acceptanceType": AcceptanceType.OFFLINE,  // Type of acceptance (e.g., online, offline).
            "acceptedAt": 0  // Timestamp when the acceptance was made (Unix timestamp, seconds since epoch).
        }
    };
}

function _buildVoidRequest(connectorTransactionId: string): types.IPaymentServiceVoidRequest {
    return {
        "merchantVoidId": "probe_void_001",  // Identification.
        "connectorTransactionId": connectorTransactionId
    };
}


// ANCHOR: scenario_functions
// Flow: PaymentService.Capture
async function capture(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const captureResponse = await paymentClient.capture(_buildCaptureRequest('probe_connector_txn_001'));

    return captureResponse;
}

// Flow: MerchantAuthenticationService.CreateClientAuthenticationToken
async function createClientAuthenticationToken(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const merchantAuthenticationClient = new MerchantAuthenticationClient(config);

    const createResponse = await merchantAuthenticationClient.createClientAuthenticationToken(_buildCreateClientAuthenticationTokenRequest());

    return createResponse;
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

// Flow: PaymentService.ProxySetupRecurring
async function proxySetupRecurring(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const proxyResponse = await paymentClient.proxySetupRecurring(_buildProxySetupRecurringRequest());

    return proxyResponse;
}

// Flow: PaymentService.Refund
async function refund(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const refundResponse = await paymentClient.refund(_buildRefundRequest('probe_connector_txn_001'));

    return refundResponse;
}

// Flow: PaymentService.Reverse
async function reverse(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const reverseResponse = await paymentClient.reverse(_buildReverseRequest('probe_connector_txn_001'));

    return reverseResponse;
}

// Flow: PaymentService.SetupRecurring
async function setupRecurring(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const setupResponse = await paymentClient.setupRecurring(_buildSetupRecurringRequest());

    return setupResponse;
}

// Flow: PaymentService.Void
async function voidPayment(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const voidResponse = await paymentClient.void(_buildVoidRequest('probe_connector_txn_001'));

    return voidResponse;
}


// Export all process* functions for the smoke test
export {
    capture, createClientAuthenticationToken, get, handleEvent, parseEvent, proxySetupRecurring, refund, reverse, setupRecurring, voidPayment, _buildCaptureRequest, _buildCreateClientAuthenticationTokenRequest, _buildGetRequest, _buildHandleEventRequest, _buildParseEventRequest, _buildProxySetupRecurringRequest, _buildRefundRequest, _buildReverseRequest, _buildSetupRecurringRequest, _buildVoidRequest
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
