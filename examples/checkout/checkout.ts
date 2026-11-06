// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py checkout
//
// Checkout — all integration scenarios and flows in one file.
// Run a scenario:  npx tsx checkout.ts checkout_autocapture

import { PaymentClient, DisputeClient, EventClient, RecurringPaymentClient, RefundClient, types } from 'hyperswitch-prism';
const { Environment, AcceptanceType, AuthenticationType, CaptureMethod, CardNetwork, CountryAlpha2, Currency, FutureUsage, HttpMethod, PaymentMethodType, TransactionStatus } = types;
export const SUPPORTED_FLOWS = ["authorize", "capture", "dispute_accept", "dispute_defend", "get", "parse_event", "proxy_authorize", "proxy_setup_recurring", "recurring_charge", "refund", "refund_get", "setup_recurring", "token_authorize", "token_setup_recurring", "void"];

const _defaultConfig: types.IConnectorConfig = {
    options: {
        environment: Environment.SANDBOX,
    },
    connectorConfig: {
        checkout: {
            apiKey: { value: 'YOUR_API_KEY' },
            apiSecret: { value: 'YOUR_API_SECRET' },
            processingChannelId: { value: 'YOUR_PROCESSING_CHANNEL_ID' },
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
            "email": {"value": "customer.probe@example.com"}
        },
        "address": {  // Address Information.
            "shippingAddress": {
                "firstName": {"value": "Jane"},
                "line1": {"value": "123 Main St"},
                "city": {"value": "Seattle"},
                "zipCode": {"value": "98101"},
                "countryAlpha2Code": CountryAlpha2.US
            },
            "billingAddress": {
                "phoneNumber": {"value": "4155552671"},
                "phoneCountryCode": "+1"
            }
        },
        "authType": AuthenticationType.NO_THREE_DS,  // Authentication Details.
        "authenticationData": {
            "eci": "05",  // Electronic Commerce Indicator (ECI) from 3DS.
            "cavv": "AAABAWFlmQAAAABjRWWZEEFgFz8=",  // Cardholder Authentication Verification Value (CAVV).
            "messageVersion": "2.2.0",  // 3DS Message Version (e.g., "2.1.0", "2.2.0").
            "dsTransactionId": "97267598-fae6-48f2-8083-c23433990361",  // Directory Server Transaction ID (DS Trans ID).
            "transStatus": TransactionStatus.TRANSACTION_STATUS_SUCCESS,  // Transaction status from authentication/verification process.
            "createdAt": 1704067200  // Authentication timestamp (Unix epoch seconds, UTC) when the 3DS authentication was performed. Used by connectors (e.g. Cybersource) that send an authenticationDate in their request.
        },
        "returnUrl": "https://example.com/return"  // URLs for Redirection and Webhooks.
    };
}

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

function _buildDisputeAcceptRequest(): types.IDisputeServiceAcceptRequest {
    return {
        "merchantDisputeId": "probe_dispute_001",  // Identification.
        "connectorTransactionId": "probe_txn_001",
        "connectorDisputeId": "probe_dispute_id_001"
    };
}

