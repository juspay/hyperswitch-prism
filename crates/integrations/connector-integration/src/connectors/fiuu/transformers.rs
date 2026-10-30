use std::{collections::HashMap, str::FromStr};

use base64::Engine;
use cards::CardNumber;
use common_enums::{BankNames, CaptureMethod, Currency};
use common_utils::{
    consts,
    crypto::{self, GenerateDigest, SignMessage, VerifySignature},
    errors::CustomResult,
    ext_traits::Encode,
    pii::Email,
    request::{Method, MultipartData},
    types::{AmountConvertor, SemanticVersion, StringMajorUnit, StringMajorUnitForConnector},
};
use domain_types::{
    connector_flow::{
        Authenticate, Authorize, Capture, PSync, PostAuthenticate, RSync, Refund, RepeatPayment,
        SetupMandate, Void,
    },
    connector_types::{
        EventType, MandateReference, MandateReferenceId, PaymentFlowData, PaymentVoidData,
        PaymentsAuthenticateData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsPostAuthenticateData, PaymentsResponseData, PaymentsSyncData, RefundFlowData,
        RefundSyncData, RefundWebhookDetailsResponse, RefundsData, RefundsResponseData,
        RepeatPaymentData, ResponseId, SetupMandateRequestData,
    },
    errors::{
        ConnectorError, IntegrationError, IntegrationErrorContext,
        ResponseTransformationErrorContext,
    },
    payment_method_data::{
        ApplePayDecryptedData, ApplePayPaymentData, ApplePayWalletData, BankRedirectData, Card,
        CardDetailsForNetworkTransactionId, GooglePayWalletData, GpayTokenizationData,
        PaymentMethodData, PaymentMethodDataTypes, RawCardNumber, RealTimePaymentData, WalletData,
    },
    router_data::{
        AdditionalPaymentMethodConnectorResponse, ConnectorResponseData, ConnectorSpecificConfig,
        ErrorResponse, FlowStatus,
    },
    router_data_v2::RouterDataV2,
    router_request_types::AuthenticationData,
    router_response_types::RedirectForm,
    utils,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use strum::Display;
use url::Url;

use crate::{
    connectors::{fiuu::FiuuRouterData, macros::GetFormData},
    types::ResponseRouterData,
    utils::{
        build_form_from_struct,
        qr_code::{QrCodeInformation, QrImage},
    },
};

// These needs to be accepted from SDK, need to be done after 1.0.0 stability as API contract will change
const GOOGLEPAY_API_VERSION_MINOR: u8 = 0;
const GOOGLEPAY_API_VERSION: u8 = 2;

const FIUU_DIRECT_API_DOC_URL: &str = "https://fiuu.com/developer/";

pub struct FiuuAuthType {
    pub(super) merchant_id: Secret<String>,
    pub(super) verify_key: Secret<String>,
    pub(super) secret_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for FiuuAuthType {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Fiuu {
                merchant_id,
                verify_key,
                secret_key,
                ..
            } => Ok(Self {
                merchant_id: merchant_id.to_owned(),
                verify_key: verify_key.to_owned(),
                secret_key: secret_key.to_owned(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
            .into()),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "UPPERCASE")]
pub enum TxnType {
    Sals,
    Auts,
}

impl TryFrom<Option<CaptureMethod>> for TxnType {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(capture_method: Option<CaptureMethod>) -> Result<Self, Self::Error> {
        match capture_method {
            Some(CaptureMethod::Automatic) | Some(CaptureMethod::SequentialAutomatic) | None => {
                Ok(Self::Sals)
            }
            Some(CaptureMethod::Manual) => Ok(Self::Auts),
            // Fiuu TxnType is SALS or AUTS only (Fiuu Direct Server API).
            Some(CaptureMethod::ManualMultiple) | Some(CaptureMethod::Scheduled) => {
                Err(IntegrationError::CaptureMethodNotSupported {
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Use capture_method AUTOMATIC (TxnType SALS) or MANUAL (TxnType AUTS)"
                                .to_owned(),
                        ),
                        doc_url: Some(FIUU_DIRECT_API_DOC_URL.to_owned()),
                        additional_context: Some(
                            "Fiuu Direct API supports TxnType SALS and AUTS only".to_owned(),
                        ),
                    },
                }
                .into())
            }
        }
    }
}

#[derive(Serialize, Deserialize, Display, Debug, Clone)]
enum TxnChannel {
    #[serde(rename = "CREDITAN")]
    #[strum(serialize = "CREDITAN")]
    Creditan,
    #[serde(rename = "RPP_DUITNOWQR")]
    #[strum(serialize = "RPP_DUITNOWQR")]
    RppDuitNowQr,
}

#[derive(Serialize, Deserialize, Display, Debug, Clone)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum FPXTxnChannel {
    FpxAbb,
    FpxUob,
    FpxAbmb,
    FpxScb,
    FpxBsn,
    FpxKfh,
    FpxBmmb,
    FpxBkrm,
    FpxHsbc,
    FpxAgrobank,
    FpxBocm,
    FpxMb2u,
    FpxCimbclicks,
    FpxAmb,
    FpxHlb,
    FpxPbb,
    FpxRhb,
    FpxBimb,
    FpxOcbc,
}
#[derive(Debug, Clone, Serialize)]
pub enum BankCode {
    PHBMMYKL,
    AGOBMYK1,
    MFBBMYKL,
    ARBKMYKL,
    BKCHMYKL,
    BIMBMYKL,
    BMMBMYKL,
    BKRMMYK1,
    BSNAMYK1,
    CIBBMYKL,
    HLBBMYKL,
    HBMBMYKL,
    KFHOMYKL,
    MBBEMYKL,
    PBBEMYKL,
    RHBBMYKL,
    SCBLMYKX,
    UOVBMYKL,
    OCBCMYKL,
}

impl TryFrom<BankNames> for BankCode {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(bank: BankNames) -> Result<Self, Self::Error> {
        match bank {
            BankNames::AffinBank => Ok(Self::PHBMMYKL),
            BankNames::AgroBank => Ok(Self::AGOBMYK1),
            BankNames::AllianceBank => Ok(Self::MFBBMYKL),
            BankNames::AmBank => Ok(Self::ARBKMYKL),
            BankNames::BankOfChina => Ok(Self::BKCHMYKL),
            BankNames::BankIslam => Ok(Self::BIMBMYKL),
            BankNames::BankMuamalat => Ok(Self::BMMBMYKL),
            BankNames::BankRakyat => Ok(Self::BKRMMYK1),
            BankNames::BankSimpananNasional => Ok(Self::BSNAMYK1),
            BankNames::CimbBank => Ok(Self::CIBBMYKL),
            BankNames::HongLeongBank => Ok(Self::HLBBMYKL),
            BankNames::HsbcBank => Ok(Self::HBMBMYKL),
            BankNames::KuwaitFinanceHouse => Ok(Self::KFHOMYKL),
            BankNames::Maybank => Ok(Self::MBBEMYKL),
            BankNames::PublicBank => Ok(Self::PBBEMYKL),
            BankNames::RhbBank => Ok(Self::RHBBMYKL),
            BankNames::StandardCharteredBank => Ok(Self::SCBLMYKX),
            BankNames::UobBank => Ok(Self::UOVBMYKL),
            BankNames::OcbcBank => Ok(Self::OCBCMYKL),
            bank => Err(IntegrationError::NotSupported {
                message: format!("Invalid BankName for FPX Refund: {bank:?}"),
                connector: "Fiuu",
                context: Default::default(),
            })?,
        }
    }
}

impl TryFrom<BankNames> for FPXTxnChannel {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(bank_names: BankNames) -> Result<Self, Self::Error> {
        match bank_names {
            BankNames::AffinBank => Ok(Self::FpxAbb),
            BankNames::AgroBank => Ok(Self::FpxAgrobank),
            BankNames::AllianceBank => Ok(Self::FpxAbmb),
            BankNames::AmBank => Ok(Self::FpxAmb),
            BankNames::BankOfChina => Ok(Self::FpxBocm),
            BankNames::BankIslam => Ok(Self::FpxBimb),
            BankNames::BankMuamalat => Ok(Self::FpxBmmb),
            BankNames::BankRakyat => Ok(Self::FpxBkrm),
            BankNames::BankSimpananNasional => Ok(Self::FpxBsn),
            BankNames::CimbBank => Ok(Self::FpxCimbclicks),
            BankNames::HongLeongBank => Ok(Self::FpxHlb),
            BankNames::HsbcBank => Ok(Self::FpxHsbc),
            BankNames::KuwaitFinanceHouse => Ok(Self::FpxKfh),
            BankNames::Maybank => Ok(Self::FpxMb2u),
            BankNames::PublicBank => Ok(Self::FpxPbb),
            BankNames::RhbBank => Ok(Self::FpxRhb),
            BankNames::StandardCharteredBank => Ok(Self::FpxScb),
            BankNames::UobBank => Ok(Self::FpxUob),
            BankNames::OcbcBank => Ok(Self::FpxOcbc),
            _ => Err(IntegrationError::NotImplemented(
                utils::get_unimplemented_payment_method_error_message("Fiuu"),
                Default::default(),
            ))?,
        }
    }
}

#[derive(Serialize, Debug, Clone)]
pub struct FiuuMandateRequest {
    #[serde(rename = "0")]
    mandate_request: Secret<String>,
}

#[derive(Serialize, Debug, Clone)]
pub struct FiuuRecurringRequest {
    record_type: FiuuRecordType,
    merchant_id: Secret<String>,
    token: Secret<String>,
    order_id: String,
    currency: Currency,
    amount: StringMajorUnit,
    billing_name: Secret<String>,
    email: Email,
    verify_key: Secret<String>,
}

#[derive(Serialize, Debug, Clone, strum::Display)]
pub enum FiuuRecordType {
    T,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        &FiuuRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for FiuuMandateRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: &FiuuRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth: FiuuAuthType = FiuuAuthType::try_from(&item.router_data.connector_config)?;
        let record_type = FiuuRecordType::T;
        let merchant_id = auth.merchant_id;
        let order_id = item
            .router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();
        let currency = item.router_data.request.currency;
        let amount = item
            .connector
            .amount_converter
            .convert(&item.router_data.request.amount)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;
        let billing_name = item
            .router_data
            .resource_common_data
            .get_billing_full_name()?;

        // Mirror the Direct path (hyperswitch `fiuu/transformers.rs:268-269`), which falls back to
        // the request email when the billing address carries none. Without this a recurring payment
        // with no billing email hard-fails on UCS while succeeding on Direct.
        let email = item
            .router_data
            .resource_common_data
            .get_billing_email()
            .or_else(|err| item.router_data.request.email.clone().ok_or(err))?;
        let token = Secret::new(item.router_data.request.connector_mandate_id().ok_or(
            IntegrationError::MissingRequiredField {
                field_name: "connector_mandate_id",
                context: Default::default(),
            },
        )?);
        let verify_key = auth.verify_key;
        let recurring_request = FiuuRecurringRequest {
            record_type: record_type.clone(),
            merchant_id: merchant_id.clone(),
            token: token.clone(),
            order_id: order_id.clone(),
            currency,
            amount: amount.clone(),
            billing_name: billing_name.clone(),
            email: email.clone(),
            verify_key: verify_key.clone(),
        };
        let check_sum = calculate_check_sum(recurring_request)?;
        let mandate_request = format!(
            "{}|{}||{}|{}|{}|{}|{}|{}|||{}",
            record_type,
            merchant_id.peek(),
            token.peek(),
            order_id,
            currency,
            amount.get_amount_as_string(),
            billing_name.peek(),
            email.peek(),
            check_sum.peek()
        );
        Ok(Self {
            mandate_request: mandate_request.into(),
        })
    }
}

pub fn calculate_check_sum(
    req: FiuuRecurringRequest,
) -> CustomResult<Secret<String>, IntegrationError> {
    let formatted_string = format!(
        "{}{}{}{}{}{}{}",
        req.record_type,
        req.merchant_id.peek(),
        req.token.peek(),
        req.order_id,
        req.currency,
        req.amount.get_amount_as_string(),
        req.verify_key.peek()
    );
    Ok(Secret::new(hex::encode(
        crypto::Md5
            .generate_digest(formatted_string.as_bytes())
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?,
    )))
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct FiuuPaymentRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    #[serde(rename = "MerchantID")]
    merchant_id: Secret<String>,
    reference_no: String,
    txn_type: TxnType,
    txn_currency: Currency,
    txn_amount: StringMajorUnit,
    signature: Secret<String>,
    #[serde(rename = "ReturnURL")]
    return_url: Option<String>,
    #[serde(rename = "NotificationURL")]
    notification_url: Option<Url>,
    #[serde(flatten)]
    customer_details: FiuuCustomerDetails,
    #[serde(flatten)]
    payment_method_data: FiuuPaymentMethodData<T>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(untagged)]
pub enum FiuuPaymentMethodData<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    FiuuQRData(Box<FiuuQRData>),
    FiuuCardData(Box<FiuuCardData<T>>),
    FiuuCardWithNTI(Box<FiuuCardWithNTI>),
    FiuuCardWithOriginalTxn(Box<FiuuCardWithOriginalTxn<T>>),
    FiuuFpxData(Box<FiuuFPXData>),
    FiuuGooglePayData(Box<FiuuGooglePayData>),
    FiuuGooglePayDecryptedData(Box<FiuuGooglePayDecryptedData>),
    FiuuApplePayData(Box<FiuuApplePayData>),
    FiuuApplePayEncryptedData(Box<FiuuApplePayEncryptedData>),
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct FiuuFPXData {
    #[serde(rename = "non_3DS")]
    non_3ds: i32,
    txn_channel: FPXTxnChannel,
}
#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct FiuuQRData {
    txn_channel: TxnChannel,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub struct FiuuCardData<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    #[serde(rename = "non_3DS")]
    non_3ds: i32,
    #[serde(rename = "TxnChannel")]
    txn_channel: TxnChannel,
    cc_pan: RawCardNumber<T>,
    cc_cvv2: Secret<String>,
    cc_month: Secret<String>,
    cc_year: Secret<String>,
    #[serde(rename = "mpstokenstatus")]
    mps_token_status: Option<i32>,
    #[serde(flatten)]
    three_ds_auth: Option<FiuuThreeDsAuthData>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub struct FiuuCardWithNTI {
    #[serde(rename = "TxnChannel")]
    txn_channel: TxnChannel,
    cc_pan: CardNumber,
    cc_month: Secret<String>,
    cc_year: Secret<String>,
    #[serde(rename = "OriginalSchemeID")]
    original_scheme_id: Secret<String>,
    /// 1 = merchant initiated (Fiuu Direct Server API).
    #[serde(rename = "MerchantInitiated")]
    merchant_initiated: i32,
}

/// Direct API MIT on a card verified by Zero Dollar Verification (Fiuu Direct Server API): `OriginalTranID`
/// carries the `verificationID_<id>` mandate. No `OriginalSchemeID`: a connector mandate id
/// carries no network transaction id. No CVV on a MIT.
#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub struct FiuuCardWithOriginalTxn<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    #[serde(rename = "TxnChannel")]
    txn_channel: TxnChannel,
    cc_pan: RawCardNumber<T>,
    cc_month: Secret<String>,
    cc_year: Secret<String>,
    #[serde(rename = "OriginalTranID")]
    original_tran_id: Secret<String>,
    #[serde(rename = "MerchantInitiated")]
    merchant_initiated: i32,
    /// 4 = merchant is passing a stored card number for payment (Fiuu Direct Server API).
    #[serde(rename = "mpstokenstatus")]
    mps_token_status: i32,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub struct FiuuApplePayData {
    #[serde(rename = "TxnChannel")]
    txn_channel: TxnChannel,
    cc_month: Secret<String>,
    cc_year: Secret<String>,
    cc_token: CardNumber,
    eci: Option<String>,
    token_cryptogram: Secret<String>,
    token_type: FiuuTokenType,
    /// `LAST4` accompanies `CC_TOKEN`: last four digits of the DPAN (Fiuu Direct Server API).
    last4: Secret<String>,
    #[serde(rename = "non_3DS")]
    non_3ds: i32,
}

/// Apple Pay PSP decryption: Fiuu holds the Apple processing certificate
/// (Fiuu Mobile XDK X ApplePay), so the whole token goes in `ApplePay` (base64 JSON) with
/// `Method=app` (Fiuu Direct Server API).
#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct FiuuApplePayEncryptedData {
    apple_pay: Secret<String>,
    method: FiuuMethod,
    txn_channel: TxnChannel,
    #[serde(rename = "non_3DS")]
    non_3ds: i32,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
pub enum FiuuMethod {
    App,
}

/// Pre-decrypted Google Pay (network token) on the Direct API: `CC_TOKEN` (DPAN),
/// `TOKEN_TYPE=GooglePay`, `TOKEN_CRYPTOGRAM`, `ECI`, `LAST4`, `CC_MONTH`, `CC_YEAR`
/// (Fiuu Direct Server API).
#[serde_with::skip_serializing_none]
#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub struct FiuuGooglePayDecryptedData {
    #[serde(rename = "TxnChannel")]
    txn_channel: TxnChannel,
    cc_token: CardNumber,
    cc_month: Secret<String>,
    cc_year: Secret<String>,
    token_cryptogram: Secret<String>,
    eci: Option<String>,
    last4: Secret<String>,
    token_type: FiuuTokenType,
    #[serde(rename = "non_3DS")]
    non_3ds: i32,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "PascalCase")]
pub enum FiuuTokenType {
    ApplePay,
    GooglePay,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct FiuuGooglePayData {
    txn_channel: TxnChannel,
    #[serde(rename = "GooglePay[apiVersion]")]
    api_version: u8,
    #[serde(rename = "GooglePay[apiVersionMinor]")]
    api_version_minor: u8,
    #[serde(rename = "GooglePay[paymentMethodData][info][assuranceDetails][accountVerified]")]
    account_verified: Option<bool>,
    #[serde(
        rename = "GooglePay[paymentMethodData][info][assuranceDetails][cardHolderAuthenticated]"
    )]
    card_holder_authenticated: Option<bool>,
    #[serde(rename = "GooglePay[paymentMethodData][info][cardDetails]")]
    card_details: String,
    #[serde(rename = "GooglePay[paymentMethodData][info][cardNetwork]")]
    card_network: String,
    #[serde(rename = "GooglePay[paymentMethodData][tokenizationData][token]")]
    token: Secret<String>,
    #[serde(rename = "GooglePay[paymentMethodData][tokenizationData][type]")]
    tokenization_data_type: Secret<String>,
    #[serde(rename = "GooglePay[paymentMethodData][type]")]
    pm_type: String,
    #[serde(rename = "TOKEN_TYPE")]
    token_type: FiuuTokenType,
    #[serde(rename = "non_3DS")]
    non_3ds: i32,
}

