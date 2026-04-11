// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py stax
//
// Stax — all integration scenarios and flows in one file.
// Run a scenario:  npx tsx stax.ts checkout_autocapture

import { PaymentClient, CustomerClient, FraudClient, RefundClient, PaymentMethodClient, types } from 'hyperswitch-prism';
const { ConnectorConfig, ConnectorSpecificConfig, SdkOptions, Environment, CaptureMethod, Currency } = types;

const _defaultConfig: ConnectorConfig = {
    options: {
        environment: Environment.SANDBOX,
    },
};
// Standalone credentials (field names depend on connector auth type):
// _defaultConfig.connectorConfig = {
//     stax: { apiKey: { value: 'YOUR_API_KEY' } }
// };


function _buildCaptureRequest(connectorTransactionId: string): PaymentServiceCaptureRequest {
    return {
        "merchantCaptureId": "probe_capture_001",  // Identification.
        "connectorTransactionId": connectorTransactionId,
        "amountToCapture": {  // Capture Details.
            "minorAmount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            "currency": Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        }
    };
}

function _buildCreateCustomerRequest(): CustomerServiceCreateRequest {
    return {
        "merchantCustomerId": "cust_probe_123",  // Identification.
        "customerName": "John Doe",  // Name of the customer.
        "email": {"value": "test@example.com"},  // Email address of the customer.
        "phoneNumber": "4155552671"  // Phone number of the customer.
    };
}

function _buildGetRequest(connectorTransactionId): FraudServiceGetRequest {
    return {
        "merchantTransactionId": "probe_merchant_txn_001",
        "connectorTransactionId": connectorTransactionId,
        "amount": {
            "minorAmount": 1000,
            "currency": "USD"
        }
    };
}

function _buildRefundRequest(connectorTransactionId: string): PaymentServiceRefundRequest {
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

function _buildRefundGetRequest(): RefundServiceGetRequest {
    return {
        "merchantRefundId": "probe_refund_001",  // Identification.
        "connectorTransactionId": "probe_connector_txn_001",
        "refundId": "probe_refund_id_001"
    };
}

function _buildTokenAuthorizeRequest(): PaymentServiceTokenAuthorizeRequest {
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

function _buildTokenizeRequest(): PaymentMethodServiceTokenizeRequest {
    return {
        "amount": {  // Payment Information.
            "minorAmount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
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
        "customer": {  // Customer Information.
            "id": "cust_probe_123"  // Internal customer ID.
        },
        "address": {  // Address Information.
            "billingAddress": {
            }
        }
    };
}

function _buildVoidRequest(connectorTransactionId: string): PaymentServiceVoidRequest {
    return {
        "merchantVoidId": "probe_void_001",  // Identification.
        "connectorTransactionId": connectorTransactionId
    };
}


// ANCHOR: scenario_functions
// Flow: PaymentService.Capture
async function capture(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<PaymentServiceCaptureResponse> {
    const paymentClient = new PaymentClient(config);

    const captureResponse = await paymentClient.capture(_buildCaptureRequest('probe_connector_txn_001'));

    return { status: captureResponse.status };
}

// Flow: CustomerService.Create
async function createCustomer(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<CustomerServiceCreateResponse> {
    const customerClient = new CustomerClient(config);

    const createResponse = await customerClient.create(_buildCreateCustomerRequest());

    return { status: createResponse.status };
}

// Flow: FraudService.Get
async function get(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<FraudServiceGetResponse> {
    const fraudClient = new FraudClient(config);

    const getResponse = await fraudClient.get(_buildGetRequest('probe_connector_txn_001'));

    return { status: getResponse.status };
}

// Flow: PaymentService.Refund
async function refund(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<RefundResponse> {
    const paymentClient = new PaymentClient(config);

    const refundResponse = await paymentClient.refund(_buildRefundRequest('probe_connector_txn_001'));

    return { status: refundResponse.status };
}

// Flow: RefundService.Get
async function refundGet(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<RefundResponse> {
    const refundClient = new RefundClient(config);

    const refundResponse = await refundClient.refundGet(_buildRefundGetRequest());

    return { status: refundResponse.status };
}

// Flow: PaymentService.TokenAuthorize
async function tokenAuthorize(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<PaymentServiceAuthorizeResponse> {
    const paymentClient = new PaymentClient(config);

    const tokenResponse = await paymentClient.tokenAuthorize(_buildTokenAuthorizeRequest());

    return { status: tokenResponse.status };
}

// Flow: PaymentMethodService.Tokenize
async function tokenize(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<PaymentMethodServiceTokenizeResponse> {
    const paymentMethodClient = new PaymentMethodClient(config);

    const tokenizeResponse = await paymentMethodClient.tokenize(_buildTokenizeRequest());

    return { status: tokenizeResponse.status };
}

// Flow: PaymentService.Void
async function voidPayment(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<PaymentServiceVoidResponse> {
    const paymentClient = new PaymentClient(config);

    const voidResponse = await paymentClient.void(_buildVoidRequest('probe_connector_txn_001'));

    return { status: voidResponse.status };
}


// Export all process* functions for the smoke test
export {
    capture, createCustomer, get, refund, refundGet, tokenAuthorize, tokenize, voidPayment, _buildCaptureRequest, _buildCreateCustomerRequest, _buildGetRequest, _buildRefundRequest, _buildRefundGetRequest, _buildTokenAuthorizeRequest, _buildTokenizeRequest, _buildVoidRequest
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
