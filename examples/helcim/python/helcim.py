# Auto-generated for helcim
import asyncio
from payments import PaymentClient

async def process_checkout_autocapture(merchant_id, config):
    """Card Payment (Automatic Capture)"""
    client = PaymentClient(config)
    # Step 1: Authorize — reserve funds on the payment method
    result = await client.authorize(...)
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