pub fn calculate_signature(
    signature_data: String,
) -> Result<Secret<String>, error_stack::Report<IntegrationError>> {
    let message = signature_data.as_bytes();
    let encoded_data = hex::encode(crypto::Md5.generate_digest(message).change_context(
        IntegrationError::RequestEncodingFailed {
            context: Default::default(),
        },
    )?);
    Ok(Secret::new(encoded_data))
}

/// Lowercase-hex HMAC-SHA256 of `message` keyed by `key` — the hash the Standalone Card APIs
/// (cc_verification.php, authentication.php, authn_query.php) expect in `Signature`.
pub fn calculate_hmac_sha256_signature(
    message: &str,
    key: &Secret<String>,
) -> Result<Secret<String>, error_stack::Report<IntegrationError>> {
    let signature = crypto::HmacSha256
        .sign_message(key.peek().as_bytes(), message.as_bytes())
        .change_context(IntegrationError::RequestEncodingFailed {
            context: IntegrationErrorContext {
                suggested_action: Some(
                    "Verify the Fiuu verify_key configured for this merchant".to_owned(),
                ),
                doc_url: Some(FIUU_DIRECT_API_DOC_URL.to_owned()),
                additional_context: Some(
                    "HMAC-SHA256 request signature could not be computed".to_owned(),
                ),
            },
        })?;
    Ok(Secret::new(hex::encode(signature)))
}

/// `CustName`/`CustEmail`/`CustContact`/`CustDesc`/`DisplayName` of the Direct API payment
/// request; all optional, so absent values are not sent as empty fields.
#[serde_with::skip_serializing_none]
#[derive(Serialize, Debug, Clone, Default)]
#[serde(rename_all = "PascalCase")]
pub struct FiuuCustomerDetails {
    cust_name: Option<Secret<String>>,
    cust_email: Option<Email>,
    cust_contact: Option<Secret<String>>,
    cust_desc: Option<String>,
    display_name: Option<String>,
}

/// Externally-obtained 3DS authentication values for the Direct API.
#[serde_with::skip_serializing_none]
#[derive(Serialize, Debug, Clone)]
pub struct FiuuThreeDsAuthData {
    #[serde(rename = "ECI")]
    eci: Option<String>,
    #[serde(rename = "Cryptogram")]
    cryptogram: Secret<String>,
    #[serde(rename = "dsTransID")]
    ds_trans_id: Option<String>,
    #[serde(rename = "3DS_version")]
    three_ds_version: Option<String>,
}

impl TryFrom<&AuthenticationData> for FiuuThreeDsAuthData {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(auth: &AuthenticationData) -> Result<Self, Self::Error> {
        // Sending non_3DS=1 without the CAVV would downgrade an authenticated payment.
        let cryptogram = auth
            .cavv
            .clone()
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "authentication_data.cavv",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Pass the CAVV obtained from the 3DS server in authentication_data.cavv"
                            .to_owned(),
                    ),
                    doc_url: Some(FIUU_DIRECT_API_DOC_URL.to_owned()),
                    additional_context: Some(
                        "Fiuu Direct API requires Cryptogram when external 3DS data is supplied"
                            .to_owned(),
                    ),
                },
            })?;
        Ok(Self {
            eci: auth.eci.clone(),
            cryptogram,
            ds_trans_id: auth.ds_trans_id.clone(),
            three_ds_version: auth.message_version.as_ref().map(ToString::to_string),
        })
    }
}

fn get_authorize_customer_details<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    item: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
) -> FiuuCustomerDetails {
    FiuuCustomerDetails {
        cust_name: item.resource_common_data.get_optional_billing_full_name(),
        cust_email: item
            .resource_common_data
            .get_optional_billing_email()
            .or_else(|| item.request.email.clone()),
        cust_contact: item
            .resource_common_data
            .get_optional_billing_phone_number(),
        cust_desc: item.resource_common_data.description.clone(),
        display_name: item
            .request
            .billing_descriptor
            .as_ref()
            .and_then(|descriptor| descriptor.statement_descriptor.clone()),
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        FiuuRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for FiuuPaymentRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: FiuuRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = FiuuAuthType::try_from(&item.router_data.connector_config)?;
        let merchant_id = auth.merchant_id.peek().to_string();
        let txn_currency = item.router_data.request.currency;
        let amount = item
            .connector
            .amount_converter
            .convert(&item.router_data.request.amount)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;
        let txn_amount = amount;
        let reference_no = item
            .router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();
        let verify_key = auth.verify_key.peek().to_string();
        let signature = calculate_signature(format!(
            "{}{merchant_id}{reference_no}{verify_key}",
            txn_amount.get_amount_as_string()
        ))?;
        // ManualMultiple / Scheduled capture are refused before any request.
        let txn_type = TxnType::try_from(item.router_data.request.capture_method)?;
        let return_url = item.router_data.request.router_return_url.clone();
        let non_3ds = match item.router_data.resource_common_data.is_three_ds() {
            false => 1,
            true => 0,
        };
        // Status finality comes from the notification, so it is required.
        let webhook_url =
            item.router_data
                .request
                .webhook_url
                .as_deref()
                .ok_or(IntegrationError::MissingRequiredField {
                field_name: "webhook_url",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Pass webhook_url; Fiuu sends the final payment status to NotificationURL"
                            .to_owned(),
                    ),
                    doc_url: Some(FIUU_DIRECT_API_DOC_URL.to_owned()),
                    additional_context: None,
                },
            })?;
        let notification_url = Some(Url::parse(webhook_url).change_context(
            IntegrationError::InvalidDataFormat {
                field_name: "webhook_url",
                context: IntegrationErrorContext {
                    suggested_action: Some("Pass webhook_url as an absolute URL".to_owned()),
                    doc_url: Some(FIUU_DIRECT_API_DOC_URL.to_owned()),
                    additional_context: None,
                },
            },
        )?);
        let customer_details = get_authorize_customer_details(&item.router_data);

        let payment_method_data = match item.router_data.request.payment_method_data {
            PaymentMethodData::Card(ref card) => {
                FiuuPaymentMethodData::try_from((card, &item.router_data))
            }
            PaymentMethodData::RealTimePayment(ref real_time_payment_data) => {
                match *real_time_payment_data.clone() {
                    RealTimePaymentData::DuitNow {} => {
                        Ok(FiuuPaymentMethodData::FiuuQRData(Box::new(FiuuQRData {
                            txn_channel: TxnChannel::RppDuitNowQr,
                        })))
                    }
                    RealTimePaymentData::Fps {}
                    | RealTimePaymentData::PromptPay {}
                    | RealTimePaymentData::VietQr {} => Err(IntegrationError::NotImplemented(
                        utils::get_unimplemented_payment_method_error_message("fiuu"),
                        Default::default(),
                    )
                    .into()),
                }
            }
            PaymentMethodData::BankRedirect(ref bank_redirect_data) => match bank_redirect_data {
                BankRedirectData::OnlineBankingFpx { ref issuer } => {
                    Ok(FiuuPaymentMethodData::FiuuFpxData(Box::new(FiuuFPXData {
                        txn_channel: FPXTxnChannel::try_from(*issuer)?,
                        non_3ds,
                    })))
                }
                BankRedirectData::BancontactCard { .. }
                | BankRedirectData::Bizum {}
                | BankRedirectData::Blik { .. }
                | BankRedirectData::Eft { .. }
                | BankRedirectData::Eps { .. }
                | BankRedirectData::Giropay { .. }
                | BankRedirectData::Ideal { .. }
                | BankRedirectData::Interac { .. }
                | BankRedirectData::OnlineBankingCzechRepublic { .. }
                | BankRedirectData::OnlineBankingFinland { .. }
                | BankRedirectData::OnlineBankingPoland { .. }
                | BankRedirectData::OnlineBankingSlovakia { .. }
                | BankRedirectData::OpenBankingUk { .. }
                | BankRedirectData::Przelewy24 { .. }
                | BankRedirectData::Sofort { .. }
                | BankRedirectData::Trustly { .. }
                | BankRedirectData::OnlineBankingThailand { .. }
                | BankRedirectData::LocalBankRedirect {}
                | BankRedirectData::OpenBanking { .. }
                | BankRedirectData::Netbanking { .. } => {
                    Err(error_stack::report!(IntegrationError::NotSupported {
                        message: utils::get_unimplemented_payment_method_error_message("fiuu"),
                        connector: "Fiuu",
                        context: Default::default(),
                    }))
                }
            },
            PaymentMethodData::Wallet(ref wallet_data) => wallet_payment_method_data(wallet_data),
            PaymentMethodData::CardRedirect(_)
            | PaymentMethodData::PayLater(_)
            | PaymentMethodData::BankDebit(_)
            | PaymentMethodData::BankTransfer(_)
            | PaymentMethodData::Crypto(_)
            | PaymentMethodData::MandatePayment
            | PaymentMethodData::CardWithNoCvc(_)
            | PaymentMethodData::MobilePayment(_)
            | PaymentMethodData::Reward
            | PaymentMethodData::Upi(_)
            | PaymentMethodData::Voucher(_)
            | PaymentMethodData::GiftCard(_)
            | PaymentMethodData::PaymentMethodToken(_)
            | PaymentMethodData::OpenBanking(_)
            | PaymentMethodData::NetworkToken(_)
            | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
            | PaymentMethodData::CardDetailsForNetworkTransactionId(_) => {
                Err(IntegrationError::NotImplemented(
                    utils::get_unimplemented_payment_method_error_message("fiuu"),
                    Default::default(),
                )
                .into())
            }
        }?;

        Ok(Self {
            merchant_id: auth.merchant_id,
            reference_no,
            txn_type,
            txn_currency,
            txn_amount,
            return_url,
            payment_method_data,
            signature,
            notification_url,
            customer_details,
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        &FiuuRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for FiuuPaymentRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: &FiuuRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = FiuuAuthType::try_from(&item.router_data.connector_config)?;
        let merchant_id = auth.merchant_id.peek().to_string();
        let txn_currency = item.router_data.request.currency;
        let amount = item
            .connector
            .amount_converter
            .convert(&item.router_data.request.amount)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;
        let txn_amount = amount;
        let reference_no = item
            .router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();
        let verify_key = auth.verify_key.peek().to_string();
        let signature = calculate_signature(format!(
            "{}{merchant_id}{reference_no}{verify_key}",
            txn_amount.get_amount_as_string()
        ))?;
        let txn_type = match item.router_data.request.is_auto_capture() {
            true => TxnType::Sals,
            false => TxnType::Auts,
        };
        let return_url = item.router_data.request.router_return_url.clone();
        let notification_url = Some(
            Url::parse(&item.router_data.request.get_webhook_url()?).change_context(
                IntegrationError::RequestEncodingFailed {
                    context: Default::default(),
                },
            )?,
        );
        let payment_method_data = match (
            &item.router_data.request.mandate_reference,
            get_verification_mandate_id(&item.router_data.request.mandate_reference),
        ) {
            // $0-verified mandate: OriginalTranID=verificationID_<id> (Fiuu Direct Server API).
            (_, Some(original_tran_id)) => FiuuPaymentMethodData::try_from((
                &item.router_data.request.payment_method_data,
                original_tran_id,
            )),
            (MandateReferenceId::NetworkMandateId(network_transaction_id), None) => {
                match item.router_data.request.payment_method_data {
                    PaymentMethodData::CardDetailsForNetworkTransactionId(ref raw_card_details) => {
                        FiuuPaymentMethodData::try_from((
                            raw_card_details,
                            network_transaction_id.network_transaction_id.clone(),
                        ))
                    }
                    _ => Err(IntegrationError::NotImplemented(
                        utils::get_unimplemented_payment_method_error_message("fiuu"),
                        Default::default(),
                    )
                    .into()),
                }
            }
            _ => Err(IntegrationError::NotImplemented(
                utils::get_unimplemented_payment_method_error_message("fiuu"),
                Default::default(),
            )
            .into()),
        }?;

        Ok(Self {
            merchant_id: auth.merchant_id,
            reference_no,
            txn_type,
            txn_currency,
            txn_amount,
            return_url,
            payment_method_data,
            signature,
            notification_url,
            // ponytail: RepeatPayment unit owns any customer fields on the MIT request.
            customer_details: FiuuCustomerDetails::default(),
        })
    }
}

// =============================================================================
// SETUP MANDATE (Pay.SetupRecurring) FLOW - REQUEST TRANSFORMER
// =============================================================================
// Amount absent or zero -> Zero Dollar Verification (`cc_verification.php`, Fiuu Standalone
// Card API): card-only, nothing is charged. Amount > 0 -> the Direct API
// (`RMS/API/Direct/1.4.0`) with `mpstokenstatus=1`; Direct card channels need a real amount
// (Fiuu Direct Server API), so a 0.00 Direct request cannot tokenize.

/// `true` when SetupMandate goes to Zero Dollar Verification: amount absent or zero.
pub fn is_zero_auth<T: PaymentMethodDataTypes>(request: &SetupMandateRequestData<T>) -> bool {
    request
        .amount
        .as_ref()
        .is_none_or(|money| money.amount.get_amount_as_i64() == 0)
}

/// The `verificationID_<id>` mandate SetupMandate issues for Zero Dollar Verification. It is
/// charged through the Direct API (`OriginalTranID`), not the Recurring token API.
pub fn get_verification_mandate_id(mandate_reference: &MandateReferenceId) -> Option<String> {
    match mandate_reference {
        MandateReferenceId::ConnectorMandateId(mandate) => mandate
            .get_connector_mandate_id()
            .filter(|id| id.starts_with("verificationID_")),
        MandateReferenceId::NetworkMandateId(_) | MandateReferenceId::NetworkTokenWithNTI(_) => {
            None
        }
    }
}

#[derive(Serialize, Debug, Clone)]
#[serde(untagged)]
pub enum FiuuSetupMandateRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    Direct(Box<FiuuPaymentRequest<T>>),
    ZeroAuth(Box<FiuuZeroAuthRequest<T>>),
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize> GetFormData
    for FiuuSetupMandateRequest<T>
{
    fn get_form_data(&self) -> MultipartData {
        match self {
            Self::Direct(req) => {
                build_form_from_struct(req).unwrap_or_else(|_| MultipartData::new())
            }
            Self::ZeroAuth(req) => {
                build_form_from_struct(req).unwrap_or_else(|_| MultipartData::new())
            }
        }
    }
}

