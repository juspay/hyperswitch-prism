// This file is auto-generated. Do not edit manually.
// Replace YOUR_API_KEY and placeholder values with real data.
// Regenerate: python3 scripts/generate-connector-docs.py jpmorgan
//
// Jpmorgan — all integration scenarios and flows in one file.
// Run a scenario:  npx tsx jpmorgan.ts checkout_autocapture

import { PaymentClient, types } from 'hyperswitch-prism';
const { Environment } = types;
export const SUPPORTED_FLOWS = ["authorize", "capture", "create_client_authentication_token", "create_server_authentication_token", "get", "proxy_authorize", "refund", "refund_get", "token_authorize", "void"];

const _defaultConfig: types.IConnectorConfig = {
    options: {
        environment: Environment.SANDBOX,
    },
    // connectorConfig: { jpmorgan: { apiKey: { value: 'YOUR_API_KEY' } } },
};


// ANCHOR: scenario_functions
// One-step Payment (Authorize + Capture)
// Simple payment that authorizes and captures in one call. Use for immediate charges.
async function processCheckoutAutocapture(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    // Step 1: Authorize — reserve funds on the payment method
    const authorizeResponse = await paymentClient.authorize({
        "merchantTransactionId": "probe_txn_001",
        "amount": {
        },
        "paymentMethod": {
        },
        "captureMethod": "AUTOMATIC",
        "address": {
        },
        "authType": "NO_THREE_DS",
        "returnUrl": "https://example.com/return",
        "state": {
        }
    });

    if (authorizeResponse.status === types.PaymentStatus.FAILURE) {
        throw new Error(`Payment failed: ${JSON.stringify(authorizeResponse.error)}`);
    }
    if (authorizeResponse.status === types.PaymentStatus.PENDING) {
        // Awaiting async confirmation — handle via webhook
        return { status: 'pending', transactionId: authorizeResponse.connectorTransactionId } as any;
    }

    return { status: authorizeResponse.status, transactionId: authorizeResponse.connectorTransactionId!, error: authorizeResponse.error } as any;
}

// Card Payment (Authorize + Capture)
// Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.
async function processCheckoutCard(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    // Step 1: Authorize — reserve funds on the payment method
    const authorizeResponse = await paymentClient.authorize({
        "merchantTransactionId": "probe_txn_001",
        "amount": {
        },
        "paymentMethod": {
        },
        "captureMethod": "MANUAL",
        "address": {
        },
        "authType": "NO_THREE_DS",
        "returnUrl": "https://example.com/return",
        "state": {
        }
    });

    if (authorizeResponse.status === types.PaymentStatus.FAILURE) {
        throw new Error(`Payment failed: ${JSON.stringify(authorizeResponse.error)}`);
    }
    if (authorizeResponse.status === types.PaymentStatus.PENDING) {
        // Awaiting async confirmation — handle via webhook
        return { status: 'pending', transactionId: authorizeResponse.connectorTransactionId } as any;
    }

    // Step 2: Capture — settle the reserved funds
    const captureResponse = await paymentClient.capture({
        "merchantCaptureId": "probe_capture_001",
        "connectorTransactionId": authorizeResponse.connectorTransactionId,  // from authorize response
        "amountToCapture": {
        },
        "state": {
        }
    });

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
    const authorizeResponse = await paymentClient.authorize({
        "merchantTransactionId": "probe_txn_001",
        "amount": {
        },
        "paymentMethod": {
        },
        "captureMethod": "AUTOMATIC",
        "address": {
        },
        "authType": "NO_THREE_DS",
        "returnUrl": "https://example.com/return",
        "state": {
        }
    });

    if (authorizeResponse.status === types.PaymentStatus.FAILURE) {
        throw new Error(`Payment failed: ${JSON.stringify(authorizeResponse.error)}`);
    }
    if (authorizeResponse.status === types.PaymentStatus.PENDING) {
        // Awaiting async confirmation — handle via webhook
        return { status: 'pending', transactionId: authorizeResponse.connectorTransactionId } as any;
    }

    // Step 2: Refund — return funds to the customer
    const refundResponse = await paymentClient.refund({
        "merchantRefundId": "probe_refund_001",
        "connectorTransactionId": authorizeResponse.connectorTransactionId,  // from authorize response
        "paymentAmount": 1000,
        "refundAmount": {
        },
        "reason": "customer_request",
        "state": {
        }
    });

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
    const authorizeResponse = await paymentClient.authorize({
        "merchantTransactionId": "probe_txn_001",
        "amount": {
        },
        "paymentMethod": {
        },
        "captureMethod": "MANUAL",
        "address": {
        },
        "authType": "NO_THREE_DS",
        "returnUrl": "https://example.com/return",
        "state": {
        }
    });

    if (authorizeResponse.status === types.PaymentStatus.FAILURE) {
        throw new Error(`Payment failed: ${JSON.stringify(authorizeResponse.error)}`);
    }
    if (authorizeResponse.status === types.PaymentStatus.PENDING) {
        // Awaiting async confirmation — handle via webhook
        return { status: 'pending', transactionId: authorizeResponse.connectorTransactionId } as any;
    }

    // Step 2: Void — release reserved funds (cancel authorization)
    const voidResponse = await paymentClient.void({
        "merchantVoidId": "probe_void_001",
        "connectorTransactionId": authorizeResponse.connectorTransactionId,  // from authorize response
        "state": {
        }
    });

    return { status: voidResponse.status, transactionId: authorizeResponse.connectorTransactionId!, error: voidResponse.error } as any;
}

