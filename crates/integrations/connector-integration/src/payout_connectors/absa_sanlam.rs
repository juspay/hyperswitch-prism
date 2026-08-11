pub mod transformers;

use crate::{
    connectors::sanlam_common::transformers::KafkaEnqueueResponse, types::ResponseRouterData,
};
use common_enums::CurrencyUnit;
use common_utils::{
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
    request::{KafkaRecord, KafkaRecordBuilder, RequestContent, TransportType},
};
use domain_types::{
    connector_flow::{
        PayoutCreate, PayoutCreateLink, PayoutCreateRecipient, PayoutEnrollDisburseAccount,
        PayoutGet, PayoutStage, PayoutTransfer, PayoutVoid, ServerAuthenticationToken,
    },
    connector_types::{
        ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
    },
    errors::{
        ConnectorError, IntegrationError, IntegrationErrorContext,
        ResponseTransformationErrorContext,
    },
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payouts::payouts_types::{
        PayoutCreateLinkRequest, PayoutCreateLinkResponse, PayoutCreateRecipientRequest,
        PayoutCreateRecipientResponse, PayoutCreateRequest, PayoutCreateResponse,
        PayoutEnrollDisburseAccountRequest, PayoutEnrollDisburseAccountResponse, PayoutFlowData,
        PayoutGetRequest, PayoutGetResponse, PayoutStageRequest, PayoutStageResponse,
        PayoutTransferRequest, PayoutTransferResponse, PayoutVoidRequest, PayoutVoidResponse,
    },
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::{Mask, Maskable, PeekInterface};
use interfaces::{
    api::ConnectorCommon,
    connector_integration_v2::ConnectorIntegrationV2,
    connector_types::{
        PayoutCreateLinkV2, PayoutCreateRecipientV2, PayoutCreateV2, PayoutEnrollDisburseAccountV2,
        PayoutGetV2, PayoutServiceTrait, PayoutStageV2, PayoutTransferV2, PayoutVoidV2,
        ServerAuthentication,
    },
};
use transformers::{AbsaSanlamPayoutAuthType, AbsaSanlamPayoutTransferRequest};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    pub(crate) const MERCHANT_ID: &str = "Merchant-Id";
}

pub struct AbsaSanlamPayouts;

impl AbsaSanlamPayouts {
    pub const fn new() -> &'static Self {
        &Self
    }
}

impl ConnectorCommon for AbsaSanlamPayouts {
    fn id(&self) -> &'static str {
        "absa_sanlam"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.absa_sanlam.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = AbsaSanlamPayoutAuthType::try_from(auth_type).change_context(
            IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            },
        )?;
        Ok(vec![
            (
                headers::AUTHORIZATION.to_string(),
                auth.api_key.peek().to_owned().into_masked(),
            ),
            (
                headers::MERCHANT_ID.to_string(),
                auth.merchant_id.peek().to_owned().into(),
            ),
        ])
    }
}

impl PayoutServiceTrait for AbsaSanlamPayouts {}
impl ServerAuthentication for AbsaSanlamPayouts {}
impl PayoutTransferV2 for AbsaSanlamPayouts {}