/// Zero Dollar Verification request (Fiuu Standalone Card API, "Zero Dollar Verification Request").
#[derive(Serialize, Debug, Clone)]
pub struct FiuuZeroAuthRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    #[serde(rename = "MerchantID")]
    merchant_id: Secret<String>,
    #[serde(rename = "TxnChannel")]
    txn_channel: TxnChannel,
    #[serde(rename = "ReferenceNo")]
    reference_no: String,
    #[serde(rename = "TxnCurrency")]
    txn_currency: Currency,
    /// HMAC_SHA256(TxnCurrency.MerchantID.ReferenceNo, verify_key).
    #[serde(rename = "Signature")]
    signature: Secret<String>,
    #[serde(rename = "CC_PAN")]
    cc_pan: RawCardNumber<T>,
    #[serde(rename = "CC_CVV2")]
    cc_cvv2: Secret<String>,
    #[serde(rename = "CC_MONTH")]
    cc_month: Secret<String>,
    #[serde(rename = "CC_YEAR")]
    cc_year: Secret<String>,
    #[serde(rename = "mpstokenstatus")]
    mps_token_status: i32,
    #[serde(rename = "non_3DS")]
    non_3ds: i32,
    #[serde(rename = "ReturnURL", skip_serializing_if = "Option::is_none")]
    return_url: Option<String>,
    #[serde(rename = "NotifyURL", skip_serializing_if = "Option::is_none")]
    notify_url: Option<String>,
    /// Only CustName / CustEmail / CustContact are set; cc_verification has no CustDesc/DisplayName.
    #[serde(flatten)]
    customer_details: FiuuCustomerDetails,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        &RouterDataV2<
            SetupMandate,
            PaymentFlowData,
            SetupMandateRequestData<T>,
            PaymentsResponseData,
        >,
    > for FiuuZeroAuthRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: &RouterDataV2<
            SetupMandate,
            PaymentFlowData,
            SetupMandateRequestData<T>,
            PaymentsResponseData,
        >,
    ) -> Result<Self, Self::Error> {
        // cc_verification verifies a card; any other method is refused
        // before a request is built.
        let card =
            match item.request.payment_method_data {
                PaymentMethodData::Card(ref card) => card,
                _ => return Err(IntegrationError::NotSupported {
                    message: "Zero-amount SetupMandate for a non-card payment method".to_owned(),
                    connector: "Fiuu",
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Use a card, or pass an amount greater than zero for wallet mandates"
                                .to_owned(),
                        ),
                        doc_url: Some(FIUU_DIRECT_API_DOC_URL.to_owned()),
                        additional_context: Some(
                            "Fiuu Zero Dollar Verification (cc_verification.php) is card-only"
                                .to_owned(),
                        ),
                    },
                }
                .into()),
            };
        let auth = FiuuAuthType::try_from(&item.connector_config)?;
        let txn_currency = item.request.currency;
        let reference_no = item
            .resource_common_data
            .connector_request_reference_id
            .clone();
        let signature = calculate_hmac_sha256_signature(
            &format!("{txn_currency}{}{reference_no}", auth.merchant_id.peek()),
            &auth.verify_key,
        )?;
        let non_3ds = match item.resource_common_data.is_three_ds() {
            false => 1,
            true => 0,
        };
        Ok(Self {
            merchant_id: auth.merchant_id,
            txn_channel: TxnChannel::Creditan,
            reference_no,
            txn_currency,
            signature,
            cc_pan: card.card_number.clone(),
            cc_cvv2: card.card_cvc.clone(),
            cc_month: card.get_card_expiry_month_2_digit()?,
            cc_year: card.get_expiry_year_4_digit(),
            // 1 = request a new PG token.
            mps_token_status: 1,
            non_3ds,
            return_url: item.request.router_return_url.clone(),
            notify_url: item.request.webhook_url.clone(),
            customer_details: FiuuCustomerDetails {
                cust_name: item.resource_common_data.get_optional_billing_full_name(),
                cust_email: item
                    .request
                    .email
                    .clone()
                    .or_else(|| item.resource_common_data.get_optional_billing_email()),
                cust_contact: item
                    .resource_common_data
                    .get_optional_billing_phone_number(),
                cust_desc: None,
                display_name: None,
            },
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        FiuuRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for FiuuSetupMandateRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: FiuuRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Same predicate as get_url, so the body always matches the endpoint.
        if is_zero_auth(&item.router_data.request) {
            Ok(Self::ZeroAuth(Box::new(FiuuZeroAuthRequest::try_from(
                &item.router_data,
            )?)))
        } else {
            Ok(Self::Direct(Box::new(FiuuPaymentRequest::try_from(item)?)))
        }
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        FiuuRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for FiuuPaymentRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: FiuuRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = FiuuAuthType::try_from(&item.router_data.connector_config)?;
        let merchant_id = auth.merchant_id.peek().to_string();
        let txn_currency = item.router_data.request.currency;
        // Direct path only: FiuuSetupMandateRequest routes an absent/zero amount to
        // Zero Dollar Verification, so a missing amount here is a caller bug, not a 0.00 charge.
        let money =
            item.router_data
                .request
                .amount
                .as_ref()
                .ok_or(IntegrationError::MissingRequiredField {
                field_name: "amount",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Pass an amount greater than zero, or omit it for zero-dollar verification"
                            .to_owned(),
                    ),
                    doc_url: Some(FIUU_DIRECT_API_DOC_URL.to_owned()),
                    additional_context: Some(
                        "Fiuu Direct API SetupMandate requires a non-zero TxnAmount".to_owned(),
                    ),
                },
            })?;
        let amount = item
            .connector
            .amount_converter
            .convert(money)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;
        let txn_amount = amount;
        let reference_no = item
            .router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();
        let verify_key = auth.verify_key.peek().to_string();
        let signature = calculate_signature(format!(
            "{}{merchant_id}{reference_no}{verify_key}",
            txn_amount.get_amount_as_string()
        ))?;
        // ManualMultiple / Scheduled are refused before any payment method
        // arm (Card and Wallet) is built.
        let txn_type = TxnType::try_from(item.router_data.request.capture_method)?;
        let return_url = item.router_data.request.router_return_url.clone();
        let non_3ds = match item.router_data.resource_common_data.is_three_ds() {
            false => 1,
            true => 0,
        };
        let notification_url = Some(
            Url::parse(&item.router_data.request.get_webhook_url()?).change_context(
                IntegrationError::RequestEncodingFailed {
                    context: Default::default(),
                },
            )?,
        );

        // CustEmail stays card-only on SetupMandate (it moved off FiuuCardData into
        // FiuuCustomerDetails so the Direct request emits it once).
        let customer_details = FiuuCustomerDetails {
            cust_email: matches!(
                item.router_data.request.payment_method_data,
                PaymentMethodData::Card(_)
            )
            .then(|| {
                item.router_data.request.email.clone().or_else(|| {
                    item.router_data
                        .resource_common_data
                        .get_optional_billing_email()
                })
            })
            .flatten(),
            ..Default::default()
        };

        let payment_method_data = match item.router_data.request.payment_method_data {
            PaymentMethodData::Card(ref card) => {
                // Force `mpstokenstatus=1` so Fiuu returns a token on the
                // first call — this is what differentiates SetupMandate from
                // a plain Authorize. Pass `CustEmail` when present so the
                // connector has enough context to register the token; mirrors
                // the request-email-with-billing-fallback pattern used by
                // shift4 / airwallex / cybersource.
                Ok(FiuuPaymentMethodData::FiuuCardData(Box::new(
                    FiuuCardData {
                        txn_channel: TxnChannel::Creditan,
                        non_3ds,
                        cc_pan: card.card_number.clone(),
                        cc_cvv2: card.card_cvc.clone(),
                        cc_month: card.get_card_expiry_month_2_digit()?,
                        cc_year: card.get_expiry_year_4_digit(),
                        mps_token_status: Some(1),
                        three_ds_auth: None,
                    },
                )))
            }
            PaymentMethodData::RealTimePayment(ref real_time_payment_data) => {
                match *real_time_payment_data.clone() {
                    RealTimePaymentData::DuitNow {} => {
                        Ok(FiuuPaymentMethodData::FiuuQRData(Box::new(FiuuQRData {
                            txn_channel: TxnChannel::RppDuitNowQr,
                        })))
                    }
                    RealTimePaymentData::Fps {}
                    | RealTimePaymentData::PromptPay {}
                    | RealTimePaymentData::VietQr {} => Err(IntegrationError::NotImplemented(
                        utils::get_unimplemented_payment_method_error_message("fiuu"),
                        Default::default(),
                    )
                    .into()),
                }
            }
            PaymentMethodData::BankRedirect(ref bank_redirect_data) => match bank_redirect_data {
                BankRedirectData::OnlineBankingFpx { ref issuer } => {
                    Ok(FiuuPaymentMethodData::FiuuFpxData(Box::new(FiuuFPXData {
                        txn_channel: FPXTxnChannel::try_from(*issuer)?,
                        non_3ds,
                    })))
                }
                BankRedirectData::BancontactCard { .. }
                | BankRedirectData::Bizum {}
                | BankRedirectData::Blik { .. }
                | BankRedirectData::Eft { .. }
                | BankRedirectData::Eps { .. }
                | BankRedirectData::Giropay { .. }
                | BankRedirectData::Ideal { .. }
                | BankRedirectData::Interac { .. }
                | BankRedirectData::OnlineBankingCzechRepublic { .. }
                | BankRedirectData::OnlineBankingFinland { .. }
                | BankRedirectData::OnlineBankingPoland { .. }
                | BankRedirectData::OnlineBankingSlovakia { .. }
                | BankRedirectData::OpenBankingUk { .. }
                | BankRedirectData::Przelewy24 { .. }
                | BankRedirectData::Sofort { .. }
                | BankRedirectData::Trustly { .. }
                | BankRedirectData::OnlineBankingThailand { .. }
                | BankRedirectData::LocalBankRedirect {}
                | BankRedirectData::OpenBanking { .. }
                | BankRedirectData::Netbanking { .. } => {
                    Err(error_stack::report!(IntegrationError::NotSupported {
                        message: utils::get_unimplemented_payment_method_error_message("fiuu"),
                        connector: "Fiuu",
                        context: Default::default(),
                    }))
                }
            },
            PaymentMethodData::Wallet(ref wallet_data) => wallet_payment_method_data(wallet_data),
            PaymentMethodData::CardRedirect(_)
            | PaymentMethodData::PayLater(_)
            | PaymentMethodData::BankDebit(_)
            | PaymentMethodData::BankTransfer(_)
            | PaymentMethodData::Crypto(_)
            | PaymentMethodData::MandatePayment
            | PaymentMethodData::CardWithNoCvc(_)
            | PaymentMethodData::MobilePayment(_)
            | PaymentMethodData::Reward
            | PaymentMethodData::Upi(_)
            | PaymentMethodData::Voucher(_)
            | PaymentMethodData::GiftCard(_)
            | PaymentMethodData::PaymentMethodToken(_)
            | PaymentMethodData::OpenBanking(_)
            | PaymentMethodData::NetworkToken(_)
            | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
            | PaymentMethodData::CardDetailsForNetworkTransactionId(_) => {
                Err(IntegrationError::NotImplemented(
                    utils::get_unimplemented_payment_method_error_message("fiuu"),
                    Default::default(),
                )
                .into())
            }
        }?;

        Ok(Self {
            merchant_id: auth.merchant_id,
            reference_no,
            txn_type,
            txn_currency,
            txn_amount,
            return_url,
            payment_method_data,
            signature,
            notification_url,
            customer_details,
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<(
        &Card<T>,
        &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    )> for FiuuPaymentMethodData<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        (req_card, item): (
            &Card<T>,
            &RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
        ),
    ) -> Result<Self, Self::Error> {
        let mps_token_status = if item.request.is_customer_initiated_mandate_payment() {
            // PG tokenization (mpstokenstatus=1) needs CustEmail, which
            // FiuuCustomerDetails takes from the billing email, else the request email.
            if item
                .resource_common_data
                .get_optional_billing_email()
                .is_none()
                && item.request.email.is_none()
            {
                return Err(IntegrationError::MissingRequiredField {
                    field_name: "email",
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Pass customer email or billing email for a CIT mandate payment"
                                .to_owned(),
                        ),
                        doc_url: Some(FIUU_DIRECT_API_DOC_URL.to_owned()),
                        additional_context: Some(
                            "Fiuu requires CustEmail with mpstokenstatus=1".to_owned(),
                        ),
                    },
                }
                .into());
            }
            Some(1)
        } else {
            Some(3)
        };
        // External 3DS values make this an authenticated non_3DS=1 payment.
        let three_ds_auth = item
            .request
            .authentication_data
            .as_ref()
            .map(FiuuThreeDsAuthData::try_from)
            .transpose()?;
        let non_3ds = match (
            three_ds_auth.is_some(),
            item.resource_common_data.is_three_ds(),
        ) {
            (true, _) | (false, false) => 1,
            (false, true) => 0,
        };
        Ok(Self::FiuuCardData(Box::new(FiuuCardData {
            txn_channel: TxnChannel::Creditan,
            non_3ds,
            cc_pan: req_card.card_number.clone(),
            cc_cvv2: req_card.card_cvc.clone(),
            cc_month: req_card.get_card_expiry_month_2_digit()?,
            cc_year: req_card.get_expiry_year_4_digit(),
            mps_token_status,
            three_ds_auth,
        })))
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<(&CardDetailsForNetworkTransactionId<T>, String)> for FiuuPaymentMethodData<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        (raw_card_data, network_transaction_id): (&CardDetailsForNetworkTransactionId<T>, String),
    ) -> Result<Self, Self::Error> {
        Ok(Self::FiuuCardWithNTI(Box::new(FiuuCardWithNTI {
            txn_channel: TxnChannel::Creditan,
            cc_pan: raw_card_data.card_number.try_card_number("Fiuu")?,
            cc_month: raw_card_data.card_exp_month.clone(),
            cc_year: raw_card_data.get_expiry_year_4_digit(),
            original_scheme_id: Secret::new(network_transaction_id),
            merchant_initiated: 1,
        })))
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<(&PaymentMethodData<T>, String)> for FiuuPaymentMethodData<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        (payment_method_data, original_tran_id): (&PaymentMethodData<T>, String),
    ) -> Result<Self, Self::Error> {
        let (cc_pan, cc_month, cc_year) = match payment_method_data {
            PaymentMethodData::Card(card) => (
                card.card_number.clone(),
                card.get_card_expiry_month_2_digit()?,
                card.get_expiry_year_4_digit(),
            ),
            PaymentMethodData::CardDetailsForNetworkTransactionId(card) => (
                card.card_number.clone(),
                card.card_exp_month.clone(),
                card.get_expiry_year_4_digit(),
            ),
            // Direct MIT needs CC_PAN or CC_TOKEN (Fiuu Direct Server API); a
            // verificationID_ mandate alone cannot be charged, so fail closed.
            _ => Err(IntegrationError::MissingRequiredField {
                field_name: "payment_method_data.card",
                context: fiuu_error_context("Send the card details with a verificationID_ mandate; Fiuu charges it via the Direct API", None),
            })?,
        };
        Ok(Self::FiuuCardWithOriginalTxn(Box::new(
            FiuuCardWithOriginalTxn {
                txn_channel: TxnChannel::Creditan,
                cc_pan,
                cc_month,
                cc_year,
                original_tran_id: Secret::new(original_tran_id),
                merchant_initiated: 1,
                mps_token_status: 4,
            },
        )))
    }
}

fn fiuu_error_context(
    suggested_action: &str,
    additional_context: Option<&str>,
) -> IntegrationErrorContext {
    IntegrationErrorContext {
        suggested_action: Some(suggested_action.to_owned()),
        doc_url: Some(FIUU_DIRECT_API_DOC_URL.to_owned()),
        additional_context: additional_context.map(str::to_owned),
    }
}

/// Wallet arm of the Direct API payment request, shared by the Authorize and SetupMandate
/// builders so both send the same wallet shape.
fn wallet_payment_method_data<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    wallet_data: &WalletData,
) -> Result<FiuuPaymentMethodData<T>, error_stack::Report<IntegrationError>> {
    match wallet_data {
        WalletData::GooglePay(google_pay_data) => {
            FiuuPaymentMethodData::try_from(google_pay_data.as_ref())
        }
        WalletData::ApplePay(apple_pay_data) => match &apple_pay_data.payment_data {
            ApplePayPaymentData::Decrypted(decrypt_data) => {
                FiuuPaymentMethodData::try_from(decrypt_data.clone())
            }
            ApplePayPaymentData::Encrypted(_) => {
                Ok(FiuuPaymentMethodData::FiuuApplePayEncryptedData(Box::new(
                    FiuuApplePayEncryptedData {
                        apple_pay: fiuu_apple_pay_token(apple_pay_data)?,
                        method: FiuuMethod::App,
                        txn_channel: TxnChannel::Creditan,
                        // Wallets are non-3DS on the Direct API, as for the decrypted arms.
                        non_3ds: 1,
                    },
                )))
            }
        },
        // Fiuu runs these in the browser (Apple Pay xdk.js, Google Pay createTxn.php /
        // payment_v2.php); there is no server-to-server API for them (Fiuu Direct Server API).
        WalletData::ApplePayRedirect(_)
        | WalletData::ApplePayThirdPartySdk(_)
        | WalletData::GooglePayRedirect(_)
        | WalletData::GooglePayThirdPartySdk(_) => {
            Err(error_stack::report!(IntegrationError::NotSupported {
                message: "Apple Pay / Google Pay redirect and third-party SDK flows run in the \
                          browser on Fiuu and have no Direct API"
                    .to_owned(),
                connector: "Fiuu",
                context: fiuu_error_context(
                    "Send the Apple Pay / Google Pay token (encrypted or pre-decrypted) instead",
                    None
                ),
            }))
        }
        WalletData::AliPayQr(_)
        | WalletData::AliPayRedirect(_)
        | WalletData::AliPayHkRedirect(_)
        | WalletData::AmazonPayRedirect(_)
        | WalletData::MomoRedirect(_)
        | WalletData::KakaoPayRedirect(_)
        | WalletData::GoPayRedirect(_)
        | WalletData::GcashRedirect(_)
        | WalletData::DanaRedirect {}
        | WalletData::GrabpayRedirect {}
        | WalletData::MbWayRedirect(_)
        | WalletData::MobilePayRedirect(_)
        | WalletData::PaypalRedirect(_)
        | WalletData::PaypalSdk(_)
        | WalletData::Paze(_)
        | WalletData::SamsungPay(_)
        | WalletData::TwintRedirect {}
        | WalletData::VippsRedirect {}
        | WalletData::TouchNGoRedirect(_)
        | WalletData::WeChatPayRedirect(_)
        | WalletData::WeChatPayQr(_)
        | WalletData::CashappQr(_)
        | WalletData::SwishQr(_)
        | WalletData::Mifinity(_)
        | WalletData::RevolutPay(_)
        | WalletData::BluecodeRedirect { .. }
        | WalletData::MbWay(_)
        | WalletData::Satispay(_)
        | WalletData::Wero(_)
        | WalletData::LazyPayRedirect(_)
        | WalletData::PhonePeRedirect(_)
        | WalletData::BillDeskRedirect(_)
        | WalletData::CashfreeRedirect(_)
        | WalletData::PayURedirect(_)
        | WalletData::EaseBuzzRedirect(_)
        | WalletData::PaymayaRedirect(_)
        | WalletData::PayhereRedirect {}
        | WalletData::QwikcilverWalletDirect(_)
        | WalletData::Skrill(_)
        | WalletData::Neteller(_) => Err(IntegrationError::NotImplemented(
            utils::get_unimplemented_payment_method_error_message("fiuu"),
            fiuu_error_context(
                "Use Apple Pay or Google Pay; Fiuu accepts no other wallet here",
                None,
            ),
        )
        .into()),
    }
}

