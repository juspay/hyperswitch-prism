# Auto-generated for zift
import asyncio
from payments import PaymentClient

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

if __name__ == "__main__":
    asyncio.run(process_checkout_card("order_001"))