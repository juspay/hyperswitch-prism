# Auto-generated for finix
import asyncio
from payments import CustomerClient
from payments import PaymentClient
from payments import PaymentMethodClient

async def process_checkout_card(merchant_id, config):
    """Card Payment (Authorize + Capture)"""
    client = PaymentClient(config)
    # Step 1: Authorize — reserve funds on the payment method
    result = await client.authorize(...)
    # Step 2: Capture — settle the reserved funds
    result = await client.capture(...)
    return result
async def process_checkout_autocapture(merchant_id, config):
    """Card Payment (Automatic Capture)"""
    client = PaymentClient(config)
    # Step 1: Authorize — reserve funds on the payment method
    result = await client.authorize(...)
    return result
async def process_checkout_wallet(merchant_id, config):
    """Wallet Payment (Google Pay / Apple Pay)"""
    client = PaymentClient(config)
    # Step 1: Authorize — reserve funds on the payment method
    result = await client.authorize(...)
    return result
async def process_checkout_bank(merchant_id, config):
    """Bank Transfer (SEPA / ACH / BACS)"""
    client = PaymentClient(config)
    # Step 1: Authorize — reserve funds on the payment method
    result = await client.authorize(...)
    return result
async def process_refund(merchant_id, config):
    """Refund a Payment"""
    client = PaymentClient(config)
    # Step 1: Authorize — reserve funds on the payment method
    result = await client.authorize(...)
    # Step 2: Refund — return funds to the customer
    result = await client.refund(...)
    return result
async def process_void_payment(merchant_id, config):
    """Void a Payment"""
    client = PaymentClient(config)
    # Step 1: Authorize — reserve funds on the payment method
    result = await client.authorize(...)
    # Step 2: Void — release reserved funds (cancel authorization)
    result = await client.void(...)
    return result
async def process_get_payment(merchant_id, config):
    """Get Payment Status"""
    client = PaymentClient(config)
    # Step 1: Authorize — reserve funds on the payment method
    result = await client.authorize(...)
    # Step 2: Get — retrieve current payment status from the connector
    result = await client.get(...)
    return result
async def process_create_customer(merchant_id, config):
    """Create Customer"""
    customer_client = CustomerClient(config)
    # Step 1: Create Customer — register customer record in the connector
    result = await customer_client.create(...)
    return result
async def process_tokenize(merchant_id, config):
    """Tokenize Payment Method"""
    pm_client = PaymentMethodClient(config)
    # Step 1: Tokenize — store card details and return a reusable token
    result = await pm_client.tokenize(...)
    return result

if __name__ == "__main__":
    asyncio.run(process_checkout_card("order_001"))