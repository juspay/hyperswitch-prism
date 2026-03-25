use std::collections::HashSet;
use std::str::FromStr;

use common_enums::{AttemptStatus, CaptureMethod, PaymentMethod, PaymentMethodType};
use common_utils::{CustomResult, SecretSerdeValue};
use domain_types::{
    connector_flow,
    connector_types::{
        AcceptDisputeData, AccessTokenRequestData, AccessTokenResponseData, ConnectorCustomerData,
        ConnectorCustomerResponse, ConnectorEnum, ConnectorSpecifications, ConnectorWebhookSecrets,
        DisputeDefendData, DisputeFlowData, DisputeResponseData, DisputeWebhookDetailsResponse,
        EventType, MandateRevokeRequestData, MandateRevokeResponseData, PaymentCreateOrderData,
        PaymentCreateOrderResponse, PaymentFlowData, PaymentMethodTokenResponse,
        PaymentMethodTokenizationData, PaymentVoidData, PaymentsAuthenticateData,
        PaymentsAuthorizeData, PaymentsCancelPostCaptureData, PaymentsCaptureData,
        PaymentsIncrementalAuthorizationData, PaymentsPostAuthenticateData,
        PaymentsPreAuthenticateData, PaymentsResponseData, PaymentsSdkSessionTokenData,
        PaymentsSyncData, RedirectDetailsResponse, RefundFlowData, RefundSyncData,
        RefundWebhookDetailsResponse, RefundsData, RefundsResponseData, RepeatPaymentData,
        RequestDetails, SessionTokenRequestData, SessionTokenResponseData, SetupMandateRequestData,
        SubmitEvidenceData, VerifyWebhookSourceFlowData, WebhookDetailsResponse,
    },
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes},
    payouts::payouts_types::{
        PayoutCreateLinkRequest, PayoutCreateLinkResponse, PayoutCreateRecipientRequest,
        PayoutCreateRecipientResponse, PayoutCreateRequest, PayoutCreateResponse,
        PayoutEnrollDisburseAccountRequest, PayoutEnrollDisburseAccountResponse, PayoutFlowData,
        PayoutGetRequest, PayoutGetResponse, PayoutStageRequest, PayoutStageResponse,
        PayoutTransferRequest, PayoutTransferResponse, PayoutVoidRequest, PayoutVoidResponse,
    },
    router_data::ConnectorSpecificConfig,
    router_request_types::VerifyWebhookSourceRequestData,
    router_response_types::VerifyWebhookSourceResponseData,
    types::{PaymentMethodDataType, PaymentMethodDetails, SupportedPaymentMethods},
};
use error_stack::ResultExt;

use crate::{
    api::ConnectorCommon,
    connector_integration_v2::ConnectorIntegrationV2,
    decode::BodyDecoding,
    verification::{ConnectorSourceVerificationSecrets, SourceVerification},
};

pub trait ConnectorServiceTrait<T: PaymentMethodDataTypes>:
    ConnectorCommon
    + ValidationTrait
    + PaymentAuthorizeV2<T>
    + PaymentSyncV2
    + PaymentOrderCreate
    + PaymentSessionToken
    + PaymentAccessToken
    + CreateConnectorCustomer
    + PaymentTokenV2<T>
    + PaymentVoidV2
    + PaymentVoidPostCaptureV2
    + IncomingWebhook
    + RefundV2
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
    + SdkSessionTokenV2
    + PaymentIncrementalAuthorization
    + MandateRevokeV2
    + VerifyWebhookSourceV2
    + VerifyRedirectResponse
    + PayoutCreateV2
    + PayoutTransferV2
    + PayoutGetV2
    + PayoutVoidV2
    + PayoutStageV2
    + PayoutCreateLinkV2
    + PayoutCreateRecipientV2
    + PayoutEnrollDisburseAccountV2
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

pub type BoxedConnector<T> = Box<&'static (dyn ConnectorServiceTrait<T> + Sync)>;

pub trait ValidationTrait: ConnectorCommon {
    fn should_do_order_create(&self) -> bool {
        false
    }

    fn should_do_session_token(&self) -> bool {
        false
    }

    fn should_do_access_token(&self, _payment_method: Option<PaymentMethod>) -> bool {
        false
    }

    fn should_create_connector_customer(&self) -> bool {
        false
    }