/// `ApplePay` field for PSP decryption: base64 of the whole Apple Pay token JSON
/// (`paymentData`, `paymentMethod`, `transactionIdentifier`), as Fiuu's Apple Pay plugin
/// (`APManager.swift`) posts it. Hyperswitch carries `paymentData` base64-encoded, so it is
/// decoded first.
fn fiuu_apple_pay_token(
    apple_pay_data: &ApplePayWalletData,
) -> Result<Secret<String>, error_stack::Report<IntegrationError>> {
    let invalid_token = || IntegrationError::InvalidWalletToken {
        wallet_name: "Apple Pay".to_owned(),
        context: fiuu_error_context(
            "Pass the Apple Pay paymentData exactly as received, base64-encoded",
            None,
        ),
    };
    let payment_data: Value =
        serde_json::from_str(apple_pay_data.get_applepay_decoded_payment_data()?.peek())
            .change_context(invalid_token())?;
    let token = serde_json::json!({
        "paymentData": payment_data,
        "paymentMethod": {
            "displayName": apple_pay_data.payment_method.display_name,
            "network": apple_pay_data.payment_method.network,
            "type": apple_pay_data.payment_method.pm_type,
        },
        "transactionIdentifier": apple_pay_data.transaction_identifier,
    });
    let token = serde_json::to_vec(&token).change_context(invalid_token())?;
    Ok(Secret::new(consts::BASE64_ENGINE.encode(token)))
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<&GooglePayWalletData> for FiuuPaymentMethodData<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(data: &GooglePayWalletData) -> Result<Self, Self::Error> {
        match &data.tokenization_data {
            GpayTokenizationData::Encrypted(encrypted) => {
                // PSP decryption needs the Google Pay token and its type.
                if encrypted.token.is_empty() || encrypted.token_type.is_empty() {
                    return Err(error_stack::report!(IntegrationError::MissingRequiredField {
                        field_name: "gpay wallet_token",
                        context: fiuu_error_context("Pass tokenization_data.token and tokenization_data.type from Google Pay", None),
                    }));
                }
                Ok(Self::FiuuGooglePayData(Box::new(FiuuGooglePayData {
                    txn_channel: TxnChannel::Creditan,
                    api_version: GOOGLEPAY_API_VERSION,
                    api_version_minor: GOOGLEPAY_API_VERSION_MINOR,
                    account_verified: data
                        .info
                        .assurance_details
                        .as_ref()
                        .map(|details| details.account_verified),
                    card_holder_authenticated: data
                        .info
                        .assurance_details
                        .as_ref()
                        .map(|details| details.card_holder_authenticated),
                    card_details: data.info.card_details.clone(),
                    card_network: data.info.card_network.clone(),
                    token: Secret::new(encrypted.token.clone()),
                    tokenization_data_type: Secret::new(encrypted.token_type.clone()),
                    pm_type: data.pm_type.clone(),
                    token_type: FiuuTokenType::GooglePay,
                    // non_3ds field Applicable to card processing via specific processor using specific currency for pre-approved partner only.
                    // Equal to 0 by default and 1 for non-3DS transaction, That is why it is hardcoded to 1 for googlepay transactions.
                    non_3ds: 1,
                })))
            }
            GpayTokenizationData::Decrypted(decrypted) => {
                // TOKEN_CRYPTOGRAM is required with CC_TOKEN (Fiuu Direct Server API).
                let token_cryptogram = decrypted.cryptogram.clone().ok_or_else(|| {
                    error_stack::report!(IntegrationError::MissingRequiredField {
                        field_name: "tokenization_data.cryptogram",
                        context: fiuu_error_context(
                            "Pass the cryptogram of the decrypted Google Pay network token",
                            None
                        ),
                    })
                })?;
                let invalid_expiry =
                    |field_name: &'static str| IntegrationError::InvalidDataFormat {
                        field_name,
                        context: fiuu_error_context(
                            "Pass card_exp_month as MM and card_exp_year as YY or YYYY",
                            None,
                        ),
                    };
                Ok(Self::FiuuGooglePayDecryptedData(Box::new(
                    FiuuGooglePayDecryptedData {
                        txn_channel: TxnChannel::Creditan,
                        cc_month: decrypted
                            .get_expiry_month()
                            .change_context(invalid_expiry("card_exp_month"))?,
                        cc_year: decrypted
                            .get_four_digit_expiry_year()
                            .change_context(invalid_expiry("card_exp_year"))?,
                        last4: Secret::new(
                            decrypted.application_primary_account_number.get_last4(),
                        ),
                        cc_token: decrypted.application_primary_account_number.clone(),
                        token_cryptogram,
                        eci: decrypted.eci_indicator.clone(),
                        token_type: FiuuTokenType::GooglePay,
                        // Wallets are non-3DS on the Direct API, as for encrypted Google Pay.
                        non_3ds: 1,
                    },
                )))
            }
        }
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ApplePayDecryptedData> for FiuuPaymentMethodData<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(decrypt_data: ApplePayDecryptedData) -> Result<Self, Self::Error> {
        Ok(Self::FiuuApplePayData(Box::new(FiuuApplePayData {
            txn_channel: TxnChannel::Creditan,
            cc_month: decrypt_data.get_expiry_month(),
            cc_year: decrypt_data.get_four_digit_expiry_year(),
            last4: Secret::new(decrypt_data.application_primary_account_number.get_last4()),
            cc_token: decrypt_data.application_primary_account_number,
            eci: decrypt_data.payment_data.eci_indicator,
            token_cryptogram: decrypt_data.payment_data.online_payment_cryptogram,
            token_type: FiuuTokenType::ApplePay,
            // non_3ds field Applicable to card processing via specific processor using specific currency for pre-approved partner only.
            // Equal to 0 by default and 1 for non-3DS transaction, That is why it is hardcoded to 1 for apple pay decrypt flow transactions.
            non_3ds: 1,
        })))
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PaymentsResponse {
    pub reference_no: Option<String>,
    #[serde(rename = "TxnID")]
    pub txn_id: String,
    pub txn_type: Option<String>,
    pub txn_currency: Option<Currency>,
    pub txn_amount: Option<StringMajorUnit>,
    pub txn_channel: Option<String>,
    pub txn_data: TxnData,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DuitNowQrCodeResponse {
    pub reference_no: Option<String>,
    pub txn_type: Option<String>,
    pub txn_currency: Option<Currency>,
    pub txn_amount: Option<StringMajorUnit>,
    pub txn_channel: Option<String>,
    #[serde(rename = "TxnID")]
    pub txn_id: String,
    pub txn_data: QrTxnData,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct QrTxnData {
    pub request_data: QrRequestData,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QrRequestData {
    pub qr_data: Secret<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FiuuPaymentsResponse {
    PaymentResponse(Box<PaymentsResponse>),
    QRPaymentResponse(Box<DuitNowQrCodeResponse>),
    Error(FiuuErrorResponse),
    RecurringResponse(Vec<Box<FiuuRecurringResponse>>),
}

/// SetupMandate response: Zero Dollar Verification (`VerificationID` present) or the Direct API.
/// `ZeroAuth` is tried first because a Direct body never carries `VerificationID`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FiuuSetupMandateResponse {
    ZeroAuth(Box<FiuuZeroAuthResponse>),
    Direct(FiuuPaymentsResponse),
}

/// Zero Dollar Verification response (Fiuu Standalone Card API,
/// "Zero Dollar Verification Response").
#[derive(Debug, Serialize, Deserialize)]
pub struct FiuuZeroAuthResponse {
    #[serde(rename = "VerificationID")]
    pub verification_id: String,
    /// 00 success, 11 failed verification.
    pub status: FiuuDirectTxnStatus,
    #[serde(rename = "schemeTranID")]
    pub scheme_tran_id: Option<Secret<String>>,
    #[serde(rename = "ProcessorCCVResponse")]
    pub processor_ccv_response: Option<String>,
    #[serde(rename = "ProcessorResponseCode")]
    pub processor_response_code: Option<String>,
    #[serde(rename = "MerchantAdviceCode")]
    pub merchant_advice_code: Option<String>,
    #[serde(rename = "ECI")]
    pub eci: Option<String>,
    pub error_code: Option<String>,
    pub error_desc: Option<String>,
    /// Present only when the verification needs a 3DS redirect.
    #[serde(rename = "TxnData")]
    pub txn_data: Option<TxnData>,
}

impl FiuuZeroAuthResponse {
    /// The verification's scheme/CVV fields share the Direct `extraP` meaning, so they reuse
    /// its network-id and payment-checks mapping.
    fn as_extra_parameters(&self) -> ExtraParameters {
        ExtraParameters {
            token: None,
            scheme_transaction_id: self.scheme_tran_id.clone(),
            processor_response_code: self.processor_response_code.clone(),
            processor_cvv_response: self.processor_ccv_response.clone(),
            merchant_advice_code: self.merchant_advice_code.clone(),
            avs_response: None,
            avs_street: None,
            avs_zip: None,
            eci: self.eci.clone(),
            cc_brand: None,
        }
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<FiuuSetupMandateResponse, Self>>
    for RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<FiuuSetupMandateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;
        let data = match response {
            FiuuSetupMandateResponse::Direct(response) => {
                return Self::try_from(ResponseRouterData {
                    response,
                    router_data,
                    http_code,
                })
            }
            FiuuSetupMandateResponse::ZeroAuth(data) => data,
        };
        // A verification that needs 3DS returns TxnData REDIRECT.
        if let Some(txn_data) = data
            .txn_data
            .as_ref()
            .filter(|txn_data| txn_data.request_type == Some(FiuuAuthnRequestType::Redirect))
        {
            let form_fields = match &txn_data.request_data {
                RequestData::RedirectData(fields) => fields.clone().unwrap_or_default(),
                RequestData::NonThreeDS(_) => HashMap::new(),
            };
            let redirection_data = RedirectForm::Form {
                endpoint: txn_data.request_url.clone(),
                method: txn_data.request_method.into(),
                form_fields,
            };
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::AuthenticationPending,
                    ..router_data.resource_common_data
                },
                response: Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(data.verification_id.clone()),
                    redirection_data: Some(Box::new(redirection_data)),
                    mandate_reference: None,
                    connector_metadata: None,
                    network_txn_id: None,
                    network_txn_link_id: None,
                    connector_response_reference_id: None,
                    incremental_authorization_allowed: None,
                    status_code: http_code,
                    splits: None,
                    payment_account_reference: None,
                }),
                ..router_data
            });
        }
        let status = match data.status {
            FiuuDirectTxnStatus::Success => common_enums::AttemptStatus::Charged,
            FiuuDirectTxnStatus::Failure => common_enums::AttemptStatus::Failure,
            FiuuDirectTxnStatus::Pending => common_enums::AttemptStatus::Pending,
            // Undocumented status is inconclusive: keep it non-terminal.
            FiuuDirectTxnStatus::Unknown => {
                tracing::warn!("fiuu: unknown cc_verification status; mapping to Pending");
                common_enums::AttemptStatus::Pending
            }
        };
        let extra_parameters = data.as_extra_parameters();
        let (network_txn_id, network_decline_code, network_advice_code) =
            extra_parameters.network_ids();
        let response = if status == common_enums::AttemptStatus::Failure {
            Err(ErrorResponse {
                code: data
                    .error_code
                    .clone()
                    .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string()),
                message: data
                    .error_desc
                    .clone()
                    .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string()),
                reason: data.error_desc.clone(),
                status_code: http_code,
                attempt_status: Some(FlowStatus::Payment(status)),
                connector_transaction_id: Some(data.verification_id.clone()),
                network_advice_code,
                network_decline_code,
                network_error_message: None,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            })
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(data.verification_id.clone()),
                redirection_data: None,
                // Direct MIT takes `OriginalTranID=verificationID_<id>` for a mandate set up by
                // Zero Dollar Verification.
                mandate_reference: Some(Box::new(MandateReference {
                    connector_mandate_id: Some(format!("verificationID_{}", data.verification_id)),
                    payment_method_id: None,
                    connector_mandate_request_reference_id: None,
                    mandate_metadata: None,
                })),
                connector_metadata: None,
                network_txn_id,
                network_txn_link_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                status_code: http_code,
                splits: None,
                payment_account_reference: None,
            })
        };
        let connector_response = extra_parameters
            .card_connector_response()
            .or(router_data.resource_common_data.connector_response.clone());
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                connector_response,
                ..router_data.resource_common_data
            },
            response,
            ..router_data
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FiuuRecurringResponse {
    status: FiuuRecurringStautus,
    #[serde(rename = "orderid")]
    order_id: Option<String>,
    #[serde(rename = "tranID")]
    tran_id: Option<u64>,
    reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum FiuuRecurringStautus {
    Accepted,
    Failed,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct TxnData {
    #[serde(rename = "RequestURL")]
    pub request_url: String,
    pub request_type: Option<FiuuAuthnRequestType>,
    pub request_data: RequestData,
    pub request_method: FiuuRequestMethod,
}

/// `RequestMethod` [GET/POST] of a Fiuu redirect.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum FiuuRequestMethod {
    Get,
    Post,
}

impl From<FiuuRequestMethod> for Method {
    fn from(m: FiuuRequestMethod) -> Self {
        match m {
            FiuuRequestMethod::Get => Self::Get,
            FiuuRequestMethod::Post => Self::Post,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RequestData {
    NonThreeDS(Box<NonThreeDSResponseData>),
    RedirectData(Option<HashMap<String, String>>),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QrCodeData {
    #[serde(rename = "tranID")]
    pub tran_id: String,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NonThreeDSResponseData {
    #[serde(rename = "tranID")]
    pub tran_id: String,
    pub status: FiuuDirectTxnStatus,
    #[serde(rename = "extraP")]
    pub extra_parameters: Option<ExtraParameters>,
    pub error_code: Option<String>,
    pub error_desc: Option<String>,
}

/// Direct API `RequestData.status` for RequestType RESPONSE: 00 success, 11 failure,
/// 22 pending. Shared by Authorize, SetupMandate and RepeatPayment (Direct path).
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum FiuuDirectTxnStatus {
    #[serde(rename = "00")]
    Success,
    #[serde(rename = "11")]
    Failure,
    #[serde(rename = "22")]
    Pending,
    #[serde(other)]
    Unknown,
}

/// `extraP` of the Direct response, notification and requery. Every field is optional so
/// payloads that carry only `token` still parse.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExtraParameters {
    pub token: Option<Secret<String>>,
    #[serde(rename = "SchemeTransactionID")]
    pub scheme_transaction_id: Option<Secret<String>>,
    #[serde(rename = "ProcessorResponseCode")]
    pub processor_response_code: Option<String>,
    #[serde(rename = "ProcessorCVVResponse")]
    pub processor_cvv_response: Option<String>,
    #[serde(rename = "MerchantAdviceCode")]
    pub merchant_advice_code: Option<String>,
    #[serde(rename = "AvsResponse")]
    pub avs_response: Option<String>,
    #[serde(rename = "AvsStreet")]
    pub avs_street: Option<String>,
    #[serde(rename = "AvsZip")]
    pub avs_zip: Option<String>,
    #[serde(rename = "ECI")]
    pub eci: Option<String>,
    #[serde(rename = "ccbrand")]
    pub cc_brand: Option<String>,
}

impl ExtraParameters {
    /// (network_txn_id, network_decline_code, network_advice_code) from
    /// SchemeTransactionID / ProcessorResponseCode / MerchantAdviceCode.
    pub fn network_ids(&self) -> (Option<String>, Option<String>, Option<String>) {
        (
            self.scheme_transaction_id
                .as_ref()
                .map(|id| id.peek().to_owned()),
            self.processor_response_code.clone(),
            self.merchant_advice_code.clone(),
        )
    }

    /// CVV / AVS / ECI results as card `payment_checks`; `None` when Fiuu returned none of them.
    pub fn card_connector_response(&self) -> Option<ConnectorResponseData> {
        let checks = [
            ("cvv_result", &self.processor_cvv_response),
            ("avs_result", &self.avs_response),
            ("avs_street", &self.avs_street),
            ("avs_zip", &self.avs_zip),
            ("eci", &self.eci),
        ]
        .into_iter()
        .filter_map(|(key, value)| value.clone().map(|v| (key.to_owned(), Value::String(v))))
        .collect::<serde_json::Map<String, Value>>();
        if checks.is_empty() && self.cc_brand.is_none() {
            return None;
        }
        Some(ConnectorResponseData::with_additional_payment_method_data(
            AdditionalPaymentMethodConnectorResponse::Card {
                authentication_data: None,
                payment_checks: (!checks.is_empty()).then_some(Value::Object(checks)),
                card_network: self.cc_brand.clone(),
                domestic_network: None,
                auth_code: None,
                processor_card_network: None,
                card_subtype: None,
                card_segment_type: None,
                funding_source: None,
                card_type: None,
                issuer_name: None,
                issuer_country: None,
            },
        ))
    }
}

trait GetRequestIsAutoCapture {
    fn is_auto_capture(&self) -> bool;
}

impl<T: PaymentMethodDataTypes> GetRequestIsAutoCapture for PaymentsAuthorizeData<T> {
    fn is_auto_capture(&self) -> bool {
        Self::is_auto_capture(self)
    }
}

impl<T: PaymentMethodDataTypes> GetRequestIsAutoCapture for RepeatPaymentData<T> {
    fn is_auto_capture(&self) -> bool {
        Self::is_auto_capture(self)
    }
}

impl<T: PaymentMethodDataTypes> GetRequestIsAutoCapture for SetupMandateRequestData<T> {
    fn is_auto_capture(&self) -> bool {
        // SetupMandateRequestData does not expose a dedicated
        // `is_auto_capture` helper; mirror the same capture-method intent
        // mapping used by PaymentsAuthorizeData / RepeatPaymentData.
        !matches!(
            self.capture_method,
            Some(CaptureMethod::Manual)
                | Some(CaptureMethod::ManualMultiple)
                | Some(CaptureMethod::Scheduled)
        )
    }
}

impl<F, R> TryFrom<ResponseRouterData<FiuuPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, R, PaymentsResponseData>
where
    R: GetRequestIsAutoCapture,
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<FiuuPaymentsResponse, Self>) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code: _http_code,
        } = item;
        match response {
            FiuuPaymentsResponse::QRPaymentResponse(ref response) => Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::AuthenticationPending,
                    ..router_data.resource_common_data
                },
                response: Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(response.txn_id.clone()),
                    redirection_data: None,
                    mandate_reference: None,
                    connector_metadata: get_qr_metadata(response)?,
                    network_txn_id: None,
                    network_txn_link_id: None,
                    connector_response_reference_id: None,
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                    splits: None,
                    payment_account_reference: None,
                }),
                ..router_data
            }),
            FiuuPaymentsResponse::Error(error) => Ok(Self {
                response: Err(ErrorResponse {
                    code: error.error_code.clone(),
                    message: error.error_desc.clone(),
                    reason: Some(error.error_desc),
                    status_code: item.http_code,
                    // Direct API validation failure (e.g. P03, P10): the payment did not happen.
                    attempt_status: Some(FlowStatus::Payment(common_enums::AttemptStatus::Failure)),
                    connector_transaction_id: None,
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                }),
                ..router_data
            }),
            FiuuPaymentsResponse::PaymentResponse(data) => match data.txn_data.request_data {
                RequestData::RedirectData(redirection_data) => {
                    let redirection_data = Some(RedirectForm::Form {
                        endpoint: data.txn_data.request_url.to_string(),
                        method: data.txn_data.request_method.into(),
                        form_fields: redirection_data.unwrap_or_default(),
                    });
                    Ok(Self {
                        resource_common_data: PaymentFlowData {
                            status: common_enums::AttemptStatus::AuthenticationPending,
                            ..router_data.resource_common_data
                        },
                        response: Ok(PaymentsResponseData::TransactionResponse {
                            resource_id: ResponseId::ConnectorTransactionId(data.txn_id),
                            redirection_data: redirection_data.map(Box::new),
                            mandate_reference: None,
                            connector_metadata: None,
                            network_txn_id: None,
                            network_txn_link_id: None,
                            connector_response_reference_id: data.reference_no,
                            incremental_authorization_allowed: None,
                            status_code: item.http_code,
                            splits: None,
                            payment_account_reference: None,
                        }),
                        ..router_data
                    })
                }
                RequestData::NonThreeDS(non_threeds_data) => {
                    // RequestType RESPONSE status.
                    let status = match non_threeds_data.status {
                        FiuuDirectTxnStatus::Success => {
                            if router_data.request.is_auto_capture() {
                                common_enums::AttemptStatus::Charged
                            } else {
                                common_enums::AttemptStatus::Authorized
                            }
                        }
                        FiuuDirectTxnStatus::Failure => common_enums::AttemptStatus::Failure,
                        FiuuDirectTxnStatus::Pending => common_enums::AttemptStatus::Pending,
                        // An undocumented status is inconclusive: keep it non-terminal so the
                        // notification / requery settles it.
                        FiuuDirectTxnStatus::Unknown => {
                            tracing::warn!(
                                "fiuu: unknown Direct API RESPONSE status; mapping to Pending"
                            );
                            common_enums::AttemptStatus::Pending
                        }
                    };
                    let (network_txn_id, network_decline_code, network_advice_code) =
                        non_threeds_data
                            .extra_parameters
                            .as_ref()
                            .map(ExtraParameters::network_ids)
                            .unwrap_or((None, None, None));
                    let response = if status == common_enums::AttemptStatus::Failure {
                        Err(ErrorResponse {
                            code: non_threeds_data
                                .error_code
                                .clone()
                                .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string()),
                            message: non_threeds_data
                                .error_desc
                                .clone()
                                .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string()),
                            reason: non_threeds_data.error_desc.clone(),
                            status_code: item.http_code,
                            attempt_status: Some(FlowStatus::Payment(status)),
                            connector_transaction_id: Some(data.txn_id),
                            network_advice_code,
                            network_decline_code,
                            network_error_message: None,
                            typed_connector_response: None,
                            raw_connector_response: None,
                            raw_connector_request: None,
                            typed_connector_request: None,
                        })
                    } else {
                        let mandate_reference = non_threeds_data
                            .extra_parameters
                            .as_ref()
                            .and_then(|extra_p| {
                                extra_p.token.as_ref().map(|token| MandateReference {
                                    connector_mandate_id: Some(token.clone().expose()),
                                    payment_method_id: None,
                                    connector_mandate_request_reference_id: None,
                                    mandate_metadata: None,
                                })
                            });
                        Ok(PaymentsResponseData::TransactionResponse {
                            resource_id: ResponseId::ConnectorTransactionId(data.txn_id.clone()),
                            redirection_data: None,
                            mandate_reference: mandate_reference.map(Box::new),
                            connector_metadata: None,
                            network_txn_id,
                            network_txn_link_id: None,
                            connector_response_reference_id: data.reference_no.clone(),
                            incremental_authorization_allowed: None,
                            status_code: item.http_code,
                            splits: None,
                            payment_account_reference: None,
                        })
                    };
                    let connector_response = non_threeds_data
                        .extra_parameters
                        .as_ref()
                        .and_then(ExtraParameters::card_connector_response)
                        .or(router_data.resource_common_data.connector_response.clone());
                    Ok(Self {
                        resource_common_data: PaymentFlowData {
                            status,
                            connector_response,
                            ..router_data.resource_common_data
                        },
                        response,
                        ..router_data
                    })
                }
            },
            FiuuPaymentsResponse::RecurringResponse(ref recurring_response_vec) => {
                // The response is an array of records; pick ours by orderid (Fiuu Recurring API).
                let order_id = &router_data
                    .resource_common_data
                    .connector_request_reference_id;
                let recurring_response_item = recurring_response_vec
                    .iter()
                    .find(|record| record.order_id.as_ref() == Some(order_id));
                let router_data_response = match recurring_response_item {
                    Some(recurring_response) => {
                        let status = match recurring_response.status {
                            FiuuRecurringStautus::Accepted => common_enums::AttemptStatus::Charged,
                            FiuuRecurringStautus::Failed => common_enums::AttemptStatus::Failure,
                            // Undocumented status is inconclusive: keep the prior status.
                            FiuuRecurringStautus::Unknown => {
                                tracing::warn!(
                                    "fiuu: unknown recurring status; keeping prior attempt status"
                                );
                                router_data.resource_common_data.status
                            }
                        };
                        let connector_transaction_id = recurring_response
                            .tran_id
                            .map_or(ResponseId::NoResponseId, |tran_id| {
                                ResponseId::ConnectorTransactionId(tran_id.to_string())
                            });
                        let response = if status == common_enums::AttemptStatus::Failure {
                            Err(ErrorResponse {
                                code: recurring_response
                                    .reason
                                    .clone()
                                    .unwrap_or_else(|| consts::NO_ERROR_CODE.to_string()),
                                message: recurring_response
                                    .reason
                                    .clone()
                                    .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string()),
                                reason: recurring_response.reason.clone(),
                                status_code: item.http_code,
                                attempt_status: Some(FlowStatus::Payment(status)),
                                connector_transaction_id: recurring_response
                                    .tran_id
                                    .map(|id| id.to_string()),
                                network_advice_code: None,
                                network_decline_code: None,
                                network_error_message: None,
                                typed_connector_response: None,
                                raw_connector_response: None,
                                raw_connector_request: None,
                                typed_connector_request: None,
                            })
                        } else {
                            Ok(PaymentsResponseData::TransactionResponse {
                                resource_id: connector_transaction_id,
                                redirection_data: None,
                                mandate_reference: None,
                                connector_metadata: None,
                                network_txn_id: None,
                                network_txn_link_id: None,
                                connector_response_reference_id: None,
                                incremental_authorization_allowed: None,
                                status_code: item.http_code,
                                splits: None,
                                payment_account_reference: None,
                            })
                        };
                        Self {
                            resource_common_data: PaymentFlowData {
                                status,
                                ..router_data.resource_common_data
                            },
                            response,
                            ..router_data
                        }
                    }
                    None => {
                        // No record for our orderid: nothing to update the payment with, keep the prior status.
                        let response = Ok(PaymentsResponseData::TransactionResponse {
                            resource_id: ResponseId::NoResponseId,
                            redirection_data: None,
                            mandate_reference: None,
                            connector_metadata: None,
                            network_txn_id: None,
                            network_txn_link_id: None,
                            connector_response_reference_id: None,
                            incremental_authorization_allowed: None,
                            status_code: item.http_code,
                            splits: None,
                            payment_account_reference: None,
                        });
                        Self {
                            response,
                            ..router_data
                        }
                    }
                };
                Ok(router_data_response)
            }
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct FiuuRefundRequest {
    pub refund_type: RefundType,
    #[serde(rename = "MerchantID")]
    pub merchant_id: Secret<String>,
    #[serde(rename = "RefID")]
    pub ref_id: String,
    #[serde(rename = "TxnID")]
    pub txn_id: String,
    pub amount: StringMajorUnit,
    pub signature: Secret<String>,
    #[serde(rename = "notify_url")]
    pub notify_url: Option<Url>,
}
#[derive(Debug, Serialize, Display)]
pub enum RefundType {
    #[serde(rename = "P")]
    #[strum(serialize = "P")]
    Partial,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        FiuuRouterData<RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    > for FiuuRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: FiuuRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth: FiuuAuthType = FiuuAuthType::try_from(&item.router_data.connector_config)?;
        let merchant_id = auth.merchant_id.peek().to_string();
        let amount = item
            .connector
            .amount_converter
            .convert(&item.router_data.request.refund_amount)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Pass refund_amount in a currency Fiuu supports".to_owned(),
                    ),
                    doc_url: Some(FIUU_DIRECT_API_DOC_URL.to_owned()),
                    additional_context: Some(
                        "refund Amount is sent in major units (e.g. 5.00)".to_owned(),
                    ),
                },
            })?;
        let txn_amount = amount;
        // RefID is mandatory (Fiuu API Spec for Merchant, "Advanced Full/Partial Refund").
        let reference_no = item
            .router_data
            .resource_common_data
            .refund_id
            .clone()
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "refund_id",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Pass refund_id; Fiuu requires a unique merchant RefID per refund"
                            .to_owned(),
                    ),
                    doc_url: Some(FIUU_DIRECT_API_DOC_URL.to_owned()),
                    additional_context: None,
                },
            })?;
        // The final refund status (00/11) only arrives via notify_url.
        let webhook_url = item.router_data.request.webhook_url.as_deref().ok_or(
            IntegrationError::MissingRequiredField {
                field_name: "webhook_url",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Pass webhook_url; Fiuu sends the final refund status to notify_url"
                            .to_owned(),
                    ),
                    doc_url: Some(FIUU_DIRECT_API_DOC_URL.to_owned()),
                    additional_context: None,
                },
            },
        )?;
        let notify_url =
            Url::parse(webhook_url).change_context(IntegrationError::InvalidDataFormat {
                field_name: "webhook_url",
                context: IntegrationErrorContext {
                    suggested_action: Some("Pass webhook_url as an absolute URL".to_owned()),
                    doc_url: Some(FIUU_DIRECT_API_DOC_URL.to_owned()),
                    additional_context: None,
                },
            })?;
        let txn_id = item.router_data.request.connector_transaction_id.clone();
        let secret_key = auth.secret_key.peek().to_string();
        Ok(Self {
            refund_type: RefundType::Partial,
            merchant_id: auth.merchant_id,
            ref_id: reference_no.clone(),
            txn_id: txn_id.clone(),
            amount: txn_amount.clone(),
            signature: calculate_signature(format!(
                "{}{merchant_id}{reference_no}{txn_id}{}{secret_key}",
                RefundType::Partial,
                txn_amount.get_amount_as_string()
            ))?,
            notify_url: Some(notify_url),
        })
    }
}

