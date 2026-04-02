# volt SDK Examples

from google.protobuf.json_format import ParseDict
from payments.generated import payment_pb2

def _build_refund_request(arg=None):
    """Build RefundServiceGetRequest proto."""
    from google.protobuf.json_format import ParseDict
    from payments.generated import payment_pb2
    payload = {'merchant_refund_id': 'probe_refund_001', 'connector_transaction_id': 'probe_connector_txn_001', 'payment_amount': 1000, 'refund_amount': {'minor_amount': 1000, 'currency': 'USD'}, 'reason': 'customer_request', 'state': {'access_token': {'token': {'value': 'probe_access_token'}, 'expires_in_seconds': 3600, 'token_type': 'Bearer'}}}
    if arg:
        if arg in ('AUTOMATIC', 'MANUAL'):
            payload['capture_method'] = arg
        elif isinstance(arg, str) and 'connector_transaction_id' in payload:
            payload['connector_transaction_id'] = arg
    return ParseDict(payload, payment_pb2.RefundServiceGetRequest())

def _build_get_request(arg=None):
    """Build PaymentServiceGetRequest proto."""
    from google.protobuf.json_format import ParseDict
    from payments.generated import payment_pb2
    payload = {'merchant_transaction_id': 'probe_merchant_txn_001', 'connector_transaction_id': 'probe_connector_txn_001', 'amount': {'minor_amount': 1000, 'currency': 'USD'}, 'state': {'access_token': {'token': {'value': 'probe_access_token'}, 'expires_in_seconds': 3600, 'token_type': 'Bearer'}}}
    if arg:
        if arg in ('AUTOMATIC', 'MANUAL'):
            payload['capture_method'] = arg
        elif isinstance(arg, str) and 'connector_transaction_id' in payload:
            payload['connector_transaction_id'] = arg
    return ParseDict(payload, payment_pb2.PaymentServiceGetRequest())

async def process_refund_payment(txn_id, config=None):
    """Refund a completed payment"""
    # Create client from config
    from payments import PaymentClient
    client = PaymentClient(config)
    
    # Use txn_id in merchant_transaction_id
    import copy
    request = _build_refund_request('AUTOMATIC')
    request.merchant_transaction_id = txn_id
    response = await client.refund(request)
    return response

async def process_get_payment_status(txn_id, config=None):
    """Retrieve current status of a payment"""
    # Create client from config
    from payments import PaymentClient
    client = PaymentClient(config)
    
    # Use txn_id in merchant_transaction_id
    import copy
    request = _build_get_request('AUTOMATIC')
    request.merchant_transaction_id = txn_id
    response = await client.get(request)
    return response
