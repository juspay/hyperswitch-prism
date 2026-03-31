use std::str::FromStr;

use common_enums::{self, CaptureMethod, Currency};
use common_utils::{pii::IpAddress, types::SemanticVersion, Email, MinorUnit};
use error_stack::ResultExt;
use hyperswitch_masking::Secret;
use serde::Serialize;

use crate::utils::ForeignFrom;
use grpc_api_types::payments;

use crate::{
    connector_types::ConnectorWebhookSecrets,
    errors,
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes},
    utils,
};

pub type Error = error_stack::Report<errors::ConnectorError>;

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct BrowserInformation {
    pub color_depth: Option<u8>,
    pub java_enabled: Option<bool>,
    pub java_script_enabled: Option<bool>,
    pub language: Option<String>,
    pub screen_height: Option<u32>,
    pub screen_width: Option<u32>,
    pub time_zone: Option<i32>,
    pub ip_address: Option<std::net::IpAddr>,
    pub accept_header: Option<String>,
    pub user_agent: Option<String>,
    pub os_type: Option<String>,
    pub os_version: Option<String>,
    pub device_model: Option<String>,
    pub accept_language: Option<String>,
    pub referer: Option<String>,
}

impl BrowserInformation {
    pub fn get_ip_address(&self) -> Result<Secret<String, IpAddress>, Error> {
        let ip_address = self
            .ip_address
            .ok_or_else(utils::missing_field_err("browser_info.ip_address"))?;
        Ok(Secret::new(ip_address.to_string()))
    }
    pub fn get_accept_header(&self) -> Result<String, Error> {
        self.accept_header
            .clone()
            .ok_or_else(utils::missing_field_err("browser_info.accept_header"))
    }
    pub fn get_language(&self) -> Result<String, Error> {
        self.language
            .clone()
            .ok_or_else(utils::missing_field_err("browser_info.language"))
    }
    pub fn get_screen_height(&self) -> Result<u32, Error> {
        self.screen_height
            .ok_or_else(utils::missing_field_err("browser_info.screen_height"))
    }
    pub fn get_screen_width(&self) -> Result<u32, Error> {
        self.screen_width
            .ok_or_else(utils::missing_field_err("browser_info.screen_width"))
    }
    pub fn get_color_depth(&self) -> Result<u8, Error> {
        self.color_depth
            .ok_or_else(utils::missing_field_err("browser_info.color_depth"))
    }
    pub fn get_user_agent(&self) -> Result<String, Error> {
        self.user_agent
            .clone()
            .ok_or_else(utils::missing_field_err("browser_info.user_agent"))
    }
    pub fn get_time_zone(&self) -> Result<i32, Error> {
        self.time_zone
            .ok_or_else(utils::missing_field_err("browser_info.time_zone"))
    }
    pub fn get_java_enabled(&self) -> Result<bool, Error> {
        self.java_enabled
            .ok_or_else(utils::missing_field_err("browser_info.java_enabled"))
    }
    pub fn get_java_script_enabled(&self) -> Result<bool, Error> {
        self.java_script_enabled
            .ok_or_else(utils::missing_field_err("browser_info.java_script_enabled"))
    }
    pub fn get_referer(&self) -> Result<String, Error> {
        self.referer
            .clone()
            .ok_or_else(utils::missing_field_err("browser_info.referer"))
    }
}

#[derive(Debug, Default, Clone)]
pub enum SyncRequestType {
    MultipleCaptureSync,
    #[default]
    SinglePaymentSync,
}

#[derive(Debug, Default, Clone)]
pub struct PaymentsCancelData {
    pub amount: Option<i64>,
    pub currency: Option<Currency>,
    pub connector_transaction_id: String,
    pub cancellation_reason: Option<String>,
    pub connector_meta: Option<serde_json::Value>,
    pub browser_info: Option<BrowserInformation>,
    pub metadata: Option<serde_json::Value>,
    // This metadata is used to store the metadata shared during the payment intent request.

    // minor amount data for amount framework
    pub minor_amount: Option<MinorUnit>,
    pub webhook_url: Option<String>,
    pub capture_method: Option<CaptureMethod>,
}

/// Represents additional network-level parameters for 3DS processing.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NetworkParams {
    /// Parameters specific to Cartes Bancaires network, if applicable.
    pub cartes_bancaires: Option<CartesBancairesParams>,
}

/// Represents network-specific parameters for the Cartes Bancaires 3DS process.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CartesBancairesParams {
    /// The algorithm used to generate the CAVV value.
    pub cavv_algorithm: common_enums::CavvAlgorithm,
    /// Exemption indicator specific to Cartes Bancaires network (e.g., "low_value", "trusted_merchant")
    pub cb_exemption: String,
    /// Cartes Bancaires risk score assigned during 3DS authentication.
    pub cb_score: i32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AuthenticationData {
    pub trans_status: Option<common_enums::TransactionStatus>,
    pub eci: Option<String>,
    pub cavv: Option<Secret<String>>,
    // This is mastercard specific field
    pub ucaf_collection_indicator: Option<String>,
    pub threeds_server_transaction_id: Option<String>,
    pub message_version: Option<SemanticVersion>,
    pub ds_trans_id: Option<String>,
    pub acs_transaction_id: Option<String>,
    pub transaction_id: Option<String>,
    pub network_params: Option<NetworkParams>,
    pub exemption_indicator: Option<common_enums::ExemptionIndicator>,
}