// Get Payment Status
// Retrieve current payment status from the connector.
async function processGetPayment(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    const paymentClient = new PaymentClient(config);

    // Step 1: Authorize — reserve funds on the payment method
    const authorizeResponse = await paymentClient.authorize({
        "merchantTransactionId": "probe_txn_001",
        "amount": {
        },
        "paymentMethod": {
        },
        "captureMethod": "MANUAL",
        "address": {
        },
        "authType": "NO_THREE_DS",
        "returnUrl": "https://example.com/return",
        "state": {
        }
    });

    if (authorizeResponse.status === types.PaymentStatus.FAILURE) {
        throw new Error(`Payment failed: ${JSON.stringify(authorizeResponse.error)}`);
    }
    if (authorizeResponse.status === types.PaymentStatus.PENDING) {
        // Awaiting async confirmation — handle via webhook
        return { status: 'pending', transactionId: authorizeResponse.connectorTransactionId } as any;
    }

    // Step 2: Get — retrieve current payment status from the connector
    const getResponse = await paymentClient.get({
        "merchantTransactionId": "probe_merchant_txn_001",
        "connectorTransactionId": authorizeResponse.connectorTransactionId,  // from authorize response
        "amount": {
        },
        "state": {
        }
    });

    return { status: getResponse.status, transactionId: getResponse.connectorTransactionId!, error: getResponse.error } as any;
}

// Flow: PaymentService.authorize (Card)
async function authorize(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    // Step 1: Authorize — reserve funds on the payment method
    const authorizeResponse = await paymentClient.authorize({
        "merchantTransactionId": "probe_txn_001",
        "amount": {
        },
        "paymentMethod": {
        },
        "captureMethod": "AUTOMATIC",
        "address": {
        },
        "authType": "NO_THREE_DS",
        "returnUrl": "https://example.com/return",
        "state": {
        }
    });

    if (authorizeResponse.status === types.PaymentStatus.FAILURE) {
        throw new Error(`Payment failed: ${JSON.stringify(authorizeResponse.error)}`);
    }
    if (authorizeResponse.status === types.PaymentStatus.PENDING) {
        // Awaiting async confirmation — handle via webhook
        return { status: 'pending', transactionId: authorizeResponse.connectorTransactionId } as any;
    }

    return authorizeResponse;
}

