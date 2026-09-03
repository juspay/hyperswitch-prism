use std::collections::HashSet;
use std::str::FromStr;

use common_enums::{AttemptStatus, CaptureMethod, PaymentMethod, PaymentMethodType};
use common_utils::{CustomResult, SecretSerdeValue};
pub use domain_types::connector_types::WebhookIntegrityCheck;
pub use domain_types::flow_status::ConnectorTerminalMapping;
use domain_types::{
    connector_flow,
    connector_types::{
        AcceptDisputeData, ClientAuthenticationTokenRequestData, ConnectorCustomerData,
        ConnectorCustomerResponse, ConnectorEnum, ConnectorSpecifications, ConnectorWebhookSecrets,
        CreatePaymentMethodData, CreatePaymentMethodResponseData, DisputeDefendData,
        DisputeFlowData, DisputeResponseData, DisputeWebhookDetailsResponse, EventType,
        GetPaymentMethodData, GetPaymentMethodResponseData, MandateRevokeRequestData,
        MandateRevokeResponseData, PaymentCreateOrderData, PaymentCreateOrderResponse,
        PaymentFlowData, PaymentMethodEligibilityData, PaymentMethodEligibilityResponse,
        PaymentMethodTokenResponse, PaymentMethodTokenizationData, PaymentVoidData,
        PaymentsAuthenticateData, PaymentsAuthorizeData, PaymentsCancelPostCaptureData,
        PaymentsCaptureData, PaymentsIncrementalAuthorizationData, PaymentsPostAuthenticateData,
        PaymentsPreAuthenticateData, PaymentsResponseData, PaymentsSyncData, RechargeRequestData,
        RechargeResponseData, RedirectDetailsResponse, RefreshPaymentMethodData,
        RefreshPaymentMethodFlowData, RefreshPaymentMethodResponseData, RefundFlowData,
        RefundSyncData, RefundVoidPostRefundData, RefundWebhookDetailsResponse, RefundsData,
        RefundsResponseData, RepeatPaymentData, RequestDetails,
        ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData,
        ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData,
        SetupMandateRequestData, SubmitEvidenceData, VerifyWebhookSourceFlowData,
        WebhookDetailsResponse, WebhookResourceReference,
    },
    errors::WebhookError,
    frm::frm_types::{
        FrmChargebackReceivedRequest, FrmChargebackReceivedResponse, FrmFlowData,
        FrmPaymentOutcomeRequest, FrmPaymentOutcomeResponse, FrmRefundProcessedRequest,
        FrmRefundProcessedResponse, PostRiskCheckRequest, PostRiskCheckResponse,
        PreRiskCheckRequest, PreRiskCheckResponse,
    },
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes},
    payouts::payouts_types::{
        PayoutCreateLinkRequest, PayoutCreateLinkResponse, PayoutCreateRecipientRequest,
        PayoutCreateRecipientResponse, PayoutCreateRequest, PayoutCreateResponse,
        PayoutEligibilityRequest, PayoutEligibilityResponse, PayoutEnrollDisburseAccountRequest,
        PayoutEnrollDisburseAccountResponse, PayoutFlowData, PayoutGetRequest, PayoutGetResponse,
        PayoutStageRequest, PayoutStageResponse, PayoutTransferRequest, PayoutTransferResponse,
        PayoutVoidRequest, PayoutVoidResponse,
    },
    router_data::ConnectorSpecificConfig,
    router_request_types::VerifyWebhookSourceRequestData,
    router_response_types::VerifyWebhookSourceResponseData,
    surcharge::surcharge_types::{
        SurchargeCalculateRequest, SurchargeCalculateResponse, SurchargeFlowData,
        SurchargePaymentSucceededRequest, SurchargePaymentSucceededResponse,
        SurchargeRefundSucceededRequest, SurchargeRefundSucceededResponse,
    },
    types::{PaymentMethodDataType, PaymentMethodDetails, SupportedPaymentMethods},
};
use error_stack::ResultExt;