/// Refund API `Status` values (Fiuu API Spec for Merchant, "Advanced Full/Partial Refund").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, strum::Display)]
pub enum FiuuRefundApiStatus {
    #[serde(rename = "00")]
    #[strum(serialize = "00")]
    Success,
    #[serde(rename = "11")]
    #[strum(serialize = "11")]
    Rejected,
    #[serde(rename = "22")]
    #[strum(serialize = "22")]
    Pending,
    #[serde(other)]
    #[strum(serialize = "unknown")]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct FiuuRefundSuccessResponse {
    #[serde(rename = "RefundID")]
    refund_id: i64,
    status: FiuuRefundApiStatus,
    #[serde(rename = "reason")]
    reason: Option<String>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FiuuRefundResponse {
    Success(FiuuRefundSuccessResponse),
    Error(FiuuErrorResponse),
}
impl<F> TryFrom<ResponseRouterData<FiuuRefundResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<FiuuRefundResponse, Self>) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code: _http_code,
        } = item;
        match response {
            FiuuRefundResponse::Error(error) => Ok(Self {
                response: Err(ErrorResponse {
                    code: error.error_code.clone(),
                    message: error.error_desc.clone(),
                    reason: Some(error.error_desc),
                    status_code: item.http_code,
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                }),
                ..router_data
            }),
            FiuuRefundResponse::Success(refund_data) => {
                let refund_status = match refund_data.status {
                    FiuuRefundApiStatus::Success => common_enums::RefundStatus::Success,
                    FiuuRefundApiStatus::Rejected => common_enums::RefundStatus::Failure,
                    FiuuRefundApiStatus::Pending => common_enums::RefundStatus::Pending,
                    // Undocumented Status: keep the refund open; RSync / notify_url settles it.
                    FiuuRefundApiStatus::Unknown => common_enums::RefundStatus::Pending,
                };
                if refund_status == common_enums::RefundStatus::Failure {
                    Ok(Self {
                        response: Err(ErrorResponse {
                            code: refund_data.status.to_string(),
                            message: refund_data
                                .reason
                                .clone()
                                .unwrap_or(consts::NO_ERROR_MESSAGE.to_string()),
                            reason: refund_data.reason.clone(),
                            status_code: item.http_code,
                            attempt_status: None,
                            connector_transaction_id: Some(refund_data.refund_id.to_string()),
                            network_advice_code: None,
                            network_decline_code: None,
                            network_error_message: None,
                            typed_connector_response: None,
                            raw_connector_response: None,
                            raw_connector_request: None,
                            typed_connector_request: None,
                        }),
                        ..router_data
                    })
                } else {
                    Ok(Self {
                        response: Ok(RefundsResponseData {
                            connector_refund_id: refund_data.refund_id.clone().to_string(),
                            refund_status,
                            status_code: item.http_code,
                            acquirer_reference_number: None,
                        }),
                        ..router_data
                    })
                }
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FiuuErrorResponse {
    pub error_code: String,
    pub error_desc: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FiuuPaymentSyncRequest {
    amount: StringMajorUnit,
    #[serde(rename = "txID")]
    tx_id: String,
    domain: String,
    skey: Secret<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum FiuuPaymentResponse {
    FiuuPaymentSyncResponse(FiuuPaymentSyncResponse),
    FiuuWebhooksPaymentResponse(FiuuWebhooksPaymentResponse),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct FiuuPaymentSyncResponse {
    stat_code: StatCode,
    stat_name: StatName,
    #[serde(rename = "TranID")]
    tran_id: String,
    error_code: Option<String>,
    error_desc: Option<String>,
    #[serde(rename = "miscellaneous")]
    miscellaneous: Option<HashMap<String, Secret<String>>>,
    /// Requery carries SchemeTransactionID / ProcessorResponseCode / ProcessorCVVResponse /
    /// MerchantAdviceCode / ECI as flat keys, the same names as `extraP`
    /// (Fiuu Direct Status Requery API).
    #[serde(flatten)]
    extra_parameters: ExtraParameters,
}

#[derive(Debug, Serialize, Deserialize, Display, Clone, PartialEq)]
pub enum StatCode {
    #[serde(rename = "00")]
    Success,
    #[serde(rename = "11")]
    Failure,
    #[serde(rename = "22")]
    Pending,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Serialize, Deserialize, Display, Clone, Copy, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum StatName {
    Captured,
    Settled,
    Authorized,
    Failed,
    Cancelled,
    Chargeback,
    Release,
    #[serde(rename = "reject/hold")]
    RejectHold,
    Blocked,
    #[serde(rename = "ReqCancel")]
    ReqCancel,
    #[serde(rename = "ReqChargeback")]
    ReqChargeback,
    #[serde(rename = "Pending")]
    Pending,
    #[serde(other)]
    Unknown,
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        FiuuRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for FiuuPaymentSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: FiuuRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = FiuuAuthType::try_from(&item.router_data.connector_config)?;
        let txn_id = item
            .router_data
            .request
            .connector_transaction_id
            .get_connector_transaction_id()
            .change_context(IntegrationError::MissingConnectorTransactionID {
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Pass the Fiuu TxnID returned by Authorize as connector_transaction_id"
                            .to_owned(),
                    ),
                    doc_url: Some(FIUU_DIRECT_API_DOC_URL.to_owned()),
                    additional_context: Some(
                        "gate-query txID is required to requery the payment".to_owned(),
                    ),
                },
            })?;
        let merchant_id = auth.merchant_id.peek().to_string();
        let verify_key = auth.verify_key.peek().to_string();
        let amount = item
            .connector
            .amount_converter
            .convert(&item.router_data.request.amount)
            .change_context(IntegrationError::AmountConversionFailed {
                context: Default::default(),
            })?;
        Ok(Self {
            amount: amount.clone(),
            tx_id: txn_id.clone(),
            domain: merchant_id.clone(),
            skey: calculate_signature(format!(
                "{txn_id}{merchant_id}{verify_key}{}",
                amount.get_amount_as_string()
            ))?,
        })
    }
}

struct ErrorInputs {
    encoded_data: Option<String>,
    response_error_code: Option<String>,
    response_error_desc: Option<String>,
}

struct ErrorDetails {
    pub code: String,
    pub message: String,
    pub reason: Option<String>,
}

impl TryFrom<ErrorInputs> for ErrorDetails {
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(value: ErrorInputs) -> Result<Self, Self::Error> {
        let query_params = value
            .encoded_data
            .as_ref()
            .map(|encoded_data| {
                serde_urlencoded::from_str::<FiuuPaymentRedirectResponse>(encoded_data)
            })
            .transpose()
            .change_context(ConnectorError::response_handling_failed_http_status_unknown())
            .attach_printable("Failed to deserialize FiuuPaymentRedirectResponse")?;
        let error_message = value
            .response_error_desc
            .as_ref()
            .filter(|s| !s.is_empty())
            .cloned()
            .or_else(|| {
                query_params
                    .as_ref()
                    .and_then(|qp| qp.error_desc.as_ref())
                    .filter(|s| !s.is_empty())
                    .cloned()
            });
        let error_code = value
            .response_error_code
            .as_ref()
            .filter(|s| !s.is_empty())
            .cloned()
            .or_else(|| {
                query_params
                    .as_ref()
                    .and_then(|qp| qp.error_code.as_ref())
                    .filter(|s| !s.is_empty())
                    .cloned()
            })
            .unwrap_or_else(|| consts::NO_ERROR_CODE.to_owned());
        Ok(Self {
            code: error_code,
            message: error_message
                .clone()
                .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_owned()),
            reason: error_message,
        })
    }
}

impl<F> TryFrom<ResponseRouterData<FiuuPaymentResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<FiuuPaymentResponse, Self>) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;
        match response {
            FiuuPaymentResponse::FiuuPaymentSyncResponse(response) => {
                let stat_name = response.stat_name;
                let stat_code = response.stat_code.clone();
                let txn_id = response.tran_id;
                let status = common_enums::AttemptStatus::try_from(FiuuSyncStatus {
                    stat_name,
                    stat_code,
                    prev_status: router_data.resource_common_data.status,
                })?;
                let (network_txn_id, network_decline_code, network_advice_code) =
                    response.extra_parameters.network_ids();
                let error_response = if status == common_enums::AttemptStatus::Failure {
                    let error_details = ErrorDetails::try_from(ErrorInputs {
                        encoded_data: router_data.request.encoded_data.clone(),
                        response_error_code: response.error_code.clone(),
                        response_error_desc: response.error_desc.clone(),
                    })?;
                    // No dispute API (spec §16): a chargeback is only visible as this StatName,
                    // so surface it as the code when Fiuu sends no ErrorCode.
                    let code = match (stat_name, error_details.code == consts::NO_ERROR_CODE) {
                        (StatName::Chargeback, true) => "chargeback".to_owned(),
                        (StatName::ReqChargeback, true) => "ReqChargeback".to_owned(),
                        _ => error_details.code,
                    };
                    Some(ErrorResponse {
                        status_code: http_code,
                        code,
                        message: error_details.message,
                        reason: error_details.reason,
                        attempt_status: Some(FlowStatus::Payment(
                            common_enums::AttemptStatus::Failure,
                        )),
                        connector_transaction_id: Some(txn_id.clone()),
                        network_advice_code,
                        network_decline_code,
                        network_error_message: None,
                        typed_connector_response: None,
                        raw_connector_response: None,
                        raw_connector_request: None,
                        typed_connector_request: None,
                    })
                } else {
                    None
                };
                let payments_response_data = PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(txn_id.clone().to_string()),
                    redirection_data: None,
                    mandate_reference: None,
                    connector_metadata: None,
                    network_txn_id,
                    network_txn_link_id: None,
                    connector_response_reference_id: None,
                    incremental_authorization_allowed: None,
                    status_code: http_code,
                    splits: None,
                    payment_account_reference: None,
                };
                let connector_response = response
                    .extra_parameters
                    .card_connector_response()
                    .or(router_data.resource_common_data.connector_response.clone());
                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status,
                        connector_response,
                        ..router_data.resource_common_data
                    },
                    response: error_response.map_or_else(|| Ok(payments_response_data), Err),
                    ..router_data
                })
            }
            FiuuPaymentResponse::FiuuWebhooksPaymentResponse(response) => {
                let status = common_enums::AttemptStatus::try_from(FiuuWebhookStatus {
                    capture_method: router_data.request.capture_method,
                    status: response.status,
                })?;
                let txn_id = response.tran_id;
                let extra_parameters = response.extra_parameters.as_ref().and_then(|extra_p| {
                    serde_json::from_str::<ExtraParameters>(&extra_p.clone().expose()).ok()
                });
                let mandate_reference = extra_parameters.as_ref().and_then(|extra_p| {
                    extra_p.token.as_ref().map(|token| MandateReference {
                        connector_mandate_id: Some(token.clone().expose()),
                        payment_method_id: None,
                        connector_mandate_request_reference_id: None,
                        mandate_metadata: None,
                    })
                });
                // extraP.SchemeTransactionID is the NTID (spec §12).
                let network_txn_id = extra_parameters
                    .as_ref()
                    .and_then(|extra_p| extra_p.network_ids().0);
                let error_response = if status == common_enums::AttemptStatus::Failure {
                    let error_details = ErrorDetails::try_from(ErrorInputs {
                        encoded_data: router_data.request.encoded_data.clone(),
                        response_error_code: response.error_code.clone(),
                        response_error_desc: response.error_desc.clone(),
                    })?;
                    Some(ErrorResponse {
                        status_code: http_code,
                        code: error_details.code,
                        message: error_details.message,
                        reason: error_details.reason,
                        attempt_status: Some(FlowStatus::Payment(
                            common_enums::AttemptStatus::Failure,
                        )),
                        connector_transaction_id: Some(txn_id.clone()),
                        network_advice_code: None,
                        network_decline_code: None,
                        network_error_message: None,
                        typed_connector_response: None,
                        raw_connector_response: None,
                        raw_connector_request: None,
                        typed_connector_request: None,
                    })
                } else {
                    None
                };
                let payments_response_data = PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(txn_id.clone().to_string()),
                    redirection_data: None,
                    mandate_reference: mandate_reference.map(Box::new),
                    connector_metadata: None,
                    network_txn_id,
                    network_txn_link_id: None,
                    connector_response_reference_id: None,
                    incremental_authorization_allowed: None,
                    status_code: http_code,
                    splits: None,
                    payment_account_reference: None,
                };
                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status,
                        ..router_data.resource_common_data
                    },
                    response: error_response.map_or_else(|| Ok(payments_response_data), Err),
                    ..router_data
                })
            }
        }
    }
}

