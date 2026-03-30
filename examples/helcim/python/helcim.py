# Auto-generated for helcim
import asyncio
from payments import PaymentClient
from payments.generated.payment_methods_pb2 import (
    CardDetails,
    CardNumberType,
    PaymentMethod,
    SecretString,
)
from payments.generated.payment_pb2 import (
    Address,
    AuthenticationType,
    BrowserInformation,
    CaptureMethod,
    Currency,
    Money,
    PaymentAddress,
    PaymentServiceAuthorizeRequest,
    PaymentServiceGetRequest,
)

async def process_checkout_autocapture(merchant_id, config):
    """Card Payment (Automatic Capture)"""
    client = PaymentClient(config)
    # Step 1: Authorize — reserve funds on the payment method
    result = await client.authorize(PaymentServiceAuthorizeRequest(
            merchant_transaction_id='probe_txn_001',
            amount=Money(
                minor_amount=1000,
                currency=Currency.Value('USD'),
            ),
            payment_method=PaymentMethod(
                card=CardDetails(
                    card_number=CardNumberType(value='4111111111111111'),
                    card_exp_month=SecretString(value='03'),
                    card_exp_year=SecretString(value='2030'),
                    card_cvc=SecretString(value='737'),
                    card_holder_name=SecretString(value='John Doe'),
                ),
            ),
            capture_method=CaptureMethod.Value('AUTOMATIC'),
            address=PaymentAddress(
                billing_address=Address(
                    first_name=SecretString(value='John'),
                    line1=SecretString(value='123 Main St'),
                    zip_code=SecretString(value='98101'),
                ),
            ),
            auth_type=AuthenticationType.Value('NO_THREE_DS'),
            return_url='https://example.com/return',
            browser_info=BrowserInformation(
                ip_address='1.2.3.4',
            ),
        ))
    return result
async def process_get_payment(merchant_id, config):
    """Get Payment Status"""
    client = PaymentClient(config)
    # Step 1: Authorize — reserve funds on the payment method
    result = await client.authorize(PaymentServiceAuthorizeRequest(
            merchant_transaction_id='probe_txn_001',
            amount=Money(
                minor_amount=1000,
                currency=Currency.Value('USD'),
            ),
            payment_method=PaymentMethod(
                card=CardDetails(
                    card_number=CardNumberType(value='4111111111111111'),
                    card_exp_month=SecretString(value='03'),
                    card_exp_year=SecretString(value='2030'),
                    card_cvc=SecretString(value='737'),
                    card_holder_name=SecretString(value='John Doe'),
                ),
            ),
            capture_method=CaptureMethod.Value('MANUAL'),
            address=PaymentAddress(
                billing_address=Address(
                    first_name=SecretString(value='John'),
                    line1=SecretString(value='123 Main St'),
                    zip_code=SecretString(value='98101'),
                ),
            ),
            auth_type=AuthenticationType.Value('NO_THREE_DS'),
            return_url='https://example.com/return',
            browser_info=BrowserInformation(
                ip_address='1.2.3.4',
            ),
        ))
    # Step 2: Get — retrieve current payment status from the connector
    result = await client.get(PaymentServiceGetRequest(
            merchant_transaction_id='probe_merchant_txn_001',
            connector_transaction_id='probe_connector_txn_001',
            amount=Money(
                minor_amount=1000,
                currency=Currency.Value('USD'),
            ),
        ))
    return result

if __name__ == "__main__":
    asyncio.run(process_checkout_card("order_001"))