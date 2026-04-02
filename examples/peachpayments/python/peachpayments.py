# peachpayments SDK Examples

from google.protobuf.json_format import ParseDict
from payments.generated import payment_pb2

def _build_authorize_request(arg=None):
    """Build PaymentServiceAuthorizeRequest proto."""
    from google.protobuf.json_format import ParseDict
    from payments.generated import payment_pb2
    payload = {'merchant_transaction_id': 'probe_txn_001', 'amount': {'minor_amount': 1000, 'currency': 'USD'}, 'payment_method': {'card': {'card_number': {'value': '4111111111111111'}, 'card_exp_month': {'value': '03'}, 'card_exp_year': {'value': '2030'}, 'card_cvc': {'value': '737'}, 'card_holder_name': {'value': 'John Doe'}}}, 'capture_method': 'AUTOMATIC', 'address': {'billing_address': {}}, 'auth_type': 'NO_THREE_DS', 'return_url': 'https://example.com/return'}
    if arg:
        if arg in ('AUTOMATIC', 'MANUAL'):
            payload['capture_method'] = arg
        elif isinstance(arg, str) and 'connector_transaction_id' in payload:
            payload['connector_transaction_id'] = arg
    return ParseDict(payload, payment_pb2.PaymentServiceAuthorizeRequest())

def _build_capture_request(arg=None):
    """Build PaymentServiceCaptureRequest proto."""
    from google.protobuf.json_format import ParseDict
    from payments.generated import payment_pb2
    payload = {'merchant_capture_id': 'probe_capture_001', 'connector_transaction_id': 'probe_connector_txn_001', 'amount_to_capture': {'minor_amount': 1000, 'currency': 'USD'}}
    if arg:
        if arg in ('AUTOMATIC', 'MANUAL'):
            payload['capture_method'] = arg
        elif isinstance(arg, str) and 'connector_transaction_id' in payload:
            payload['connector_transaction_id'] = arg
    return ParseDict(payload, payment_pb2.PaymentServiceCaptureRequest())

def _build_refund_request(arg=None):
    """Build RefundServiceGetRequest proto."""
    from google.protobuf.json_format import ParseDict
    from payments.generated import payment_pb2
    payload = {'merchant_refund_id': 'probe_refund_001', 'connector_transaction_id': 'probe_connector_txn_001', 'payment_amount': 1000, 'refund_amount': {'minor_amount': 1000, 'currency': 'USD'}, 'reason': 'customer_request'}
    if arg:
        if arg in ('AUTOMATIC', 'MANUAL'):
            payload['capture_method'] = arg
        elif isinstance(arg, str) and 'connector_transaction_id' in payload:
            payload['connector_transaction_id'] = arg
    return ParseDict(payload, payment_pb2.RefundServiceGetRequest())

def _build_void_request(arg=None):
    """Build PaymentServiceVoidRequest proto."""
    from google.protobuf.json_format import ParseDict
    from payments.generated import payment_pb2
    payload = {'merchant_void_id': 'probe_void_001', 'connector_transaction_id': 'probe_connector_txn_001', 'amount': {'minor_amount': 1000, 'currency': 'USD'}}
    if arg:
        if arg in ('AUTOMATIC', 'MANUAL'):
            payload['capture_method'] = arg
        elif isinstance(arg, str) and 'connector_transaction_id' in payload:
            payload['connector_transaction_id'] = arg
    return ParseDict(payload, payment_pb2.PaymentServiceVoidRequest())

def _build_get_request(arg=None):
    """Build PaymentServiceGetRequest proto."""
    from google.protobuf.json_format import ParseDict
    from payments.generated import payment_pb2
    payload = {'merchant_transaction_id': 'probe_merchant_txn_001', 'connector_transaction_id': 'probe_connector_txn_001', 'amount': {'minor_amount': 1000, 'currency': 'USD'}}
    if arg:
        if arg in ('AUTOMATIC', 'MANUAL'):
            payload['capture_method'] = arg
        elif isinstance(arg, str) and 'connector_transaction_id' in payload:
            payload['connector_transaction_id'] = arg
    return ParseDict(payload, payment_pb2.PaymentServiceGetRequest())