use crate::{
    api::ConnectorCommon,
    connector_integration_v2::ConnectorIntegrationV2,
    decode::BodyDecoding,
    verification::{ConnectorSourceVerificationSecrets, SourceVerification},
};

#[derive(Debug, Clone, Copy)]
pub enum IncomingWebhookFlowError {
    ResourceNotFound,
    InternalError,
}

/// Represents the next authentication step for composite authorize flow.
/// Connectors implement `next_authentication_step` to guide the flow controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthenticationStep {
    /// Run PreAuthenticate (typically device data collection setup)
    PreAuthenticate,
    /// Run Authenticate (typically challenge initiation)
    Authenticate,
    /// Run PostAuthenticate (typically challenge validation)
    PostAuthenticate,
    /// Stop authentication loop and proceed to Authorize
    Authorize,
}

/// Represents the redirect state for composite authorize flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedirectState {
    InitialRequest,
    RedirectWithParams,
    RedirectWithoutParams,
}

/// Describes which merchant-side identifier a connector uses as its order reference.
/// Some connectors do not distinguish between order and transaction and expect
/// merchant_transaction_id wherever an order identifier is required.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MerchantOrderIdSource {
    /// Connector uses merchant_order_id (default)
    OrderId,
    /// Connector treats order and transaction as the same entity;
    /// merchant_transaction_id is used wherever an order identifier is needed
    TransactionId,
}

pub trait ConnectorServiceTrait<T: PaymentMethodDataTypes>:
    ConnectorCommon
    + ValidationTrait
    + PaymentAuthorizeV2<T>
    + PaymentSyncV2
    + PaymentOrderCreate
    + ServerSessionAuthentication
    + ServerAuthentication
    + CreateConnectorCustomer
    + GetConnectorCustomer
    + PaymentTokenV2<T>
    + RechargeV2
    + CreatePaymentMethodV2
    + GetPaymentMethodV2
    + RefreshPaymentMethodV2<T>
    + PaymentVoidV2
    + PaymentVoidPostCaptureV2
    + IncomingWebhook
    + RefundV2
    + RefundVoidPostRefundV2
    + PaymentCapture
    + SetupMandateV2<T>
    + RepeatPaymentV2<T>
    + AcceptDispute
    + RefundSyncV2
    + DisputeDefend
    + SubmitEvidenceV2
    + PaymentPreAuthenticateV2<T>
    + PaymentAuthenticateV2<T>
    + PaymentPostAuthenticateV2<T>
    + ClientAuthentication
    + PaymentIncrementalAuthorization
    + MandateRevokeV2
    + VerifyWebhookSourceV2
    + VerifyRedirectResponse
    + PaymentMethodEligibilityV2
{
}

pub trait SurchargeServiceTrait:
    ConnectorCommon
    + ValidationTrait
    + SurchargeCalculateV2
    + SurchargePaymentSucceededV2
    + SurchargeRefundSucceededV2
{
}

pub trait FrmServiceTrait:
    ConnectorCommon
    + ValidationTrait
    + ServerAuthentication
    + PreRiskCheckV2
    + PostRiskCheckV2
    + FrmPaymentOutcomeV2
    + FrmRefundProcessedV2
    + FrmChargebackReceivedV2
    + PaymentPreAuthenticateV2<domain_types::payment_method_data::DefaultPCIHolder>
{
}

pub trait PayoutServiceTrait:
    ConnectorCommon
    + ServerAuthentication
    + PayoutCreateV2
    + PayoutTransferV2
    + PayoutGetV2
    + PayoutVoidV2
    + PayoutStageV2
    + PayoutCreateLinkV2
    + PayoutCreateRecipientV2
    + PayoutEnrollDisburseAccountV2
    + PayoutEligibilityV2
{
}

pub trait PaymentVoidV2:
    ConnectorIntegrationV2<connector_flow::Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
}

