// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py paypal
//
// Paypal — all integration scenarios and flows in one file.
// Run a scenario:  npx tsx paypal.ts checkout_autocapture

import { PaymentClient, PaymentMethodAuthenticationClient, MerchantAuthenticationClient, EventClient, RecurringPaymentClient, RefundClient, types } from 'hyperswitch-prism';
const { Environment, AcceptanceType, AuthenticationType, CaptureMethod, CardNetwork, CountryAlpha2, Currency, FutureUsage, HttpMethod, PaymentMethodType } = types;
export const SUPPORTED_FLOWS = ["authenticate", "authorize", "capture", "create_client_authentication_token", "create_order", "create_server_authentication_token", "get", "parse_event", "post_authenticate", "proxy_authorize", "recurring_charge", "refund", "refund_get", "setup_recurring", "void"];

const _defaultConfig: types.IConnectorConfig = {
    options: {
        environment: Environment.SANDBOX,
    },
    connectorConfig: {
        paypal: {
            clientId: { value: 'YOUR_CLIENT_ID' },
            clientSecret: { value: 'YOUR_CLIENT_SECRET' },
            payerId: { value: 'YOUR_PAYER_ID' },
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
            "shippingAddress": {
                "firstName": {"value": "Jane"},  // Personal Information.
                "lastName": {"value": "Doe"},
                "line1": {"value": "12 Main Street"},  // Address Details.
                "city": {"value": "Seattle"},
                "state": {"value": "WA"},
                "zipCode": {"value": "98101"},
                "countryAlpha2Code": CountryAlpha2.US
            },
            "billingAddress": {
            }
        },
        "connectorFeatureData": {"value": "{\"authentication_order_context\":{\"version\":1,\"request\":{\"merchant_transaction_id\":\"probe_paypal_native\",\"merchant_order_id\":\"probe_native_order\",\"order_tax_amount\":100,\"shipping_cost\":200,\"setup_future_usage\":\"OFF_SESSION\",\"billing_descriptor\":{\"statement_descriptor\":\"PROBE NATIVE\"},\"l2_l3_data\":{\"order_info\":{\"merchant_order_reference_id\":\"probe_native_l23\",\"order_details\":[]}},\"description\":\"Native safe context probe\"}}}"},  // Connector-specific metadata for the transaction.
        "returnUrl": "https://example.com/3ds-return",  // URLs for Redirection.
        "state": {  // State Information.
            "accessToken": {  // Access token obtained from connector.
                "token": {"value": "probe_access_token"},  // The token string.
                "expiresInSeconds": 3600,  // Expiration timestamp (seconds since epoch).
                "tokenType": "Bearer"  // Token type (e.g., "Bearer", "Basic").
            }
        }
    };
}