async def process_checkout_card(txn_id, config=None):
    """Standard card authorization and capture flow"""
    # Create client from config
    from payments import PaymentClient
    client = PaymentClient(config)
    
    # Use txn_id in merchant_transaction_id
    import copy
    request = _build_authorize_request('AUTOMATIC')
    request.merchant_transaction_id = txn_id
    response = await client.authorize(request)
    if response.status != 8:  # CHARGED
        raise Exception(f'authorize failed with status: {response.status}')
    request = _build_capture_request(response.connector_transaction_id)
    response = await client.capture(request)
    return response

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

async def process_void_authorization(txn_id, config=None):
    """Cancel an uncaptured authorization"""
    # Create client from config
    from payments import PaymentClient
    client = PaymentClient(config)
    
    # Use txn_id in merchant_transaction_id
    import copy
    request = _build_void_request('AUTOMATIC')
    request.merchant_transaction_id = txn_id
    response = await client.void(request)
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

async def process_partial_refund(txn_id, config=None):
    """Refund a portion of a captured payment"""
    # Create client from config
    from payments import PaymentClient
    client = PaymentClient(config)
    
    # Use txn_id in merchant_transaction_id
    import copy
    request = _build_authorize_request('AUTOMATIC')
    request.merchant_transaction_id = txn_id
    response = await client.authorize(request)
    if response.status != 8:  # CHARGED
        raise Exception(f'authorize failed with status: {response.status}')
    request = _build_capture_request(response.connector_transaction_id)
    response = await client.capture(request)
    request = _build_refund_request(response.connector_transaction_id)
    response = await client.refund(request)
    return response

async def process_multi_capture(txn_id, config=None):
    """Split a single authorization into multiple captures (e.g., for split shipments)"""
    # Create client from config
    from payments import PaymentClient
    client = PaymentClient(config)
    
    # Use txn_id in merchant_transaction_id
    import copy
    request = _build_authorize_request('AUTOMATIC')
    request.merchant_transaction_id = txn_id
    response = await client.authorize(request)
    if response.status != 8:  # CHARGED
        raise Exception(f'authorize failed with status: {response.status}')
    request = _build_capture_request(response.connector_transaction_id)
    response = await client.capture(request)
    return response

async def process_incremental_authorization(txn_id, config=None):
    """Increase the authorized amount after initial authorization"""
    # Create client from config
    from payments import PaymentClient
    client = PaymentClient(config)
    
    # Use txn_id in merchant_transaction_id
    import copy
    request = _build_authorize_request('AUTOMATIC')
    request.merchant_transaction_id = txn_id
    response = await client.authorize(request)
    if response.status != 8:  # CHARGED
        raise Exception(f'authorize failed with status: {response.status}')
    request = _build_authorize_request(response.connector_transaction_id)
    response = await client.authorize(request)
    if response.status != 8:  # CHARGED
        raise Exception(f'authorize failed with status: {response.status}')
    request = _build_capture_request(response.connector_transaction_id)
    response = await client.capture(request)
    return response

async def process_checkout_3ds(txn_id, config=None):
    """Card payment with 3D Secure authentication"""
    # Create client from config
    from payments import PaymentClient
    client = PaymentClient(config)
    
    # Use txn_id in merchant_transaction_id
    import copy
    request = _build_authorize_request('AUTOMATIC')
    request.merchant_transaction_id = txn_id
    response = await client.authorize(request)
    if response.status != 8:  # CHARGED
        raise Exception(f'authorize failed with status: {response.status}')
    request = _build_authenticate_request(response.connector_transaction_id)
    response = await client.authenticate(request)
    request = _build_capture_request(response.connector_transaction_id)
    response = await client.capture(request)
    return response