pub trait PaymentVoidPostCaptureV2:
    ConnectorIntegrationV2<
    connector_flow::VoidPC,
    PaymentFlowData,
    PaymentsCancelPostCaptureData,
    PaymentsResponseData,
>
{
}

pub trait PaymentMethodEligibilityV2:
    ConnectorIntegrationV2<
    connector_flow::PaymentMethodEligibility,
    PaymentFlowData,
    PaymentMethodEligibilityData,
    PaymentMethodEligibilityResponse,
>
{
}

pub trait AuthenticatorServiceTrait<T: PaymentMethodDataTypes>:
    ConnectorCommon + ValidationTrait + ClientAuthentication + PaymentTokenV2<T> + GetPaymentMethodV2
{
}

pub type BoxedConnector<T> = Box<&'static (dyn ConnectorServiceTrait<T> + Sync)>;

pub type BoxedSurchargeConnector = Box<&'static (dyn SurchargeServiceTrait + Sync)>;

pub type BoxedFrmConnector = Box<&'static (dyn FrmServiceTrait + Sync)>;

pub type BoxedPayoutConnector = Box<&'static (dyn PayoutServiceTrait + Sync)>;

pub type BoxedAuthenticatorConnector = Box<
    &'static (dyn AuthenticatorServiceTrait<domain_types::payment_method_data::DefaultPCIHolder>
                  + Sync),
>;

pub trait ValidationTrait: ConnectorCommon {
    fn should_do_order_create(&self) -> bool {
        false
    }

    fn should_do_session_token(
        &self,
        _connector_feature_data: Option<&hyperswitch_masking::Secret<String>>,
    ) -> bool {
        false
    }

    fn should_do_access_token(&self, _payment_method: Option<PaymentMethod>) -> bool {
        false
    }

    fn should_create_connector_customer(&self) -> bool {
        false
    }

    fn should_get_connector_customer(&self) -> bool {
        false
    }

    fn should_do_payment_method_token(
        &self,
        _payment_method: PaymentMethod,
        _payment_method_type: Option<PaymentMethodType>,
        _is_wallet_decrypted_network_token: bool,
    ) -> bool {
        false
    }

    /// Returns true if this connector is in the config set of connectors that require
    /// an external API call for webhook source verification (e.g. PayPal).
    fn requires_external_webhook_verification(
        &self,
        connectors_requiring_external_verification: Option<&HashSet<ConnectorEnum>>,
    ) -> bool {
        connectors_requiring_external_verification
            .map(|connector_set| {
                ConnectorEnum::from_str(self.id())
                    .ok()
                    .map(|connector_enum| connector_set.contains(&connector_enum))
                    .unwrap_or(false)
            })
            .unwrap_or(false)
    }

    /// Returns the next authentication step for composite authorize flow.
    /// The connector examines the current state and returns which step should execute next.
    fn next_authentication_step(
        &self,
        _auth_type: common_enums::AuthenticationType,
        _payment_method: PaymentMethod,
        _redirect_state: RedirectState,
        _completed_step: Option<AuthenticationStep>,
    ) -> AuthenticationStep {
        AuthenticationStep::Authorize
    }

    /// Returns whether this connector requires Authorize to be called
    /// after VerifyRedirectResponse in the composite flow.
    fn requires_authorize_post_redirect(&self) -> bool {
        false
    }

    /// Returns which merchant identifier this connector uses as its order reference.
    /// Connectors that treat order and transaction as the same entity should return
    /// `MerchantOrderIdSource::TransactionId`.
    fn merchant_order_id_source(&self) -> MerchantOrderIdSource {
        MerchantOrderIdSource::OrderId
    }
}

pub trait PaymentOrderCreate:
    ConnectorIntegrationV2<
    connector_flow::CreateOrder,
    PaymentFlowData,
    PaymentCreateOrderData,
    PaymentCreateOrderResponse,
>
{
}