function _buildAuthorizeRequest(captureMethod: types.CaptureMethod): types.IPaymentServiceAuthorizeRequest {
    return {
        "merchantTransactionId": "probe_txn_001",  // Identification.
        "amount": {  // The amount for the payment.
            "minorAmount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            "currency": Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        },
        "orderTaxAmount": 100,  // Tax amount for the order.
        "shippingCost": 200,  // Cost of shipping for the order.
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
            "shippingAddress": {
                "firstName": {"value": "Jane"},  // Personal Information.
                "lastName": {"value": "Doe"},
                "line1": {"value": "12 Main Street"},  // Address Details.
                "city": {"value": "Seattle"},
                "state": {"value": "WA"},
                "zipCode": {"value": "98101"},
                "countryAlpha2Code": CountryAlpha2.US
            },
            "billingAddress": {
                "line2": {"value": "Suite 10"},
                "state": {"value": "WA"}
            }
        },
        "authType": AuthenticationType.NO_THREE_DS,  // Authentication Details.
        "connectorFeatureData": {"value": "{\"authorize_id\":\"probe_auth_001\",\"capture_id\":\"probe_cap_001\",\"psync_flow\":\"CAPTURE\",\"order_id\":\"probe_order_001\"}"},  // Connector-specific feature data for the transaction.
        "returnUrl": "https://example.com/return",  // URLs for Redirection and Webhooks.
        "billingDescriptor": {  // Statement Descriptor.
            "statementDescriptor": "PROBE DESCRIPTOR"  // Complete description that appears on customers' statements.
        },
        "state": {  // State Information.
            "accessToken": {  // Access token obtained from connector.
                "token": {"value": "probe_access_token"},  // The token string.
                "expiresInSeconds": 3600,  // Expiration timestamp (seconds since epoch).
                "tokenType": "Bearer"  // Token type (e.g., "Bearer", "Basic").
            }
        },
        "l2L3Data": {  // Level 2 / Level 3 data for enhanced payment processing.
            "orderInfo": {  // Order-level information.
                "orderDetails": [  // Line items for the order.
                    {
                        "productName": "Probe item",  // Name of the product that is being purchased.
                        "quantity": 1,  // The quantity of the product to be purchased.
                        "amount": 700,  // Amount per quantity (in minor currency units).
                        "totalTaxAmount": 100,  // Total tax amount (in minor currency units).
                        "productId": "sku_probe_001",  // ID of the product that is being purchased.
                        "description": "Probe line item",  // Description for the item.
                        "unitOfMeasure": "EACH",  // Unit of measure used for the item quantity.
                        "commodityCode": "4900"  // Commodity code describing the product classification.
                    }
                ],
                "merchantOrderReferenceId": "probe_invoice_001",  // Merchant's internal reference ID for the order.
                "shippingCost": 200  // Shipping cost for the order (in minor currency units).
            },
            "taxInfo": {  // Tax information.
                "orderTaxAmount": 100  // Total tax amount for the order (in minor currency units).
            }
        }
    };
}

