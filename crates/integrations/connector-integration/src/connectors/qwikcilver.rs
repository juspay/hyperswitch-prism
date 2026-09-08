pub mod transformers;

use std::fmt::Debug;

use common_enums::CurrencyUnit;
use common_utils::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    errors::CustomResult,
    events,
    ext_traits::ByteSliceExt,
};
use domain_types::{
    connector_flow::{
        Authorize, CreatePaymentMethod, GetPaymentMethod, PaymentMethodEligibility, Recharge,
        Refund, ServerAuthenticationToken,
    },
    connector_types::{
        CreatePaymentMethodData, CreatePaymentMethodResponseData, GetPaymentMethodData,
        GetPaymentMethodResponseData, PaymentFlowData, PaymentMethodEligibilityData,
        PaymentMethodEligibilityResponse, PaymentsAuthorizeData, PaymentsResponseData,
        RechargeRequestData, RechargeResponseData, RefundFlowData, RefundsData,
        RefundsResponseData, ServerAuthenticationTokenRequestData,
        ServerAuthenticationTokenResponseData,
    },
    errors::{ConnectorError, IntegrationError},
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::{Mask, Maskable, PeekInterface};
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
    decode::BodyDecoding, verification::SourceVerification,
};
use serde::Serialize;
use transformers::{
    self as qwikcilver, QwikcilverAuthType, QwikcilverAuthorizeRequest,
    QwikcilverAuthorizeResponse, QwikcilverCancelRedeemBody, QwikcilverCancelRedeemResponse,
    QwikcilverCreateWalletRequest, QwikcilverEligibilityResponse, QwikcilverEmptyBody,
    QwikcilverErrorResponse, QwikcilverGetWalletResponse, QwikcilverRechargeRequest,
    QwikcilverRechargeResponse, QwikcilverRedeemRequest, QwikcilverRedeemResponse,
    QwikcilverWalletEnvelope,
};

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
    pub(crate) const DATE_AT_CLIENT: &str = "DateAtClient";
    pub(crate) const TRANSACTION_ID: &str = "TransactionId";
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for Qwikcilver<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for Qwikcilver<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for Qwikcilver<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RechargeV2 for Qwikcilver<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::CreatePaymentMethodV2 for Qwikcilver<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::GetPaymentMethodV2 for Qwikcilver<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentMethodEligibilityV2 for Qwikcilver<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ServerAuthentication for Qwikcilver<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::VerifyRedirectResponse for Qwikcilver<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::IncomingWebhook for Qwikcilver<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Qwikcilver<T>
{
}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Qwikcilver<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for Qwikcilver<T>
{
    fn should_do_access_token(&self, _payment_method: Option<common_enums::PaymentMethod>) -> bool {
        true
    }
}

macros::macro_connector_payout_implementation!(
    connector: Qwikcilver,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);

macros::create_amount_converter_wrapper!(connector_name: Qwikcilver, amount_type: FloatMajorUnit);