pub trait ServerSessionAuthentication:
    ConnectorIntegrationV2<
    connector_flow::ServerSessionAuthenticationToken,
    MerchantAuthenticationFlowData,
    ServerSessionAuthenticationTokenRequestData,
    ServerSessionAuthenticationTokenResponseData,
>
{
}

pub trait ClientAuthentication:
    ConnectorIntegrationV2<
    connector_flow::ClientAuthenticationToken,
    MerchantAuthenticationFlowData,
    ClientAuthenticationTokenRequestData,
    PaymentsResponseData,
>
{
}

pub trait ServerAuthentication:
    ConnectorIntegrationV2<
    connector_flow::ServerAuthenticationToken,
    MerchantAuthenticationFlowData,
    ServerAuthenticationTokenRequestData,
    ServerAuthenticationTokenResponseData,
>
{
}

pub trait CreateConnectorCustomer:
    ConnectorIntegrationV2<
    connector_flow::CreateConnectorCustomer,
    PaymentFlowData,
    ConnectorCustomerData,
    ConnectorCustomerResponse,
>
{
}

pub trait GetConnectorCustomer:
    ConnectorIntegrationV2<
    connector_flow::GetConnectorCustomer,
    PaymentFlowData,
    ConnectorCustomerData,
    ConnectorCustomerResponse,
>
{
}

pub trait PaymentTokenV2<T: PaymentMethodDataTypes>:
    ConnectorIntegrationV2<
    connector_flow::PaymentMethodToken,
    PaymentFlowData,
    PaymentMethodTokenizationData<T>,
    PaymentMethodTokenResponse,
>
{
}

pub trait RechargeV2:
    ConnectorIntegrationV2<
    connector_flow::Recharge,
    PaymentFlowData,
    RechargeRequestData,
    RechargeResponseData,
>
{
}

pub trait CreatePaymentMethodV2:
    ConnectorIntegrationV2<
    connector_flow::CreatePaymentMethod,
    PaymentFlowData,
    CreatePaymentMethodData,
    CreatePaymentMethodResponseData,
>
{
}

pub trait GetPaymentMethodV2:
    ConnectorIntegrationV2<
    connector_flow::GetPaymentMethod,
    PaymentFlowData,
    GetPaymentMethodData,
    GetPaymentMethodResponseData,
>
{
}

pub trait RefreshPaymentMethodV2<T: PaymentMethodDataTypes>:
    ConnectorIntegrationV2<
    connector_flow::RefreshPaymentMethod,
    RefreshPaymentMethodFlowData,
    RefreshPaymentMethodData<T>,
    RefreshPaymentMethodResponseData,
>
{
}

pub trait PaymentAuthorizeV2<T: PaymentMethodDataTypes>:
    ConnectorIntegrationV2<
    connector_flow::Authorize,
    PaymentFlowData,
    PaymentsAuthorizeData<T>,
    PaymentsResponseData,
>
{
}

pub trait PaymentSyncV2:
    ConnectorIntegrationV2<
    connector_flow::PSync,
    PaymentFlowData,
    PaymentsSyncData,
    PaymentsResponseData,
>
{
}

pub trait RefundV2:
    ConnectorIntegrationV2<connector_flow::Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
}

pub trait RefundVoidPostRefundV2:
    ConnectorIntegrationV2<
    connector_flow::VoidPostRefund,
    RefundFlowData,
    RefundVoidPostRefundData,
    RefundsResponseData,
>
{
}

pub trait RefundSyncV2:
    ConnectorIntegrationV2<connector_flow::RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
}

pub trait PaymentCapture:
    ConnectorIntegrationV2<
    connector_flow::Capture,
    PaymentFlowData,
    PaymentsCaptureData,
    PaymentsResponseData,
>
{
}

pub trait SetupMandateV2<T: PaymentMethodDataTypes>:
    ConnectorIntegrationV2<
    connector_flow::SetupMandate,
    PaymentFlowData,
    SetupMandateRequestData<T>,
    PaymentsResponseData,
>
{
}

