// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py givepayments
//
// Givepayments — all integration scenarios and flows in one file.
// Run a scenario:  npx tsx givepayments.ts checkout_autocapture

import { PaymentClient, EventClient, RecurringPaymentClient, RefundClient, types } from 'hyperswitch-prism';
const { Environment, AuthenticationType, CaptureMethod, CardNetwork, Currency, HttpMethod, PaymentMethodType } = types;
export const SUPPORTED_FLOWS = ["authorize", "get", "parse_event", "proxy_authorize", "recurring_charge", "refund", "refund_get"];

const _defaultConfig: types.IConnectorConfig = {
    options: {
        environment: Environment.SANDBOX,
    },
    connectorConfig: {
        givepayments: {
            apiKey: { value: 'YOUR_API_KEY' },
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
        "customer": {  // Customer Information.
            "email": {"value": "test@example.com"}
        },
        "address": {  // Address Information.
            "billingAddress": {
            }
        },
        "authType": AuthenticationType.NO_THREE_DS,  // Authentication Details.
        "returnUrl": "https://example.com/return",  // URLs for Redirection and Webhooks.
        "browserInfo": {
            "userAgent": "Mozilla/5.0 (probe-bot)",
            "ipAddress": "1.2.3.4"  // Device Information.
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
            "body": {"value": new Uint8Array([123, 34, 105, 100, 34, 58, 32, 34, 71, 83, 95, 69, 86, 95, 112, 109, 86, 48, 76, 79, 121, 81, 118, 72, 89, 110, 71, 49, 86, 78, 90, 68, 50, 81, 101, 69, 34, 44, 34, 99, 114, 101, 97, 116, 101, 100, 95, 97, 116, 34, 58, 32, 34, 49, 54, 54, 49, 57, 57, 48, 52, 48, 48, 34, 44, 34, 116, 121, 112, 101, 34, 58, 32, 34, 112, 97, 121, 109, 101, 110, 116, 46, 99, 97, 112, 116, 117, 114, 101, 100, 34, 44, 34, 100, 97, 116, 97, 34, 58, 32, 123, 32, 34, 116, 121, 112, 101, 34, 58, 32, 34, 112, 97, 121, 109, 101, 110, 116, 34, 44, 32, 34, 111, 98, 106, 101, 99, 116, 34, 58, 32, 123, 32, 34, 105, 100, 34, 58, 32, 34, 71, 83, 95, 84, 88, 78, 95, 99, 75, 80, 49, 99, 116, 109, 119, 84, 104, 89, 97, 65, 53, 85, 74, 114, 85, 71, 54, 55, 65, 34, 44, 32, 34, 105, 100, 34, 58, 32, 34, 71, 83, 95, 84, 88, 78, 95, 99, 75, 80, 49, 99, 116, 109, 119, 84, 104, 89, 97, 65, 53, 85, 74, 114, 85, 71, 54, 55, 65, 34, 44, 32, 34, 99, 114, 101, 97, 116, 101, 100, 95, 97, 116, 34, 58, 32, 49, 54, 54, 49, 57, 57, 48, 52, 48, 48, 44, 32, 34, 117, 112, 100, 97, 116, 101, 100, 95, 97, 116, 34, 58, 32, 49, 54, 54, 49, 57, 57, 48, 52, 48, 48, 44, 32, 34, 115, 101, 116, 116, 108, 101, 100, 95, 97, 116, 34, 58, 32, 110, 117, 108, 108, 44, 32, 34, 115, 116, 97, 116, 117, 115, 34, 58, 32, 34, 115, 117, 99, 99, 101, 115, 115, 102, 117, 108, 34, 44, 32, 34, 112, 114, 111, 99, 101, 115, 115, 105, 110, 103, 95, 115, 116, 97, 116, 101, 34, 58, 32, 34, 99, 97, 112, 116, 117, 114, 101, 100, 34, 44, 32, 34, 116, 111, 116, 97, 108, 95, 97, 109, 111, 117, 110, 116, 34, 58, 32, 53, 48, 48, 44, 32, 34, 110, 101, 116, 95, 97, 109, 111, 117, 110, 116, 34, 58, 32, 53, 48, 48, 44, 32, 34, 102, 101, 101, 95, 97, 109, 111, 117, 110, 116, 34, 58, 32, 52, 52, 44, 32, 34, 102, 101, 101, 115, 95, 112, 97, 105, 100, 95, 98, 121, 34, 58, 32, 34, 109, 101, 114, 99, 104, 97, 110, 116, 34, 44, 32, 34, 100, 101, 115, 99, 114, 105, 112, 116, 105, 111, 110, 34, 58, 32, 34, 34, 44, 32, 34, 114, 101, 118, 101, 114, 115, 97, 108, 95, 115, 116, 97, 116, 117, 115, 34, 58, 32, 34, 110, 111, 116, 95, 114, 101, 118, 101, 114, 115, 101, 100, 34, 44, 32, 34, 98, 105, 108, 108, 105, 110, 103, 95, 100, 101, 115, 99, 114, 105, 112, 116, 111, 114, 34, 58, 32, 34, 49, 79, 70, 70, 73, 67, 69, 83, 85, 80, 80, 76, 73, 69, 83, 83, 84, 79, 82, 69, 34, 44, 32, 34, 114, 105, 115, 107, 34, 58, 32, 123, 32, 34, 113, 117, 97, 114, 97, 110, 116, 105, 110, 101, 34, 58, 32, 102, 97, 108, 115, 101, 44, 32, 34, 114, 105, 115, 107, 95, 108, 101, 118, 101, 108, 34, 58, 32, 34, 108, 111, 119, 34, 44, 32, 34, 97, 115, 115, 101, 115, 115, 109, 101, 110, 116, 34, 58, 32, 34, 34, 32, 125, 44, 32, 34, 112, 97, 121, 109, 101, 116, 104, 111, 100, 34, 58, 32, 123, 32, 34, 116, 121, 112, 101, 34, 58, 32, 34, 99, 97, 114, 100, 34, 44, 32, 34, 99, 97, 114, 100, 34, 58, 32, 123, 32, 34, 105, 100, 34, 58, 32, 34, 71, 83, 95, 80, 77, 67, 95, 55, 99, 109, 97, 102, 120, 55, 65, 53, 51, 50, 117, 73, 105, 90, 97, 71, 82, 115, 69, 52, 68, 34, 44, 32, 34, 99, 114, 101, 97, 116, 101, 100, 95, 97, 116, 34, 58, 32, 49, 54, 54, 49, 57, 57, 48, 52, 48, 48, 44, 32, 34, 117, 112, 100, 97, 116, 101, 100, 95, 97, 116, 34, 58, 32, 49, 54, 54, 49, 57, 57, 48, 52, 48, 48, 44, 32, 34, 98, 114, 97, 110, 100, 34, 58, 32, 34, 118, 105, 115, 97, 34, 44, 32, 34, 110, 97, 109, 101, 34, 58, 32, 34, 74, 97, 99, 107, 32, 70, 114, 97, 110, 99, 105, 115, 34, 44, 32, 34, 110, 117, 109, 98, 101, 114, 95, 108, 97, 115, 116, 52, 34, 58, 32, 34, 49, 49, 49, 49, 34, 44, 32, 34, 101, 120, 112, 95, 121, 101, 97, 114, 34, 58, 32, 50, 48, 50, 51, 44, 32, 34, 101, 120, 112, 95, 109, 111, 110, 116, 104, 34, 58, 32, 56, 44, 32, 34, 105, 115, 95, 100, 101, 98, 105, 116, 34, 58, 32, 102, 97, 108, 115, 101, 44, 32, 34, 117, 115, 101, 114, 34, 58, 32, 34, 71, 83, 95, 85, 83, 82, 95, 53, 122, 57, 81, 120, 73, 49, 99, 71, 49, 89, 65, 65, 90, 71, 86, 57, 110, 111, 115, 52, 66, 34, 44, 32, 34, 97, 100, 100, 114, 101, 115, 115, 34, 58, 32, 110, 117, 108, 108, 32, 125, 125, 44, 32, 34, 99, 117, 115, 116, 111, 109, 101, 114, 34, 58, 32, 34, 71, 83, 95, 67, 85, 83, 95, 50, 114, 98, 122, 114, 69, 97, 101, 66, 78, 119, 78, 77, 97, 102, 82, 120, 75, 66, 102, 83, 98, 34, 44, 32, 34, 109, 101, 114, 99, 104, 97, 110, 116, 34, 58, 32, 34, 71, 83, 95, 77, 69, 82, 95, 79, 86, 67, 51, 83, 75, 121, 109, 68, 51, 52, 83, 72, 53, 78, 106, 69, 104, 80, 97, 56, 68, 34, 32, 125, 125, 44, 32, 34, 109, 101, 114, 99, 104, 97, 110, 116, 34, 58, 32, 34, 71, 83, 95, 77, 69, 82, 95, 79, 86, 67, 51, 83, 75, 121, 109, 68, 51, 52, 83, 72, 53, 78, 106, 69, 104, 80, 97, 56, 68, 34, 125])},  // Secret-typed replacements. Presence takes precedence over the legacy field, including an explicitly empty body, map, URI or query string.
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
            "body": {"value": new Uint8Array([123, 34, 105, 100, 34, 58, 32, 34, 71, 83, 95, 69, 86, 95, 112, 109, 86, 48, 76, 79, 121, 81, 118, 72, 89, 110, 71, 49, 86, 78, 90, 68, 50, 81, 101, 69, 34, 44, 34, 99, 114, 101, 97, 116, 101, 100, 95, 97, 116, 34, 58, 32, 34, 49, 54, 54, 49, 57, 57, 48, 52, 48, 48, 34, 44, 34, 116, 121, 112, 101, 34, 58, 32, 34, 112, 97, 121, 109, 101, 110, 116, 46, 99, 97, 112, 116, 117, 114, 101, 100, 34, 44, 34, 100, 97, 116, 97, 34, 58, 32, 123, 32, 34, 116, 121, 112, 101, 34, 58, 32, 34, 112, 97, 121, 109, 101, 110, 116, 34, 44, 32, 34, 111, 98, 106, 101, 99, 116, 34, 58, 32, 123, 32, 34, 105, 100, 34, 58, 32, 34, 71, 83, 95, 84, 88, 78, 95, 99, 75, 80, 49, 99, 116, 109, 119, 84, 104, 89, 97, 65, 53, 85, 74, 114, 85, 71, 54, 55, 65, 34, 44, 32, 34, 105, 100, 34, 58, 32, 34, 71, 83, 95, 84, 88, 78, 95, 99, 75, 80, 49, 99, 116, 109, 119, 84, 104, 89, 97, 65, 53, 85, 74, 114, 85, 71, 54, 55, 65, 34, 44, 32, 34, 99, 114, 101, 97, 116, 101, 100, 95, 97, 116, 34, 58, 32, 49, 54, 54, 49, 57, 57, 48, 52, 48, 48, 44, 32, 34, 117, 112, 100, 97, 116, 101, 100, 95, 97, 116, 34, 58, 32, 49, 54, 54, 49, 57, 57, 48, 52, 48, 48, 44, 32, 34, 115, 101, 116, 116, 108, 101, 100, 95, 97, 116, 34, 58, 32, 110, 117, 108, 108, 44, 32, 34, 115, 116, 97, 116, 117, 115, 34, 58, 32, 34, 115, 117, 99, 99, 101, 115, 115, 102, 117, 108, 34, 44, 32, 34, 112, 114, 111, 99, 101, 115, 115, 105, 110, 103, 95, 115, 116, 97, 116, 101, 34, 58, 32, 34, 99, 97, 112, 116, 117, 114, 101, 100, 34, 44, 32, 34, 116, 111, 116, 97, 108, 95, 97, 109, 111, 117, 110, 116, 34, 58, 32, 53, 48, 48, 44, 32, 34, 110, 101, 116, 95, 97, 109, 111, 117, 110, 116, 34, 58, 32, 53, 48, 48, 44, 32, 34, 102, 101, 101, 95, 97, 109, 111, 117, 110, 116, 34, 58, 32, 52, 52, 44, 32, 34, 102, 101, 101, 115, 95, 112, 97, 105, 100, 95, 98, 121, 34, 58, 32, 34, 109, 101, 114, 99, 104, 97, 110, 116, 34, 44, 32, 34, 100, 101, 115, 99, 114, 105, 112, 116, 105, 111, 110, 34, 58, 32, 34, 34, 44, 32, 34, 114, 101, 118, 101, 114, 115, 97, 108, 95, 115, 116, 97, 116, 117, 115, 34, 58, 32, 34, 110, 111, 116, 95, 114, 101, 118, 101, 114, 115, 101, 100, 34, 44, 32, 34, 98, 105, 108, 108, 105, 110, 103, 95, 100, 101, 115, 99, 114, 105, 112, 116, 111, 114, 34, 58, 32, 34, 49, 79, 70, 70, 73, 67, 69, 83, 85, 80, 80, 76, 73, 69, 83, 83, 84, 79, 82, 69, 34, 44, 32, 34, 114, 105, 115, 107, 34, 58, 32, 123, 32, 34, 113, 117, 97, 114, 97, 110, 116, 105, 110, 101, 34, 58, 32, 102, 97, 108, 115, 101, 44, 32, 34, 114, 105, 115, 107, 95, 108, 101, 118, 101, 108, 34, 58, 32, 34, 108, 111, 119, 34, 44, 32, 34, 97, 115, 115, 101, 115, 115, 109, 101, 110, 116, 34, 58, 32, 34, 34, 32, 125, 44, 32, 34, 112, 97, 121, 109, 101, 116, 104, 111, 100, 34, 58, 32, 123, 32, 34, 116, 121, 112, 101, 34, 58, 32, 34, 99, 97, 114, 100, 34, 44, 32, 34, 99, 97, 114, 100, 34, 58, 32, 123, 32, 34, 105, 100, 34, 58, 32, 34, 71, 83, 95, 80, 77, 67, 95, 55, 99, 109, 97, 102, 120, 55, 65, 53, 51, 50, 117, 73, 105, 90, 97, 71, 82, 115, 69, 52, 68, 34, 44, 32, 34, 99, 114, 101, 97, 116, 101, 100, 95, 97, 116, 34, 58, 32, 49, 54, 54, 49, 57, 57, 48, 52, 48, 48, 44, 32, 34, 117, 112, 100, 97, 116, 101, 100, 95, 97, 116, 34, 58, 32, 49, 54, 54, 49, 57, 57, 48, 52, 48, 48, 44, 32, 34, 98, 114, 97, 110, 100, 34, 58, 32, 34, 118, 105, 115, 97, 34, 44, 32, 34, 110, 97, 109, 101, 34, 58, 32, 34, 74, 97, 99, 107, 32, 70, 114, 97, 110, 99, 105, 115, 34, 44, 32, 34, 110, 117, 109, 98, 101, 114, 95, 108, 97, 115, 116, 52, 34, 58, 32, 34, 49, 49, 49, 49, 34, 44, 32, 34, 101, 120, 112, 95, 121, 101, 97, 114, 34, 58, 32, 50, 48, 50, 51, 44, 32, 34, 101, 120, 112, 95, 109, 111, 110, 116, 104, 34, 58, 32, 56, 44, 32, 34, 105, 115, 95, 100, 101, 98, 105, 116, 34, 58, 32, 102, 97, 108, 115, 101, 44, 32, 34, 117, 115, 101, 114, 34, 58, 32, 34, 71, 83, 95, 85, 83, 82, 95, 53, 122, 57, 81, 120, 73, 49, 99, 71, 49, 89, 65, 65, 90, 71, 86, 57, 110, 111, 115, 52, 66, 34, 44, 32, 34, 97, 100, 100, 114, 101, 115, 115, 34, 58, 32, 110, 117, 108, 108, 32, 125, 125, 44, 32, 34, 99, 117, 115, 116, 111, 109, 101, 114, 34, 58, 32, 34, 71, 83, 95, 67, 85, 83, 95, 50, 114, 98, 122, 114, 69, 97, 101, 66, 78, 119, 78, 77, 97, 102, 82, 120, 75, 66, 102, 83, 98, 34, 44, 32, 34, 109, 101, 114, 99, 104, 97, 110, 116, 34, 58, 32, 34, 71, 83, 95, 77, 69, 82, 95, 79, 86, 67, 51, 83, 75, 121, 109, 68, 51, 52, 83, 72, 53, 78, 106, 69, 104, 80, 97, 56, 68, 34, 32, 125, 125, 44, 32, 34, 109, 101, 114, 99, 104, 97, 110, 116, 34, 58, 32, 34, 71, 83, 95, 77, 69, 82, 95, 79, 86, 67, 51, 83, 75, 121, 109, 68, 51, 52, 83, 72, 53, 78, 106, 69, 104, 80, 97, 56, 68, 34, 125])},  // Secret-typed replacements. Presence takes precedence over the legacy field, including an explicitly empty body, map, URI or query string.
            "uri": {"value": "https://example.com/webhook"}
        }
    };
}

function _buildProxyAuthorizeRequest(): types.IPaymentServiceProxyAuthorizeRequest {
    return {
        "merchantTransactionId": "probe_proxy_txn_001",
        "amount": {
            "minorAmount": 1000,
            "currency": Currency.USD
        },
        "cardProxy": {  // Card proxy for vault-aliased payments (VGS, Basis Theory, Spreedly). Real card values are substituted by the proxy before reaching the connector.
            "cardNumber": {"value": "4111111111111111"},  // Card Identification.
            "cardExpMonth": {"value": "03"},
            "cardExpYear": {"value": "2030"},
            "cardCvc": {"value": "123"},
            "cardHolderName": {"value": "John Doe"},  // Cardholder Information.
            "cardNetwork": CardNetwork.VISA
        },
        "customer": {
            "email": {"value": "test@example.com"}
        },
        "address": {
            "billingAddress": {
            }
        },
        "captureMethod": CaptureMethod.AUTOMATIC,
        "authType": AuthenticationType.NO_THREE_DS,
        "returnUrl": "https://example.com/return",
        "browserInfo": {
            "userAgent": "Mozilla/5.0 (probe-bot)",
            "ipAddress": "1.2.3.4"  // Device Information.
        }
    };
}

function _buildRecurringChargeRequest(): types.IRecurringPaymentServiceChargeRequest {
    return {
        "connectorRecurringPaymentId": {  // Reference to existing mandate.
        },
        "amount": {  // Amount Information.
            "minorAmount": 1000,
            "currency": Currency.USD
        },
        "paymentMethod": {  // Optional payment Method Information (for network transaction flows).
            "token": {  // Payment tokens.
                "token": {"value": "probe_pm_token"}  // The token string representing a payment method.
            }
        },
        "returnUrl": "https://example.com/recurring-return",
        "email": {"value": "test@example.com"},  // Customer Information.
        "connectorCustomerId": "cust_probe_123",
        "browserInfo": {  // Browser Information.
            "colorDepth": 24,  // Display Information.
            "screenHeight": 900,
            "screenWidth": 1440,
            "javaEnabled": false,  // Browser Settings.
            "javaScriptEnabled": true,
            "language": "en-US",
            "timeZoneOffsetMinutes": -480,
            "acceptHeader": "application/json",  // Browser Headers.
            "userAgent": "Mozilla/5.0 (probe-bot)",
            "acceptLanguage": "en-US,en;q=0.9",
            "ipAddress": "1.2.3.4"  // Device Information.
        },
        "paymentMethodType": PaymentMethodType.PAY_PAL,
        "offSession": true  // Behavioral Flags and Preferences.
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

function _buildRefundGetRequest(): types.IRefundServiceGetRequest {
    return {
        "merchantRefundId": "probe_refund_001",  // Identification.
        "connectorTransactionId": "probe_connector_txn_001",
        "refundId": "probe_refund_id_001"  // Deprecated.
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

// Flow: PaymentService.ProxyAuthorize
async function proxyAuthorize(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const proxyResponse = await paymentClient.proxyAuthorize(_buildProxyAuthorizeRequest());

    return proxyResponse;
}

// Flow: RecurringPaymentService.Charge
async function recurringCharge(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const recurringPaymentClient = new RecurringPaymentClient(config);

    const recurringResponse = await recurringPaymentClient.charge(_buildRecurringChargeRequest());

    return recurringResponse;
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


// Export all process* functions for the smoke test
export {
    processCheckoutAutocapture, processRefund, processGetPayment, authorize, get, handleEvent, parseEvent, proxyAuthorize, recurringCharge, refund, refundGet, _buildAuthorizeRequest, _buildGetRequest, _buildHandleEventRequest, _buildParseEventRequest, _buildProxyAuthorizeRequest, _buildRecurringChargeRequest, _buildRefundRequest, _buildRefundGetRequest
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
