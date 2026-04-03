use common_enums::CurrencyUnit;
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    events, CustomResult,
};
use domain_types::{
    api::{GenericLinks, PaymentLinkAction, RedirectionFormData},
    errors::{ConnectorResponseTransformationError, IntegrationError},
    payment_address::RedirectionResponse,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    types::Connectors,
};
use hyperswitch_masking;

pub trait ConnectorCommon {
    /// Name of the connector (in lowercase).
    fn id(&self) -> &'static str;

    /// Connector accepted currency unit as either "Base" or "Minor"
    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor // Default implementation should be remove once it is implemented in all connectors
    }

    /// HTTP header used for authorization.
    fn get_auth_header(
        &self,
        _auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, hyperswitch_masking::Maskable<String>)>, IntegrationError> {
        Ok(Vec::new())
    }

    /// HTTP `Content-Type` to be used for POST requests.
    /// Defaults to `application/json`.
    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    // FIXME write doc - think about this
    // fn headers(&self) -> Vec<(&str, &str)>;

    /// The base URL for interacting with the connector's API.
    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str;

    /// common error response for a connector if it is same in all case
    fn build_error_response(
        &self,
        res: domain_types::router_response_types::Response,
        _event_builder: Option<&mut events::Event>,
    ) -> CustomResult<ErrorResponse, ConnectorResponseTransformationError> {
        Ok(ErrorResponse {
            status_code: res.status_code,
            code: NO_ERROR_CODE.to_string(),
            message: NO_ERROR_MESSAGE.to_string(),
            reason: None,
            attempt_status: None,
            connector_transaction_id: None,
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
        })
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum ApplicationResponse<R> {
    Json(R),
    StatusOk,
    TextPlain(String),
    JsonForRedirection(RedirectionResponse),
    Form(Box<RedirectionFormData>),
    PaymentLinkForm(Box<PaymentLinkAction>),
    FileData((Vec<u8>, mime::Mime)),
    JsonWithHeaders((R, Vec<(String, hyperswitch_masking::Maskable<String>)>)),
    GenericLinkForm(Box<GenericLinks>),
}
