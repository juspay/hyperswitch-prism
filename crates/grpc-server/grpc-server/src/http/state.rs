type CompositePaymentsService = composite_service::payments::Payments<
    crate::server::payments::Payments,
    crate::server::payments::MerchantAuthentication,
    crate::server::payments::Customer,
    crate::server::refunds::Refunds,
    crate::server::payments::PaymentMethodAuthentication,
>;

type CompositeEventService = composite_service::events::CompositeEvents<
    crate::server::events::EventServiceImpl,
    crate::server::payments::MerchantAuthentication,
>;

type CompositePaymentMethodService = composite_service::payment_methods::PaymentMethods<
    crate::server::payments::PaymentMethod,
    crate::server::payments::MerchantAuthentication,
>;

type CompositeFrmService = composite_service::frm::Frm<
    crate::server::frm::FraudAndRiskManagement,
    crate::server::payments::MerchantAuthentication,
>;
type SanityLayer<S> = crate::sanity_layer::SanityLayer<S>;

#[derive(Clone)]
pub struct AppState {
    pub composite_payments_service: SanityLayer<CompositePaymentsService>,
    pub composite_event_service: SanityLayer<CompositeEventService>,
    pub composite_payment_method_service: SanityLayer<CompositePaymentMethodService>,
    pub composite_frm_service: SanityLayer<CompositeFrmService>,
    pub payments_service: SanityLayer<crate::server::payments::Payments>,
    pub refunds_service: SanityLayer<crate::server::refunds::Refunds>,
    pub disputes_service: SanityLayer<crate::server::disputes::Disputes>,
    pub recurring_payment_service: SanityLayer<crate::server::payments::RecurringPayments>,
    pub event_service: SanityLayer<crate::server::events::EventServiceImpl>,
    pub payment_method_service: SanityLayer<crate::server::payments::PaymentMethod>,
    pub merchant_authentication_service:
        SanityLayer<crate::server::payments::MerchantAuthentication>,
    pub customer_service: SanityLayer<crate::server::payments::Customer>,
    pub payment_method_authentication_service:
        SanityLayer<crate::server::payments::PaymentMethodAuthentication>,
}

#[allow(clippy::too_many_arguments)]
impl AppState {
    pub fn new(
        composite_payments_service: SanityLayer<CompositePaymentsService>,
        composite_event_service: SanityLayer<CompositeEventService>,
        composite_payment_method_service: SanityLayer<CompositePaymentMethodService>,
        composite_frm_service: SanityLayer<CompositeFrmService>,
        payments_service: SanityLayer<crate::server::payments::Payments>,
        refund_service: SanityLayer<crate::server::refunds::Refunds>,
        dispute_service: SanityLayer<crate::server::disputes::Disputes>,
        recurring_payment_service: SanityLayer<crate::server::payments::RecurringPayments>,
        event_service: SanityLayer<crate::server::events::EventServiceImpl>,
        payment_method_service: SanityLayer<crate::server::payments::PaymentMethod>,
        merchant_authentication_service: SanityLayer<
            crate::server::payments::MerchantAuthentication,
        >,
        customer_service: SanityLayer<crate::server::payments::Customer>,
        payment_method_authentication_service: SanityLayer<
            crate::server::payments::PaymentMethodAuthentication,
        >,
    ) -> Self {
        Self {
            composite_payments_service,
            composite_event_service,
            composite_payment_method_service,
            composite_frm_service,
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