    fn should_do_payment_method_token(
        &self,
        _payment_method: PaymentMethod,
        _payment_method_type: Option<PaymentMethodType>,
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

pub trait PaymentSessionToken:
    ConnectorIntegrationV2<
    connector_flow::CreateSessionToken,
    PaymentFlowData,
    SessionTokenRequestData,
    SessionTokenResponseData,
>
{
}

pub trait SdkSessionTokenV2:
    ConnectorIntegrationV2<
    connector_flow::SdkSessionToken,
    PaymentFlowData,
    PaymentsSdkSessionTokenData,
    PaymentsResponseData,
>
{
}

pub trait PaymentAccessToken:
    ConnectorIntegrationV2<
    connector_flow::CreateAccessToken,
    PaymentFlowData,
    AccessTokenRequestData,
    AccessTokenResponseData,
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

pub trait PaymentTokenV2<T: PaymentMethodDataTypes>:
    ConnectorIntegrationV2<
    connector_flow::PaymentMethodToken,
    PaymentFlowData,
    PaymentMethodTokenizationData<T>,
    PaymentMethodTokenResponse,
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

impl<T> PayoutCreateV2 for T where T: ConnectorCommon + Sync + Send + 'static {}

impl<T>
    ConnectorIntegrationV2<
        connector_flow::PayoutCreate,
        PayoutFlowData,
        PayoutCreateRequest,
        PayoutCreateResponse,
    > for T
where
    T: ConnectorCommon + Sync + Send + 'static,
{
}

pub trait IncomingWebhook {
    fn verify_webhook_source(
        &self,
        _request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<domain_types::errors::ConnectorError>> {
        Ok(false)
    }

    /// fn get_webhook_source_verification_signature
    fn get_webhook_source_verification_signature(
        &self,
        _request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<domain_types::errors::ConnectorError>> {
        Ok(Vec::new())
    }

    /// fn get_webhook_source_verification_message
    fn get_webhook_source_verification_message(
        &self,
        _request: &RequestDetails,
        _connector_webhook_secret: &ConnectorWebhookSecrets,
    ) -> Result<Vec<u8>, error_stack::Report<domain_types::errors::ConnectorError>> {
        Ok(Vec::new())
    }

    fn get_event_type(
        &self,
        _request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<EventType, error_stack::Report<domain_types::errors::ConnectorError>> {
        Err(
            domain_types::errors::ConnectorError::NotImplemented("get_event_type".to_string())
                .into(),
        )
    }

    fn process_payment_webhook(
        &self,
        _request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<WebhookDetailsResponse, error_stack::Report<domain_types::errors::ConnectorError>>
    {
        Err(domain_types::errors::ConnectorError::NotImplemented(
            "process_payment_webhook".to_string(),
        )
        .into())
    }

    fn process_refund_webhook(
        &self,
        _request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<
        RefundWebhookDetailsResponse,
        error_stack::Report<domain_types::errors::ConnectorError>,
    > {
        Err(domain_types::errors::ConnectorError::NotImplemented(
            "process_refund_webhook".to_string(),
        )
        .into())
    }
    fn process_dispute_webhook(
        &self,
        _request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<
        DisputeWebhookDetailsResponse,
        error_stack::Report<domain_types::errors::ConnectorError>,
    > {
        Err(domain_types::errors::ConnectorError::NotImplemented(
            "process_dispute_webhook".to_string(),
        )
        .into())
    }

    /// fn get_webhook_resource_object
    fn get_webhook_resource_object(
        &self,
        _request: RequestDetails,
    ) -> Result<
        Box<dyn hyperswitch_masking::ErasedMaskSerialize>,
        error_stack::Report<domain_types::errors::ConnectorError>,
    > {
        Err(domain_types::errors::ConnectorError::NotImplemented(
            "get_webhook_resource_object".to_string(),
        )
        .into())
    }
}

pub trait VerifyRedirectResponse: SourceVerification + BodyDecoding {
    /// fn decode_redirect_response_body
    fn decode_redirect_response_body(
        &self,
        request: &RequestDetails,
        secrets: Option<ConnectorSourceVerificationSecrets>,
    ) -> CustomResult<Vec<u8>, domain_types::errors::ConnectorError> {
        self.decode(secrets, &request.body)
    }

    fn verify_redirect_response_source(
        &self,
        request: &RequestDetails,
        secrets: Option<ConnectorSourceVerificationSecrets>,
    ) -> CustomResult<bool, domain_types::errors::ConnectorError> {
        let connector_source_verifacation_secrets =
            secrets.ok_or(domain_types::errors::ConnectorError::MissingRequiredField {
                field_name: "redirect response secrets",
            })?;

        self.verify(connector_source_verifacation_secrets, &request.body)
    }

    fn process_redirect_response(
        &self,
        _request: &RequestDetails,
    ) -> CustomResult<RedirectDetailsResponse, domain_types::errors::ConnectorError> {
        Err(domain_types::errors::ConnectorError::NotImplemented(
            "process_redirect_response".to_string(),
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
    ) -> CustomResult<(), domain_types::errors::ConnectorError> {
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
            Err(domain_types::errors::ConnectorError::NotSupported {
                message: capture_method.to_string(),
                connector: self.id(),
            }
            .into())
        }
    }

    /// fn validate_mandate_payment
    fn validate_mandate_payment(
        &self,
        pm_type: Option<PaymentMethodType>,
        _pm_data: PaymentMethodData<domain_types::payment_method_data::DefaultPCIHolder>,
    ) -> CustomResult<(), domain_types::errors::ConnectorError> {
        let connector = self.id();
        match pm_type {
            Some(pm_type) => Err(domain_types::errors::ConnectorError::NotSupported {
                message: format!("{pm_type} mandate payment"),
                connector,
            }
            .into()),
            None => Err(domain_types::errors::ConnectorError::NotSupported {
                message: " mandate payment".to_string(),
                connector,
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
    ) -> CustomResult<(), domain_types::errors::ConnectorError> {
        data.connector_transaction_id
            .get_connector_transaction_id()
            .change_context(domain_types::errors::ConnectorError::MissingConnectorTransactionID)
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
) -> CustomResult<Option<PaymentMethodDetails>, domain_types::errors::ConnectorError> {
    let payment_method_details =
        supported_payment_method
            .get(&payment_method)
            .ok_or_else(|| domain_types::errors::ConnectorError::NotSupported {
                message: payment_method.to_string(),
                connector,
            })?;

    payment_method_type
        .map(|pmt| {
            payment_method_details.get(&pmt).cloned().ok_or_else(|| {
                domain_types::errors::ConnectorError::NotSupported {
                    message: format!("{payment_method} {pmt}"),
                    connector,
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
) -> Result<(), error_stack::Report<domain_types::errors::ConnectorError>> {
    if mandate_implemented_pmds.contains(&PaymentMethodDataType::from(selected_pmd.clone())) {
        Ok(())
    } else {
        match payment_method_type {
            Some(pm_type) => Err(domain_types::errors::ConnectorError::NotSupported {
                message: format!("{pm_type} mandate payment"),
                connector,
            }
            .into()),
            None => Err(domain_types::errors::ConnectorError::NotSupported {
                message: "mandate payment".to_string(),
                connector,
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

impl<T> PayoutTransferV2 for T where T: ConnectorCommon + Sync + Send + 'static {}

impl<T>
    ConnectorIntegrationV2<
        connector_flow::PayoutTransfer,
        PayoutFlowData,
        PayoutTransferRequest,
        PayoutTransferResponse,
    > for T
where
    T: ConnectorCommon + Sync + Send + 'static,
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

impl<T> PayoutGetV2 for T where T: ConnectorCommon + Sync + Send + 'static {}

impl<T>
    ConnectorIntegrationV2<
        connector_flow::PayoutGet,
        PayoutFlowData,
        PayoutGetRequest,
        PayoutGetResponse,
    > for T
where
    T: ConnectorCommon + Sync + Send + 'static,
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

impl<T> PayoutVoidV2 for T where T: ConnectorCommon + Sync + Send + 'static {}

impl<T>
    ConnectorIntegrationV2<
        connector_flow::PayoutVoid,
        PayoutFlowData,
        PayoutVoidRequest,
        PayoutVoidResponse,
    > for T
where
    T: ConnectorCommon + Sync + Send + 'static,
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

impl<T> PayoutStageV2 for T where T: ConnectorCommon + Sync + Send + 'static {}

impl<T>
    ConnectorIntegrationV2<
        connector_flow::PayoutStage,
        PayoutFlowData,
        PayoutStageRequest,
        PayoutStageResponse,
    > for T
where
    T: ConnectorCommon + Sync + Send + 'static,
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

impl<T> PayoutCreateLinkV2 for T where T: ConnectorCommon + Sync + Send + 'static {}

impl<T>
    ConnectorIntegrationV2<
        connector_flow::PayoutCreateLink,
        PayoutFlowData,
        PayoutCreateLinkRequest,
        PayoutCreateLinkResponse,
    > for T
where
    T: ConnectorCommon + Sync + Send + 'static,
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

impl<T> PayoutCreateRecipientV2 for T where T: ConnectorCommon + Sync + Send + 'static {}

impl<T>
    ConnectorIntegrationV2<
        connector_flow::PayoutCreateRecipient,
        PayoutFlowData,
        PayoutCreateRecipientRequest,
        PayoutCreateRecipientResponse,
    > for T
where
    T: ConnectorCommon + Sync + Send + 'static,
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

impl<T> PayoutEnrollDisburseAccountV2 for T where T: ConnectorCommon + Sync + Send + 'static {}

impl<T>
    ConnectorIntegrationV2<
        connector_flow::PayoutEnrollDisburseAccount,
        PayoutFlowData,
        PayoutEnrollDisburseAccountRequest,
        PayoutEnrollDisburseAccountResponse,
    > for T
where
    T: ConnectorCommon + Sync + Send + 'static,
{
}
