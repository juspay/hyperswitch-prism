// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py braintree
//
// Braintree — all integration scenarios and flows in one file.
// Run a scenario:  npx tsx braintree.ts checkout_autocapture

import { PaymentClient, MerchantAuthenticationClient, EventClient, PaymentMethodAuthenticationClient, RefundClient, types } from 'hyperswitch-prism';
const { Environment, AcceptanceType, CaptureMethod, Currency, FutureUsage, HttpMethod } = types;
export const SUPPORTED_FLOWS = ["capture", "create_client_authentication_token", "get", "parse_event", "post_authenticate", "pre_authenticate", "refund", "refund_get", "reverse", "token_authorize", "token_setup_recurring", "void"];

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
            "minorAmount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            "currency": Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        }
    };
}

function _buildCreateClientAuthenticationTokenRequest(): types.IMerchantAuthenticationServiceCreateClientAuthenticationTokenRequest {
    return {
        "merchantClientSessionId": "probe_sdk_session_001",  // Infrastructure.
        "payment": {
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

function _buildHandleEventRequest(): types.IEventServiceHandleRequest {
    return {
        "merchantEventId": "probe_event_001",
        "requestDetails": {
            "method": HttpMethod.HTTP_METHOD_POST,  // HTTP method of the request (e.g., GET, POST).
            "uri": "https://example.com/webhook",  // URI of the request.
            "headers": {  // Headers of the HTTP request.
            },
            "body": new Uint8Array(Buffer.from("bt_signature=dummy_public_key%7Cdummy_signature&bt_payload=PG5vdGlmaWNhdGlvbj48dGltZXN0YW1wIHR5cGU9ImRhdGV0aW1lIj4yMDI2LTA5LTE2VDAwOjAwOjAwWjwvdGltZXN0YW1wPjxraW5kPmRpc3B1dGVfb3BlbmVkPC9raW5kPjxzdWJqZWN0PjxkaXNwdXRlPjxpZD5kdW1teV9kaXNwdXRlX2lkXzAwMTwvaWQ%2BPGFtb3VudD4xMC4wMDwvYW1vdW50PjxhbW91bnQtZGlzcHV0ZWQ%2BMTAuMDA8L2Ftb3VudC1kaXNwdXRlZD48YW1vdW50LXdvbiBuaWw9InRydWUiLz48Y2FzZS1udW1iZXI%2BQ0FTRS0wMDE8L2Nhc2UtbnVtYmVyPjxjcmVhdGVkLWF0IHR5cGU9ImRhdGV0aW1lIj4yMDI2LTA5LTE2VDAwOjAwOjAwWjwvY3JlYXRlZC1hdD48Y3VycmVuY3ktaXNvLWNvZGU%2BVVNEPC9jdXJyZW5jeS1pc28tY29kZT48Zm9yd2FyZGVkLWNvbW1lbnRzIG5pbD0idHJ1ZSIvPjxraW5kPmNoYXJnZWJhY2s8L2tpbmQ%2BPG1lcmNoYW50LWFjY291bnQtaWQ%2BZHVtbXlfbWVyY2hhbnRfYWNjb3VudDwvbWVyY2hhbnQtYWNjb3VudC1pZD48cmVhc29uPmZyYXVkPC9yZWFzb24%2BPHJlYXNvbi1jb2RlIG5pbD0idHJ1ZSIvPjxyZWNlaXZlZC1kYXRlIHR5cGU9ImRhdGUiPjIwMjYtMDktMTY8L3JlY2VpdmVkLWRhdGU%2BPHJlZmVyZW5jZS1udW1iZXI%2BUkVGLTAwMTwvcmVmZXJlbmNlLW51bWJlcj48cmVwbHktYnktZGF0ZSB0eXBlPSJkYXRlIj4yMDI2LTA5LTMwPC9yZXBseS1ieS1kYXRlPjxzdGF0dXM%2Bb3Blbjwvc3RhdHVzPjx1cGRhdGVkLWF0IHR5cGU9ImRhdGV0aW1lIj4yMDI2LTA5LTE2VDAwOjAwOjAwWjwvdXBkYXRlZC1hdD48c3RhdHVzLWhpc3RvcnkgdHlwZT0iYXJyYXkiPjxzdGF0dXMtaGlzdG9yeT48c3RhdHVzPm9wZW48L3N0YXR1cz48dGltZXN0YW1wIHR5cGU9ImRhdGV0aW1lIj4yMDI2LTA5LTE2VDAwOjAwOjAwWjwvdGltZXN0YW1wPjwvc3RhdHVzLWhpc3Rvcnk%2BPC9zdGF0dXMtaGlzdG9yeT48ZXZpZGVuY2UgdHlwZT0iYXJyYXkiLz48dHJhbnNhY3Rpb24%2BPGlkPmR1bW15X3R4bl9pZF8wMDE8L2lkPjxhbW91bnQ%2BMTAuMDA8L2Ftb3VudD48b3JkZXItaWQ%2BZHVtbXlfb3JkZXJfMDAxPC9vcmRlci1pZD48cGF5bWVudC1pbnN0cnVtZW50LXR5cGU%2BY3JlZGl0X2NhcmQ8L3BheW1lbnQtaW5zdHJ1bWVudC10eXBlPjwvdHJhbnNhY3Rpb24%2BPGRhdGUtb3BlbmVkIHR5cGU9ImRhdGUiPjIwMjYtMDktMTY8L2RhdGUtb3BlbmVkPjwvZGlzcHV0ZT48L3N1YmplY3Q%2BPC9ub3RpZmljYXRpb24%2B", "utf-8"))  // Body of the HTTP request.
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
            "body": new Uint8Array(Buffer.from("bt_signature=dummy_public_key%7Cdummy_signature&bt_payload=PG5vdGlmaWNhdGlvbj48dGltZXN0YW1wIHR5cGU9ImRhdGV0aW1lIj4yMDI2LTA5LTE2VDAwOjAwOjAwWjwvdGltZXN0YW1wPjxraW5kPmRpc3B1dGVfb3BlbmVkPC9raW5kPjxzdWJqZWN0PjxkaXNwdXRlPjxpZD5kdW1teV9kaXNwdXRlX2lkXzAwMTwvaWQ%2BPGFtb3VudD4xMC4wMDwvYW1vdW50PjxhbW91bnQtZGlzcHV0ZWQ%2BMTAuMDA8L2Ftb3VudC1kaXNwdXRlZD48YW1vdW50LXdvbiBuaWw9InRydWUiLz48Y2FzZS1udW1iZXI%2BQ0FTRS0wMDE8L2Nhc2UtbnVtYmVyPjxjcmVhdGVkLWF0IHR5cGU9ImRhdGV0aW1lIj4yMDI2LTA5LTE2VDAwOjAwOjAwWjwvY3JlYXRlZC1hdD48Y3VycmVuY3ktaXNvLWNvZGU%2BVVNEPC9jdXJyZW5jeS1pc28tY29kZT48Zm9yd2FyZGVkLWNvbW1lbnRzIG5pbD0idHJ1ZSIvPjxraW5kPmNoYXJnZWJhY2s8L2tpbmQ%2BPG1lcmNoYW50LWFjY291bnQtaWQ%2BZHVtbXlfbWVyY2hhbnRfYWNjb3VudDwvbWVyY2hhbnQtYWNjb3VudC1pZD48cmVhc29uPmZyYXVkPC9yZWFzb24%2BPHJlYXNvbi1jb2RlIG5pbD0idHJ1ZSIvPjxyZWNlaXZlZC1kYXRlIHR5cGU9ImRhdGUiPjIwMjYtMDktMTY8L3JlY2VpdmVkLWRhdGU%2BPHJlZmVyZW5jZS1udW1iZXI%2BUkVGLTAwMTwvcmVmZXJlbmNlLW51bWJlcj48cmVwbHktYnktZGF0ZSB0eXBlPSJkYXRlIj4yMDI2LTA5LTMwPC9yZXBseS1ieS1kYXRlPjxzdGF0dXM%2Bb3Blbjwvc3RhdHVzPjx1cGRhdGVkLWF0IHR5cGU9ImRhdGV0aW1lIj4yMDI2LTA5LTE2VDAwOjAwOjAwWjwvdXBkYXRlZC1hdD48c3RhdHVzLWhpc3RvcnkgdHlwZT0iYXJyYXkiPjxzdGF0dXMtaGlzdG9yeT48c3RhdHVzPm9wZW48L3N0YXR1cz48dGltZXN0YW1wIHR5cGU9ImRhdGV0aW1lIj4yMDI2LTA5LTE2VDAwOjAwOjAwWjwvdGltZXN0YW1wPjwvc3RhdHVzLWhpc3Rvcnk%2BPC9zdGF0dXMtaGlzdG9yeT48ZXZpZGVuY2UgdHlwZT0iYXJyYXkiLz48dHJhbnNhY3Rpb24%2BPGlkPmR1bW15X3R4bl9pZF8wMDE8L2lkPjxhbW91bnQ%2BMTAuMDA8L2Ftb3VudD48b3JkZXItaWQ%2BZHVtbXlfb3JkZXJfMDAxPC9vcmRlci1pZD48cGF5bWVudC1pbnN0cnVtZW50LXR5cGU%2BY3JlZGl0X2NhcmQ8L3BheW1lbnQtaW5zdHJ1bWVudC10eXBlPjwvdHJhbnNhY3Rpb24%2BPGRhdGUtb3BlbmVkIHR5cGU9ImRhdGUiPjIwMjYtMDktMTY8L2RhdGUtb3BlbmVkPjwvZGlzcHV0ZT48L3N1YmplY3Q%2BPC9ub3RpZmljYXRpb24%2B", "utf-8"))  // Body of the HTTP request.
        }
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
        },
        "connectorOrderReferenceId": "probe_order_ref_001"
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

function _buildRefundRequest(connectorTransactionId: string): types.IPaymentServiceRefundRequest {
    return {
        "merchantRefundId": "probe_refund_001",  // Identification.
        "connectorTransactionId": connectorTransactionId,
        "paymentAmount": 1000,  // Amount Information.
        "refundAmount": {
            "minorAmount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            "currency": Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        },
        "reason": "customer_request"  // Reason for the refund.
    };
}

function _buildRefundGetRequest(): types.IRefundServiceGetRequest {
    return {
        "merchantRefundId": "probe_refund_001",  // Identification.
        "connectorTransactionId": "probe_connector_txn_001",
        "refundId": "probe_refund_id_001"  // Deprecated.
    };
}

function _buildReverseRequest(connectorTransactionId: string): types.IPaymentServiceReverseRequest {
    return {
        "merchantReverseId": "probe_reverse_001",  // Identification.
        "connectorTransactionId": connectorTransactionId
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

function _buildTokenSetupRecurringRequest(): types.IPaymentServiceTokenSetupRecurringRequest {
    return {
        "merchantRecurringPaymentId": "probe_tokenized_mandate_001",
        "amount": {
            "minorAmount": 0,  // Amount in minor units (e.g., 1000 = $10.00).
            "currency": Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        },
        "connectorToken": {"value": "pm_1AbcXyzStripeTestToken"},
        "address": {
            "billingAddress": {
            }
        },
        "customerAcceptance": {
            "acceptanceType": AcceptanceType.ONLINE,  // Type of acceptance (e.g., online, offline).
            "acceptedAt": 0,  // Timestamp when the acceptance was made (Unix timestamp, seconds since epoch).
            "onlineMandateDetails": {  // Details if the acceptance was an online mandate.
                "ipAddress": "127.0.0.1",  // IP address from which the mandate was accepted.
                "userAgent": "Mozilla/5.0"  // User agent string of the browser used for mandate acceptance.
            }
        },
        "setupMandateDetails": {
            "mandateType": {  // Type of mandate (single_use or multi_use) with amount details.
                "multiUse": {  // Multi use mandate with amount details (for recurring payments).
                    "amount": 0,  // Use amount_money instead (will be removed in a future release).
                    "currency": Currency.USD,  // Use amount_money.currency instead (will be removed in a future release).
                    "amountMoney": {  // Amount in Money type.
                        "minorAmount": 0,  // Amount in minor units (e.g., 1000 = $10.00).
                        "currency": Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
                    }
                }
            }
        },
        "setupFutureUsage": FutureUsage.OFF_SESSION
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

// Flow: PaymentService.Refund
async function refund(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const refundResponse = await paymentClient.refund(_buildRefundRequest('probe_connector_txn_001'));

    return refundResponse;
}

// Flow: RefundService.Get
async function refundGet(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const refundClient = new RefundClient(config);

    const refundResponse = await refundClient.refundGet(_buildRefundGetRequest());

    return refundResponse;
}

// Flow: PaymentService.Reverse
async function reverse(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const reverseResponse = await paymentClient.reverse(_buildReverseRequest('probe_connector_txn_001'));

    return reverseResponse;
}

// Flow: PaymentService.TokenAuthorize
async function tokenAuthorize(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const tokenResponse = await paymentClient.tokenAuthorize(_buildTokenAuthorizeRequest());

    return tokenResponse;
}

// Flow: PaymentService.TokenSetupRecurring
async function tokenSetupRecurring(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const tokenResponse = await paymentClient.tokenSetupRecurring(_buildTokenSetupRecurringRequest());

    return tokenResponse;
}

// Flow: PaymentService.Void
async function voidPayment(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const voidResponse = await paymentClient.void(_buildVoidRequest('probe_connector_txn_001'));

    return voidResponse;
}


// Export all process* functions for the smoke test
export {
    capture, createClientAuthenticationToken, get, handleEvent, parseEvent, postAuthenticate, preAuthenticate, refund, refundGet, reverse, tokenAuthorize, tokenSetupRecurring, voidPayment, _buildCaptureRequest, _buildCreateClientAuthenticationTokenRequest, _buildGetRequest, _buildHandleEventRequest, _buildParseEventRequest, _buildPostAuthenticateRequest, _buildPreAuthenticateRequest, _buildRefundRequest, _buildRefundGetRequest, _buildReverseRequest, _buildTokenAuthorizeRequest, _buildTokenSetupRecurringRequest, _buildVoidRequest
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
