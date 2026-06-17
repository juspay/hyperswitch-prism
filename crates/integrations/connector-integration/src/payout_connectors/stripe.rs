pub mod transformers;

use std::fmt::Debug;

use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
};
use domain_types::{
    connector_flow::{
        PayoutCreate, PayoutCreateLink, PayoutCreateRecipient, PayoutEnrollDisburseAccount,
        PayoutGet, PayoutStage, PayoutTransfer, PayoutVoid,
    },
    errors::{ConnectorError, IntegrationError},
    payment_method_data::PaymentMethodDataTypes,
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
use hyperswitch_masking::{Mask, Maskable, PeekInterface, Secret};
use interfaces::{
    api::ConnectorCommon,
    connector_integration_v2::ConnectorIntegrationV2,
    connector_types::{
        PayoutCreateLinkV2, PayoutCreateRecipientV2, PayoutCreateV2, PayoutEnrollDisburseAccountV2,
        PayoutGetV2, PayoutServiceTrait, PayoutStageV2, PayoutTransferV2, PayoutVoidV2,
    },
};
use serde::Serialize;

use super::super::connectors::macros;
use crate::types::ResponseRouterData;
use transformers::{
    self as stripe, StripeConnectPayoutCreateRequest, StripeConnectPayoutCreateResponse,
    StripeConnectPayoutFulfillRequest, StripeConnectPayoutFulfillResponse,
    StripeConnectPayoutRetrieveResponse, StripeConnectRecipientAccountCreateRequest,
    StripeConnectRecipientAccountCreateResponse, StripeConnectRecipientCreateRequest,
    StripeConnectRecipientCreateResponse, StripeConnectReversalRequest,
    StripeConnectReversalResponse,
};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    pub(crate) const STRIPE_API_VERSION: &str = "stripe-version";
    pub(crate) const STRIPE_VERSION: &str = "2022-11-15";
    pub(crate) const STRIPE_COMPATIBLE_CONNECT_ACCOUNT: &str = "Stripe-Account";
}

// ===== CONNECTOR COMMON =====

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for StripePayouts<T>
{
    fn id(&self) -> &'static str {
        "stripe"
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/x-www-form-urlencoded"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.stripe.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = stripe::StripeAuthType::try_from(auth_type)?;
        Ok(vec![
            (
                headers::AUTHORIZATION.to_string(),
                format!("Bearer {}", auth.api_key.peek()).into_masked(),
            ),
            (
                headers::STRIPE_API_VERSION.to_string(),
                headers::STRIPE_VERSION.to_string().into_masked(),
            ),
        ])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: stripe::StripeConnectErrorResponse = res
            .response
            .parse_struct("StripeConnectErrorResponse")
            .change_context(crate::utils::response_handling_fail_for_connector(
                res.status_code,
                "stripe",
            ))?;

        event_builder.map(|i| i.set_connector_response(&response));

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response
                .error
                .code
                .clone()
                .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
            message: if response.error.message.is_empty() {
                NO_ERROR_MESSAGE.to_string()
            } else {
                response.error.message.clone()
            },
            reason: response.error.decline_code.clone(),
            attempt_status: None,
            connector_transaction_id: None,
            network_advice_code: response.error.network_advice_code.clone(),
            network_decline_code: response.error.network_decline_code.clone(),
            network_error_message: response
                .error
                .decline_code
                .clone()
                .or(response.error.advice_code.clone()),
        })
    }
}

// ===== PAYOUT SERVICE TRAITS =====

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> PayoutServiceTrait
    for StripePayouts<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> PayoutCreateV2
    for StripePayouts<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> PayoutTransferV2
    for StripePayouts<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> PayoutGetV2
    for StripePayouts<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> PayoutVoidV2
    for StripePayouts<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> PayoutCreateRecipientV2
    for StripePayouts<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    PayoutEnrollDisburseAccountV2 for StripePayouts<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> PayoutStageV2
    for StripePayouts<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> PayoutCreateLinkV2
    for StripePayouts<T>
{
}

// ===== PREREQUISITES (struct, bridges, shared member fns) =====