impl TryFrom<payments::AuthenticationData> for AuthenticationData {
    type Error = error_stack::Report<errors::ApplicationErrorResponse>;
    fn try_from(value: payments::AuthenticationData) -> Result<Self, Self::Error> {
        let payments::AuthenticationData {
            eci,
            cavv,
            threeds_server_transaction_id,
            message_version,
            ds_transaction_id,
            trans_status,
            acs_transaction_id,
            connector_transaction_id,
            ucaf_collection_indicator,
            exemption_indicator,
            network_params,
        } = value;
        let message_version = message_version.map(|message_version|{
            SemanticVersion::from_str(&message_version).change_context(errors::ApplicationErrorResponse::BadRequest(errors::ApiError{
                sub_code: "INVALID_SEMANTIC_VERSION_DATA".to_owned(),
                error_identifier: 400,
                error_message: "Invalid semantic version format. Expected format: 'major.minor.patch' (e.g., '2.1.0')".to_string(),
                error_object: Some(serde_json::json!({
                    "field": "message_version",
                    "provided_value": message_version,
                    "expected_format": "major.minor.patch",
                    "examples": ["1.0.0", "2.1.0", "2.2.0"],
                    "validation_rule": "Must be in format X.Y.Z where X, Y, Z are non-negative integers"
                })),
            }))
        }).transpose()?;
        let trans_status = trans_status.map(|trans_status|{
            payments::TransactionStatus::try_from(trans_status).change_context(errors::ApplicationErrorResponse::BadRequest(errors::ApiError{
                sub_code: "INVALID_TRANSACTION_STATUS".to_owned(),
                error_identifier: 400,
                error_message: "Invalid transaction status format. Expected one of the valid 3DS transaction status values".to_string(),
                error_object: Some(serde_json::json!({
                    "field": "transaction_status",
                    "provided_value": trans_status,
                    "expected_values": [
                        "Y (Success)",
                        "N (Failure)", 
                        "U (Verification Not Performed)",
                        "A (Not Verified)",
                        "R (Rejected)",
                        "C (Challenge Required)",
                        "D (Challenge Required - Decoupled Authentication)",
                        "I (Information Only)"
                    ],
                    "validation_rule": "Must be one of the valid 3DS transaction status codes (Y, N, U, A, R, C, D, I)",
                    "description": "Transaction status represents the result of 3D Secure authentication/verification process"
                })),
            }))}).transpose()?.map(common_enums::TransactionStatus::foreign_from);

        Ok(Self {
            ucaf_collection_indicator,
            trans_status,
            eci,
            cavv: cavv.map(Secret::new),
            threeds_server_transaction_id,
            message_version,
            ds_trans_id: ds_transaction_id,
            acs_transaction_id,
            transaction_id: connector_transaction_id,
            network_params: network_params.map(NetworkParams::try_from).transpose()?,
            exemption_indicator: exemption_indicator
                .map(payments::ExemptionIndicator::try_from)
                .transpose()
                .ok()
                .flatten()
                .map(common_enums::ExemptionIndicator::foreign_from),
        })
    }
}

impl TryFrom<payments::NetworkParams> for NetworkParams {
    type Error = error_stack::Report<errors::ApplicationErrorResponse>;
    fn try_from(value: payments::NetworkParams) -> Result<Self, Self::Error> {
        Ok(Self {
            cartes_bancaires: value
                .cartes_bancaires
                .map(CartesBancairesParams::try_from)
                .transpose()?,
        })
    }
}

impl TryFrom<payments::CartesBancairesParams> for CartesBancairesParams {
    type Error = error_stack::Report<errors::ApplicationErrorResponse>;
    fn try_from(value: payments::CartesBancairesParams) -> Result<Self, Self::Error> {
        let cavv_algorithm = payments::CavvAlgorithm::try_from(value.cavv_algorithm)
            .ok()
            .map(common_enums::CavvAlgorithm::foreign_from)
            .ok_or_else(|| {
                errors::ApplicationErrorResponse::BadRequest(errors::ApiError {
                    sub_code: "INVALID_CAVV_ALGORITHM".to_owned(),
                    error_identifier: 400,
                    error_message: "Invalid CAVV algorithm value".to_string(),
                    error_object: Some(serde_json::json!({
                        "field": "cavv_algorithm",
                        "provided_value": value.cavv_algorithm,
                    })),
                })
            })?;
        Ok(Self {
            cavv_algorithm,
            cb_exemption: value.cb_exemption,
            cb_score: value.cb_score,
        })
    }
}