// Flow: PaymentService.capture
async function capture(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    // Step 1: Capture — settle the reserved funds
    const captureResponse = await paymentClient.capture({
        "merchantCaptureId": "probe_capture_001",
        "connectorTransactionId": "probe_connector_txn_001",
        "amountToCapture": {
        },
        "state": {
        }
    });

    if (captureResponse.status === types.PaymentStatus.FAILURE) {
        throw new Error(`Capture failed: ${JSON.stringify(captureResponse.error)}`);
    }

    return captureResponse;
}

// Flow: PaymentService.create_client_authentication_token
async function createClientAuthenticationToken(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    // Step 1: create_client_authentication_token
    const createResponse = await paymentClient.createClientAuthenticationToken({
        "merchantClientSessionId": "probe_sdk_session_001",
        "domainContext": {
        }
    });

    return createResponse;
}

// Flow: PaymentService.create_server_authentication_token
async function createServerAuthenticationToken(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    // Step 1: create_server_authentication_token
    const createResponse = await paymentClient.createServerAuthenticationToken({
        // No required fields
    });

    return createResponse;
}

// Flow: PaymentService.get
async function get(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    // Step 1: Get — retrieve current payment status from the connector
    const getResponse = await paymentClient.get({
        "merchantTransactionId": "probe_merchant_txn_001",
        "connectorTransactionId": "probe_connector_txn_001",
        "amount": {
        },
        "state": {
        }
    });

    return getResponse;
}

// Flow: PaymentService.proxy_authorize
async function proxyAuthorize(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    // Step 1: proxy_authorize
    const proxyResponse = await paymentClient.proxyAuthorize({
        "merchantTransactionId": "probe_proxy_txn_001",
        "amount": {
        },
        "cardProxy": {
        },
        "address": {
        },
        "captureMethod": "AUTOMATIC",
        "authType": "NO_THREE_DS",
        "returnUrl": "https://example.com/return",
        "state": {
        }
    });

    return proxyResponse;
}

// Flow: PaymentService.refund
async function refund(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    // Step 1: Refund — return funds to the customer
    const refundResponse = await paymentClient.refund({
        "merchantRefundId": "probe_refund_001",
        "connectorTransactionId": "probe_connector_txn_001",
        "paymentAmount": 1000,
        "refundAmount": {
        },
        "reason": "customer_request",
        "state": {
        }
    });

    if (refundResponse.status === types.RefundStatus.REFUND_FAILURE) {
        throw new Error(`Refund failed: ${JSON.stringify(refundResponse.error)}`);
    }

    return refundResponse;
}

// Flow: PaymentService.refund_get
async function refundGet(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    // Step 1: refund_get
    const refundResponse = await paymentClient.refundGet({
        "merchantRefundId": "probe_refund_001",
        "connectorTransactionId": "probe_connector_txn_001",
        "refundId": "probe_refund_id_001",
        "state": {
        }
    });

    return refundResponse;
}

// Flow: PaymentService.token_authorize
async function tokenAuthorize(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    // Step 1: token_authorize
    const tokenResponse = await paymentClient.tokenAuthorize({
        "merchantTransactionId": "probe_tokenized_txn_001",
        "amount": {
        },
        "connectorToken": "pm_1AbcXyzStripeTestToken",
        "address": {
        },
        "captureMethod": "AUTOMATIC",
        "returnUrl": "https://example.com/return",
        "state": {
        }
    });

    return tokenResponse;
}

// Flow: PaymentService.void
async function voidPayment(merchantTransactionId: string, config: types.IConnectorConfig = _defaultConfig) {
    // Step 1: Void — release reserved funds (cancel authorization)
    const voidResponse = await paymentClient.void({
        "merchantVoidId": "probe_void_001",
        "connectorTransactionId": "probe_connector_txn_001",
        "state": {
        }
    });

    return voidResponse;
}


// Export all process* functions for the smoke test
export {
    processCheckoutAutocapture, processCheckoutCard, processRefund, processVoidPayment, processGetPayment, authorize, capture, createClientAuthenticationToken, createServerAuthenticationToken, get, proxyAuthorize, refund, refundGet, tokenAuthorize, voidPayment
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