impl
    ConnectorIntegrationV2<
        PayoutTransfer,
        PayoutFlowData,
        PayoutTransferRequest,
        PayoutTransferResponse,
    > for AbsaSanlamPayouts
{
    fn get_transport_type(&self) -> TransportType {
        TransportType::Kafka
    }

    fn get_url(
        &self,
        _req: &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(IntegrationError::connector_flow_not_supported(
            self.id(),
            "payout_transfer_http",
            IntegrationErrorContext::default(),
        )
        .into())
    }

    fn get_headers(
        &self,
        req: &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let mut header = vec![(
            headers::CONTENT_TYPE.to_string(),
            Self::common_get_content_type(self).to_string().into(),
        )];

        let mut api_key = self.get_auth_header(&req.connector_config)?;
        header.append(&mut api_key);

        Ok(header)
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
    ) -> CustomResult<Option<RequestContent>, IntegrationError> {
        let connector_req = AbsaSanlamPayoutTransferRequest::try_from(req)?;
        Ok(Some(RequestContent::Json(Box::new(connector_req))))
    }

    fn get_kafka_topic(
        &self,
        req: &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Ok(format!(
            "{}_payouts_queue",
            self.base_url(&req.resource_common_data.connectors)
        ))
    }

    fn build_kafka_record(
        &self,
        req: &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
    ) -> CustomResult<Option<KafkaRecord>, IntegrationError> {
        Ok(Some(
            KafkaRecordBuilder::new()
                .topic(self.get_kafka_topic(req)?.as_str())
                .attach_default_headers()
                .headers(self.get_headers(req)?)
                .set_optional_payload(self.get_request_body(req)?)
                .build(),
        ))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<
            PayoutTransfer,
            PayoutFlowData,
            PayoutTransferRequest,
            PayoutTransferResponse,
        >,
        event_builder: Option<&mut events::Event>,
        res: Response,
    ) -> CustomResult<
        RouterDataV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest, PayoutTransferResponse>,
        ConnectorError,
    > {
        let response: KafkaEnqueueResponse = res
            .response
            .parse_struct("KafkaEnqueueResponse")
            .change_context(ConnectorError::ResponseDeserializationFailed {
                context: ResponseTransformationErrorContext {
                    http_status_code: Some(res.status_code),
                    additional_context: Some("Failed to parse KafkaEnqueueResponse".to_string()),
                },
            })?;

        event_builder.map(|i| i.set_connector_response(&response));

        RouterDataV2::try_from(ResponseRouterData {
            response,
            router_data: data.clone(),
            http_code: res.status_code,
        })
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        self.build_error_response(res, event_builder, connector_config)
    }
}

macro_rules! impl_absa_sanlam_payout_stub {
    ($trait:ident, $flow:ty, $request:ty, $response:ty, $flow_name:literal) => {
        impl $trait for AbsaSanlamPayouts {}

        impl ConnectorIntegrationV2<$flow, PayoutFlowData, $request, $response>
            for AbsaSanlamPayouts
        {
            fn get_url(
                &self,
                _req: &RouterDataV2<$flow, PayoutFlowData, $request, $response>,
            ) -> CustomResult<String, IntegrationError> {
                Err(IntegrationError::connector_flow_not_implemented(
                    self.id(),
                    $flow_name,
                    IntegrationErrorContext::default(),
                )
                .into())
            }
        }
    };
}

impl
    ConnectorIntegrationV2<
        ServerAuthenticationToken,
        MerchantAuthenticationFlowData,
        ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    > for AbsaSanlamPayouts
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            ServerAuthenticationToken,
            MerchantAuthenticationFlowData,
            ServerAuthenticationTokenRequestData,
            ServerAuthenticationTokenResponseData,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(IntegrationError::connector_flow_not_implemented(
            self.id(),
            "server_authentication_token",
            IntegrationErrorContext::default(),
        )
        .into())
    }
}
impl_absa_sanlam_payout_stub!(
    PayoutCreateV2,
    PayoutCreate,
    PayoutCreateRequest,
    PayoutCreateResponse,
    "payout_create"
);
impl_absa_sanlam_payout_stub!(
    PayoutGetV2,
    PayoutGet,
    PayoutGetRequest,
    PayoutGetResponse,
    "payout_get"
);
impl_absa_sanlam_payout_stub!(
    PayoutVoidV2,
    PayoutVoid,
    PayoutVoidRequest,
    PayoutVoidResponse,
    "payout_void"
);
impl_absa_sanlam_payout_stub!(
    PayoutStageV2,
    PayoutStage,
    PayoutStageRequest,
    PayoutStageResponse,
    "payout_stage"
);
impl_absa_sanlam_payout_stub!(
    PayoutCreateLinkV2,
    PayoutCreateLink,
    PayoutCreateLinkRequest,
    PayoutCreateLinkResponse,
    "payout_create_link"
);
impl_absa_sanlam_payout_stub!(
    PayoutCreateRecipientV2,
    PayoutCreateRecipient,
    PayoutCreateRecipientRequest,
    PayoutCreateRecipientResponse,
    "payout_create_recipient"
);
impl_absa_sanlam_payout_stub!(
    PayoutEnrollDisburseAccountV2,
    PayoutEnrollDisburseAccount,
    PayoutEnrollDisburseAccountRequest,
    PayoutEnrollDisburseAccountResponse,
    "payout_enroll_disburse_account"
);
