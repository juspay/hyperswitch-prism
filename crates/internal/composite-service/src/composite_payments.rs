// use connector_integration::types::ConnectorData;
// use domain_types::{connector_types::ConnectorEnum, utils::ForeignTryFrom as _};
// use grpc_api_types::payments::{
//     composite_payment_service_server::CompositePaymentService,
//     payment_service_server::PaymentService, CompositeAuthorizeRequest, CompositeAuthorizeResponse,
//     PaymentServiceAuthorizeResponse, MerchantAuthenticationServiceCreateAccessTokenResponse,
//     CustomerServiceCreateResponse,
// };

#[derive(Clone)]
pub struct CompositePayments<S> {
    _payment_service: S,
}

// impl<S> CompositePayments<S> {
//     pub fn new(payment_service: S) -> Self {
//         Self { payment_service }
//     }
// }
