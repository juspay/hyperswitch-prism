// This file is auto-generated. Do not edit manually.
// Replace placeholder credentials with real values.
// Regenerate: python3 scripts/generate-connector-docs.py bamboraapac
//
// Bamboraapac — all integration scenarios and flows in one file.
// Run a scenario:  npx ts-node bamboraapac.ts checkout_card

import { PaymentClient, RecurringPaymentClient } from 'hyperswitch-prism';
import { types } from 'hyperswitch-prism';

const { ConnectorConfig, ConnectorSpecificConfig, SdkOptions, Environment } = types;

const _defaultConfig: types.IConnectorConfig = ConnectorConfig.create({
    options: SdkOptions.create({ environment: Environment.SANDBOX }),
    // connectorConfig: ConnectorSpecificConfig.create({
    //     bamboraapac: { apiKey: { value: 'YOUR_API_KEY' } }
    // }),
});


function _buildRecurringChargeRequest() {
    return {
        "connectorRecurringPaymentId": {
            "mandateIdType": {
                "connectorMandateId": {
                    "connectorMandateId": "probe-mandate-123"
                }
            }
        },
        "amount": {  // Amount Information
            "minorAmount": 1000,  // Amount in minor units (e.g., 1000 = $10.00)
            "currency": "USD"  // ISO 4217 currency code (e.g., "USD", "EUR")
        },
        "paymentMethod": {  // Optional payment Method Information (for network transaction flows)
            "token": {  // Payment tokens
                "token": {"value": "probe_pm_token"}  // The token string representing a payment method.
            }
        },
        "returnUrl": "https://example.com/recurring-return",
        "connectorCustomerId": "cust_probe_123",
        "paymentMethodType": "PAY_PAL",
        "offSession": true  // Behavioral Flags and Preferences
    };
}


// ANCHOR: scenario_functions
// Card Payment (Authorize + Capture)
// Flow: authorize → capture
// Reserve funds with Authorize, then settle with a separate Capture call. Use for physical goods or delayed fulfillment where capture happens later.
async function processCheckoutCard(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig): Promise<unknown> {
    const paymentClient = new PaymentClient(config);

    // Step 1: Authorize — reserve funds on the payment method
    const authorizeResponse = await paymentClient.authorize({
        "merchantTransactionId": "probe_txn_001",
        "amount": {
            "minorAmount": 1000,
            "currency": "USD"
        },
        "paymentMethod": {
            "card": {
                "cardNumber": "4111111111111111",
                "cardExpMonth": "03",
                "cardExpYear": "2030",
                "cardCvc": "737",
                "cardHolderName": "John Doe"
            }
        },
        "captureMethod": "MANUAL",
        "address": {
            "billingAddress": {
            }
        },
        "authType": "NO_THREE_DS",
        "returnUrl": "https://example.com/return"
    });

    if (authorizeResponse.status === 'FAILED') {
        throw new Error(`Payment failed: ${authorizeResponse.error?.message}`);
    }
    if (authorizeResponse.status === 'PENDING') {
        // Awaiting async confirmation — handle via webhook
        return { status: 'pending', transactionId: authorizeResponse.connectorTransactionId };
    }

    // Step 2: Capture — settle the reserved funds
    const captureResponse = await paymentClient.capture({
        "merchantCaptureId": "probe_capture_001",
        "connectorTransactionId": authorizeResponse.connectorTransactionId,  // from authorize response
        "amountToCapture": {
            "minorAmount": 1000,
            "currency": "USD"
        }
    });

    if (captureResponse.status === 'FAILED') {
        throw new Error(`Capture failed: ${captureResponse.error?.message}`);
    }

    return { status: captureResponse.status, transactionId: authorizeResponse.connectorTransactionId, error: authorizeResponse.error };
}

