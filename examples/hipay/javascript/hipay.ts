// hipay SDK Examples

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

function _buildtokenizeRequest(arg) {
    const payload = {
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
    "address": {
        "billing_address": {}
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

function _buildvoidRequest(arg) {
    const payload = {
    "merchant_void_id": "probe_void_001",
    "connector_transaction_id": "probe_connector_txn_001"
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
