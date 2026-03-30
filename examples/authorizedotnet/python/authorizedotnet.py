# Auto-generated for authorizedotnet
import asyncio
from payments import CustomerClient
from payments import PaymentClient
from payments import RecurringPaymentClient
from payments.generated.payment_methods_pb2 import (
    Ach,
    CardDetails,
    CardNumberType,
    PaymentMethod,
    SecretString,
    TokenPaymentMethodType,
)
from payments.generated.payment_pb2 import (
    AcceptanceType,
    Address,
    AuthenticationType,
    CaptureMethod,
    ConnectorMandateReferenceId,
    Currency,
    Customer,
    CustomerAcceptance,
    CustomerServiceCreateRequest,
    FutureUsage,
    MandateReference,
    Money,
    PaymentAddress,
    PaymentMethodType,
    PaymentServiceAuthorizeRequest,
    PaymentServiceCaptureRequest,
    PaymentServiceGetRequest,
    PaymentServiceRefundRequest,
    PaymentServiceSetupRecurringRequest,
    PaymentServiceVoidRequest,
    RecurringPaymentServiceChargeRequest,
)

async def process_checkout_card(merchant_id, config):
    """Card Payment (Authorize + Capture)"""
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
                billing_address=Address(),
            ),
            auth_type=AuthenticationType.Value('NO_THREE_DS'),
            return_url='https://example.com/return',
        ))
    # Step 2: Capture — settle the reserved funds
    result = await client.capture(PaymentServiceCaptureRequest(
            merchant_capture_id='probe_capture_001',
            connector_transaction_id='probe_connector_txn_001',
            amount_to_capture=Money(
                minor_amount=1000,
                currency=Currency.Value('USD'),
            ),
        ))
    return result
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
                billing_address=Address(),
            ),
            auth_type=AuthenticationType.Value('NO_THREE_DS'),
            return_url='https://example.com/return',
        ))
    return result
async def process_checkout_bank(merchant_id, config):
    """Bank Transfer (SEPA / ACH / BACS)"""
    client = PaymentClient(config)
    # Step 1: Authorize — reserve funds on the payment method
    result = await client.authorize(PaymentServiceAuthorizeRequest(
            merchant_transaction_id='probe_txn_001',
            amount=Money(
                minor_amount=1000,
                currency=Currency.Value('USD'),
            ),
            payment_method=PaymentMethod(
                ach=Ach(
                    account_number=SecretString(value='000123456789'),
                    routing_number=SecretString(value='110000000'),
                    bank_account_holder_name=SecretString(value='John Doe'),
                ),
            ),
            capture_method=CaptureMethod.Value('AUTOMATIC'),
            address=PaymentAddress(
                billing_address=Address(),
            ),
            auth_type=AuthenticationType.Value('NO_THREE_DS'),
            return_url='https://example.com/return',
        ))
    return result
async def process_refund(merchant_id, config):
    """Refund a Payment"""
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
                billing_address=Address(),
            ),
            auth_type=AuthenticationType.Value('NO_THREE_DS'),
            return_url='https://example.com/return',
        ))
    # Step 2: Refund — return funds to the customer
    result = await client.refund(PaymentServiceRefundRequest(
            merchant_refund_id='probe_refund_001',
            connector_transaction_id='probe_connector_txn_001',
            payment_amount=1000,
            refund_amount=Money(
                minor_amount=1000,
                currency=Currency.Value('USD'),
            ),
            reason='customer_request',
        ))
    return result
async def process_recurring(merchant_id, config):
    """Recurring / Mandate Payments"""
    client = PaymentClient(config)
    recurring_client = RecurringPaymentClient(config)
    # Step 1: Setup Recurring — store the payment mandate
    result = await client.setup_recurring(PaymentServiceSetupRecurringRequest(
            merchant_recurring_payment_id='probe_mandate_001',
            amount=Money(
                minor_amount=0,
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
            customer=Customer(
                connector_customer_id='cust_probe_123',
            ),
            address=PaymentAddress(
                billing_address=Address(),
            ),
            auth_type=AuthenticationType.Value('NO_THREE_DS'),
            enrolled_for_3ds=False,
            return_url='https://example.com/mandate-return',
            setup_future_usage=FutureUsage.Value('OFF_SESSION'),
            request_incremental_authorization=False,
            customer_acceptance=CustomerAcceptance(
                acceptance_type=AcceptanceType.Value('OFFLINE'),
                accepted_at=0,
            ),
        ))
    # Step 2: Recurring Charge — charge against the stored mandate
    result = await recurring_client.charge(RecurringPaymentServiceChargeRequest(
            connector_recurring_payment_id=MandateReference(
                connector_mandate_id=ConnectorMandateReferenceId(
                    connector_mandate_id='probe-mandate-123',
                ),
            ),
            amount=Money(
                minor_amount=1000,
                currency=Currency.Value('USD'),
            ),
            payment_method=PaymentMethod(
                token=TokenPaymentMethodType(
                    token=SecretString(value='probe_pm_token'),
                ),
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
                billing_address=Address(),
            ),
            auth_type=AuthenticationType.Value('NO_THREE_DS'),
            return_url='https://example.com/return',
        ))
    # Step 2: Void — release reserved funds (cancel authorization)
    result = await client.void(PaymentServiceVoidRequest(
            merchant_void_id='probe_void_001',
            connector_transaction_id='probe_connector_txn_001',
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
                billing_address=Address(),
            ),
            auth_type=AuthenticationType.Value('NO_THREE_DS'),
            return_url='https://example.com/return',
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

if __name__ == "__main__":
    asyncio.run(process_checkout_card("order_001"))