// Card Payment (Automatic Capture)
// Flow: authorize
// Authorize and capture in one call using `capture_method=AUTOMATIC`. Use for digital goods or immediate fulfillment.
async function processCheckoutAutocapture(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig): Promise<unknown> {
    const paymentClient = new PaymentClient(config);

    // Step 1: Authorize — reserve funds on the payment method
    const authorizeResponse = await paymentClient.authorize({
        "merchantTransactionId": "probe_txn_001",
        "amount": {
            "minorAmount": 1000,
            "currency": "USD"
        },
        "paymentMethod": {
            "card": {
                "cardNumber": "4111111111111111",
                "cardExpMonth": "03",
                "cardExpYear": "2030",
                "cardCvc": "737",
                "cardHolderName": "John Doe"
            }
        },
        "captureMethod": "AUTOMATIC",
        "address": {
            "billingAddress": {
            }
        },
        "authType": "NO_THREE_DS",
        "returnUrl": "https://example.com/return"
    });

    if (authorizeResponse.status === 'FAILED') {
        throw new Error(`Payment failed: ${authorizeResponse.error?.message}`);
    }
    if (authorizeResponse.status === 'PENDING') {
        // Awaiting async confirmation — handle via webhook
        return { status: 'pending', transactionId: authorizeResponse.connectorTransactionId };
    }

    return { status: authorizeResponse.status, transactionId: authorizeResponse.connectorTransactionId, error: authorizeResponse.error };
}

// Refund a Payment
// Flow: authorize → refund
// Authorize with automatic capture, then refund the captured amount. `connector_transaction_id` from the Authorize response is reused for the Refund call.
async function processRefund(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig): Promise<unknown> {
    const paymentClient = new PaymentClient(config);

    // Step 1: Authorize — reserve funds on the payment method
    const authorizeResponse = await paymentClient.authorize({
        "merchantTransactionId": "probe_txn_001",
        "amount": {
            "minorAmount": 1000,
            "currency": "USD"
        },
        "paymentMethod": {
            "card": {
                "cardNumber": "4111111111111111",
                "cardExpMonth": "03",
                "cardExpYear": "2030",
                "cardCvc": "737",
                "cardHolderName": "John Doe"
            }
        },
        "captureMethod": "AUTOMATIC",
        "address": {
            "billingAddress": {
            }
        },
        "authType": "NO_THREE_DS",
        "returnUrl": "https://example.com/return"
    });

    if (authorizeResponse.status === 'FAILED') {
        throw new Error(`Payment failed: ${authorizeResponse.error?.message}`);
    }
    if (authorizeResponse.status === 'PENDING') {
        // Awaiting async confirmation — handle via webhook
        return { status: 'pending', transactionId: authorizeResponse.connectorTransactionId };
    }

    // Step 2: Refund — return funds to the customer
    const refundResponse = await paymentClient.refund({
        "merchantRefundId": "probe_refund_001",
        "connectorTransactionId": authorizeResponse.connectorTransactionId,  // from authorize response
        "paymentAmount": 1000,
        "refundAmount": {
            "minorAmount": 1000,
            "currency": "USD"
        },
        "reason": "customer_request"
    });

    if (refundResponse.status === 'FAILED') {
        throw new Error(`Refund failed: ${refundResponse.error?.message}`);
    }

    return { status: refundResponse.status, error: refundResponse.error };
}

// Recurring / Mandate Payments
// Flow: setup_recurring → recurring_charge
// Store a payment mandate with SetupRecurring, then charge it repeatedly with RecurringPaymentService.Charge without requiring customer action.
async function processRecurring(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig): Promise<unknown> {
    const paymentClient = new PaymentClient(config);
    const recurringPaymentClient = new RecurringPaymentClient(config);

    // Step 1: Setup Recurring — store the payment mandate
    const setupResponse = await paymentClient.setupRecurring({
        "merchantRecurringPaymentId": "probe_mandate_001",
        "amount": {
            "minorAmount": 0,
            "currency": "USD"
        },
        "paymentMethod": {
            "card": {
                "cardNumber": "4111111111111111",
                "cardExpMonth": "03",
                "cardExpYear": "2030",
                "cardCvc": "737",
                "cardHolderName": "John Doe"
            }
        },
        "address": {
            "billingAddress": {
            }
        },
        "authType": "NO_THREE_DS",
        "enrolledFor3Ds": false,
        "returnUrl": "https://example.com/mandate-return",
        "setupFutureUsage": "OFF_SESSION",
        "requestIncrementalAuthorization": false,
        "customerAcceptance": {
            "acceptanceType": "OFFLINE",
            "acceptedAt": 0
        }
    });

    if (setupResponse.status === 'FAILED') {
        throw new Error(`Recurring setup failed: ${setupResponse.error?.message}`);
    }

    // Step 2: Recurring Charge — charge against the stored mandate
    const recurringResponse = await recurringPaymentClient.charge({
        "connectorRecurringPaymentId": { connectorMandateId: { connectorMandateId: setupResponse.mandateReference?.connectorMandateId?.connectorMandateId } },  // from setup response
        "amount": {  // Amount Information
            "minorAmount": 1000,  // Amount in minor units (e.g., 1000 = $10.00)
            "currency": "USD"  // ISO 4217 currency code (e.g., "USD", "EUR")
        },
        "returnUrl": "https://example.com/recurring-return",
        "connectorCustomerId": "cust_probe_123",
        "offSession": true  // Behavioral Flags and Preferences
    });

    if (recurringResponse.status === 'FAILED') {
        throw new Error(`Recurring Charge failed: ${recurringResponse.error?.message}`);
    }

    return { status: recurringResponse.status, transactionId: recurringResponse.connectorTransactionId ?? '', error: recurringResponse.error };
}