function _buildDisputeDefendRequest(): types.IDisputeServiceDefendRequest {
    return {
        "merchantDisputeId": "probe_dispute_001",  // Identification.
        "connectorTransactionId": "probe_txn_001",
        "connectorDisputeId": "probe_dispute_id_001",
        "reasonCode": "probe_reason"  // Defend Details.
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
            "body": {"value": new Uint8Array([123, 34, 105, 100, 34, 58, 34, 101, 118, 116, 95, 100, 106, 54, 116, 112, 107, 109, 98, 104, 101, 119, 51, 98, 110, 108, 52, 110, 55, 115, 120, 122, 114, 102, 109, 115, 105, 34, 44, 34, 116, 121, 112, 101, 34, 58, 34, 112, 97, 121, 109, 101, 110, 116, 95, 99, 97, 112, 116, 117, 114, 101, 100, 34, 44, 34, 118, 101, 114, 115, 105, 111, 110, 34, 58, 34, 49, 46, 48, 46, 54, 34, 44, 34, 99, 114, 101, 97, 116, 101, 100, 95, 111, 110, 34, 58, 34, 50, 48, 50, 48, 45, 48, 56, 45, 49, 55, 84, 49, 52, 58, 49, 50, 58, 53, 57, 90, 34, 44, 34, 100, 97, 116, 97, 34, 58, 123, 34, 105, 100, 34, 58, 34, 112, 97, 121, 95, 121, 51, 111, 113, 104, 102, 52, 54, 112, 121, 122, 117, 120, 106, 98, 99, 110, 50, 103, 105, 97, 113, 110, 98, 52, 52, 34, 44, 34, 97, 99, 116, 105, 111, 110, 95, 105, 100, 34, 58, 34, 97, 99, 116, 95, 121, 51, 111, 113, 104, 102, 52, 54, 112, 121, 122, 117, 120, 106, 98, 99, 110, 50, 103, 105, 97, 113, 110, 98, 52, 52, 34, 44, 34, 97, 109, 111, 117, 110, 116, 34, 58, 57, 57, 57, 44, 34, 99, 117, 114, 114, 101, 110, 99, 121, 34, 58, 34, 85, 83, 68, 34, 44, 34, 97, 112, 112, 114, 111, 118, 101, 100, 34, 58, 116, 114, 117, 101, 44, 34, 115, 116, 97, 116, 117, 115, 34, 58, 34, 67, 97, 112, 116, 117, 114, 101, 100, 34, 44, 34, 97, 117, 116, 104, 95, 99, 111, 100, 101, 34, 58, 34, 57, 53, 54, 48, 56, 52, 34, 44, 34, 114, 101, 115, 112, 111, 110, 115, 101, 95, 99, 111, 100, 101, 34, 58, 34, 49, 48, 48, 48, 48, 34, 44, 34, 114, 101, 115, 112, 111, 110, 115, 101, 95, 115, 117, 109, 109, 97, 114, 121, 34, 58, 34, 65, 112, 112, 114, 111, 118, 101, 100, 34, 44, 34, 114, 101, 102, 101, 114, 101, 110, 99, 101, 34, 58, 34, 79, 82, 68, 45, 53, 48, 50, 51, 45, 52, 69, 56, 57, 34, 44, 34, 112, 97, 121, 109, 101, 110, 116, 95, 105, 100, 34, 58, 34, 112, 97, 121, 95, 121, 51, 111, 113, 104, 102, 52, 54, 112, 121, 122, 117, 120, 106, 98, 99, 110, 50, 103, 105, 97, 113, 110, 98, 52, 52, 34, 44, 34, 97, 99, 116, 105, 111, 110, 95, 116, 121, 112, 101, 34, 58, 34, 67, 97, 112, 116, 117, 114, 101, 34, 44, 34, 112, 114, 111, 99, 101, 115, 115, 101, 100, 95, 111, 110, 34, 58, 34, 50, 48, 50, 48, 45, 48, 56, 45, 49, 55, 84, 49, 52, 58, 49, 50, 58, 53, 57, 90, 34, 44, 34, 112, 114, 111, 99, 101, 115, 115, 105, 110, 103, 34, 58, 123, 34, 97, 99, 113, 117, 105, 114, 101, 114, 95, 116, 114, 97, 110, 115, 97, 99, 116, 105, 111, 110, 95, 105, 100, 34, 58, 34, 56, 54, 54, 52, 54, 49, 52, 51, 49, 51, 54, 48, 48, 50, 53, 34, 44, 34, 97, 99, 113, 117, 105, 114, 101, 114, 95, 114, 101, 102, 101, 114, 101, 110, 99, 101, 95, 110, 117, 109, 98, 101, 114, 34, 58, 34, 48, 48, 50, 54, 48, 57, 55, 49, 51, 55, 53, 54, 49, 56, 52, 52, 54, 52, 56, 50, 52, 51, 56, 34, 125, 125, 44, 34, 95, 108, 105, 110, 107, 115, 34, 58, 123, 34, 115, 101, 108, 102, 34, 58, 123, 34, 104, 114, 101, 102, 34, 58, 34, 104, 116, 116, 112, 115, 58, 47, 47, 97, 112, 105, 46, 115, 97, 110, 100, 98, 111, 120, 46, 99, 104, 101, 99, 107, 111, 117, 116, 46, 99, 111, 109, 47, 119, 111, 114, 107, 102, 108, 111, 119, 115, 47, 101, 118, 101, 110, 116, 115, 47, 101, 118, 116, 95, 100, 106, 54, 116, 112, 107, 109, 98, 104, 101, 119, 51, 98, 110, 108, 52, 110, 55, 115, 120, 122, 114, 102, 109, 115, 105, 34, 125, 125, 125])},  // Secret-typed replacements. Presence takes precedence over the legacy field, including an explicitly empty body, map, URI or query string.
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
            "body": {"value": new Uint8Array([123, 34, 105, 100, 34, 58, 34, 101, 118, 116, 95, 100, 106, 54, 116, 112, 107, 109, 98, 104, 101, 119, 51, 98, 110, 108, 52, 110, 55, 115, 120, 122, 114, 102, 109, 115, 105, 34, 44, 34, 116, 121, 112, 101, 34, 58, 34, 112, 97, 121, 109, 101, 110, 116, 95, 99, 97, 112, 116, 117, 114, 101, 100, 34, 44, 34, 118, 101, 114, 115, 105, 111, 110, 34, 58, 34, 49, 46, 48, 46, 54, 34, 44, 34, 99, 114, 101, 97, 116, 101, 100, 95, 111, 110, 34, 58, 34, 50, 48, 50, 48, 45, 48, 56, 45, 49, 55, 84, 49, 52, 58, 49, 50, 58, 53, 57, 90, 34, 44, 34, 100, 97, 116, 97, 34, 58, 123, 34, 105, 100, 34, 58, 34, 112, 97, 121, 95, 121, 51, 111, 113, 104, 102, 52, 54, 112, 121, 122, 117, 120, 106, 98, 99, 110, 50, 103, 105, 97, 113, 110, 98, 52, 52, 34, 44, 34, 97, 99, 116, 105, 111, 110, 95, 105, 100, 34, 58, 34, 97, 99, 116, 95, 121, 51, 111, 113, 104, 102, 52, 54, 112, 121, 122, 117, 120, 106, 98, 99, 110, 50, 103, 105, 97, 113, 110, 98, 52, 52, 34, 44, 34, 97, 109, 111, 117, 110, 116, 34, 58, 57, 57, 57, 44, 34, 99, 117, 114, 114, 101, 110, 99, 121, 34, 58, 34, 85, 83, 68, 34, 44, 34, 97, 112, 112, 114, 111, 118, 101, 100, 34, 58, 116, 114, 117, 101, 44, 34, 115, 116, 97, 116, 117, 115, 34, 58, 34, 67, 97, 112, 116, 117, 114, 101, 100, 34, 44, 34, 97, 117, 116, 104, 95, 99, 111, 100, 101, 34, 58, 34, 57, 53, 54, 48, 56, 52, 34, 44, 34, 114, 101, 115, 112, 111, 110, 115, 101, 95, 99, 111, 100, 101, 34, 58, 34, 49, 48, 48, 48, 48, 34, 44, 34, 114, 101, 115, 112, 111, 110, 115, 101, 95, 115, 117, 109, 109, 97, 114, 121, 34, 58, 34, 65, 112, 112, 114, 111, 118, 101, 100, 34, 44, 34, 114, 101, 102, 101, 114, 101, 110, 99, 101, 34, 58, 34, 79, 82, 68, 45, 53, 48, 50, 51, 45, 52, 69, 56, 57, 34, 44, 34, 112, 97, 121, 109, 101, 110, 116, 95, 105, 100, 34, 58, 34, 112, 97, 121, 95, 121, 51, 111, 113, 104, 102, 52, 54, 112, 121, 122, 117, 120, 106, 98, 99, 110, 50, 103, 105, 97, 113, 110, 98, 52, 52, 34, 44, 34, 97, 99, 116, 105, 111, 110, 95, 116, 121, 112, 101, 34, 58, 34, 67, 97, 112, 116, 117, 114, 101, 34, 44, 34, 112, 114, 111, 99, 101, 115, 115, 101, 100, 95, 111, 110, 34, 58, 34, 50, 48, 50, 48, 45, 48, 56, 45, 49, 55, 84, 49, 52, 58, 49, 50, 58, 53, 57, 90, 34, 44, 34, 112, 114, 111, 99, 101, 115, 115, 105, 110, 103, 34, 58, 123, 34, 97, 99, 113, 117, 105, 114, 101, 114, 95, 116, 114, 97, 110, 115, 97, 99, 116, 105, 111, 110, 95, 105, 100, 34, 58, 34, 56, 54, 54, 52, 54, 49, 52, 51, 49, 51, 54, 48, 48, 50, 53, 34, 44, 34, 97, 99, 113, 117, 105, 114, 101, 114, 95, 114, 101, 102, 101, 114, 101, 110, 99, 101, 95, 110, 117, 109, 98, 101, 114, 34, 58, 34, 48, 48, 50, 54, 48, 57, 55, 49, 51, 55, 53, 54, 49, 56, 52, 52, 54, 52, 56, 50, 52, 51, 56, 34, 125, 125, 44, 34, 95, 108, 105, 110, 107, 115, 34, 58, 123, 34, 115, 101, 108, 102, 34, 58, 123, 34, 104, 114, 101, 102, 34, 58, 34, 104, 116, 116, 112, 115, 58, 47, 47, 97, 112, 105, 46, 115, 97, 110, 100, 98, 111, 120, 46, 99, 104, 101, 99, 107, 111, 117, 116, 46, 99, 111, 109, 47, 119, 111, 114, 107, 102, 108, 111, 119, 115, 47, 101, 118, 101, 110, 116, 115, 47, 101, 118, 116, 95, 100, 106, 54, 116, 112, 107, 109, 98, 104, 101, 119, 51, 98, 110, 108, 52, 110, 55, 115, 120, 122, 114, 102, 109, 115, 105, 34, 125, 125, 125])},  // Secret-typed replacements. Presence takes precedence over the legacy field, including an explicitly empty body, map, URI or query string.
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
        "address": {
            "billingAddress": {
            }
        },
        "captureMethod": CaptureMethod.AUTOMATIC,
        "authType": AuthenticationType.NO_THREE_DS,
        "returnUrl": "https://example.com/return"
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
        "connectorCustomerId": "cust_probe_123",
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

function _buildTokenAuthorizeRequest(): types.IPaymentServiceTokenAuthorizeRequest {
    return {
        "merchantTransactionId": "probe_tokenized_txn_001",
        "amount": {
            "minorAmount": 1000,
            "currency": Currency.USD
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
            "minorAmount": 0,
            "currency": Currency.USD
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
                        "minorAmount": 0,
                        "currency": Currency.USD
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

// Void Payment
// Cancel an authorized but not-yet-captured payment.
async function processVoidPayment(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
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

    // Step 2: Void — release reserved funds (cancel authorization)
    const voidResponse = await paymentClient.void(_buildVoidRequest(authorizeResponse.connectorTransactionId!));

    return { status: voidResponse.status, transactionId: authorizeResponse.connectorTransactionId!, error: voidResponse.error } as any;
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

// Flow: PaymentService.Capture
async function capture(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const captureResponse = await paymentClient.capture(_buildCaptureRequest('probe_connector_txn_001'));

    return captureResponse;
}

// Flow: DisputeService.Accept
async function disputeAccept(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const disputeClient = new DisputeClient(config);

    const disputeResponse = await disputeClient.accept(_buildDisputeAcceptRequest());

    return disputeResponse;
}

// Flow: DisputeService.Defend
async function disputeDefend(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const disputeClient = new DisputeClient(config);

    const disputeResponse = await disputeClient.defend(_buildDisputeDefendRequest());

    return disputeResponse;
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

// Flow: PaymentService.ProxySetupRecurring
async function proxySetupRecurring(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const proxyResponse = await paymentClient.proxySetupRecurring(_buildProxySetupRecurringRequest());

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

// Flow: PaymentService.SetupRecurring
async function setupRecurring(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const setupResponse = await paymentClient.setupRecurring(_buildSetupRecurringRequest());

    return setupResponse;
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
    processCheckoutAutocapture, processCheckoutCard, processRefund, processVoidPayment, processGetPayment, authorize, capture, disputeAccept, disputeDefend, get, handleEvent, parseEvent, proxyAuthorize, proxySetupRecurring, recurringCharge, refund, refundGet, setupRecurring, tokenAuthorize, tokenSetupRecurring, voidPayment, _buildAuthorizeRequest, _buildCaptureRequest, _buildDisputeAcceptRequest, _buildDisputeDefendRequest, _buildGetRequest, _buildHandleEventRequest, _buildParseEventRequest, _buildProxyAuthorizeRequest, _buildProxySetupRecurringRequest, _buildRecurringChargeRequest, _buildRefundRequest, _buildRefundGetRequest, _buildSetupRecurringRequest, _buildTokenAuthorizeRequest, _buildTokenSetupRecurringRequest, _buildVoidRequest
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