pub trait RepeatPaymentV2<T: PaymentMethodDataTypes>:
    ConnectorIntegrationV2<
    connector_flow::RepeatPayment,
    PaymentFlowData,
    RepeatPaymentData<T>,
    PaymentsResponseData,
>
{
}

pub trait MandateRevokeV2:
    ConnectorIntegrationV2<
    connector_flow::MandateRevoke,
    PaymentFlowData,
    MandateRevokeRequestData,
    MandateRevokeResponseData,
>
{
}

pub trait AcceptDispute:
    ConnectorIntegrationV2<
    connector_flow::Accept,
    DisputeFlowData,
    AcceptDisputeData,
    DisputeResponseData,
>
{
}

pub trait SubmitEvidenceV2:
    ConnectorIntegrationV2<
    connector_flow::SubmitEvidence,
    DisputeFlowData,
    SubmitEvidenceData,
    DisputeResponseData,
>
{
}

pub trait DisputeDefend:
    ConnectorIntegrationV2<
    connector_flow::DefendDispute,
    DisputeFlowData,
    DisputeDefendData,
    DisputeResponseData,
>
{
}

pub trait PaymentPreAuthenticateV2<T: PaymentMethodDataTypes>:
    ConnectorIntegrationV2<
    connector_flow::PreAuthenticate,
    PaymentFlowData,
    PaymentsPreAuthenticateData<T>,
    PaymentsResponseData,
>
{
}

pub trait PaymentAuthenticateV2<T: PaymentMethodDataTypes>:
    ConnectorIntegrationV2<
    connector_flow::Authenticate,
    PaymentFlowData,
    PaymentsAuthenticateData<T>,
    PaymentsResponseData,
>
{
}

pub trait PaymentPostAuthenticateV2<T: PaymentMethodDataTypes>:
    ConnectorIntegrationV2<
    connector_flow::PostAuthenticate,
    PaymentFlowData,
    PaymentsPostAuthenticateData<T>,
    PaymentsResponseData,
>
{
}

pub trait PaymentIncrementalAuthorization:
    ConnectorIntegrationV2<
    connector_flow::IncrementalAuthorization,
    PaymentFlowData,
    PaymentsIncrementalAuthorizationData,
    PaymentsResponseData,
>
{
}

pub trait VerifyWebhookSourceV2:
    ConnectorIntegrationV2<
    connector_flow::VerifyWebhookSource,
    VerifyWebhookSourceFlowData,
    VerifyWebhookSourceRequestData,
    VerifyWebhookSourceResponseData,
>
{
}

pub trait PayoutCreateV2:
    ConnectorIntegrationV2<
    connector_flow::PayoutCreate,
    PayoutFlowData,
    PayoutCreateRequest,
    PayoutCreateResponse,
>
{
}