pub struct FiuuWebhookStatus {
    pub capture_method: Option<CaptureMethod>,
    pub status: FiuuPaymentWebhookStatus,
}

impl TryFrom<FiuuWebhookStatus> for common_enums::AttemptStatus {
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(webhook_status: FiuuWebhookStatus) -> Result<Self, Self::Error> {
        match webhook_status.status {
            FiuuPaymentWebhookStatus::Success => match webhook_status.capture_method {
                Some(CaptureMethod::Automatic) | Some(CaptureMethod::SequentialAutomatic) => {
                    Ok(Self::Charged)
                }
                Some(CaptureMethod::Manual) => Ok(Self::Authorized),
                _ => Err(error_stack::Report::from(
                    ConnectorError::unexpected_response_error_http_status_unknown(),
                )
                .attach_printable(webhook_status.status.to_string())),
            },
            FiuuPaymentWebhookStatus::Failure => Ok(Self::Failure),
            FiuuPaymentWebhookStatus::Pending => Ok(Self::AuthenticationPending),
            FiuuPaymentWebhookStatus::Unknown => {
                tracing::warn!(
                    "Unknown fiuu payment webhook status received; mapping to unknown attempt status"
                );
                Ok(Self::Unknown)
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaymentCaptureRequest {
    domain: String,
    #[serde(rename = "tranID")]
    tran_id: String,
    amount: StringMajorUnit,
    #[serde(rename = "RefID")]
    ref_id: String,
    skey: Secret<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PaymentCaptureResponse {
    #[serde(rename = "TranID")]
    tran_id: String,
    stat_code: FiuuCaptureStatCode,
}

/// Capture `StatCode` values (Fiuu API Spec for Merchant, "Capture Request").
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, strum::Display)]
pub enum FiuuCaptureStatCode {
    #[strum(serialize = "00")]
    #[serde(rename = "00")]
    Success,
    #[strum(serialize = "11")]
    #[serde(rename = "11")]
    Failure,
    #[strum(serialize = "12")]
    #[serde(rename = "12")]
    InvalidHash,
    #[strum(serialize = "13")]
    #[serde(rename = "13")]
    NotCreditCard,
    #[strum(serialize = "15")]
    #[serde(rename = "15")]
    SettlementDay,
    #[strum(serialize = "16")]
    #[serde(rename = "16")]
    Forbidden,
    #[strum(serialize = "17")]
    #[serde(rename = "17")]
    TransactionNotFound,
    #[strum(serialize = "18")]
    #[serde(rename = "18")]
    MissingParameter,
    #[strum(serialize = "19")]
    #[serde(rename = "19")]
    DomainNotFound,
    #[strum(serialize = "20")]
    #[serde(rename = "20")]
    TemporaryOutOfService,
    #[strum(serialize = "21")]
    #[serde(rename = "21")]
    AuthorizationExpired,
    /// Pre-v13.45 "pending" code, retired in favour of 23; kept for HS parity.
    #[strum(serialize = "22")]
    #[serde(rename = "22")]
    LegacyPending,
    #[strum(serialize = "23")]
    #[serde(rename = "23")]
    PartialCaptureNotAllowed,
    #[strum(serialize = "24")]
    #[serde(rename = "24")]
    AlreadyCaptured,
    #[strum(serialize = "25")]
    #[serde(rename = "25")]
    AmountExceedsCapturable,
    #[strum(serialize = "99")]
    #[serde(rename = "99")]
    GeneralError,
    #[serde(other)]
    Unknown,
}

pub struct FiuuSyncStatus {
    pub stat_name: StatName,
    pub stat_code: StatCode,
    /// Status before this sync; kept when Fiuu answers with an undocumented StatCode.
    pub prev_status: common_enums::AttemptStatus,
}

impl TryFrom<FiuuSyncStatus> for common_enums::AttemptStatus {
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(sync_status: FiuuSyncStatus) -> Result<Self, Self::Error> {
        match (sync_status.stat_code, sync_status.stat_name) {
            (StatCode::Success, StatName::Captured | StatName::Settled) => Ok(Self::Charged), // For Success as StatCode we can only expect Captured,Settled and Authorized as StatName.
            (StatCode::Success, StatName::Authorized) => Ok(Self::Authorized),
            (StatCode::Pending, StatName::Pending) => Ok(Self::AuthenticationPending), // For Pending as StatCode we can only expect Pending and Unknown as StatName.
            (StatCode::Pending, StatName::Unknown) => Ok(Self::Pending),
            (StatCode::Failure, StatName::Cancelled) | (StatCode::Failure, StatName::ReqCancel) => {
                Ok(Self::Voided)
            }
            (StatCode::Failure, _) => Ok(Self::Failure),
            // Undocumented StatCode (S5 lists only 00/11/22): inconclusive, retain the prior
            // status so the next sync can settle it (HS parity).
            (StatCode::Unknown, _) => {
                tracing::warn!(
                    "fiuu: unknown payment sync StatCode; retaining previous status {:?}",
                    sync_status.prev_status
                );
                Ok(sync_status.prev_status)
            }
            (other, _) => Err(error_stack::Report::from(
                ConnectorError::unexpected_response_error_http_status_unknown(),
            )
            .attach_printable(other.to_string())),
        }
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        FiuuRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for PaymentCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: FiuuRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = FiuuAuthType::try_from(&item.router_data.connector_config)?;
        let merchant_id = auth.merchant_id.peek().to_string();
        let amount = item
            .connector
            .amount_converter
            .convert(&item.router_data.request.amount_to_capture)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: Default::default(),
            })?;
        let txn_id = match item.router_data.request.connector_transaction_id {
            ResponseId::ConnectorTransactionId(tid) => tid,
            _ => {
                return Err(IntegrationError::MissingConnectorTransactionID {
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Pass the Fiuu TxnID returned by Authorize as connector_transaction_id"
                                .to_owned(),
                        ),
                        doc_url: Some(FIUU_DIRECT_API_DOC_URL.to_owned()),
                        additional_context: Some(
                            "capstxn tranID is required to capture an AUTS transaction".to_owned(),
                        ),
                    },
                }
                .into());
            }
        };
        let verify_key = auth.verify_key.peek().to_string();
        let signature = calculate_signature(format!(
            "{txn_id}{}{merchant_id}{verify_key}",
            amount.get_amount_as_string()
        ))?;
        Ok(Self {
            domain: merchant_id,
            tran_id: txn_id,
            amount,
            ref_id: item
                .router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            skey: signature,
        })
    }
}
fn capture_status_codes() -> HashMap<&'static str, &'static str> {
    [
        ("00", "Capture successful"),
        ("11", "Capture failed"),
        ("12", "Invalid or unmatched security hash string"),
        ("13", "Not a credit card transaction"),
        ("15", "Requested day is on settlement day"),
        ("16", "Forbidden transaction"),
        ("17", "Transaction not found"),
        ("18", "Missing required parameter"),
        ("19", "Domain not found"),
        ("20", "Temporary out of service"),
        ("21", "Authorization expired"),
        ("23", "Partial capture not allowed"),
        ("24", "Transaction already captured"),
        ("25", "Requested amount exceeds available capture amount"),
        ("99", "General error (contact payment gateway support)"),
    ]
    .into_iter()
    .collect()
}

impl<F> TryFrom<ResponseRouterData<PaymentCaptureResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<PaymentCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code: _http_code,
        } = item;
        let status = match response.stat_code {
            FiuuCaptureStatCode::Success => common_enums::AttemptStatus::Charged,
            FiuuCaptureStatCode::Failure
            | FiuuCaptureStatCode::InvalidHash
            | FiuuCaptureStatCode::NotCreditCard
            | FiuuCaptureStatCode::SettlementDay
            | FiuuCaptureStatCode::Forbidden
            | FiuuCaptureStatCode::TransactionNotFound
            | FiuuCaptureStatCode::MissingParameter
            | FiuuCaptureStatCode::DomainNotFound
            | FiuuCaptureStatCode::AuthorizationExpired
            | FiuuCaptureStatCode::PartialCaptureNotAllowed
            | FiuuCaptureStatCode::AlreadyCaptured
            | FiuuCaptureStatCode::AmountExceedsCapturable
            | FiuuCaptureStatCode::GeneralError => common_enums::AttemptStatus::CaptureFailed,
            // 20 Temporary out of service: the capture was not processed; non-terminal, PSync
            // settles it. Hyperswitch maps 20 to Failure; diverging so an outage stays retryable.
            FiuuCaptureStatCode::LegacyPending | FiuuCaptureStatCode::TemporaryOutOfService => {
                common_enums::AttemptStatus::Pending
            }
            // Undocumented StatCode: inconclusive, never terminal; a later PSync settles it.
            FiuuCaptureStatCode::Unknown => {
                tracing::warn!("fiuu: unknown capture StatCode; mapping to Pending");
                common_enums::AttemptStatus::Pending
            }
        };
        let error_response = if status == common_enums::AttemptStatus::CaptureFailed {
            let code = response.stat_code.to_string();
            let optional_message = capture_status_codes()
                .get(code.as_str())
                .copied()
                .map(String::from);
            Some(ErrorResponse {
                status_code: item.http_code,
                code,
                message: optional_message
                    .clone()
                    .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string()),
                reason: optional_message,
                attempt_status: Some(FlowStatus::Payment(
                    common_enums::AttemptStatus::CaptureFailed,
                )),
                connector_transaction_id: Some(response.tran_id.clone()),
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            })
        } else {
            None
        };
        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(response.tran_id.clone().to_string()),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            network_txn_link_id: None,
            connector_response_reference_id: None,
            incremental_authorization_allowed: None,
            status_code: item.http_code,
            splits: None,
            payment_account_reference: None,
        };
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data
            },
            response: error_response.map_or_else(|| Ok(payments_response_data), Err),
            ..router_data
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FiuuPaymentCancelRequest {
    #[serde(rename = "txnID")]
    txn_id: String,
    domain: String,
    skey: Secret<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct FiuuPaymentCancelResponse {
    #[serde(rename = "TranID")]
    tran_id: String,
    stat_code: FiuuVoidStatCode,
    #[serde(rename = "miscellaneous")]
    miscellaneous: Option<HashMap<String, Secret<String>>>,
}

/// Reversal `StatCode` values (spec "3. Reversal Request").
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, strum::Display)]
pub enum FiuuVoidStatCode {
    #[strum(serialize = "00")]
    #[serde(rename = "00")]
    Success,
    #[strum(serialize = "11")]
    #[serde(rename = "11")]
    Failure,
    #[strum(serialize = "12")]
    #[serde(rename = "12")]
    InvalidHash,
    #[strum(serialize = "13")]
    #[serde(rename = "13")]
    NotRefundable,
    #[strum(serialize = "14")]
    #[serde(rename = "14")]
    TransactionTooOld,
    #[strum(serialize = "15")]
    #[serde(rename = "15")]
    SettlementDay,
    #[strum(serialize = "16")]
    #[serde(rename = "16")]
    Forbidden,
    #[strum(serialize = "17")]
    #[serde(rename = "17")]
    TransactionNotFound,
    #[strum(serialize = "18")]
    #[serde(rename = "18")]
    DuplicatePartialRefund,
    #[strum(serialize = "19")]
    #[serde(rename = "19")]
    MerchantNotFound,
    #[strum(serialize = "20")]
    #[serde(rename = "20")]
    MissingParameter,
    #[strum(serialize = "21")]
    #[serde(rename = "21")]
    InvalidTransactionStatus,
    #[strum(serialize = "22")]
    #[serde(rename = "22")]
    DuplicateRequest,
    #[serde(other)]
    Unknown,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        FiuuRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for FiuuPaymentCancelRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: FiuuRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = FiuuAuthType::try_from(&item.router_data.connector_config)?;
        let txn_id = item.router_data.request.connector_transaction_id.clone();
        if txn_id.is_empty() {
            return Err(IntegrationError::MissingConnectorTransactionID {
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Pass the Fiuu TxnID returned by Authorize as connector_transaction_id"
                            .to_owned(),
                    ),
                    doc_url: Some(FIUU_DIRECT_API_DOC_URL.to_owned()),
                    additional_context: Some(
                        "reversal txnID is required to void a transaction".to_owned(),
                    ),
                },
            }
            .into());
        }
        let merchant_id = auth.merchant_id.peek().to_string();
        // skey key choice: verify_key (HS parity), not the docs' secret_key.
        let verify_key = auth.verify_key.peek().to_string();
        Ok(Self {
            txn_id: txn_id.clone(),
            domain: merchant_id.clone(),
            skey: calculate_signature(format!("{txn_id}{merchant_id}{verify_key}"))?,
        })
    }
}