function _buildCaptureRequest(connectorTransactionId: string): types.IPaymentServiceCaptureRequest {
    return {
        "merchantCaptureId": "probe_capture_001",  // Identification.
        "connectorTransactionId": connectorTransactionId,
        "amountToCapture": {  // Capture Details.
            "minorAmount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            "currency": Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        },
        "connectorFeatureData": {"value": "{\"authorize_id\":\"probe_auth_001\",\"capture_id\":\"probe_cap_001\",\"psync_flow\":\"CAPTURE\",\"order_id\":\"probe_order_001\"}"},  // Connector-specific metadata for the transaction.
        "captureMethod": CaptureMethod.MANUAL,  // Capture Settings.
        "state": {  // State Information.
            "accessToken": {  // Access token obtained from connector.
                "token": {"value": "probe_access_token"},  // The token string.
                "expiresInSeconds": 3600,  // Expiration timestamp (seconds since epoch).
                "tokenType": "Bearer"  // Token type (e.g., "Bearer", "Basic").
            }
        }
    };
}

function _buildCreateClientAuthenticationTokenRequest(): types.IMerchantAuthenticationServiceCreateClientAuthenticationTokenRequest {
    return {
        "merchantClientSessionId": "probe_sdk_session_001",  // Infrastructure.
        "connectorFeatureData": {"value": "{\"authorize_id\":\"probe_auth_001\",\"capture_id\":\"probe_cap_001\",\"psync_flow\":\"CAPTURE\",\"order_id\":\"probe_order_001\"}"},
        "payment": {
            "amount": {
                "minorAmount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
                "currency": Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
            }
        }
    };
}

function _buildCreateOrderRequest(): types.IPaymentServiceCreateOrderRequest {
    return {
        "merchantOrderId": "probe_order_001",  // Identification.
        "amount": {  // Amount Information.
            "minorAmount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            "currency": Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        },
        "connectorFeatureData": {"value": "{\"authorize_id\":\"probe_auth_001\",\"capture_id\":\"probe_cap_001\",\"psync_flow\":\"CAPTURE\",\"order_id\":\"probe_order_001\"}"},  // Connector-specific metadata for the transaction.
        "state": {  // State Information.
            "accessToken": {  // Access token obtained from connector.
                "token": {"value": "probe_access_token"},  // The token string.
                "expiresInSeconds": 3600,  // Expiration timestamp (seconds since epoch).
                "tokenType": "Bearer"  // Token type (e.g., "Bearer", "Basic").
            }
        }
    };
}

function _buildCreateServerAuthenticationTokenRequest(): types.IMerchantAuthenticationServiceCreateServerAuthenticationTokenRequest {
    return {
        "connectorFeatureData": {"value": "{\"authorize_id\":\"probe_auth_001\",\"capture_id\":\"probe_cap_001\",\"psync_flow\":\"CAPTURE\",\"order_id\":\"probe_order_001\"}"}  // Connector-specific metadata for the transaction.
    };
}

function _buildGetRequest(connectorTransactionId: string): types.IPaymentServiceGetRequest {
    return {
        "merchantTransactionId": "probe_merchant_txn_001",  // Identification.
        "connectorTransactionId": connectorTransactionId,
        "amount": {  // Amount Information.
            "minorAmount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            "currency": Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        },
        "state": {  // State Information.
            "accessToken": {  // Access token obtained from connector.
                "token": {"value": "probe_access_token"},  // The token string.
                "expiresInSeconds": 3600,  // Expiration timestamp (seconds since epoch).
                "tokenType": "Bearer"  // Token type (e.g., "Bearer", "Basic").
            }
        },
        "connectorFeatureData": {"value": "{\"authorize_id\":\"probe_auth_001\",\"capture_id\":\"probe_cap_001\",\"psync_flow\":\"CAPTURE\",\"order_id\":\"probe_order_001\"}"}  // Connector-specific metadata for the transaction.
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
            "body": new Uint8Array(Buffer.from("{\"event_type\":\"PAYMENT.CAPTURE.COMPLETED\",\"resource\":{\"id\":\"probe_capture_001\",\"status\":\"COMPLETED\",\"amount\":{\"value\":\"10.00\",\"currency_code\":\"USD\"}}}", "utf-8"))  // Body of the HTTP request.
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
            "body": new Uint8Array(Buffer.from("{\"event_type\":\"PAYMENT.CAPTURE.COMPLETED\",\"resource\":{\"id\":\"probe_capture_001\",\"status\":\"COMPLETED\",\"amount\":{\"value\":\"10.00\",\"currency_code\":\"USD\"}}}", "utf-8"))  // Body of the HTTP request.
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
        "connectorFeatureData": {"value": "{\"authorize_id\":\"probe_auth_001\",\"capture_id\":\"probe_cap_001\",\"psync_flow\":\"CAPTURE\",\"order_id\":\"probe_order_001\"}"},  // Connector-specific metadata for the transaction.
        "state": {  // State Information.
            "accessToken": {  // Access token obtained from connector.
                "token": {"value": "probe_access_token"},  // The token string.
                "expiresInSeconds": 3600,  // Expiration timestamp (seconds since epoch).
                "tokenType": "Bearer"  // Token type (e.g., "Bearer", "Basic").
            }
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
            }
        },
        "captureMethod": CaptureMethod.AUTOMATIC,
        "authType": AuthenticationType.NO_THREE_DS,
        "returnUrl": "https://example.com/return",
        "connectorFeatureData": {"value": "{\"authorize_id\":\"probe_auth_001\",\"capture_id\":\"probe_cap_001\",\"psync_flow\":\"CAPTURE\",\"order_id\":\"probe_order_001\"}"},
        "state": {
            "accessToken": {  // Access token obtained from connector.
                "token": {"value": "probe_access_token"},  // The token string.
                "expiresInSeconds": 3600,  // Expiration timestamp (seconds since epoch).
                "tokenType": "Bearer"  // Token type (e.g., "Bearer", "Basic").
            }
        }
    };
}

function _buildRecurringChargeRequest(): types.IRecurringPaymentServiceChargeRequest {
    return {
        "merchantChargeId": "probe_paypal_mit_charge",  // Reference ID for tracking.
        "connectorRecurringPaymentId": {  // Reference to existing mandate.
            "networkMandateId": {  // network_mandate_id sent by Issuer to connector, Used for PG agnostic mandate txns along with card data.
                "networkTransactionId": "123456789012345"
            }
        },
        "amount": {  // Amount Information.
            "minorAmount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            "currency": Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        },
        "paymentMethod": {  // Optional payment Method Information (for network transaction flows).
            "cardDetailsForNetworkTransactionId": {  // Card details with network transaction ID.
                "cardNumber": {"value": "4111111111111111"},
                "cardExpMonth": {"value": "03"},
                "cardExpYear": {"value": "2030"},
                "cardNetwork": CardNetwork.VISA,
                "cardHolderName": {"value": "John Doe"}
            }
        },
        "metadata": {"value": "{\"paypal_mit\":{\"version\":1,\"previous_network_transaction_reference\":{\"id\":\"123456789012345\",\"network\":\"VISA\",\"date\":\"1014\"}}}"},  // Additional metadata for the connector.
        "connectorFeatureData": {"value": "{\"authorize_id\":\"probe_auth_001\",\"capture_id\":\"probe_cap_001\",\"psync_flow\":\"CAPTURE\",\"order_id\":\"probe_order_001\"}"},  // Connector-specific metadata for the transaction.
        "returnUrl": "https://example.com/recurring-return",
        "address": {  // Address Information.
            "shippingAddress": {
                "firstName": {"value": "Jane"},  // Personal Information.
                "lastName": {"value": "Doe"},
                "line1": {"value": "12 Main Street"},  // Address Details.
                "city": {"value": "Seattle"},
                "state": {"value": "WA"},
                "zipCode": {"value": "98101"},
                "countryAlpha2Code": CountryAlpha2.US
            }
        },
        "connectorCustomerId": "cust_probe_123",
        "paymentMethodType": PaymentMethodType.PAY_PAL,
        "offSession": true,  // Behavioral Flags and Preferences.
        "state": {  // State Information.
            "accessToken": {  // Access token obtained from connector.
                "token": {"value": "probe_access_token"},  // The token string.
                "expiresInSeconds": 3600,  // Expiration timestamp (seconds since epoch).
                "tokenType": "Bearer"  // Token type (e.g., "Bearer", "Basic").
            }
        },
        "billingDescriptor": {  // Billing Descriptor.
            "statementDescriptor": "PROBE PAYPAL"  // Complete description that appears on customers' statements.
        },
        "l2L3Data": {  // Level 2 / Level 3 data for enhanced payment processing.
            "orderInfo": {  // Order-level information.
                "merchantOrderReferenceId": "probe_paypal_mit"  // Merchant's internal reference ID for the order.
            }
        }
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
        "reason": "customer_request",  // Reason for the refund.
        "connectorFeatureData": {"value": "{\"authorize_id\":\"probe_auth_001\",\"capture_id\":\"probe_cap_001\",\"psync_flow\":\"CAPTURE\",\"order_id\":\"probe_order_001\"}"},  // Connector-specific metadata for the transaction.
        "state": {  // State data for access token storage and.
            "accessToken": {  // Access token obtained from connector.
                "token": {"value": "probe_access_token"},  // The token string.
                "expiresInSeconds": 3600,  // Expiration timestamp (seconds since epoch).
                "tokenType": "Bearer"  // Token type (e.g., "Bearer", "Basic").
            }
        }
    };
}

function _buildRefundGetRequest(): types.IRefundServiceGetRequest {
    return {
        "merchantRefundId": "probe_refund_001",  // Identification.
        "connectorTransactionId": "probe_connector_txn_001",
        "refundId": "probe_refund_id_001",  // Deprecated.
        "state": {  // State Information.
            "accessToken": {  // Access token obtained from connector.
                "token": {"value": "probe_access_token"},  // The token string.
                "expiresInSeconds": 3600,  // Expiration timestamp (seconds since epoch).
                "tokenType": "Bearer"  // Token type (e.g., "Bearer", "Basic").
            }
        }
    };
}

function _buildSetupRecurringRequest(): types.IPaymentServiceSetupRecurringRequest {
    return {
        "merchantRecurringPaymentId": "probe_mandate_001",  // Identification.
        "amount": {  // Mandate Details.
            "minorAmount": 0,  // Amount in minor units (e.g., 1000 = $10.00).
            "currency": Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
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
        "connectorFeatureData": {"value": "{\"authorize_id\":\"probe_auth_001\",\"capture_id\":\"probe_cap_001\",\"psync_flow\":\"CAPTURE\",\"order_id\":\"probe_order_001\"}"},  // Connector-specific metadata for the transaction.
        "returnUrl": "https://example.com/mandate-return",  // URL to redirect after setup.
        "setupFutureUsage": FutureUsage.OFF_SESSION,  // Indicates future usage intention.
        "requestIncrementalAuthorization": false,  // Indicates if incremental authorization is requested.
        "customerAcceptance": {  // Details of customer acceptance.
            "acceptanceType": AcceptanceType.OFFLINE,  // Type of acceptance (e.g., online, offline).
            "acceptedAt": 0  // Timestamp when the acceptance was made (Unix timestamp, seconds since epoch).
        },
        "state": {  // State data for access token storage and.
            "accessToken": {  // Access token obtained from connector.
                "token": {"value": "probe_access_token"},  // The token string.
                "expiresInSeconds": 3600,  // Expiration timestamp (seconds since epoch).
                "tokenType": "Bearer"  // Token type (e.g., "Bearer", "Basic").
            }
        }
    };
}

function _buildVoidRequest(connectorTransactionId: string): types.IPaymentServiceVoidRequest {
    return {
        "merchantVoidId": "probe_void_001",  // Identification.
        "connectorTransactionId": connectorTransactionId,
        "connectorFeatureData": {"value": "{\"authorize_id\":\"probe_auth_001\",\"capture_id\":\"probe_cap_001\",\"psync_flow\":\"CAPTURE\",\"order_id\":\"probe_order_001\"}"},  // Connector-specific metadata for the transaction.
        "state": {  // State Information.
            "accessToken": {  // Access token obtained from connector.
                "token": {"value": "probe_access_token"},  // The token string.
                "expiresInSeconds": 3600,  // Expiration timestamp (seconds since epoch).
                "tokenType": "Bearer"  // Token type (e.g., "Bearer", "Basic").
            }
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

// Flow: PaymentMethodAuthenticationService.Authenticate
async function authenticate(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentMethodAuthenticationClient = new PaymentMethodAuthenticationClient(config);

    const authenticateResponse = await paymentMethodAuthenticationClient.authenticate(_buildAuthenticateRequest());

    return authenticateResponse;
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

    const captureResponse = await paymentClient.capture(_buildCaptureRequest('probe_auth_001'));

    return captureResponse;
}

// Flow: MerchantAuthenticationService.CreateClientAuthenticationToken
async function createClientAuthenticationToken(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const merchantAuthenticationClient = new MerchantAuthenticationClient(config);

    const createResponse = await merchantAuthenticationClient.createClientAuthenticationToken(_buildCreateClientAuthenticationTokenRequest());

    return createResponse;
}

// Flow: PaymentService.CreateOrder
async function createOrder(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const createResponse = await paymentClient.createOrder(_buildCreateOrderRequest());

    return createResponse;
}

// Flow: MerchantAuthenticationService.CreateServerAuthenticationToken
async function createServerAuthenticationToken(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const merchantAuthenticationClient = new MerchantAuthenticationClient(config);

    const createResponse = await merchantAuthenticationClient.createServerAuthenticationToken(_buildCreateServerAuthenticationTokenRequest());

    return createResponse;
}

// Flow: PaymentService.Get
async function get(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const getResponse = await paymentClient.get(_buildGetRequest('probe_cap_001'));

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

    const refundResponse = await paymentClient.refund(_buildRefundRequest('probe_cap_001'));

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

// Flow: PaymentService.Void
async function voidPayment(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const voidResponse = await paymentClient.void(_buildVoidRequest('probe_auth_001'));

    return voidResponse;
}


// Export all process* functions for the smoke test
export {
    processCheckoutAutocapture, processCheckoutCard, processRefund, processVoidPayment, processGetPayment, authenticate, authorize, capture, createClientAuthenticationToken, createOrder, createServerAuthenticationToken, get, handleEvent, parseEvent, postAuthenticate, proxyAuthorize, recurringCharge, refund, refundGet, setupRecurring, voidPayment, _buildAuthenticateRequest, _buildAuthorizeRequest, _buildCaptureRequest, _buildCreateClientAuthenticationTokenRequest, _buildCreateOrderRequest, _buildCreateServerAuthenticationTokenRequest, _buildGetRequest, _buildHandleEventRequest, _buildParseEventRequest, _buildPostAuthenticateRequest, _buildProxyAuthorizeRequest, _buildRecurringChargeRequest, _buildRefundRequest, _buildRefundGetRequest, _buildSetupRecurringRequest, _buildVoidRequest
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