macros::create_all_prerequisites!(
    connector_name: StripePayouts,
    generic_type: T,
    api: [
        (
            flow: PayoutCreate,
            request_body: StripeConnectPayoutCreateRequest,
            response_body: StripeConnectPayoutCreateResponse,
            router_data: RouterDataV2<PayoutCreate, PayoutFlowData, PayoutCreateRequest, PayoutCreateResponse>,
        ),
        (
            flow: PayoutTransfer,
            request_body: StripeConnectPayoutFulfillRequest,
            response_body: StripeConnectPayoutFulfillResponse,
            router_data: RouterDataV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest, PayoutTransferResponse>,
        ),
        (
            flow: PayoutGet,
            response_body: StripeConnectPayoutRetrieveResponse,
            router_data: RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
        ),
        (
            flow: PayoutVoid,
            request_body: StripeConnectReversalRequest,
            response_body: StripeConnectReversalResponse,
            router_data: RouterDataV2<PayoutVoid, PayoutFlowData, PayoutVoidRequest, PayoutVoidResponse>,
        ),
        (
            flow: PayoutCreateRecipient,
            request_body: StripeConnectRecipientCreateRequest,
            response_body: StripeConnectRecipientCreateResponse,
            router_data: RouterDataV2<PayoutCreateRecipient, PayoutFlowData, PayoutCreateRecipientRequest, PayoutCreateRecipientResponse>,
        ),
        (
            flow: PayoutEnrollDisburseAccount,
            request_body: StripeConnectRecipientAccountCreateRequest,
            response_body: StripeConnectRecipientAccountCreateResponse,
            router_data: RouterDataV2<PayoutEnrollDisburseAccount, PayoutFlowData, PayoutEnrollDisburseAccountRequest, PayoutEnrollDisburseAccountResponse>,
        )
    ],
    amount_converters: [],
    member_functions: {
        /// Base headers shared by every payout flow: content type + bearer auth + api version.
        pub fn build_payout_headers(
            &self,
            connector_config: &ConnectorSpecificConfig,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let auth = stripe::StripeAuthType::try_from(connector_config)?;
            Ok(vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    self.common_get_content_type().to_string().into(),
                ),
                (
                    headers::AUTHORIZATION.to_string(),
                    format!("Bearer {}", auth.api_key.peek()).into_masked(),
                ),
                (
                    headers::STRIPE_API_VERSION.to_string(),
                    headers::STRIPE_VERSION.to_string().into_masked(),
                ),
            ])
        }
    }
);

// ===== PAYOUT CREATE (TRANSFER CREATE) =====

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: StripePayouts,
    curl_request: FormUrlEncoded(StripeConnectPayoutCreateRequest),
    curl_response: StripeConnectPayoutCreateResponse,
    flow_name: PayoutCreate,
    resource_common_data: PayoutFlowData,
    flow_request: PayoutCreateRequest,
    flow_response: PayoutCreateResponse,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PayoutCreate, PayoutFlowData, PayoutCreateRequest, PayoutCreateResponse>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_payout_headers(&req.connector_config)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<PayoutCreate, PayoutFlowData, PayoutCreateRequest, PayoutCreateResponse>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}v1/transfers",
                self.base_url(&req.resource_common_data.connectors)
            ))
        }
    }
);

// ===== PAYOUT TRANSFER (PAYOUT CREATE) =====

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: StripePayouts,
    curl_request: FormUrlEncoded(StripeConnectPayoutFulfillRequest),
    curl_response: StripeConnectPayoutFulfillResponse,
    flow_name: PayoutTransfer,
    resource_common_data: PayoutFlowData,
    flow_request: PayoutTransferRequest,
    flow_response: PayoutTransferResponse,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest, PayoutTransferResponse>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut headers = self.build_payout_headers(&req.connector_config)?;
            if let Some(ref account_id) = req.request.connector_payout_method_id {
                headers.push((
                    headers::STRIPE_COMPATIBLE_CONNECT_ACCOUNT.to_string(),
                    Secret::new(account_id.clone()).into_masked(),
                ));
            }
            Ok(headers)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<PayoutTransfer, PayoutFlowData, PayoutTransferRequest, PayoutTransferResponse>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}v1/payouts",
                self.base_url(&req.resource_common_data.connectors)
            ))
        }
    }
);

// ===== PAYOUT GET (PAYOUT RETRIEVE) =====

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: StripePayouts,
    curl_response: StripeConnectPayoutRetrieveResponse,
    flow_name: PayoutGet,
    resource_common_data: PayoutFlowData,
    flow_request: PayoutGetRequest,
    flow_response: PayoutGetResponse,
    http_method: Get,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut headers = self.build_payout_headers(&req.connector_config)?;
            if let Some(ref account_id) = req.request.connector_payout_method_id {
                headers.push((
                    headers::STRIPE_COMPATIBLE_CONNECT_ACCOUNT.to_string(),
                    Secret::new(account_id.clone()).into_masked(),
                ));
            }
            Ok(headers)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
        ) -> CustomResult<String, IntegrationError> {
            let payout_id = req.request.connector_payout_id.clone().ok_or_else(|| {
                IntegrationError::MissingConnectorTransactionID {
                    context: Default::default(),
                }
            })?;
            Ok(format!(
                "{}v1/payouts/{}",
                self.base_url(&req.resource_common_data.connectors),
                payout_id
            ))
        }
    }
);