fn void_status_codes() -> HashMap<&'static str, &'static str> {
    [
        ("00", "Success (will proceed the request)"),
        ("11", "Failure"),
        ("12", "Invalid or unmatched security hash string"),
        ("13", "Not a refundable transaction"),
        ("14", "Transaction date more than 180 days"),
        ("15", "Requested day is on settlement day"),
        ("16", "Forbidden transaction"),
        ("17", "Transaction not found"),
        ("18", "Duplicate partial refund request"),
        ("19", "Merchant not found"),
        ("20", "Missing required parameter"),
        (
            "21",
            "Transaction must be in authorized/captured/settled status",
        ),
        ("22", "Duplicate request is not allowed"),
    ]
    .into_iter()
    .collect()
}
impl<F> TryFrom<ResponseRouterData<FiuuPaymentCancelResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<FiuuPaymentCancelResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code: _http_code,
        } = item;
        // StatCode map: spec "3. Reversal Request".
        let status = match response.stat_code {
            FiuuVoidStatCode::Success => common_enums::AttemptStatus::Voided,
            FiuuVoidStatCode::Failure
            | FiuuVoidStatCode::InvalidHash
            | FiuuVoidStatCode::NotRefundable
            | FiuuVoidStatCode::TransactionTooOld
            | FiuuVoidStatCode::SettlementDay
            | FiuuVoidStatCode::Forbidden
            | FiuuVoidStatCode::TransactionNotFound
            | FiuuVoidStatCode::DuplicatePartialRefund
            | FiuuVoidStatCode::MerchantNotFound
            | FiuuVoidStatCode::MissingParameter
            | FiuuVoidStatCode::InvalidTransactionStatus
            | FiuuVoidStatCode::DuplicateRequest => common_enums::AttemptStatus::VoidFailed,
            // Undocumented StatCode: inconclusive, never terminal; a later PSync settles it.
            FiuuVoidStatCode::Unknown => {
                tracing::warn!("fiuu: unknown void StatCode; mapping to VoidInitiated");
                common_enums::AttemptStatus::VoidInitiated
            }
        };
        let error_response = if status == common_enums::AttemptStatus::VoidFailed {
            let code = response.stat_code.to_string();
            let optional_message = void_status_codes()
                .get(code.as_str())
                .copied()
                .map(String::from);

            Some(ErrorResponse {
                status_code: item.http_code,
                code,
                message: optional_message
                    .clone()
                    .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string()),
                reason: optional_message,
                attempt_status: Some(FlowStatus::Payment(common_enums::AttemptStatus::VoidFailed)),
                connector_transaction_id: Some(response.tran_id.clone()),
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
                typed_connector_response: None,
                raw_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
            })
        } else {
            None
        };
        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(response.tran_id.clone().to_string()),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            network_txn_link_id: None,
            connector_response_reference_id: None,
            incremental_authorization_allowed: None,
            status_code: item.http_code,
            splits: None,
            payment_account_reference: None,
        };
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data
            },
            response: error_response.map_or_else(|| Ok(payments_response_data), Err),
            ..router_data
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct FiuuRefundSyncRequest {
    #[serde(rename = "TxnID")]
    txn_id: String,
    #[serde(rename = "MerchantID")]
    merchant_id: Secret<String>,
    signature: Secret<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        FiuuRouterData<RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>, T>,
    > for FiuuRefundSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: FiuuRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = FiuuAuthType::try_from(&item.router_data.connector_config)?;
        let txn_id = item.router_data.request.connector_transaction_id.clone();
        // q_by_txn looks refunds up by the payment TxnID (Fiuu API Spec for Merchant, "Partial
        // Refund Status Inquiry by TxnID"); refuse before building the request.
        if txn_id.is_empty() {
            return Err(IntegrationError::MissingConnectorTransactionID {
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Pass the Fiuu TxnID returned by Authorize as connector_transaction_id"
                            .to_owned(),
                    ),
                    doc_url: Some(FIUU_DIRECT_API_DOC_URL.to_owned()),
                    additional_context: Some(
                        "q_by_txn TxnID is required to sync a refund".to_owned(),
                    ),
                },
            }
            .into());
        }
        let (merchant_id, verify_key) = (
            auth.merchant_id.peek().to_string(),
            auth.verify_key.peek().to_string(),
        );
        let signature = calculate_signature(format!("{txn_id}{merchant_id}{verify_key}"))?;
        Ok(Self {
            txn_id,
            merchant_id: auth.merchant_id,
            signature,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FiuuRefundSyncResponse {
    Success(Vec<RefundData>),
    Error(FiuuErrorResponse),
    Webhook(FiuuWebhooksRefundResponse),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct RefundData {
    #[serde(rename = "RefundID")]
    refund_id: String,
    status: RefundStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RefundStatus {
    Success,
    Pending,
    Rejected,
    Processing,
    #[serde(other)]
    Unknown,
}

impl<F> TryFrom<ResponseRouterData<FiuuRefundSyncResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<FiuuRefundSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code: _http_code,
        } = item;
        match response {
            FiuuRefundSyncResponse::Error(error) => Ok(Self {
                response: Err(ErrorResponse {
                    code: error.error_code.clone(),
                    message: error.error_desc.clone(),
                    reason: Some(error.error_desc),
                    status_code: item.http_code,
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_advice_code: None,
                    network_decline_code: None,
                    network_error_message: None,
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                }),
                ..router_data
            }),
            FiuuRefundSyncResponse::Success(refund_data) => {
                let refund = refund_data
                    .iter()
                    .find(|refund| {
                        Some(refund.refund_id.clone())
                            == Some(router_data.request.connector_refund_id.clone())
                    })
                    .ok_or_else(|| {
                        error_stack::Report::new(
                            ConnectorError::response_handling_failed_with_context(
                                item.http_code,
                                Some(
                                    "refund sync: no row for request connector_refund_id"
                                        .to_string(),
                                ),
                            ),
                        )
                    })?;
                Ok(Self {
                    response: Ok(RefundsResponseData {
                        connector_refund_id: refund.refund_id.clone(),
                        refund_status: get_refund_status_from_sync(
                            refund.status.clone(),
                            router_data.resource_common_data.status,
                        ),
                        status_code: item.http_code,
                        acquirer_reference_number: None,
                    }),
                    ..router_data
                })
            }
            FiuuRefundSyncResponse::Webhook(fiuu_webhooks_refund_response) => Ok(Self {
                response: Ok(RefundsResponseData {
                    connector_refund_id: fiuu_webhooks_refund_response.refund_id,
                    refund_status: common_enums::RefundStatus::from(
                        fiuu_webhooks_refund_response.status.clone(),
                    ),
                    status_code: item.http_code,
                    acquirer_reference_number: None,
                }),
                ..router_data
            }),
        }
    }
}

// Fiuu API Spec for Merchant, "Partial Refund Status Inquiry by TxnID": success -> Success,
// pending|processing -> Pending, rejected -> Failure.
fn get_refund_status_from_sync(
    status: RefundStatus,
    prev_status: common_enums::RefundStatus,
) -> common_enums::RefundStatus {
    match status {
        RefundStatus::Pending => common_enums::RefundStatus::Pending,
        RefundStatus::Success => common_enums::RefundStatus::Success,
        RefundStatus::Rejected => common_enums::RefundStatus::Failure,
        RefundStatus::Processing => common_enums::RefundStatus::Pending,
        // Undocumented status: keep the refund's prior status rather than guessing (HS parity).
        RefundStatus::Unknown => {
            tracing::warn!(
                ?prev_status,
                "Unknown fiuu refund sync status received; retaining previous refund status"
            );
            prev_status
        }
    }
}

pub fn get_qr_metadata(
    response: &DuitNowQrCodeResponse,
) -> CustomResult<Option<Value>, ConnectorError> {
    let image_data = QrImage::new_colored_from_data(
        response.txn_data.request_data.qr_data.peek().clone(),
        DUIT_NOW_BRAND_COLOR,
    )
    .change_context(ConnectorError::response_handling_failed_http_status_unknown())?;

    let image_data_url = Url::parse(image_data.data.clone().as_str()).ok();
    let display_to_timestamp = None;

    if let Some(color_image_data_url) = image_data_url {
        let qr_code_info = QrCodeInformation::QrColorDataUrl {
            color_image_data_url,
            display_to_timestamp,
            display_text: Some(DUIT_NOW_BRAND_TEXT.to_string()),
            border_color: Some(DUIT_NOW_BRAND_COLOR.to_string()),
        };

        Some(qr_code_info.encode_to_value())
            .transpose()
            .change_context(ConnectorError::response_handling_failed_http_status_unknown())
    } else {
        Ok(None)
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum FiuuWebhooksResponse {
    FiuuWebhookPaymentResponse(FiuuWebhooksPaymentResponse),
    FiuuWebhookRefundResponse(FiuuWebhooksRefundResponse),
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FiuuWebhooksPaymentResponse {
    pub skey: Secret<String>,
    pub status: FiuuPaymentWebhookStatus,
    #[serde(rename = "orderid")]
    pub order_id: String,
    #[serde(rename = "tranID")]
    pub tran_id: String,
    pub nbcb: Option<String>,
    pub amount: StringMajorUnit,
    pub currency: String,
    pub domain: Secret<String>,
    pub appcode: Option<Secret<String>>,
    pub paydate: String,
    pub channel: Option<String>,
    pub error_desc: Option<String>,
    pub error_code: Option<String>,
    #[serde(rename = "extraP")]
    pub extra_parameters: Option<Secret<String>>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FiuuPaymentRedirectResponse {
    pub skey: Secret<String>,
    #[serde(rename = "tranID")]
    pub tran_id: String,
    pub status: FiuuPaymentWebhookStatus,
    pub appcode: Option<String>,
    pub error_code: Option<String>,
    pub error_desc: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct FiuuWebhooksRefundResponse {
    pub refund_type: FiuuWebhooksRefundType,
    #[serde(rename = "MerchantID")]
    pub merchant_id: Secret<String>,
    #[serde(rename = "RefID")]
    pub ref_id: String,
    #[serde(rename = "RefundID")]
    pub refund_id: String,
    #[serde(rename = "TxnID")]
    pub txn_id: String,
    pub amount: StringMajorUnit,
    pub status: FiuuRefundsWebhookStatus,
    pub signature: Secret<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, strum::Display)]
pub enum FiuuRefundsWebhookStatus {
    #[strum(serialize = "00")]
    #[serde(rename = "00")]
    RefundSuccess,
    #[strum(serialize = "11")]
    #[serde(rename = "11")]
    RefundFailure,
    #[strum(serialize = "22")]
    #[serde(rename = "22")]
    RefundPending,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize, Serialize, Clone, strum::Display)]
pub enum FiuuWebhooksRefundType {
    P,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FiuuWebhookSignature {
    pub skey: Secret<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FiuuWebhookResourceId {
    pub skey: Secret<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FiuWebhookEvent {
    pub status: FiuuPaymentWebhookStatus,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, strum::Display)]
pub enum FiuuPaymentWebhookStatus {
    #[strum(serialize = "00")]
    #[serde(rename = "00")]
    Success,
    #[strum(serialize = "11")]
    #[serde(rename = "11")]
    Failure,
    #[strum(serialize = "22")]
    #[serde(rename = "22")]
    Pending,
    #[serde(other)]
    Unknown,
}

impl From<FiuuPaymentWebhookStatus> for StatCode {
    fn from(value: FiuuPaymentWebhookStatus) -> Self {
        match value {
            FiuuPaymentWebhookStatus::Success => Self::Success,
            FiuuPaymentWebhookStatus::Failure => Self::Failure,
            FiuuPaymentWebhookStatus::Pending => Self::Pending,
            FiuuPaymentWebhookStatus::Unknown => {
                tracing::warn!(
                    "Unknown fiuu payment webhook status received; mapping to unknown status code"
                );
                Self::Unknown
            }
        }
    }
}

impl From<FiuuPaymentWebhookStatus> for interfaces::webhooks::IncomingWebhookEvent {
    fn from(value: FiuuPaymentWebhookStatus) -> Self {
        match value {
            FiuuPaymentWebhookStatus::Success => Self::PaymentIntentSuccess,
            FiuuPaymentWebhookStatus::Failure => Self::PaymentIntentFailure,
            FiuuPaymentWebhookStatus::Pending => Self::PaymentIntentProcessing,
            FiuuPaymentWebhookStatus::Unknown => {
                tracing::warn!(
                    "Unknown fiuu payment webhook status received; acknowledging without processing"
                );
                Self::EventNotSupported
            }
        }
    }
}

impl From<FiuuRefundsWebhookStatus> for interfaces::webhooks::IncomingWebhookEvent {
    fn from(value: FiuuRefundsWebhookStatus) -> Self {
        match value {
            FiuuRefundsWebhookStatus::RefundSuccess => Self::RefundSuccess,
            FiuuRefundsWebhookStatus::RefundFailure => Self::RefundFailure,
            FiuuRefundsWebhookStatus::RefundPending => Self::EventNotSupported,
            FiuuRefundsWebhookStatus::Unknown => {
                tracing::warn!(
                    "Unknown fiuu refund webhook status received; acknowledging without processing"
                );
                Self::EventNotSupported
            }
        }
    }
}

impl From<FiuuRefundsWebhookStatus> for common_enums::RefundStatus {
    fn from(value: FiuuRefundsWebhookStatus) -> Self {
        match value {
            FiuuRefundsWebhookStatus::RefundFailure => Self::Failure,
            FiuuRefundsWebhookStatus::RefundSuccess => Self::Success,
            FiuuRefundsWebhookStatus::RefundPending => Self::Pending,
            FiuuRefundsWebhookStatus::Unknown => {
                tracing::warn!(
                    "Unknown fiuu refund webhook status received; mapping to unknown refund status"
                );
                Self::Unknown
            }
        }
    }
}

//new additions  structs
#[derive(Serialize, Debug, Clone)]
#[serde(untagged)]
pub enum FiuuPaymentsRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    FiuuPaymentRequest(Box<FiuuPaymentRequest<T>>),
    FiuuMandateRequest(FiuuMandateRequest),
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize> GetFormData
    for FiuuPaymentsRequest<T>
{
    fn get_form_data(&self) -> MultipartData {
        match self {
            Self::FiuuPaymentRequest(req) => {
                build_form_from_struct(req).unwrap_or_else(|_| MultipartData::new())
            }
            Self::FiuuMandateRequest(req) => {
                build_form_from_struct(req).unwrap_or_else(|_| MultipartData::new())
            }
        }
    }
}
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize> GetFormData
    for FiuuPaymentRequest<T>
{
    fn get_form_data(&self) -> MultipartData {
        build_form_from_struct(self).unwrap_or_else(|_| MultipartData::new())
    }
}
impl GetFormData for FiuuPaymentSyncRequest {
    fn get_form_data(&self) -> MultipartData {
        build_form_from_struct(self).unwrap_or_else(|_| MultipartData::new())
    }
}
impl GetFormData for PaymentCaptureRequest {
    fn get_form_data(&self) -> MultipartData {
        build_form_from_struct(self).unwrap_or_else(|_| MultipartData::new())
    }
}
impl GetFormData for FiuuPaymentCancelRequest {
    fn get_form_data(&self) -> MultipartData {
        build_form_from_struct(self).unwrap_or_else(|_| MultipartData::new())
    }
}
impl GetFormData for FiuuRefundRequest {
    fn get_form_data(&self) -> MultipartData {
        build_form_from_struct(self).unwrap_or_else(|_| MultipartData::new())
    }
}
impl GetFormData for FiuuRefundSyncRequest {
    fn get_form_data(&self) -> MultipartData {
        build_form_from_struct(self).unwrap_or_else(|_| MultipartData::new())
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        FiuuRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for FiuuPaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: FiuuRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        match item.router_data.request.mandate_reference {
            // verificationID_ mandates go to the Direct API, same as get_url.
            MandateReferenceId::ConnectorMandateId(_)
                if get_verification_mandate_id(&item.router_data.request.mandate_reference)
                    .is_none() =>
            {
                let recurring_request = FiuuMandateRequest::try_from(&item)?;
                Ok(Self::FiuuMandateRequest(recurring_request))
            }
            MandateReferenceId::ConnectorMandateId(_)
            | MandateReferenceId::NetworkMandateId(_)
            | MandateReferenceId::NetworkTokenWithNTI(_) => {
                let payment_request: FiuuPaymentRequest<T> = FiuuPaymentRequest::try_from(&item)?;
                Ok(Self::FiuuPaymentRequest(Box::new(payment_request)))
            }
        }
    }
}

#[macro_export]
macro_rules! unimplemented_payment_method {
    ($payment_method:expr, $connector:expr) => {
        domain_types::errors::IntegrationError::NotImplemented(
            (format!("{} through {}", $payment_method, $connector)).into(),
            Default::default(),
        )
    };
    ($payment_method:expr, $flow:expr, $connector:expr) => {
        domain_types::errors::IntegrationError::NotImplemented(
            (format!("{} {} through {}", $payment_method, $flow, $connector)).into(),
            Default::default(),
        )
    };
}

pub const DUIT_NOW_BRAND_COLOR: &str = "#ED2E67";

pub const DUIT_NOW_BRAND_TEXT: &str = "MALAYSIA NATIONAL QR";

impl From<FiuuPaymentWebhookStatus> for EventType {
    fn from(value: FiuuPaymentWebhookStatus) -> Self {
        match value {
            FiuuPaymentWebhookStatus::Success => Self::PaymentIntentSuccess,
            FiuuPaymentWebhookStatus::Failure => Self::PaymentIntentFailure,
            FiuuPaymentWebhookStatus::Pending => Self::PaymentIntentProcessing,
            FiuuPaymentWebhookStatus::Unknown => {
                tracing::warn!(
                    "Unknown fiuu payment webhook status received; acknowledging without processing"
                );
                Self::IncomingWebhookEventUnspecified
            }
        }
    }
}

impl From<FiuuRefundsWebhookStatus> for EventType {
    fn from(value: FiuuRefundsWebhookStatus) -> Self {
        match value {
            FiuuRefundsWebhookStatus::RefundSuccess => Self::RefundSuccess,
            FiuuRefundsWebhookStatus::RefundFailure => Self::RefundFailure,
            FiuuRefundsWebhookStatus::RefundPending => Self::IncomingWebhookEventUnspecified,
            FiuuRefundsWebhookStatus::Unknown => {
                tracing::warn!(
                    "Unknown fiuu refund webhook status received; acknowledging without processing"
                );
                Self::IncomingWebhookEventUnspecified
            }
        }
    }
}

impl TryFrom<FiuuRefundSyncResponse> for RefundWebhookDetailsResponse {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(notif: FiuuRefundSyncResponse) -> Result<Self, Self::Error> {
        match notif {
            FiuuRefundSyncResponse::Webhook(fiuu_webhooks_refund_response) => Ok(Self {
                connector_refund_id: Some(fiuu_webhooks_refund_response.refund_id),
                merchant_transaction_id: None,
                status: common_enums::RefundStatus::from(
                    fiuu_webhooks_refund_response.status.clone(),
                ),
                status_code: 200,
                connector_response_reference_id: None,
                error_code: None,
                error_message: None,
                raw_connector_response: None,
                response_headers: None,
            }),
            _ => Err(IntegrationError::NotImplemented(
                "webhook body decoding failed".to_string(),
                Default::default(),
            ))?,
        }
    }
}

// ===== ThreeDS: Standalone Authentication + Authentication Query (Fiuu Standalone Card API) =====

/// `authentication.php` request. Issues the EMV 3DS challenge; never authorises.
#[derive(Serialize, Debug, Clone)]
pub struct FiuuAuthenticateRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    #[serde(rename = "MerchantID")]
    merchant_id: Secret<String>,
    #[serde(rename = "ReferenceNo")]
    reference_no: String,
    #[serde(rename = "TxnAmount")]
    txn_amount: StringMajorUnit,
    #[serde(rename = "TxnCurrency")]
    txn_currency: Currency,
    /// HMAC_SHA256(TxnAmount.MerchantID.ReferenceNo.TxnCurrency, verify_key).
    #[serde(rename = "Signature")]
    signature: Secret<String>,
    #[serde(rename = "ReturnURL")]
    return_url: String,
    #[serde(rename = "CC_PAN")]
    cc_pan: RawCardNumber<T>,
    #[serde(rename = "CC_CVV2")]
    cc_cvv2: Secret<String>,
    #[serde(rename = "CC_MONTH")]
    cc_month: Secret<String>,
    #[serde(rename = "CC_YEAR")]
    cc_year: Secret<String>,
    #[serde(flatten)]
    customer_details: FiuuCustomerDetails,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize> GetFormData
    for FiuuAuthenticateRequest<T>
{
    fn get_form_data(&self) -> MultipartData {
        build_form_from_struct(self).unwrap_or_else(|_| MultipartData::new())
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        FiuuRouterData<
            RouterDataV2<
                Authenticate,
                PaymentFlowData,
                PaymentsAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for FiuuAuthenticateRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: FiuuRouterData<
            RouterDataV2<
                Authenticate,
                PaymentFlowData,
                PaymentsAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        // CC_PAN/CC_MONTH/CC_YEAR are mandatory, so only a card can authenticate.
        let card = match router_data.request.payment_method_data {
            Some(PaymentMethodData::Card(ref card)) => card,
            _ => {
                return Err(IntegrationError::NotSupported {
                    message: "Fiuu standalone authentication for a non-card payment method"
                        .to_owned(),
                    connector: "Fiuu",
                    context: fiuu_error_context(
                        "Use a card for Fiuu 3DS authentication",
                        Some("Fiuu authentication.php requires CC_PAN, CC_MONTH and CC_YEAR"),
                    ),
                }
                .into())
            }
        };
        // Fiuu posts the authentication result to ReturnURL (mandatory).
        let return_url = router_data
            .request
            .router_return_url
            .as_ref()
            .map(ToString::to_string)
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "router_return_url",
                context: fiuu_error_context(
                    "Pass return_url; Fiuu posts the authentication result to it",
                    Some("ReturnURL is mandatory on Fiuu authentication.php"),
                ),
            })?;
        // TxnCurrency is signed with TxnAmount, so both come from the same Money.
        let txn_currency = router_data.request.amount.currency;
        let auth = FiuuAuthType::try_from(&router_data.connector_config)?;
        let txn_amount = item
            .connector
            .amount_converter
            .convert(&router_data.request.amount)
            .change_context(IntegrationError::RequestEncodingFailed {
                context: fiuu_error_context(
                    "Check the amount and currency",
                    Some("Amount could not be converted to Fiuu major units"),
                ),
            })?;
        let reference_no = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();
        let signature = calculate_hmac_sha256_signature(
            &format!(
                "{}{}{reference_no}{txn_currency}",
                txn_amount.get_amount_as_string(),
                auth.merchant_id.peek()
            ),
            &auth.verify_key,
        )?;
        Ok(Self {
            merchant_id: auth.merchant_id,
            reference_no,
            txn_amount,
            txn_currency,
            signature,
            return_url,
            cc_pan: card.card_number.clone(),
            cc_cvv2: card.card_cvc.clone(),
            cc_month: card.get_card_expiry_month_2_digit()?,
            cc_year: card.get_expiry_year_4_digit(),
            customer_details: FiuuCustomerDetails {
                cust_name: router_data
                    .resource_common_data
                    .get_optional_billing_full_name(),
                cust_email: router_data.request.email.clone().or_else(|| {
                    router_data
                        .resource_common_data
                        .get_optional_billing_email()
                }),
                cust_contact: None,
                cust_desc: None,
                display_name: None,
            },
        })
    }
}

/// `TxnData.RequestType` of the authentication response.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum FiuuAuthnRequestType {
    #[serde(rename = "REDIRECT")]
    Redirect,
    #[serde(rename = "POPUP")]
    Popup,
    #[serde(rename = "RESPONSE")]
    Response,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FiuuAuthnTxnData {
    #[serde(rename = "RequestURL")]
    pub request_url: Option<String>,
    #[serde(rename = "RequestMethod")]
    pub request_method: Option<FiuuRequestMethod>,
    #[serde(rename = "RequestType")]
    pub request_type: FiuuAuthnRequestType,
    #[serde(rename = "RequestData")]
    pub request_data: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FiuuAuthenticateSuccessResponse {
    #[serde(rename = "ReferenceNo")]
    pub reference_no: Option<String>,
    #[serde(rename = "AuthID")]
    pub auth_id: String,
    #[serde(rename = "TxnData")]
    pub txn_data: FiuuAuthnTxnData,
}

/// Authenticate -> PostAuthenticate round-trip of the Fiuu AuthID.
#[derive(Debug, Serialize, Deserialize)]
pub struct FiuuThreeDsFeatureData {
    pub auth_id: String,
}

/// authentication.php answers either the challenge envelope or `{status:false,error_code,error_desc}`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FiuuAuthenticateResponse {
    Success(Box<FiuuAuthenticateSuccessResponse>),
    Error(FiuuErrorResponse),
}

fn fiuu_authentication_data(transaction_id: Option<String>) -> AuthenticationData {
    AuthenticationData {
        trans_status: None,
        eci: None,
        cavv: None,
        ucaf_collection_indicator: None,
        threeds_server_transaction_id: None,
        message_version: None,
        ds_trans_id: None,
        acs_transaction_id: None,
        transaction_id,
        network_params: None,
        exemption_indicator: None,
        created_at: None,
        challenge_code: None,
        challenge_cancel: None,
        challenge_code_reason: None,
        message_extension: None,
        authentication_type: None,
    }
}

fn fiuu_authentication_failed(
    code: Option<String>,
    message: Option<String>,
    status_code: u16,
) -> ErrorResponse {
    ErrorResponse {
        code: code.unwrap_or_else(|| consts::NO_ERROR_CODE.to_string()),
        message: message
            .clone()
            .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string()),
        reason: message,
        status_code,
        attempt_status: Some(FlowStatus::Payment(
            common_enums::AttemptStatus::AuthenticationFailed,
        )),
        connector_transaction_id: None,
        network_advice_code: None,
        network_decline_code: None,
        network_error_message: None,
        typed_connector_response: None,
        raw_connector_response: None,
        raw_connector_request: None,
        typed_connector_request: None,
    }
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<FiuuAuthenticateResponse, Self>>
    for RouterDataV2<
        Authenticate,
        PaymentFlowData,
        PaymentsAuthenticateData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<FiuuAuthenticateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;
        match response {
            FiuuAuthenticateResponse::Error(error) => Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::AuthenticationFailed,
                    ..router_data.resource_common_data
                },
                response: Err(fiuu_authentication_failed(
                    Some(error.error_code),
                    Some(error.error_desc),
                    http_code,
                )),
                ..router_data
            }),
            FiuuAuthenticateResponse::Success(data) => {
                let txn_data = data.txn_data;
                // REDIRECT/POPUP: send the customer to the ACS. RESPONSE (no challenge) and an
                // undocumented type carry no redirect; PostAuthenticate reads the outcome.
                let redirection_data = match txn_data.request_type {
                    FiuuAuthnRequestType::Redirect | FiuuAuthnRequestType::Popup => {
                        txn_data.request_url.map(|endpoint| {
                            let form_fields = match txn_data.request_data {
                                Some(Value::Object(map)) => map
                                    .into_iter()
                                    .map(|(key, value)| match value {
                                        Value::String(text) => (key, text),
                                        other => (key, other.to_string()),
                                    })
                                    .collect(),
                                _ => HashMap::new(),
                            };
                            Box::new(RedirectForm::Form {
                                endpoint,
                                method: txn_data.request_method.map_or(Method::Post, Method::from),
                                form_fields,
                            })
                        })
                    }
                    FiuuAuthnRequestType::Response | FiuuAuthnRequestType::Unknown => None,
                };
                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status: common_enums::AttemptStatus::AuthenticationPending,
                        ..router_data.resource_common_data
                    },
                    response: Ok(PaymentsResponseData::AuthenticateResponse {
                        // AuthID is an authentication id, not a payment TxnID.
                        resource_id: None,
                        redirection_data,
                        authentication_data: Some(fiuu_authentication_data(Some(
                            data.auth_id.clone(),
                        ))),
                        // Round-tripped to PostAuthenticate as AuthenticationID.
                        connector_feature_data: Some(
                            serde_json::to_value(FiuuThreeDsFeatureData {
                                auth_id: data.auth_id,
                            })
                            .change_context(
                                ConnectorError::ResponseHandlingFailed {
                                    context: ResponseTransformationErrorContext {
                                        http_status_code: Some(http_code),
                                        additional_context: Some(
                                            "fiuu: AuthID could not be stored for PostAuthenticate"
                                                .to_owned(),
                                        ),
                                    },
                                },
                            )?,
                        ),
                        connector_response_reference_id: data.reference_no,
                        status_code: http_code,
                        mandate_reference: None,
                        network_txn_id: None,
                        network_txn_link_id: None,
                    }),
                    ..router_data
                })
            }
        }
    }
}