impl ForeignFrom<AuthenticationData> for payments::AuthenticationData {
    fn foreign_from(value: AuthenticationData) -> Self {
        use hyperswitch_masking::ExposeInterface;
        Self {
            ucaf_collection_indicator: value.ucaf_collection_indicator,
            eci: value.eci,
            cavv: value.cavv.map(|cavv| cavv.expose()),
            threeds_server_transaction_id: value.threeds_server_transaction_id,
            message_version: value.message_version.map(|v| v.to_string()),
            ds_transaction_id: value.ds_trans_id,
            trans_status: value
                .trans_status
                .map(payments::TransactionStatus::foreign_from)
                .map(i32::from),
            acs_transaction_id: value.acs_transaction_id,
            connector_transaction_id: value.transaction_id,
            exemption_indicator: value
                .exemption_indicator
                .map(payments::ExemptionIndicator::foreign_from)
                .map(i32::from),
            network_params: value
                .network_params
                .map(payments::NetworkParams::foreign_from),
        }
    }
}

impl ForeignFrom<NetworkParams> for payments::NetworkParams {
    fn foreign_from(value: NetworkParams) -> Self {
        Self {
            cartes_bancaires: value
                .cartes_bancaires
                .map(payments::CartesBancairesParams::foreign_from),
        }
    }
}

impl ForeignFrom<CartesBancairesParams> for payments::CartesBancairesParams {
    fn foreign_from(value: CartesBancairesParams) -> Self {
        Self {
            cavv_algorithm: payments::CavvAlgorithm::foreign_from(value.cavv_algorithm).into(),
            cb_exemption: value.cb_exemption,
            cb_score: value.cb_score,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConnectorCustomerData<T: PaymentMethodDataTypes> {
    pub description: Option<String>,
    pub email: Option<Email>,
    pub phone: Option<Secret<String>>,
    pub name: Option<Secret<String>>,
    pub preprocessing_id: Option<String>,
    pub payment_method_data: Option<PaymentMethodData<T>>,
    // pub split_payments: Option<SplitPaymentsRequest>,
}

impl<T: PaymentMethodDataTypes> ConnectorCustomerData<T> {
    pub fn get_email(&self) -> Result<Email, Error> {
        self.email
            .clone()
            .ok_or_else(utils::missing_field_err("email"))
    }
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AuthoriseIntegrityObject {
    /// Authorise amount
    pub amount: MinorUnit,
    /// Authorise currency
    pub currency: Currency,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CreateOrderIntegrityObject {
    /// Authorise amount
    pub amount: MinorUnit,
    /// Authorise currency
    pub currency: Currency,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SetupMandateIntegrityObject {
    /// Authorise amount
    pub amount: Option<MinorUnit>,
    /// Authorise currency
    pub currency: Currency,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RepeatPaymentIntegrityObject {
    /// Payment amount
    pub amount: i64,
    /// Payment currency
    pub currency: Currency,
    /// Mandate reference
    pub mandate_reference: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PaymentSynIntegrityObject {
    /// Authorise amount
    pub amount: MinorUnit,
    /// Authorise currency
    pub currency: Currency,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PaymentVoidIntegrityObject {
    pub connector_transaction_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PaymentVoidPostCaptureIntegrityObject {
    pub connector_transaction_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RefundIntegrityObject {
    pub refund_amount: MinorUnit,
    pub currency: Currency,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CaptureIntegrityObject {
    pub amount_to_capture: MinorUnit,
    pub currency: Currency,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AcceptDisputeIntegrityObject {
    pub connector_dispute_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DefendDisputeIntegrityObject {
    pub connector_dispute_id: String,
    pub defense_reason_code: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RefundSyncIntegrityObject {
    pub connector_transaction_id: String,
    pub connector_refund_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SubmitEvidenceIntegrityObject {
    pub connector_dispute_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SessionTokenIntegrityObject {
    pub amount: MinorUnit,
    pub currency: Currency,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AccessTokenIntegrityObject {
    pub grant_type: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CreateConnectorCustomerIntegrityObject {
    pub customer_id: Option<Secret<String>>,
    pub email: Option<Secret<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PaymentMethodTokenIntegrityObject {
    pub amount: MinorUnit,
    pub currency: Currency,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PreAuthenticateIntegrityObject {
    pub amount: MinorUnit,
    pub currency: Currency,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AuthenticateIntegrityObject {
    pub amount: MinorUnit,
    pub currency: Currency,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PostAuthenticateIntegrityObject {
    pub amount: MinorUnit,
    pub currency: Currency,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SdkSessionTokenIntegrityObject {}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct IncrementalAuthorizationIntegrityObject {}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MandateRevokeIntegrityObject {
    pub mandate_id: Secret<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SplitSettlementIntegrityObject {}

#[derive(Debug, Clone)]
pub struct VerifyWebhookSourceRequestData {
    pub webhook_headers: std::collections::HashMap<String, String>,
    pub webhook_body: Vec<u8>,
    pub merchant_secret: ConnectorWebhookSecrets,
    pub webhook_uri: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct VerifyWebhookSourceIntegrityObject {
    pub webhook_id: String,
}