// Get Payment Status
// Flow: authorize → get
// Authorize a payment, then poll the connector for its current status using Get. Use this to sync payment state when webhooks are unavailable or delayed.
async function processGetPayment(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig): Promise<unknown> {
    const paymentClient = new PaymentClient(config);

    // Step 1: Authorize — reserve funds on the payment method
    const authorizeResponse = await paymentClient.authorize({
        "merchantTransactionId": "probe_txn_001",
        "amount": {
            "minorAmount": 1000,
            "currency": "USD"
        },
        "paymentMethod": {
            "card": {
                "cardNumber": "4111111111111111",
                "cardExpMonth": "03",
                "cardExpYear": "2030",
                "cardCvc": "737",
                "cardHolderName": "John Doe"
            }
        },
        "captureMethod": "MANUAL",
        "address": {
            "billingAddress": {
            }
        },
        "authType": "NO_THREE_DS",
        "returnUrl": "https://example.com/return"
    });

    if (authorizeResponse.status === 'FAILED') {
        throw new Error(`Payment failed: ${authorizeResponse.error?.message}`);
    }
    if (authorizeResponse.status === 'PENDING') {
        // Awaiting async confirmation — handle via webhook
        return { status: 'pending', transactionId: authorizeResponse.connectorTransactionId };
    }

    // Step 2: Get — retrieve current payment status from the connector
    const getResponse = await paymentClient.get({
        "merchantTransactionId": "probe_merchant_txn_001",
        "connectorTransactionId": authorizeResponse.connectorTransactionId,  // from authorize response
        "amount": {
            "minorAmount": 1000,
            "currency": "USD"
        }
    });

    return { status: getResponse.status, transactionId: getResponse.connectorTransactionId, error: getResponse.error };
}

// Flow: PaymentService.authorize (Card)
async function authorize(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig): Promise<unknown> {
    // Step 1: Authorize — reserve funds on the payment method
    const authorizeResponse = await paymentClient.authorize({
        "merchantTransactionId": "probe_txn_001",
        "amount": {
            "minorAmount": 1000,
            "currency": "USD"
        },
        "paymentMethod": {
            "card": {
                "cardNumber": "4111111111111111",
                "cardExpMonth": "03",
                "cardExpYear": "2030",
                "cardCvc": "737",
                "cardHolderName": "John Doe"
            }
        },
        "captureMethod": "AUTOMATIC",
        "address": {
            "billingAddress": {
            }
        },
        "authType": "NO_THREE_DS",
        "returnUrl": "https://example.com/return"
    });

    if (authorizeResponse.status === 'FAILED') {
        throw new Error(`Payment failed: ${authorizeResponse.error?.message}`);
    }
    if (authorizeResponse.status === 'PENDING') {
        // Awaiting async confirmation — handle via webhook
        return { status: 'pending', transactionId: authorizeResponse.connectorTransactionId };
    }

    return { status: authorizeResponse.status, transactionId: authorizeResponse.connectorTransactionId };
}

