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
        PayoutCreate, PayoutCreateLink, PayoutCreateRecipient, PayoutEligibility,
        PayoutEnrollDisburseAccount, PayoutGet, PayoutStage, PayoutTransfer, PayoutVoid,
        ServerAuthenticationToken,
    },
    connector_types::{
        ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::PaymentMethodDataTypes,
    payouts::payouts_types::{
        PayoutCreateLinkRequest, PayoutCreateLinkResponse, PayoutCreateRecipientRequest,
        PayoutCreateRecipientResponse, PayoutCreateRequest, PayoutCreateResponse, PayoutCustomer,
        PayoutEligibilityRequest, PayoutEligibilityResponse, PayoutEnrollDisburseAccountRequest,
        PayoutEnrollDisburseAccountResponse, PayoutFlowData, PayoutGetRequest, PayoutGetResponse,
        PayoutStageRequest, PayoutStageResponse, PayoutTransferRequest, PayoutTransferResponse,
        PayoutVoidRequest, PayoutVoidResponse,
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
        PayoutCreateLinkV2, PayoutCreateRecipientV2, PayoutCreateV2, PayoutEligibilityV2,
        PayoutEnrollDisburseAccountV2, PayoutGetV2, PayoutServiceTrait, PayoutStageV2,
        PayoutTransferV2, PayoutVoidV2, ServerAuthentication,
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

        let typed =
            macros::serialize_typed_connector_payload(&response, "typed_connector_response");
        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response
                .error
                .code
                .clone()
                .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
            message: match response.error.message.is_empty() {
                true => NO_ERROR_MESSAGE.to_string(),
                false => response.error.message.clone(),
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
            typed_connector_response: typed,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
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
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ServerAuthentication
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

        /// Base payout headers plus `Stripe-Account` when the flow runs against a
        /// connected account. The `acct_…` rides on `customer.connector_customer_id`.
        pub fn build_connect_headers(
            &self,
            connector_config: &ConnectorSpecificConfig,
            customer: Option<&PayoutCustomer>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut headers = self.build_payout_headers(connector_config)?;
            headers.extend(
                customer
                    .and_then(|customer| customer.connector_customer_id.as_ref())
                    .map(|account_id| {
                        (
                            headers::STRIPE_COMPATIBLE_CONNECT_ACCOUNT.to_string(),
                            Secret::new(account_id.clone()).into_masked(),
                        )
                    }),
            );
            Ok(headers)
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
            self.build_connect_headers(&req.connector_config, req.request.customer.as_ref())
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
            self.build_connect_headers(&req.connector_config, req.request.customer.as_ref())
        }

        fn get_url(
            &self,
            req: &RouterDataV2<PayoutGet, PayoutFlowData, PayoutGetRequest, PayoutGetResponse>,
        ) -> CustomResult<String, IntegrationError> {
            let payout_id = req.request.connector_payout_id.clone().ok_or_else(|| {
                IntegrationError::MissingConnectorTransactionID {
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "Stripe payout retrieve needs the `po_…` id returned by the transfer call".to_string(),
                        ),
                        suggested_action: Some(
                            "Run PayoutTransfer first, or pass the connector payout id on the request".to_string(),
                        ),
                        doc_url: None,
                    },
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
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "Stripe transfer reversal needs the `tr_…` id returned by the transfer create call".to_string(),
                        ),
                        suggested_action: Some(
                            "Run PayoutCreate first, or pass the connector payout id on the request".to_string(),
                        ),
                        doc_url: None,
                    },
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
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "Stripe external-account creation needs the `acct_…` id of the connected account".to_string(),
                        ),
                        suggested_action: Some(
                            "Run PayoutCreateRecipient first, or pass the connector payout id on the request".to_string(),
                        ),
                        doc_url: None,
                    },
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

/// Stripe payouts covers create/transfer/get/void/recipient/enroll only. The remaining
/// payout flows fail at URL construction so callers get a clear not-implemented error
/// instead of a request that cannot succeed.
macro_rules! impl_unimplemented_payout_flow {
    ($trait_name:ident, $flow:ty, $request:ty, $response:ty, $flow_name:literal) => {
        impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> $trait_name
            for StripePayouts<T>
        {
        }

        impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
            ConnectorIntegrationV2<$flow, PayoutFlowData, $request, $response>
            for StripePayouts<T>
        {
            fn get_url(
                &self,
                _req: &RouterDataV2<$flow, PayoutFlowData, $request, $response>,
            ) -> CustomResult<String, IntegrationError> {
                Err(IntegrationError::connector_flow_not_implemented(
                    self.id(),
                    $flow_name,
                    IntegrationErrorContext {
                        additional_context: Some(
                            concat!("Stripe payouts does not expose a ", $flow_name, " endpoint")
                                .to_string(),
                        ),
                        suggested_action: Some(
                            "Route this flow to a connector that supports it".to_string(),
                        ),
                        doc_url: None,
                    },
                )
                .into())
            }
        }
    };
}

impl_unimplemented_payout_flow!(
    PayoutStageV2,
    PayoutStage,
    PayoutStageRequest,
    PayoutStageResponse,
    "payout_stage"
);
impl_unimplemented_payout_flow!(
    PayoutCreateLinkV2,
    PayoutCreateLink,
    PayoutCreateLinkRequest,
    PayoutCreateLinkResponse,
    "payout_create_link"
);
impl_unimplemented_payout_flow!(
    PayoutEligibilityV2,
    PayoutEligibility,
    PayoutEligibilityRequest,
    PayoutEligibilityResponse,
    "payout_eligibility"
);

// ===== SERVER AUTHENTICATION (not implemented) =====

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorIntegrationV2<
        ServerAuthenticationToken,
        MerchantAuthenticationFlowData,
        ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    > for StripePayouts<T>
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
            ConnectorCommon::id(self),
            "server_authentication_token",
            Default::default(),
        )
        .into())
    }
}