/// `authn_query.php` request. Reads the authentication result; never authorises.
#[derive(Serialize, Debug, Clone)]
pub struct FiuuAuthnQueryRequest {
    #[serde(rename = "MerchantID")]
    merchant_id: Secret<String>,
    #[serde(rename = "AuthenticationID", skip_serializing_if = "Option::is_none")]
    authentication_id: Option<String>,
    #[serde(rename = "ReferenceNo")]
    reference_no: String,
    #[serde(rename = "DateTime")]
    date_time: String,
    /// HMAC_SHA256(DateTime.MerchantID.AuthenticationID.ReferenceNo, verify_key).
    #[serde(rename = "Signature")]
    signature: Secret<String>,
}

impl GetFormData for FiuuAuthnQueryRequest {
    fn get_form_data(&self) -> MultipartData {
        build_form_from_struct(self).unwrap_or_else(|_| MultipartData::new())
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        FiuuRouterData<
            RouterDataV2<
                PostAuthenticate,
                PaymentFlowData,
                PaymentsPostAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for FiuuAuthnQueryRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: FiuuRouterData<
            RouterDataV2<
                PostAuthenticate,
                PaymentFlowData,
                PaymentsPostAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = FiuuAuthType::try_from(&router_data.connector_config)?;
        // The stored Authenticate AuthID is authoritative; the shopper-posted value is only a
        // fallback, and the response handler binds the result to this payment's ReferenceNo.
        // "Either AuthenticationID or ReferenceNo must be passed", so it may be omitted.
        let authentication_id = router_data
            .resource_common_data
            .connector_feature_data
            .as_ref()
            .and_then(|data| {
                serde_json::from_value::<FiuuThreeDsFeatureData>(data.peek().clone()).ok()
            })
            .map(|data| data.auth_id)
            .or_else(|| {
                router_data
                    .request
                    .redirect_response
                    .as_ref()
                    .and_then(|redirect| redirect.payload.as_ref())
                    .and_then(|payload| {
                        let payload = payload.peek();
                        ["transaction_id", "AuthenticationID", "AuthID"]
                            .iter()
                            .find_map(|key| payload.get(*key).and_then(Value::as_str))
                            .map(str::to_owned)
                    })
            });
        let reference_no = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();
        let date_time = common_utils::date_time::format_date(
            common_utils::date_time::now(),
            common_utils::date_time::DateFormat::YYYYMMDDHHmmss,
        )
        .change_context(IntegrationError::RequestEncodingFailed {
            context: fiuu_error_context(
                "Retry the request",
                Some("DateTime (YYYYMMDDHHmmss) for Fiuu authn_query.php could not be formatted"),
            ),
        })?;
        let signature = calculate_hmac_sha256_signature(
            &format!(
                "{date_time}{}{}{reference_no}",
                auth.merchant_id.peek(),
                authentication_id.as_deref().unwrap_or_default()
            ),
            &auth.verify_key,
        )?;
        Ok(Self {
            merchant_id: auth.merchant_id,
            authentication_id,
            reference_no,
            date_time,
            signature,
        })
    }
}

/// authn_query `status`: 00 success, 11 failed, 22 pending.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum FiuuAuthnStatus {
    #[serde(rename = "00")]
    Success,
    #[serde(rename = "11")]
    Failed,
    #[serde(rename = "22")]
    Pending,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FiuuAuthnQueryResult {
    #[serde(rename = "AuthenticationID")]
    pub authentication_id: Option<String>,
    #[serde(rename = "MerchantID")]
    pub merchant_id: Option<Secret<String>>,
    #[serde(rename = "ReferenceNo")]
    pub reference_no: Option<String>,
    #[serde(rename = "TxnCurrency")]
    pub txn_currency: Option<Currency>,
    #[serde(rename = "TxnAmount")]
    pub txn_amount: Option<StringMajorUnit>,
    pub status: FiuuAuthnStatus,
    pub error_code: Option<String>,
    pub error_desc: Option<String>,
    #[serde(rename = "dsTranID")]
    pub ds_tran_id: Option<String>,
    #[serde(rename = "threeDSServerTransID")]
    pub three_ds_server_trans_id: Option<String>,
    #[serde(rename = "ECI")]
    pub eci: Option<String>,
    pub cryptogram: Option<Secret<String>>,
    pub version: Option<String>,
    pub cclast4: Option<Secret<String>>,
    pub skey: Option<Secret<String>>,
}

impl FiuuAuthnQueryResult {
    /// skey = HMAC_SHA256(AuthenticationID.MerchantID.ReferenceNo.TxnCurrency.
    /// TxnAmount.status.threeDSServerTransID.cclast4, secret_key). Only a verified
    /// result may feed ECI/CAVV into a liability-shifted charge.
    fn verify_skey(&self, secret_key: &Secret<String>) -> bool {
        let Some(skey) = self.skey.as_ref() else {
            return false;
        };
        let Ok(signature) = hex::decode(skey.peek()) else {
            return false;
        };
        let message = format!(
            "{}{}{}{}{}00{}{}",
            self.authentication_id.as_deref().unwrap_or_default(),
            self.merchant_id
                .as_ref()
                .map(PeekInterface::peek)
                .map(String::as_str)
                .unwrap_or_default(),
            self.reference_no.as_deref().unwrap_or_default(),
            self.txn_currency
                .map(|currency| currency.to_string())
                .unwrap_or_default(),
            self.txn_amount
                .as_ref()
                .map(StringMajorUnit::get_amount_as_string)
                .unwrap_or_default(),
            self.three_ds_server_trans_id.as_deref().unwrap_or_default(),
            self.cclast4
                .as_ref()
                .map(PeekInterface::peek)
                .map(String::as_str)
                .unwrap_or_default(),
        );
        crypto::HmacSha256
            .verify_signature(secret_key.peek().as_bytes(), &signature, message.as_bytes())
            .unwrap_or(false)
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FiuuAuthnQueryResponse {
    Query(Box<FiuuAuthnQueryResult>),
    Error(FiuuErrorResponse),
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<FiuuAuthnQueryResponse, Self>>
    for RouterDataV2<
        PostAuthenticate,
        PaymentFlowData,
        PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<FiuuAuthnQueryResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;
        let result = match response {
            FiuuAuthnQueryResponse::Error(error) => {
                return Ok(Self {
                    // Request-level rejection (e.g. Q00004 signature, DateTime skew): the shopper's
                    // authentication is not known to have failed, so stay non-terminal and retryable.
                    response: Err(ErrorResponse {
                        attempt_status: None,
                        ..fiuu_authentication_failed(
                            Some(error.error_code),
                            Some(error.error_desc),
                            http_code,
                        )
                    }),
                    ..router_data
                });
            }
            FiuuAuthnQueryResponse::Query(result) => result,
        };
        // authn_query status. This leg never charges: no Charged/Authorized here.
        match result.status {
            FiuuAuthnStatus::Success => {
                let auth = FiuuAuthType::try_from(&router_data.connector_config).change_context(
                    ConnectorError::ResponseHandlingFailed {
                        context: ResponseTransformationErrorContext {
                            http_status_code: Some(http_code),
                            additional_context: Some(
                                "fiuu: connector config unavailable for authn_query skey check"
                                    .to_owned(),
                            ),
                        },
                    },
                )?;
                if !result.verify_skey(&auth.secret_key) {
                    return Err(ConnectorError::ResponseHandlingFailed {
                        context: ResponseTransformationErrorContext {
                            http_status_code: Some(http_code),
                            additional_context: Some(
                                "fiuu: authn_query skey verification failed; authentication result not trusted"
                                    .to_owned(),
                            ),
                        },
                    }
                    .into());
                }
                // Bind the result to this payment: a valid skey only proves Fiuu issued it.
                let expected = &router_data.request.amount;
                let amount_matches = result
                    .txn_amount
                    .clone()
                    .and_then(|amount| {
                        StringMajorUnitForConnector
                            .convert_back(amount, expected.currency)
                            .ok()
                    })
                    .as_ref()
                    == Some(expected);
                if result.reference_no.as_deref()
                    != Some(
                        router_data
                            .resource_common_data
                            .connector_request_reference_id
                            .as_str(),
                    )
                    || result.txn_currency != Some(expected.currency)
                    || !amount_matches
                {
                    return Err(ConnectorError::ResponseHandlingFailed {
                        context: ResponseTransformationErrorContext {
                            http_status_code: Some(http_code),
                            additional_context: Some(
                                "fiuu: authn_query result belongs to another payment (ReferenceNo/amount/currency mismatch)"
                                    .to_owned(),
                            ),
                        },
                    }
                    .into());
                }
                let result = *result;
                let authentication_data = AuthenticationData {
                    eci: result.eci,
                    cavv: result.cryptogram,
                    threeds_server_transaction_id: result.three_ds_server_trans_id,
                    message_version: result
                        .version
                        .as_deref()
                        .and_then(|version| SemanticVersion::from_str(version).ok()),
                    ds_trans_id: result.ds_tran_id,
                    ..fiuu_authentication_data(result.authentication_id)
                };
                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status: common_enums::AttemptStatus::AuthenticationSuccessful,
                        ..router_data.resource_common_data
                    },
                    response: Ok(PaymentsResponseData::PostAuthenticateResponse {
                        authentication_data: Some(authentication_data),
                        connector_response_reference_id: result.reference_no,
                        status_code: http_code,
                    }),
                    ..router_data
                })
            }
            FiuuAuthnStatus::Failed => Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::AuthenticationFailed,
                    ..router_data.resource_common_data
                },
                response: Err(fiuu_authentication_failed(
                    result.error_code,
                    result.error_desc,
                    http_code,
                )),
                ..router_data
            }),
            // 22 pending, or an undocumented status: inconclusive, never terminal.
            FiuuAuthnStatus::Pending | FiuuAuthnStatus::Unknown => Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::AuthenticationPending,
                    ..router_data.resource_common_data
                },
                response: Ok(PaymentsResponseData::PostAuthenticateResponse {
                    authentication_data: None,
                    connector_response_reference_id: result.reference_no,
                    status_code: http_code,
                }),
                ..router_data
            }),
        }
    }
}
