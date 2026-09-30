// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py paysafe
//
// Paysafe — all integration scenarios and flows in one file.
// Run a scenario:  npx tsx paysafe.ts checkout_autocapture

import { PaymentMethodAuthenticationClient, PaymentClient, CustomerClient, RefundClient, types } from 'hyperswitch-prism';
const { Environment, CaptureMethod, Currency } = types;
export const SUPPORTED_FLOWS = ["authenticate", "capture", "customer_create", "get", "pre_authenticate", "refund", "refund_get", "token_authorize", "void"];

const _defaultConfig: types.IConnectorConfig = {
    options: {
        environment: Environment.SANDBOX,
    },
    connectorConfig: {
        paysafe: {
            username: { value: 'YOUR_USERNAME' },
            password: { value: 'YOUR_PASSWORD' },
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
            "billingAddress": {
            }
        },
        "returnUrl": "https://example.com/3ds-return"  // URLs for Redirection.
    };
}

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

function _buildCustomerCreateRequest(): types.ICustomerServiceCreateRequest {
    return {
        "merchantCustomerId": "cust_probe_123",  // Identification.
        "customerName": "John Doe",  // Name of the customer.
        "email": {"value": "test@example.com"},  // Email address of the customer.
        "phoneNumber": {"value": "4155552671"}  // Phone number of the customer.
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
        "connectorOrderReferenceId": "probe_order_ref_001"  // Connector Reference Id.
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

function _buildVoidRequest(connectorTransactionId: string): types.IPaymentServiceVoidRequest {
    return {
        "merchantVoidId": "probe_void_001",  // Identification.
        "connectorTransactionId": connectorTransactionId,
        "amount": {  // Amount Information.
            "minorAmount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            "currency": Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        }
    };
}


// ANCHOR: scenario_functions
// Flow: PaymentMethodAuthenticationService.Authenticate
async function authenticate(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentMethodAuthenticationClient = new PaymentMethodAuthenticationClient(config);

    const authenticateResponse = await paymentMethodAuthenticationClient.authenticate(_buildAuthenticateRequest());

    return authenticateResponse;
}

// Flow: PaymentService.Capture
async function capture(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const captureResponse = await paymentClient.capture(_buildCaptureRequest('probe_connector_txn_001'));

    return captureResponse;
}

// Flow: CustomerService.Create
async function customerCreate(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const customerClient = new CustomerClient(config);

    const customerResponse = await customerClient.customerCreate(_buildCustomerCreateRequest());

    return customerResponse;
}

// Flow: PaymentService.Get
async function get(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const getResponse = await paymentClient.get(_buildGetRequest('probe_connector_txn_001'));

    return getResponse;
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

// Flow: PaymentService.TokenAuthorize
async function tokenAuthorize(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    const tokenResponse = await paymentClient.tokenAuthorize(_buildTokenAuthorizeRequest());

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
    authenticate, capture, customerCreate, get, preAuthenticate, refund, refundGet, tokenAuthorize, voidPayment, _buildAuthenticateRequest, _buildCaptureRequest, _buildCustomerCreateRequest, _buildGetRequest, _buildPreAuthenticateRequest, _buildRefundRequest, _buildRefundGetRequest, _buildTokenAuthorizeRequest, _buildVoidRequest
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