// Flow: PaymentService.capture
async function capture(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig): Promise<unknown> {
    // Step 1: Capture — settle the reserved funds
    const captureResponse = await paymentClient.capture({
        "merchantCaptureId": "probe_capture_001",
        "connectorTransactionId": "probe_connector_txn_001",
        "amountToCapture": {
            "minorAmount": 1000,
            "currency": "USD"
        }
    });

    if (captureResponse.status === 'FAILED') {
        throw new Error(`Capture failed: ${captureResponse.error?.message}`);
    }

    return { status: captureResponse.status };
}

// Flow: PaymentService.get
async function get(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig): Promise<unknown> {
    // Step 1: Get — retrieve current payment status from the connector
    const getResponse = await paymentClient.get({
        "merchantTransactionId": "probe_merchant_txn_001",
        "connectorTransactionId": "probe_connector_txn_001",
        "amount": {
            "minorAmount": 1000,
            "currency": "USD"
        }
    });

    return { status: getResponse.status };
}

// Flow: RecurringPaymentService.Charge
async function recurringCharge(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig): Promise<unknown> {
    const recurringPaymentClient = new RecurringPaymentClient(config);

    const recurringResponse = await recurringPaymentClient.charge(_buildRecurringChargeRequest());

    return { status: recurringResponse.status };
}

// Flow: PaymentService.refund
async function refund(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig): Promise<unknown> {
    // Step 1: Refund — return funds to the customer
    const refundResponse = await paymentClient.refund({
        "merchantRefundId": "probe_refund_001",
        "connectorTransactionId": "probe_connector_txn_001",
        "paymentAmount": 1000,
        "refundAmount": {
            "minorAmount": 1000,
            "currency": "USD"
        },
        "reason": "customer_request"
    });

    if (refundResponse.status === 'FAILED') {
        throw new Error(`Refund failed: ${refundResponse.error?.message}`);
    }

    return { status: refundResponse.status };
}

// Flow: PaymentService.setup_recurring
async function setupRecurring(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig): Promise<unknown> {
    // Step 1: Setup Recurring — store the payment mandate
    const setupResponse = await paymentClient.setupRecurring({
        "merchantRecurringPaymentId": "probe_mandate_001",
        "amount": {
            "minorAmount": 0,
            "currency": "USD"
        },
        "paymentMethod": {
            "card": {
                "cardNumber": "4111111111111111",
                "cardExpMonth": "03",
                "cardExpYear": "2030",
                "cardCvc": "737",
                "cardHolderName": "John Doe"
            }
        },
        "address": {
            "billingAddress": {
            }
        },
        "authType": "NO_THREE_DS",
        "enrolledFor3Ds": false,
        "returnUrl": "https://example.com/mandate-return",
        "setupFutureUsage": "OFF_SESSION",
        "requestIncrementalAuthorization": false,
        "customerAcceptance": {
            "acceptanceType": "OFFLINE",
            "acceptedAt": 0
        }
    });

    if (setupResponse.status === 'FAILED') {
        throw new Error(`Recurring setup failed: ${setupResponse.error?.message}`);
    }

    return { status: setupResponse.status, mandateId: setupResponse.connectorTransactionId };
}


export { processCheckoutCard, processCheckoutAutocapture, processRefund, processRecurring, processGetPayment, authorize, capture, get, recurringCharge, refund, setupRecurring, _buildRecurringChargeRequest };

const _scenarioMap: Record<string, (id: string) => Promise<unknown>> = {
    processCheckoutCard,
    processCheckoutAutocapture,
    processRefund,
    processRecurring,
    processGetPayment,
};

if (require.main === module) {
    const scenario = process.argv[2] || 'checkout_card';
    const key = 'process' + scenario.replace(/_([a-z])/g, (_: string, l: string) => l.toUpperCase()).replace(/^(.)/, (c: string) => c.toUpperCase());
    const fn = _scenarioMap[key];
    if (!fn) {
        const available = Object.keys(_scenarioMap).map(k =>
            k.replace(/^process/, '').replace(/([A-Z])/g, '_$1').toLowerCase().replace(/^_/, '')
        );
        console.error(`Unknown scenario: ${scenario}. Available: ${available.join(', ')}`);
        process.exit(1);
    }
    fn('order_001').catch(console.error);
}
