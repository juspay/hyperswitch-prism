// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py ppro
//
// Ppro — all integration scenarios and flows in one file.
// Run a scenario:  npx tsx ppro.ts checkout_autocapture

import { PaymentClient, EventClient, RecurringPaymentClient, RefundClient, types } from 'hyperswitch-prism';
const { ConnectorConfig, ConnectorSpecificConfig, SdkOptions, Environment, Currency, PaymentMethodType } = types;

const _defaultConfig: ConnectorConfig = {
    options: {
        environment: Environment.SANDBOX,
    },
};
// Standalone credentials (field names depend on connector auth type):
// _defaultConfig.connectorConfig = {
//     ppro: { apiKey: { value: 'YOUR_API_KEY' } }
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

function _buildGetRequest(connectorTransactionId: string): PaymentServiceGetRequest {
    return {
        "merchantTransactionId": "probe_merchant_txn_001",  // Identification.
        "connectorTransactionId": connectorTransactionId,
        "amount": {  // Amount Information.
            "minorAmount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            "currency": Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
        }
    };
}

function _buildHandleEventRequest(): EventServiceHandleRequest {
    return {
    };
}

function _buildRecurringChargeRequest(): RecurringPaymentServiceChargeRequest {
    return {
        "connectorRecurringPaymentId": {  // Reference to existing mandate.
            "connectorMandateId": {  // mandate_id sent by the connector.
                "connectorMandateId": "probe-mandate-123"
            }
        },
        "amount": {  // Amount Information.
            "minorAmount": 1000,  // Amount in minor units (e.g., 1000 = $10.00).
            "currency": Currency.USD  // ISO 4217 currency code (e.g., "USD", "EUR").
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

function _buildVerifyRedirectRequest(): PaymentServiceVerifyRedirectResponseRequest {
    return {
    };
}

function _buildVoidRequest(connectorTransactionId: string): PaymentServiceVoidRequest {
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
// Flow: PaymentService.Capture
async function capture(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<PaymentServiceCaptureResponse> {
    const paymentClient = new PaymentClient(config);

    const captureResponse = await paymentClient.capture(_buildCaptureRequest('probe_connector_txn_001'));

    return { status: captureResponse.status };
}

// Flow: PaymentService.Get
async function get(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<PaymentServiceGetResponse> {
    const paymentClient = new PaymentClient(config);

    const getResponse = await paymentClient.get(_buildGetRequest('probe_connector_txn_001'));

    return { status: getResponse.status };
}

// Flow: EventService.HandleEvent
async function handleEvent(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<EventServiceHandleResponse> {
    const eventClient = new EventClient(config);

    const handleResponse = await eventClient.handleEvent(_buildHandleEventRequest());

    return { status: handleResponse.status };
}

// Flow: RecurringPaymentService.Charge
async function recurringCharge(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<RecurringPaymentServiceChargeResponse> {
    const recurringPaymentClient = new RecurringPaymentClient(config);

    const recurringResponse = await recurringPaymentClient.charge(_buildRecurringChargeRequest());

    return { status: recurringResponse.status };
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

// Flow: PaymentService.VerifyRedirectResponse
async function verifyRedirect(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<PaymentServiceVerifyRedirectResponseResponse> {
    const paymentClient = new PaymentClient(config);

    const verifyResponse = await paymentClient.verifyRedirect(_buildVerifyRedirectRequest());

    return { status: verifyResponse.status };
}

// Flow: PaymentService.Void
async function voidPayment(merchantTransactionId: string, config: ConnectorConfig = _defaultConfig): Promise<PaymentServiceVoidResponse> {
    const paymentClient = new PaymentClient(config);

    const voidResponse = await paymentClient.void(_buildVoidRequest('probe_connector_txn_001'));

    return { status: voidResponse.status };
}


// Export all process* functions for the smoke test
export {
    capture, get, handleEvent, recurringCharge, refund, refundGet, verifyRedirect, voidPayment, _buildCaptureRequest, _buildGetRequest, _buildHandleEventRequest, _buildRecurringChargeRequest, _buildRefundRequest, _buildRefundGetRequest, _buildVerifyRedirectRequest, _buildVoidRequest
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
