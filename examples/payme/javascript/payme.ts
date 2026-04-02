// payme SDK Examples

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

function _buildvoidRequest(arg) {
    const payload = {
    "merchant_void_id": "probe_void_001",
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