macros::create_all_prerequisites!(
    connector_name: Qwikcilver,
    generic_type: T,
    api: [
        (
            flow: ServerAuthenticationToken,
            request_body: QwikcilverAuthorizeRequest,
            response_body: QwikcilverAuthorizeResponse,
            router_data: RouterDataV2<ServerAuthenticationToken, MerchantAuthenticationFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>,
        ),
        (
            flow: Authorize,
            request_body: QwikcilverRedeemRequest,
            response_body: QwikcilverRedeemResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: QwikcilverCancelRedeemBody,
            response_body: QwikcilverCancelRedeemResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: Recharge,
            request_body: QwikcilverRechargeRequest,
            response_body: QwikcilverRechargeResponse,
            router_data: RouterDataV2<Recharge, PaymentFlowData, RechargeRequestData, RechargeResponseData>,
        ),
        (
            flow: CreatePaymentMethod,
            request_body: QwikcilverCreateWalletRequest,
            response_body: QwikcilverWalletEnvelope,
            router_data: RouterDataV2<CreatePaymentMethod, PaymentFlowData, CreatePaymentMethodData, CreatePaymentMethodResponseData>,
        ),
        (
            flow: GetPaymentMethod,
            request_body: QwikcilverEmptyBody,
            response_body: QwikcilverGetWalletResponse,
            router_data: RouterDataV2<GetPaymentMethod, PaymentFlowData, GetPaymentMethodData, GetPaymentMethodResponseData>,
        ),
        (
            flow: PaymentMethodEligibility,
            response_body: QwikcilverEligibilityResponse,
            router_data: RouterDataV2<PaymentMethodEligibility, PaymentFlowData, PaymentMethodEligibilityData, PaymentMethodEligibilityResponse>,
        )
    ],
    amount_converters: [
        amount_converter: FloatMajorUnit
    ],
    member_functions: {
        pub fn build_authenticated_headers<F, FCD, Req, Res>(
            &self,
            _req: &RouterDataV2<F, FCD, Req, Res>,
            access_token: &hyperswitch_masking::Secret<String>,
            date_at_client: &str,
            transaction_id: u64,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError>
        where
            Self: ConnectorIntegrationV2<F, FCD, Req, Res>,
        {
            Ok(vec![
                (
                    headers::CONTENT_TYPE.to_string(),
                    self.common_get_content_type().to_string().into(),
                ),
                (
                    headers::AUTHORIZATION.to_string(),
                    format!("Bearer {}", access_token.peek()).into_masked(),
                ),
                (
                    headers::DATE_AT_CLIENT.to_string(),
                    date_at_client.to_string().into(),
                ),
                (
                    headers::TRANSACTION_ID.to_string(),
                    transaction_id.to_string().into(),
                ),
            ])
        }

        pub fn extract_access_token(
            &self,
            access_token: Option<&ServerAuthenticationTokenResponseData>,
        ) -> CustomResult<hyperswitch_masking::Secret<String>, IntegrationError> {
            access_token
                .map(|t| t.access_token.clone())
                .ok_or_else(|| {
                    IntegrationError::FailedToObtainAuthType {
                        context: qwikcilver::qc_err_ctx(
                            "Qwikcilver requires a session JWT from `/authorize` on every \
                             authenticated call, but no `access_token` was found on the flow's \
                             common data — the bootstrap step did not run or its response was \
                             discarded.",
                            "Hit the composite endpoint (e.g. `/composite/payments/authorize`) \
                             which auto-bootstraps, OR call \
                             `/payments/server_authentication_token` first and pass the resulting \
                             token back via `state.access_token` on the next request.",
                        ),
                    }
                    .into()
                })
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.qwikcilver.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.qwikcilver.base_url
        }
        pub fn connector_base_url_merchant_auth<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, MerchantAuthenticationFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.qwikcilver.base_url
        }

        /// Shared wallet lookup used by both `GetPaymentMethod` and
        /// `PaymentMethodEligibility`, which make the identical connector call.
        /// Primary: wallet number → `/wallet/{wn}`.
        /// Fallback: customer phone → `/wallet/customer?phonenumber={phone}` (Pine Labs's
        /// documented by-external-id lookup; response envelope is identical to the
        /// by-wallet-number variant).
        pub fn wallet_lookup_url(
            &self,
            base: &str,
            wallet_number: Option<&str>,
            phone: Option<&hyperswitch_masking::Secret<String>>,
        ) -> CustomResult<String, IntegrationError> {
            if let Some(wallet_number) = wallet_number {
                return Ok(format!(
                    "{base}Qwikcilver/eGMS.RestApi/api/v2/wallet/{}",
                    urlencoding::encode(wallet_number),
                ));
            }
            if let Some(phone) = phone {
                return Ok(format!(
                    "{base}Qwikcilver/eGMS.RestApi/api/v2/wallet/customer?phonenumber={}",
                    urlencoding::encode(phone.peek()),
                ));
            }
            Err(IntegrationError::MissingRequiredField {
                field_name: "connector_payment_method_id | customer.phone_number",
                context: qwikcilver::qc_err_ctx(
                    "Qwikcilver's wallet lookup accepts either the wallet number (preferred) or \
                     the customer's phone number as a fallback. Neither was supplied, so \
                     there's no way to identify which wallet to fetch.",
                    "Set `connector_payment_method_id` to the wallet number returned by a \
                     prior Create, OR set `customer.phone_number` to look up by Pine Labs's \
                     external wallet id (the customer's mobile).",
                ),
            }
            .into())
        }
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for Qwikcilver<T>
{
    fn id(&self) -> &'static str {
        "qwikcilver"
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Base
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.qwikcilver.base_url.as_ref()
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        // Only `/authorize` uses this long-lived bearer; all other calls use the session JWT via `build_authenticated_headers`.
        let auth = QwikcilverAuthType::try_from(auth_type)?;
        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            format!("Bearer {}", auth.bootstrap_bearer_token.peek()).into_masked(),
        )])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: QwikcilverErrorResponse =
            res.response.parse_struct("QwikcilverErrorResponse").change_context(
                crate::utils::response_deserialization_fail(
                    res.status_code,
                    "qwikcilver: response body did not match the expected format; confirm API version and connector documentation.",
                ),
            ).attach_printable_lazy(|| format!(
                "qwikcilver: failed to parse error response (status={}, body_len={})",
                res.status_code,
                res.response.len(),
            ))?;

        with_error_response_body!(event_builder, response);

        let code = response
            .error_code
            .clone()
            .or_else(|| response.response_code.map(|c| c.to_string()))
            .unwrap_or_else(|| NO_ERROR_CODE.to_string());
        let message = response
            .response_message
            .clone()
            .or_else(|| response.error_description.clone())
            .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string());

        let typed =
            macros::serialize_typed_connector_payload(&response, "typed_connector_response");
        Ok(ErrorResponse {
            status_code: res.status_code,
            code,
            message,
            reason: response.error_description.or(response.response_message),
            attempt_status: None,
            connector_transaction_id: None,
            network_advice_code: None,
            network_decline_code: None,
            network_error_message: None,
            typed_connector_response: typed,
            raw_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
        })
    }
}

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Qwikcilver,
    curl_request: Json(QwikcilverAuthorizeRequest),
    curl_response: QwikcilverAuthorizeResponse,
    flow_name: ServerAuthenticationToken,
    resource_common_data: MerchantAuthenticationFlowData,
    flow_request: ServerAuthenticationTokenRequestData,
    flow_response: ServerAuthenticationTokenResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<ServerAuthenticationToken, MerchantAuthenticationFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}{}",
                self.connector_base_url_merchant_auth(req),
                "Qwikcilver/eGMS.RestApi/api/v2/authorize"
            ))
        }

        fn get_headers(
            &self,
            req: &RouterDataV2<ServerAuthenticationToken, MerchantAuthenticationFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut headers = vec![(
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type().to_string().into(),
            )];
            let mut auth = self.get_auth_header(&req.connector_config)?;
            headers.append(&mut auth);
            Ok(headers)
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Qwikcilver,
    curl_request: Json(QwikcilverRedeemRequest),
    curl_response: QwikcilverRedeemResponse,
    flow_name: Authorize,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsAuthorizeData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let wallet_number = qwikcilver::qwikcilver_wallet_number_from_authorize(&req.request)?;
            Ok(format!(
                "{}Qwikcilver/eGMS.RestApi/api/v2/wallet/{}/REDEEM",
                self.connector_base_url_payments(req),
                urlencoding::encode(wallet_number.peek()),
            ))
        }

        fn get_headers(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let token = self.extract_access_token(req.resource_common_data.access_token.as_ref())?;
            let date = qwikcilver::current_datetime_qwikcilver();
            let txn_id = qwikcilver::derive_transaction_id_from_reference(
                &req.resource_common_data.connector_request_reference_id,
            );
            self.build_authenticated_headers(req, &token, &date, txn_id)
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Qwikcilver,
    curl_request: Json(QwikcilverCancelRedeemBody),
    curl_response: QwikcilverCancelRedeemResponse,
    flow_name: Refund,
    resource_common_data: RefundFlowData,
    flow_request: RefundsData,
    flow_response: RefundsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let wallet_number = qwikcilver::qwikcilver_wallet_number_from_refund(&req.request)?;
            Ok(format!(
                "{}Qwikcilver/eGMS.RestApi/api/v2/wallet/{}/CANCELREDEEM",
                self.connector_base_url_refunds(req),
                urlencoding::encode(wallet_number.peek()),
            ))
        }

        fn get_headers(
            &self,
            req: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let token = self.extract_access_token(req.resource_common_data.access_token.as_ref())?;
            let date = qwikcilver::current_datetime_qwikcilver();
            let txn_id = qwikcilver::derive_transaction_id_from_reference(
                &req.resource_common_data.connector_request_reference_id,
            );
            self.build_authenticated_headers(req, &token, &date, txn_id)
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Qwikcilver,
    curl_request: Json(QwikcilverRechargeRequest),
    curl_response: QwikcilverRechargeResponse,
    flow_name: Recharge,
    resource_common_data: PaymentFlowData,
    flow_request: RechargeRequestData,
    flow_response: RechargeResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<Recharge, PaymentFlowData, RechargeRequestData, RechargeResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let wallet_number = req
                .request
                .connector_payment_method_id
                .as_deref()
                .ok_or_else(|| {
                    IntegrationError::MissingRequiredField {
                        field_name: "connector_payment_method_id",
                        context: qwikcilver::qc_err_ctx(
                            "Qwikcilver Recharge issues a new card against an existing wallet — \
                             we URL-encode the wallet number into the path \
                             `/wallet/{wallet_number}/card`, so we cannot proceed without it.",
                            "Set `connector_payment_method_id` to the wallet number returned by \
                             a prior PaymentMethodService.Create (or any wallet you provisioned \
                             at the connector). Do not rely on `merchant_payment_method_id` for \
                             this — that's the merchant-side reference, not the wallet PAN.",
                        ),
                    }
                })?;
            Ok(format!(
                "{}Qwikcilver/eGMS.RestApi/api/v2/wallet/{}/card",
                self.connector_base_url_payments(req),
                urlencoding::encode(wallet_number),
            ))
        }

        fn get_headers(
            &self,
            req: &RouterDataV2<Recharge, PaymentFlowData, RechargeRequestData, RechargeResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let token = self.extract_access_token(req.resource_common_data.access_token.as_ref())?;
            let date = qwikcilver::current_datetime_qwikcilver();
            let txn_id = qwikcilver::derive_transaction_id_from_reference(
                &req.resource_common_data.connector_request_reference_id,
            );
            self.build_authenticated_headers(req, &token, &date, txn_id)
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Qwikcilver,
    curl_request: Json(QwikcilverCreateWalletRequest),
    curl_response: QwikcilverWalletEnvelope,
    flow_name: CreatePaymentMethod,
    resource_common_data: PaymentFlowData,
    flow_request: CreatePaymentMethodData,
    flow_response: CreatePaymentMethodResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<CreatePaymentMethod, PaymentFlowData, CreatePaymentMethodData, CreatePaymentMethodResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!(
                "{}Qwikcilver/eGMS.RestApi/api/v2/wallet",
                self.connector_base_url_payments(req),
            ))
        }

        fn get_headers(
            &self,
            req: &RouterDataV2<CreatePaymentMethod, PaymentFlowData, CreatePaymentMethodData, CreatePaymentMethodResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let token = self.extract_access_token(req.resource_common_data.access_token.as_ref())?;
            let date = qwikcilver::current_datetime_qwikcilver();
            let txn_id = qwikcilver::derive_transaction_id_from_reference(
                &req.resource_common_data.connector_request_reference_id,
            );
            self.build_authenticated_headers(req, &token, &date, txn_id)
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Qwikcilver,
    curl_request: Json(QwikcilverEmptyBody),
    curl_response: QwikcilverGetWalletResponse,
    flow_name: GetPaymentMethod,
    resource_common_data: PaymentFlowData,
    flow_request: GetPaymentMethodData,
    flow_response: GetPaymentMethodResponseData,
    http_method: Get,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<GetPaymentMethod, PaymentFlowData, GetPaymentMethodData, GetPaymentMethodResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            self.wallet_lookup_url(
                self.connector_base_url_payments(req),
                req.request.connector_payment_method_id.as_deref(),
                req.request
                    .customer
                    .as_ref()
                    .and_then(|c| c.customer_phone_number.as_ref()),
            )
        }

        fn get_headers(
            &self,
            req: &RouterDataV2<GetPaymentMethod, PaymentFlowData, GetPaymentMethodData, GetPaymentMethodResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let token = self.extract_access_token(req.resource_common_data.access_token.as_ref())?;
            let date = qwikcilver::current_datetime_qwikcilver();
            let txn_id = qwikcilver::derive_transaction_id_from_reference(
                &req.resource_common_data.connector_request_reference_id,
            );
            self.build_authenticated_headers(req, &token, &date, txn_id)
        }
    }
);

macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Qwikcilver,
    curl_response: QwikcilverEligibilityResponse,
    flow_name: PaymentMethodEligibility,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentMethodEligibilityData,
    flow_response: PaymentMethodEligibilityResponse,
    http_method: Get,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_url(
            &self,
            req: &RouterDataV2<PaymentMethodEligibility, PaymentFlowData, PaymentMethodEligibilityData, PaymentMethodEligibilityResponse>,
        ) -> CustomResult<String, IntegrationError> {
            // Performs the exact same connector call as `GetPaymentMethod` — eligibility for
            // a Qwikcilver wallet is determined from the same wallet lookup response.
            self.wallet_lookup_url(
                self.connector_base_url_payments(req),
                req.request.connector_payment_method_id.as_deref(),
                req.request
                    .customer
                    .as_ref()
                    .and_then(|c| c.customer_phone_number.as_ref()),
            )
        }

        fn get_headers(
            &self,
            req: &RouterDataV2<PaymentMethodEligibility, PaymentFlowData, PaymentMethodEligibilityData, PaymentMethodEligibilityResponse>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let token = self.extract_access_token(req.resource_common_data.access_token.as_ref())?;
            let date = qwikcilver::current_datetime_qwikcilver();
            let txn_id = qwikcilver::derive_transaction_id_from_reference(
                &req.resource_common_data.connector_request_reference_id,
            );
            self.build_authenticated_headers(req, &token, &date, txn_id)
        }
    }
);

macros::macro_connector_flow_status_impls!(
    connector: Qwikcilver,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        PSync,
        RSync,
    ],
    not_supported: [
        VoidPostRefund,
        Void,
        Capture,
        VoidPC,
        IncrementalAuthorization,
        SetupMandate,
        RepeatPayment,
        PreAuthenticate,
        Authenticate,
        PostAuthenticate,
        ClientAuthenticationToken,
        ServerSessionAuthenticationToken,
        MandateRevoke,
        CreateOrder,
        CreateConnectorCustomer,
        GetConnectorCustomer,
        PaymentMethodToken,
        Accept,
        SubmitEvidence,
        DefendDispute,
    ],
);