// ===== PAYOUT VOID (TRANSFER REVERSAL) =====

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: StripePayouts,
    curl_request: FormUrlEncoded(StripeConnectReversalRequest),
    curl_response: StripeConnectReversalResponse,
    flow_name: PayoutVoid,
    resource_common_data: PayoutFlowData,
    flow_request: PayoutVoidRequest,
    flow_response: PayoutVoidResponse,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PayoutVoid, PayoutFlowData, PayoutVoidRequest, PayoutVoidResponse>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_payout_headers(&req.connector_config)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<PayoutVoid, PayoutFlowData, PayoutVoidRequest, PayoutVoidResponse>,
        ) -> CustomResult<String, IntegrationError> {
            let transfer_id = req.request.connector_payout_id.clone().ok_or_else(|| {
                IntegrationError::MissingConnectorTransactionID {
                    context: Default::default(),
                }
            })?;
            Ok(format!(
                "{}v1/transfers/{}/reversals",
                self.base_url(&req.resource_common_data.connectors),
                transfer_id
            ))
        }
    }
);

// ===== PAYOUT CREATE RECIPIENT (CONNECTED ACCOUNT) =====

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: StripePayouts,
    curl_request: FormUrlEncoded(StripeConnectRecipientCreateRequest),
    curl_response: StripeConnectRecipientCreateResponse,
    flow_name: PayoutCreateRecipient,
    resource_common_data: PayoutFlowData,
    flow_request: PayoutCreateRecipientRequest,
    flow_response: PayoutCreateRecipientResponse,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PayoutCreateRecipient, PayoutFlowData, PayoutCreateRecipientRequest, PayoutCreateRecipientResponse>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_payout_headers(&req.connector_config)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<PayoutCreateRecipient, PayoutFlowData, PayoutCreateRecipientRequest, PayoutCreateRecipientResponse>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}v1/accounts",
                self.base_url(&req.resource_common_data.connectors)
            ))
        }
    }
);

// ===== PAYOUT ENROLL DISBURSE ACCOUNT (EXTERNAL ACCOUNT) =====

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: StripePayouts,
    curl_request: FormUrlEncoded(StripeConnectRecipientAccountCreateRequest),
    curl_response: StripeConnectRecipientAccountCreateResponse,
    flow_name: PayoutEnrollDisburseAccount,
    resource_common_data: PayoutFlowData,
    flow_request: PayoutEnrollDisburseAccountRequest,
    flow_response: PayoutEnrollDisburseAccountResponse,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PayoutEnrollDisburseAccount, PayoutFlowData, PayoutEnrollDisburseAccountRequest, PayoutEnrollDisburseAccountResponse>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_payout_headers(&req.connector_config)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<PayoutEnrollDisburseAccount, PayoutFlowData, PayoutEnrollDisburseAccountRequest, PayoutEnrollDisburseAccountResponse>,
        ) -> CustomResult<String, IntegrationError> {
            let account_id = req.request.connector_payout_id.clone().ok_or_else(|| {
                IntegrationError::MissingConnectorTransactionID {
                    context: Default::default(),
                }
            })?;
            Ok(format!(
                "{}v1/accounts/{}/external_accounts",
                self.base_url(&req.resource_common_data.connectors),
                account_id
            ))
        }
    }
);

// ===== PAYOUT STUB FLOWS (NOT SUPPORTED BY STRIPE) =====

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<PayoutStage, PayoutFlowData, PayoutStageRequest, PayoutStageResponse>
    for StripePayouts<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<PayoutStage, PayoutFlowData, PayoutStageRequest, PayoutStageResponse>,
    ) -> CustomResult<String, IntegrationError> {
        Err(IntegrationError::connector_flow_not_implemented(
            self.id(),
            "payout_stage",
            Default::default(),
        )
        .into())
    }
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        PayoutCreateLink,
        PayoutFlowData,
        PayoutCreateLinkRequest,
        PayoutCreateLinkResponse,
    > for StripePayouts<T>
{
    fn get_url(
        &self,
        _req: &RouterDataV2<
            PayoutCreateLink,
            PayoutFlowData,
            PayoutCreateLinkRequest,
            PayoutCreateLinkResponse,
        >,
    ) -> CustomResult<String, IntegrationError> {
        Err(IntegrationError::connector_flow_not_implemented(
            self.id(),
            "payout_create_link",
            Default::default(),
        )
        .into())
    }
}
