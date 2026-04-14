type CompositePaymentsService = composite_service::payments::Payments<
    crate::server::payments::Payments,
    crate::server::payments::MerchantAuthentication,
    crate::server::payments::Customer,
    crate::server::refunds::Refunds,
>;

#[derive(Clone)]
pub struct AppState {
    pub composite_payments_service: CompositePaymentsService,
    pub payments_service: crate::server::payments::Payments,
    pub refunds_service: crate::server::refunds::Refunds,
    pub disputes_service: crate::server::disputes::Disputes,
    pub recurring_payment_service: crate::server::payments::RecurringPayments,
    pub event_service: crate::server::events::EventServiceImpl,
    pub payment_method_service: crate::server::payments::PaymentMethod,
    pub merchant_authentication_service: crate::server::payments::MerchantAuthentication,
    pub customer_service: crate::server::payments::Customer,
    pub payment_method_authentication_service: crate::server::payments::PaymentMethodAuthentication,
}

#[allow(clippy::too_many_arguments)]
impl AppState {
    pub fn new(
        composite_payments_service: CompositePaymentsService,
        payments_service: crate::server::payments::Payments,
        refund_service: crate::server::refunds::Refunds,
        dispute_service: crate::server::disputes::Disputes,
        recurring_payment_service: crate::server::payments::RecurringPayments,
        event_service: crate::server::events::EventServiceImpl,
        payment_method_service: crate::server::payments::PaymentMethod,
        merchant_authentication_service: crate::server::payments::MerchantAuthentication,
        customer_service: crate::server::payments::Customer,
        payment_method_authentication_service: crate::server::payments::PaymentMethodAuthentication,
    ) -> Self {
        Self {
            composite_payments_service,
            payments_service,
            refunds_service: refund_service,
            disputes_service: dispute_service,
            recurring_payment_service,
            event_service,
            payment_method_service,
            merchant_authentication_service,
            customer_service,
            payment_method_authentication_service,
        }
    }
}