pub trait IncomingWebhook {
    fn verify_webhook_source(
        &self,
        _request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<WebhookError>> {
        Ok(false)
    }

    fn get_webhook_integrity_checks(&self) -> Vec<WebhookIntegrityCheck> {
        vec![]
    }

    /// fn get_webhook_source_verification_signature
    fn get_webhook_source_verification_signature(
        &self,
        _request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<WebhookError>> {
        Ok(Vec::new())
    }

    /// fn get_webhook_source_verification_message
    fn get_webhook_source_verification_message(
        &self,
        _request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<WebhookError>> {
        Ok(Vec::new())
    }

    fn get_event_type(
        &self,
        _request: RequestDetails,
    ) -> Result<EventType, error_stack::Report<WebhookError>> {
        Err(WebhookError::WebhooksNotImplemented {
            operation: "get_event_type",
        }
        .into())
    }

    fn get_webhook_event_reference(
        &self,
        _request: RequestDetails,
    ) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>> {
        Ok(None)
    }

    fn process_payment_webhook(
        &self,
        _request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
        _event_context: Option<domain_types::connector_types::EventContext>,
    ) -> Result<WebhookDetailsResponse, error_stack::Report<WebhookError>> {
        Err(WebhookError::WebhooksNotImplemented {
            operation: "process_payment_webhook",
        }
        .into())
    }

    fn process_refund_webhook(
        &self,
        _request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<RefundWebhookDetailsResponse, error_stack::Report<WebhookError>> {
        Err(WebhookError::WebhooksNotImplemented {
            operation: "process_refund_webhook",
        }
        .into())
    }
    fn process_dispute_webhook(
        &self,
        _request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<DisputeWebhookDetailsResponse, error_stack::Report<WebhookError>> {
        Err(WebhookError::WebhooksNotImplemented {
            operation: "process_dispute_webhook",
        }
        .into())
    }

    /// fn get_webhook_resource_object
    fn get_webhook_resource_object(
        &self,
        _request: RequestDetails,
    ) -> Result<Box<dyn hyperswitch_masking::ErasedMaskSerialize>, error_stack::Report<WebhookError>>
    {
        Err(WebhookError::WebhooksNotImplemented {
            operation: "get_webhook_resource_object",
        }
        .into())
    }

    /// A minimal, structurally valid webhook body for this connector.
    ///
    /// Used by the field-probe to verify that webhook handling is implemented
    fn sample_webhook_body(&self) -> &'static [u8] {
        b"{}"
    }

    /// fn get_webhook_api_response
    ///
    /// This is used by callers to decide what HTTP response
    /// should be sent back to the connector for webhook acknowledgement.
    fn get_webhook_api_response(
        &self,
        _request: RequestDetails,
        _error_kind: Option<IncomingWebhookFlowError>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<crate::api::EventAckResponse, error_stack::Report<WebhookError>> {
        Ok(crate::api::EventAckResponse {
            status_code: 200,
            headers: vec![],
            body: None,
        })
    }
}

pub trait VerifyRedirectResponse: SourceVerification + BodyDecoding {
    /// fn decode_redirect_response_body
    fn decode_redirect_response_body(
        &self,
        request: &RequestDetails,
        secrets: Option<ConnectorSourceVerificationSecrets>,
    ) -> CustomResult<Vec<u8>, domain_types::errors::IntegrationError> {
        self.decode(secrets, &request.body)
    }

    fn verify_redirect_response_source(
        &self,
        request: &RequestDetails,
        secrets: Option<ConnectorSourceVerificationSecrets>,
    ) -> CustomResult<bool, domain_types::errors::IntegrationError> {
        let connector_source_verifacation_secrets = secrets.ok_or(
            domain_types::errors::IntegrationError::MissingRequiredField {
                field_name: "redirect response secrets",
                context: Default::default(),
            },
        )?;

        self.verify(connector_source_verifacation_secrets, &request.body)
    }

    fn process_redirect_response(
        &self,
        _request: &RequestDetails,
        _connector_feature_data: Option<&hyperswitch_masking::Secret<String>>,
    ) -> CustomResult<RedirectDetailsResponse, domain_types::errors::IntegrationError> {
        Err(domain_types::errors::IntegrationError::NotImplemented(
            "process_redirect_response".to_string(),
            Default::default(),
        )
        .into())
    }
}

/// trait ConnectorValidation
pub trait ConnectorValidation: ConnectorCommon + ConnectorSpecifications {
    /// Validate, the payment request against the connector supported features
    fn validate_connector_against_payment_request(
        &self,
        capture_method: Option<CaptureMethod>,
        payment_method: PaymentMethod,
        pmt: Option<PaymentMethodType>,
    ) -> CustomResult<(), domain_types::errors::IntegrationError> {
        let capture_method = capture_method.unwrap_or_default();
        let is_default_capture_method = [CaptureMethod::Automatic].contains(&capture_method);
        let is_feature_supported = match self.get_supported_payment_methods() {
            Some(supported_payment_methods) => {
                let connector_payment_method_type_info = get_connector_payment_method_type_info(
                    supported_payment_methods,
                    payment_method,
                    pmt,
                    self.id(),
                )?;

                connector_payment_method_type_info
                    .map(|payment_method_type_info| {
                        payment_method_type_info
                            .supported_capture_methods
                            .contains(&capture_method)
                    })
                    .unwrap_or(true)
            }
            None => is_default_capture_method,
        };

        if is_feature_supported {
            Ok(())
        } else {
            Err(domain_types::errors::IntegrationError::NotSupported {
                message: capture_method.to_string(),
                connector: self.id(),
                context: Default::default(),
            }
            .into())
        }
    }

    /// fn validate_mandate_payment
    fn validate_mandate_payment(
        &self,
        pm_type: Option<PaymentMethodType>,
        _pm_data: PaymentMethodData<domain_types::payment_method_data::DefaultPCIHolder>,
    ) -> CustomResult<(), domain_types::errors::IntegrationError> {
        let connector = self.id();
        match pm_type {
            Some(pm_type) => Err(domain_types::errors::IntegrationError::NotSupported {
                message: format!("{pm_type} mandate payment"),
                connector,
                context: Default::default(),
            }
            .into()),
            None => Err(domain_types::errors::IntegrationError::NotSupported {
                message: " mandate payment".to_string(),
                connector,
                context: Default::default(),
            }
            .into()),
        }
    }

    /// fn validate_psync_reference_id
    fn validate_psync_reference_id(
        &self,
        data: &PaymentsSyncData,
        _is_three_ds: bool,
        _status: AttemptStatus,
        _connector_meta_data: Option<SecretSerdeValue>,
    ) -> CustomResult<(), domain_types::errors::IntegrationError> {
        data.connector_transaction_id
            .get_connector_transaction_id()
            .change_context(
                domain_types::errors::IntegrationError::MissingConnectorTransactionID {
                    context: Default::default(),
                },
            )
            .map(|_| ())
    }

    /// fn is_webhook_source_verification_mandatory
    fn is_webhook_source_verification_mandatory(&self) -> bool {
        false
    }
}

fn get_connector_payment_method_type_info(
    supported_payment_method: &SupportedPaymentMethods,
    payment_method: PaymentMethod,
    payment_method_type: Option<PaymentMethodType>,
    connector: &'static str,
) -> CustomResult<Option<PaymentMethodDetails>, domain_types::errors::IntegrationError> {
    let payment_method_details =
        supported_payment_method
            .get(&payment_method)
            .ok_or_else(|| domain_types::errors::IntegrationError::NotSupported {
                message: payment_method.to_string(),
                connector,
                context: Default::default(),
            })?;

    payment_method_type
        .map(|pmt| {
            payment_method_details.get(&pmt).cloned().ok_or_else(|| {
                domain_types::errors::IntegrationError::NotSupported {
                    message: format!("{payment_method} {pmt}"),
                    connector,
                    context: Default::default(),
                }
                .into()
            })
        })
        .transpose()
}

pub fn is_mandate_supported<T: PaymentMethodDataTypes>(
    selected_pmd: PaymentMethodData<T>,
    payment_method_type: Option<PaymentMethodType>,
    mandate_implemented_pmds: HashSet<PaymentMethodDataType>,
    connector: &'static str,
) -> Result<(), error_stack::Report<domain_types::errors::IntegrationError>> {
    if mandate_implemented_pmds.contains(&PaymentMethodDataType::from(selected_pmd.clone())) {
        Ok(())
    } else {
        match payment_method_type {
            Some(pm_type) => Err(domain_types::errors::IntegrationError::NotSupported {
                message: format!("{pm_type} mandate payment"),
                connector,
                context: Default::default(),
            }
            .into()),
            None => Err(domain_types::errors::IntegrationError::NotSupported {
                message: "mandate payment".to_string(),
                connector,
                context: Default::default(),
            }
            .into()),
        }
    }
}

// --- GENERATED PAYOUT TRAITS ---

pub trait PayoutTransferV2:
    ConnectorIntegrationV2<
    connector_flow::PayoutTransfer,
    PayoutFlowData,
    PayoutTransferRequest,
    PayoutTransferResponse,
>
{
}

pub trait PayoutGetV2:
    ConnectorIntegrationV2<
    connector_flow::PayoutGet,
    PayoutFlowData,
    PayoutGetRequest,
    PayoutGetResponse,
>
{
}

pub trait PayoutVoidV2:
    ConnectorIntegrationV2<
    connector_flow::PayoutVoid,
    PayoutFlowData,
    PayoutVoidRequest,
    PayoutVoidResponse,
>
{
}

pub trait PayoutStageV2:
    ConnectorIntegrationV2<
    connector_flow::PayoutStage,
    PayoutFlowData,
    PayoutStageRequest,
    PayoutStageResponse,
>
{
}

pub trait PayoutCreateLinkV2:
    ConnectorIntegrationV2<
    connector_flow::PayoutCreateLink,
    PayoutFlowData,
    PayoutCreateLinkRequest,
    PayoutCreateLinkResponse,
>
{
}

pub trait PayoutCreateRecipientV2:
    ConnectorIntegrationV2<
    connector_flow::PayoutCreateRecipient,
    PayoutFlowData,
    PayoutCreateRecipientRequest,
    PayoutCreateRecipientResponse,
>
{
}

pub trait PayoutEnrollDisburseAccountV2:
    ConnectorIntegrationV2<
    connector_flow::PayoutEnrollDisburseAccount,
    PayoutFlowData,
    PayoutEnrollDisburseAccountRequest,
    PayoutEnrollDisburseAccountResponse,
>
{
}

pub trait PayoutEligibilityV2:
    ConnectorIntegrationV2<
    connector_flow::PayoutEligibility,
    PayoutFlowData,
    PayoutEligibilityRequest,
    PayoutEligibilityResponse,
>
{
}

pub trait SurchargeCalculateV2:
    ConnectorIntegrationV2<
    connector_flow::SurchargeCalculate,
    SurchargeFlowData,
    SurchargeCalculateRequest,
    SurchargeCalculateResponse,
>
{
}

pub trait SurchargePaymentSucceededV2:
    ConnectorIntegrationV2<
    connector_flow::SurchargePaymentSucceeded,
    SurchargeFlowData,
    SurchargePaymentSucceededRequest,
    SurchargePaymentSucceededResponse,
>
{
}

pub trait SurchargeRefundSucceededV2:
    ConnectorIntegrationV2<
    connector_flow::SurchargeRefundSucceeded,
    SurchargeFlowData,
    SurchargeRefundSucceededRequest,
    SurchargeRefundSucceededResponse,
>
{
}

pub trait PreRiskCheckV2:
    ConnectorIntegrationV2<
    connector_flow::PreRiskCheck,
    FrmFlowData,
    PreRiskCheckRequest,
    PreRiskCheckResponse,
>
{
}

pub trait PostRiskCheckV2:
    ConnectorIntegrationV2<
    connector_flow::PostRiskCheck,
    FrmFlowData,
    PostRiskCheckRequest,
    PostRiskCheckResponse,
>
{
}

pub trait FrmPaymentOutcomeV2:
    ConnectorIntegrationV2<
    connector_flow::FrmPaymentOutcome,
    FrmFlowData,
    FrmPaymentOutcomeRequest,
    FrmPaymentOutcomeResponse,
>
{
}

pub trait FrmRefundProcessedV2:
    ConnectorIntegrationV2<
    connector_flow::FrmRefundProcessed,
    FrmFlowData,
    FrmRefundProcessedRequest,
    FrmRefundProcessedResponse,
>
{
}

pub trait FrmChargebackReceivedV2:
    ConnectorIntegrationV2<
    connector_flow::FrmChargebackReceived,
    FrmFlowData,
    FrmChargebackReceivedRequest,
    FrmChargebackReceivedResponse,
>
{
}
