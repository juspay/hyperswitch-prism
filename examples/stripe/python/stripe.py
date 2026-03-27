# Auto-generated for stripe
import asyncio
from payments import CustomerClient
from payments import PaymentMethodClient
from payments import PaymentClient
from payments import RecurringPaymentClient
from payments.generated.payment_methods_pb2 import (
    PaymentMethod,
    SecretString,
)
from payments.generated.payment_pb2 import (
    Address,
    Currency,
    CustomerServiceCreateRequest,
    Money,
    PaymentAddress,
    PaymentMethodServiceTokenizeRequest,
    PaymentMethodType,
    RecurringPaymentServiceChargeRequest,
)

async def process_checkout_card(merchant_id, config):
    """Card Payment (Authorize + Capture)"""
    client = PaymentClient(config)
    # Step 1: Authorize — reserve funds on the payment method
    result = await client.authorize()
    # Step 2: Capture — settle the reserved funds
    result = await client.capture()
    return result
async def process_checkout_autocapture(merchant_id, config):
    """Card Payment (Automatic Capture)"""
    client = PaymentClient(config)
    # Step 1: Authorize — reserve funds on the payment method
    result = await client.authorize()
    return result
async def process_checkout_wallet(merchant_id, config):
    """Wallet Payment (Google Pay / Apple Pay)"""
    client = PaymentClient(config)
    # Step 1: Authorize — reserve funds on the payment method
    result = await client.authorize()
    return result
async def process_checkout_bank(merchant_id, config):
    """Bank Transfer (SEPA / ACH / BACS)"""
    client = PaymentClient(config)
    # Step 1: Authorize — reserve funds on the payment method
    result = await client.authorize()
    return result
async def process_refund(merchant_id, config):
    """Refund a Payment"""
    client = PaymentClient(config)
    # Step 1: Authorize — reserve funds on the payment method
    result = await client.authorize()
    # Step 2: Refund — return funds to the customer
    result = await client.refund()
    return result
async def process_recurring(merchant_id, config):
    """Recurring / Mandate Payments"""
    client = PaymentClient(config)
    recurring_client = RecurringPaymentClient(config)
    # Step 1: Setup Recurring — store the payment mandate
    result = await client.setup_recurring()
    # Step 2: Recurring Charge — charge against the stored mandate
    result = await recurring_client.charge(RecurringPaymentServiceChargeRequest(
            connector_recurring_payment_id="{'mandate_id_type': {'connector_mandate_id': {'connector_mandate_id': 'probe-mandate-123'}}}",
            amount=Money(
                minor_amount=1000,
                currency=Currency.Value('USD'),
            ),
            payment_method=PaymentMethod(
                token="{'token': 'probe_pm_token'}",
            ),
            return_url='https://example.com/recurring-return',
            connector_customer_id='cust_probe_123',
            payment_method_type=PaymentMethodType.Value('PAY_PAL'),
            off_session=True,
        ))
    return result
async def process_void_payment(merchant_id, config):
    """Void a Payment"""
    client = PaymentClient(config)
    # Step 1: Authorize — reserve funds on the payment method
    result = await client.authorize()
    # Step 2: Void — release reserved funds (cancel authorization)
    result = await client.void()
    return result
async def process_get_payment(merchant_id, config):
    """Get Payment Status"""
    client = PaymentClient(config)
    # Step 1: Authorize — reserve funds on the payment method
    result = await client.authorize()
    # Step 2: Get — retrieve current payment status from the connector
    result = await client.get()
    return result
async def process_create_customer(merchant_id, config):
    """Create Customer"""
    customer_client = CustomerClient(config)
    # Step 1: Create Customer — register customer record in the connector
    result = await customer_client.create(CustomerServiceCreateRequest(
            merchant_customer_id='cust_probe_123',
            customer_name='John Doe',
            email=SecretString(value='test@example.com'),
            phone_number='4155552671',
        ))
    return result
async def process_tokenize(merchant_id, config):
    """Tokenize Payment Method"""
    pm_client = PaymentMethodClient(config)
    # Step 1: Tokenize — store card details and return a reusable token
    result = await pm_client.tokenize(PaymentMethodServiceTokenizeRequest(
            amount=Money(
                minor_amount=1000,
                currency=Currency.Value('USD'),
            ),
            payment_method=PaymentMethod(
                card="{'card_number': '4111111111111111', 'card_exp_month': '03', 'card_exp_year': '2030', 'card_cvc': '737', 'card_holder_name': 'John Doe'}",
            ),
            address=PaymentAddress(
                billing_address=Address(),
            ),
        ))
    return result

if __name__ == "__main__":
    asyncio.run(process_checkout_card("order_001"))