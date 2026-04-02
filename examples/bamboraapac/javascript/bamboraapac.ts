// bamboraapac SDK Examples

function _buildauthorizeRequest(arg) {
    const payload = {
    "merchant_transaction_id": "probe_txn_001",
    "amount": {
        "minor_amount": 1000,
        "currency": "USD"
    },
    "payment_method": {
        "card": {
            "card_number": "4111111111111111",
            "card_exp_month": "03",
            "card_exp_year": "2030",
            "card_cvc": "737",
            "card_holder_name": "John Doe"
        }
    },
    "capture_method": "AUTOMATIC",
    "address": {
        "billing_address": {}
    },
    "auth_type": "NO_THREE_DS",
    "return_url": "https://example.com/return"
};
    if (arg) {
        if (arg === 'AUTOMATIC' || arg === 'MANUAL') {
            payload.capture_method = arg;
        } else if (typeof arg === 'string') {
            payload.connector_transaction_id = arg;
        }
    }
    return payload;
}

function _buildcaptureRequest(arg) {
    const payload = {
    "merchant_capture_id": "probe_capture_001",
    "connector_transaction_id": "probe_connector_txn_001",
    "amount_to_capture": {
        "minor_amount": 1000,
        "currency": "USD"
    }
};
    if (arg) {
        if (arg === 'AUTOMATIC' || arg === 'MANUAL') {
            payload.capture_method = arg;
        } else if (typeof arg === 'string') {
            payload.connector_transaction_id = arg;
        }
    }
    return payload;
}

function _buildrefundRequest(arg) {
    const payload = {
    "merchant_refund_id": "probe_refund_001",
    "connector_transaction_id": "probe_connector_txn_001",
    "payment_amount": 1000,
    "refund_amount": {
        "minor_amount": 1000,
        "currency": "USD"
    },
    "reason": "customer_request"
};
    if (arg) {
        if (arg === 'AUTOMATIC' || arg === 'MANUAL') {
            payload.capture_method = arg;
        } else if (typeof arg === 'string') {
            payload.connector_transaction_id = arg;
        }
    }
    return payload;
}

function _buildsetupRecurringRequest(arg) {
    const payload = {
    "merchant_recurring_payment_id": "probe_mandate_001",
    "amount": {
        "minor_amount": 0,
        "currency": "USD"
    },
    "payment_method": {
        "card": {
            "card_number": "4111111111111111",
            "card_exp_month": "03",
            "card_exp_year": "2030",
            "card_cvc": "737",
            "card_holder_name": "John Doe"
        }
    },
    "address": {
        "billing_address": {}
    },
    "auth_type": "NO_THREE_DS",
    "enrolled_for_3ds": false,
    "return_url": "https://example.com/mandate-return",
    "setup_future_usage": "OFF_SESSION",
    "request_incremental_authorization": false,
    "customer_acceptance": {
        "acceptance_type": "OFFLINE",
        "accepted_at": 0
    }
};
    if (arg) {
        if (arg === 'AUTOMATIC' || arg === 'MANUAL') {
            payload.capture_method = arg;
        } else if (typeof arg === 'string') {
            payload.connector_transaction_id = arg;
        }
    }
    return payload;
}

function _buildrecurringChargeRequest(arg) {
    const payload = {
    "connector_recurring_payment_id": {
        "mandate_id_type": {
            "connector_mandate_id": {
                "connector_mandate_id": "probe-mandate-123"
            }
        }
    },
    "amount": {
        "minor_amount": 1000,
        "currency": "USD"
    },
    "payment_method": {
        "token": {
            "token": "probe_pm_token"
        }
    },
    "return_url": "https://example.com/recurring-return",
    "connector_customer_id": "cust_probe_123",
    "payment_method_type": "PAY_PAL",
    "off_session": true
};
    if (arg) {
        if (arg === 'AUTOMATIC' || arg === 'MANUAL') {
            payload.capture_method = arg;
        } else if (typeof arg === 'string') {
            payload.connector_transaction_id = arg;
        }
    }
    return payload;
}

function _buildgetRequest(arg) {
    const payload = {
    "merchant_transaction_id": "probe_merchant_txn_001",
    "connector_transaction_id": "probe_connector_txn_001",
    "amount": {
        "minor_amount": 1000,
        "currency": "USD"
    }
};
    if (arg) {
        if (arg === 'AUTOMATIC' || arg === 'MANUAL') {
            payload.capture_method = arg;
        } else if (typeof arg === 'string') {
            payload.connector_transaction_id = arg;
        }
    }
    return payload;
}
