//! Typed receiver-side Pix Automático contracts and consent-operation conversions.
use common_enums::{AttemptStatus, PaymentMethod, PaymentMethodType};
use common_utils::{metadata::MaskedMetadata, types::Money};
use error_stack::Report;
use grpc_api_types::payments as proto;
use hyperswitch_masking::{PeekInterface, Secret};
use time::{
    format_description::well_known::Rfc3339, macros::format_description, Date, OffsetDateTime,
};

use crate::{
    connector_flow::PushNotification,
    connector_types::{
        ConnectorResponseHeaders, PaymentFlowData, PaymentsResponseData,
        PushNotificationRequestData, RawConnectorRequestResponse, ResponseId,
    },
    errors::{ConnectorError, IntegrationError, ResponseTransformationErrorContext},
    router_data_v2::RouterDataV2,
    types::Connectors,
    utils::{ForeignFrom, ForeignTryFrom},
};

type Result<T> = std::result::Result<T, Report<IntegrationError>>;
fn invalid(field_name: &'static str) -> Report<IntegrationError> {
    IntegrationError::InvalidDataFormat {
        field_name,
        context: Default::default(),
    }
    .into()
}
fn required(field_name: &'static str) -> Report<IntegrationError> {
    IntegrationError::MissingRequiredField {
        field_name,
        context: Default::default(),
    }
    .into()
}
fn nonempty(value: &str, field: &'static str) -> Result<()> {
    if value.trim().is_empty() {
        return Err(required(field));
    }
    Ok(())
}
fn date(value: &str, field: &'static str) -> Result<Date> {
    Date::parse(value, format_description!("[year]-[month]-[day]")).map_err(|_| invalid(field))
}
pub fn validate_txid(value: &str) -> Result<()> {
    if !(26..=35).contains(&value.len()) || !value.bytes().all(|c| c.is_ascii_alphanumeric()) {
        return Err(invalid("initial_transaction_id"));
    }
    Ok(())
}

/// Recognize the two consent instruments without treating ordinary Pix as recurring.
pub fn is_pix_automatico<T: crate::payment_method_data::PaymentMethodDataTypes>(
    instrument: &crate::payment_method_data::PaymentMethodData<T>,
) -> bool {
    matches!(instrument, crate::payment_method_data::PaymentMethodData::BankTransfer(data)
        if matches!(data.as_ref(),
            crate::payment_method_data::BankTransferData::PixAutomaticoPush {}
            | crate::payment_method_data::BankTransferData::PixAutomaticoQr {}))
}

/// Typed configuration must never be silently accepted for an unrelated instrument.
pub fn validate_configuration(instrument_is_pix: bool, configuration_present: bool) -> Result<()> {
    match (instrument_is_pix, configuration_present) {
        (true, false) => Err(required("pix_automatico")),
        (false, true) => Err(invalid("pix_automatico")),
        (true, true) | (false, false) => Ok(()),
    }
}

pub fn is_pix_automatico_type(method: Option<PaymentMethodType>) -> bool {
    matches!(
        method,
        Some(PaymentMethodType::PixAutomaticoPush | PaymentMethodType::PixAutomaticoQr)
    )
}

#[derive(Debug, Clone, Copy)]
pub enum PixAutomaticoFrequency {
    Weekly,
    Monthly,
    Quarterly,
    Semiannually,
    Annually,
}
#[derive(Debug, Clone, Copy)]
pub enum PixAutomaticoRetryPolicy {
    NoRetry,
    ThreeRetriesInSevenDays,
}
#[derive(Debug, Clone)]
pub enum PixAutomaticoAmountRule {
    Fixed(Money),
    Minimum(Money),
}
#[derive(Debug, Clone)]
pub struct PixAutomaticoMandateDetails {
    pub start_date: String,
    pub end_date: Option<String>,
    pub frequency: PixAutomaticoFrequency,
    pub retry_policy: PixAutomaticoRetryPolicy,
    pub amount_rule: Option<PixAutomaticoAmountRule>,
    pub description: Option<String>,
    pub receiver_agreement: Option<String>,
    pub initial_transaction_id: Option<String>,
}
impl ForeignTryFrom<proto::PixAutomaticoMandateDetails> for PixAutomaticoMandateDetails {
    type Error = IntegrationError;
    fn foreign_try_from(v: proto::PixAutomaticoMandateDetails) -> Result<Self> {
        let start = date(&v.start_date, "pix_automatico.start_date")?;
        if let Some(end) = &v.end_date {
            if date(end, "pix_automatico.end_date")? < start {
                return Err(invalid("pix_automatico.end_date"));
            }
        }
        use proto::pix_automatico_mandate_details::{Frequency, RetryPolicy};
        let frequency = match Frequency::try_from(v.frequency)
            .map_err(|_| invalid("pix_automatico.frequency"))?
        {
            Frequency::Weekly => PixAutomaticoFrequency::Weekly,
            Frequency::Monthly => PixAutomaticoFrequency::Monthly,
            Frequency::Quarterly => PixAutomaticoFrequency::Quarterly,
            Frequency::Semiannually => PixAutomaticoFrequency::Semiannually,
            Frequency::Annually => PixAutomaticoFrequency::Annually,
            Frequency::Unspecified => return Err(required("pix_automatico.frequency")),
        };
        let retry_policy = match RetryPolicy::try_from(v.retry_policy)
            .map_err(|_| invalid("pix_automatico.retry_policy"))?
        {
            RetryPolicy::NoRetry => PixAutomaticoRetryPolicy::NoRetry,
            RetryPolicy::ThreeRetriesInSevenDays => {
                PixAutomaticoRetryPolicy::ThreeRetriesInSevenDays
            }
            RetryPolicy::Unspecified => return Err(required("pix_automatico.retry_policy")),
        };
        let amount_rule = v
            .amount_rule
            .map(|rule| {
                use proto::pix_automatico_mandate_details::AmountRule;
                let (money, fixed) = match rule {
                    AmountRule::FixedAmount(m) => (m, true),
                    AmountRule::MinimumAmount(m) => (m, false),
                };
                if money.currency != i32::from(proto::Currency::Brl) || money.minor_amount <= 0 {
                    return Err(invalid("pix_automatico.amount_rule"));
                }
                let m = crate::money::from_proto_money(money)?;
                Ok(if fixed {
                    PixAutomaticoAmountRule::Fixed(m)
                } else {
                    PixAutomaticoAmountRule::Minimum(m)
                })
            })
            .transpose()?;
        if v.description
            .as_ref()
            .is_some_and(|d| d.chars().count() > 35)
        {
            return Err(invalid("pix_automatico.description"));
        }
        if let Some(txid) = &v.initial_transaction_id {
            validate_txid(txid)?;
        }
        Ok(Self {
            start_date: v.start_date,
            end_date: v.end_date,
            frequency,
            retry_policy,
            amount_rule,
            description: v.description,
            receiver_agreement: v.receiver_agreement,
            initial_transaction_id: v.initial_transaction_id,
        })
    }
}
#[derive(Debug, Clone, Copy)]
pub enum PixAutomaticoAccountType {
    Checking,
    Savings,
    Payment,
}
#[derive(Debug, Clone)]
pub struct PixAutomaticoReceiverAccount {
    pub account_number: Secret<String>,
    pub branch: Option<Secret<String>>,
    pub account_type: PixAutomaticoAccountType,
}
#[derive(Debug, Clone)]
pub struct PixAutomaticoChargeDetails {
    pub due_date: String,
    pub adjust_to_business_day: bool,
    pub receiver: PixAutomaticoReceiverAccount,
}
impl ForeignTryFrom<proto::PixAutomaticoChargeDetails> for PixAutomaticoChargeDetails {
    type Error = IntegrationError;
    fn foreign_try_from(v: proto::PixAutomaticoChargeDetails) -> Result<Self> {
        date(&v.due_date, "pix_automatico.due_date")?;
        let receiver = v
            .receiver
            .ok_or_else(|| required("pix_automatico.receiver"))?;
        let account_number = receiver
            .account_number
            .ok_or_else(|| required("pix_automatico.receiver.account_number"))?;
        nonempty(
            account_number.peek(),
            "pix_automatico.receiver.account_number",
        )?;
        if account_number.peek().len() > 20 {
            return Err(invalid("pix_automatico.receiver.account_number"));
        }
        if receiver
            .branch
            .as_ref()
            .is_some_and(|value| value.peek().trim().is_empty() || value.peek().len() > 4)
        {
            return Err(invalid("pix_automatico.receiver.branch"));
        }
        use proto::pix_automatico_receiver_account::AccountType;
        let account_type = match AccountType::try_from(receiver.account_type)
            .map_err(|_| invalid("pix_automatico.receiver.account_type"))?
        {
            AccountType::Checking => PixAutomaticoAccountType::Checking,
            AccountType::Savings => PixAutomaticoAccountType::Savings,
            AccountType::Payment => PixAutomaticoAccountType::Payment,
            AccountType::Unspecified => {
                return Err(required("pix_automatico.receiver.account_type"))
            }
        };
        Ok(Self {
            due_date: v.due_date,
            adjust_to_business_day: v.adjust_to_business_day.unwrap_or(true),
            receiver: PixAutomaticoReceiverAccount {
                account_number,
                branch: receiver.branch,
                account_type,
            },
        })
    }
}
#[derive(Debug, Clone, Copy)]
pub enum PixAutomaticoDocumentType {
    Cpf,
    Cnpj,
}
#[derive(Debug, Clone)]
pub struct PixAutomaticoPayerAccount {
    pub document_number: Secret<String>,
    pub document_type: PixAutomaticoDocumentType,
    pub account_number: Secret<String>,
    pub bank_ispb: String,
    pub branch: Option<Secret<String>>,
}
impl ForeignTryFrom<proto::PixAutomaticoPayerAccount> for PixAutomaticoPayerAccount {
    type Error = IntegrationError;
    fn foreign_try_from(v: proto::PixAutomaticoPayerAccount) -> Result<Self> {
        let document_number = v
            .document_number
            .ok_or_else(|| required("payer.document_number"))?;
        let account_number = v
            .account_number
            .ok_or_else(|| required("payer.account_number"))?;
        nonempty(document_number.peek(), "payer.document_number")?;
        nonempty(account_number.peek(), "payer.account_number")?;
        use proto::pix_automatico_payer_account::DocumentType;
        let document_type = match DocumentType::try_from(v.document_type)
            .map_err(|_| invalid("payer.document_type"))?
        {
            DocumentType::Cpf => {
                if document_number.peek().len() != 11
                    || !document_number.peek().bytes().all(|c| c.is_ascii_digit())
                {
                    return Err(invalid("payer.document_number"));
                }
                PixAutomaticoDocumentType::Cpf
            }
            DocumentType::Cnpj => {
                if document_number.peek().len() != 14
                    || !document_number
                        .peek()
                        .bytes()
                        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
                {
                    return Err(invalid("payer.document_number"));
                }
                PixAutomaticoDocumentType::Cnpj
            }
            DocumentType::Unspecified => return Err(required("payer.document_type")),
        };
        if account_number.peek().len() > 20 {
            return Err(invalid("payer.account_number"));
        }
        nonempty(&v.bank_ispb, "payer.bank_ispb")?;
        if v.bank_ispb.len() != 8
            || !v
                .bank_ispb
                .bytes()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        {
            return Err(invalid("payer.bank_ispb"));
        }
        if v.branch
            .as_ref()
            .is_some_and(|b| b.peek().is_empty() || b.peek().len() > 4)
        {
            return Err(invalid("payer.branch"));
        }
        Ok(Self {
            document_number,
            document_type,
            account_number,
            bank_ispb: v.bank_ispb,
            branch: v.branch,
        })
    }
}
impl ForeignTryFrom<proto::RecurringPaymentServiceRequestConfirmationRequest>
    for PushNotificationRequestData
{
    type Error = IntegrationError;
    fn foreign_try_from(
        v: proto::RecurringPaymentServiceRequestConfirmationRequest,
    ) -> Result<Self> {
        nonempty(
            &v.connector_recurring_payment_id,
            "connector_recurring_payment_id",
        )?;
        nonempty(&v.expires_at, "expires_at")?;
        OffsetDateTime::parse(&v.expires_at, &Rfc3339).map_err(|_| invalid("expires_at"))?;
        Ok(Self {
            connector_mandate_id: v.connector_recurring_payment_id,
            payer: PixAutomaticoPayerAccount::foreign_try_from(
                v.payer.ok_or_else(|| required("payer"))?,
            )?,
            expires_at: v.expires_at,
        })
    }
}
impl
    ForeignTryFrom<(
        proto::RecurringPaymentServiceRequestConfirmationRequest,
        Connectors,
        &MaskedMetadata,
    )> for PaymentFlowData
{
    type Error = IntegrationError;
    fn foreign_try_from(
        (v, connectors, metadata): (
            proto::RecurringPaymentServiceRequestConfirmationRequest,
            Connectors,
            &MaskedMetadata,
        ),
    ) -> Result<Self> {
        let connector_customer = v
            .state
            .as_ref()
            .and_then(|s| s.connector_customer_id.clone());
        let mut common = Self::foreign_try_from((
            proto::PaymentServiceGetRequest {
                merchant_transaction_id: v.merchant_request_id.clone(),
                connector_transaction_id: v.connector_recurring_payment_id,
                merchant_request_id: v.merchant_request_id,
                state: v.state,
                test_mode: v.test_mode,
                connector_feature_data: v.connector_feature_data,
                ..Default::default()
            },
            connectors,
            metadata,
        ))?;
        common.payment_method = PaymentMethod::BankTransfer;
        common.status = AttemptStatus::Pending;
        common.connector_customer = connector_customer;
        // No financial amount is fabricated for consent operations.
        Ok(common)
    }
}

pub fn generate_recurring_confirmation_response(
    data: RouterDataV2<
        PushNotification,
        PaymentFlowData,
        PushNotificationRequestData,
        PaymentsResponseData,
    >,
) -> std::result::Result<proto::RecurringPaymentServiceConsentResponse, Report<ConnectorError>> {
    generate_consent_response(data, |r| &r.connector_mandate_id, true)
}
fn generate_consent_response<F: crate::flow_status::FlowStatusRules, Req>(
    data: RouterDataV2<F, PaymentFlowData, Req, PaymentsResponseData>,
    mandate_id: impl FnOnce(&Req) -> &str,
    confirmation: bool,
) -> std::result::Result<proto::RecurringPaymentServiceConsentResponse, Report<ConnectorError>> {
    let common = &data.resource_common_data;
    let invalid_status = |http_status_code| {
        Report::new(ConnectorError::UnexpectedResponseError {
            context: ResponseTransformationErrorContext {
                http_status_code,
                additional_context: Some(format!("Invalid consent status for {}", F::NAME)),
            },
        })
    };
    if !F::ALLOWED.contains(&common.status) {
        return Err(invalid_status(common.connector_http_status_code));
    }
    // An error's explicit status is subject to the same contract as a successful response.
    let error_status = match &data.response {
        Err(error) => error
            .attempt_status
            .as_ref()
            .map(|status| {
                status
                    .as_attempt_status()
                    .filter(|status| F::ALLOWED.contains(status))
                    .ok_or_else(|| invalid_status(Some(error.status_code)))
            })
            .transpose()?,
        Ok(_) => None,
    };
    let mut response = proto::RecurringPaymentServiceConsentResponse {
        connector_recurring_payment_id: Some(mandate_id(&data.request).to_owned()),
        status: i32::from(proto::PaymentStatus::foreign_from(
            error_status.unwrap_or(common.status),
        )),
        response_headers: common.get_connector_response_headers_as_map(),
        state: if common.access_token.is_some() || common.connector_customer.is_some() {
            Some(proto::ConnectorState {
                access_token: common.access_token.as_ref().map(|t| proto::AccessToken {
                    token: Some(t.access_token.clone()),
                    expires_in_seconds: t.expires_in,
                    token_type: t.token_type.clone(),
                }),
                connector_customer_id: common.connector_customer.clone(),
            })
        } else {
            None
        },
        raw_connector_request: common.get_raw_connector_request(),
        raw_connector_response: common.get_raw_connector_response(),
        typed_connector_request: common.get_typed_connector_request().map(Secret::new),
        typed_connector_response: common.get_typed_connector_response().map(Secret::new),
        ..Default::default()
    };
    match data.response {
        Ok(PaymentsResponseData::TransactionResponse {
            resource_id,
            redirection_data,
            connector_metadata,
            mandate_reference,
            status_code,
            ..
        }) => {
            if confirmation {
                response.connector_confirmation_id = match resource_id {
                    ResponseId::ConnectorTransactionId(id) => Some(id),
                    ResponseId::NoResponseId | ResponseId::EncodedData(_) => None,
                };
            }
            if let Some(id) = mandate_reference.and_then(|m| m.connector_mandate_id) {
                response.connector_recurring_payment_id = Some(id);
            }
            response.redirection_data = redirection_data
                .map(|r| proto::RedirectForm::foreign_try_from(*r))
                .transpose()?;
            response.connector_feature_data =
                connector_metadata.map(|v| Secret::new(v.to_string()));
            response.status_code = u32::from(status_code);
        }
        Ok(_) => {
            return Err(ConnectorError::UnexpectedResponseError {
                context: ResponseTransformationErrorContext {
                    http_status_code: common.connector_http_status_code,
                    additional_context: Some(
                        "Expected a recurring consent transaction response".to_owned(),
                    ),
                },
            }
            .into())
        }
        Err(error) => {
            response.status_code = u32::from(error.status_code);
            response.error = Some(proto::ErrorInfo {
                unified_details: None,
                connector_details: Some(proto::ConnectorErrorDetails {
                    code: Some(error.code.clone()),
                    message: Some(error.message.clone()),
                    reason: error.reason.clone(),
                    connector_transaction_id: error.connector_transaction_id.clone(),
                    status: error.attempt_status.as_ref().map(ForeignFrom::foreign_from),
                }),
                issuer_details: Some(proto::IssuerErrorDetails::from(&error)),
            });
        }
    }
    Ok(response)
}

impl ForeignTryFrom<proto::RecurringPaymentServiceGetQrRequest>
    for crate::connector_types::GenerateQrRequestData
{
    type Error = IntegrationError;
    fn foreign_try_from(v: proto::RecurringPaymentServiceGetQrRequest) -> Result<Self> {
        nonempty(
            &v.connector_recurring_payment_id,
            "connector_recurring_payment_id",
        )?;
        if let Some(txid) = &v.initial_transaction_id {
            validate_txid(txid)?;
        }
        Ok(Self {
            connector_mandate_id: v.connector_recurring_payment_id,
            initial_transaction_id: v.initial_transaction_id,
        })
    }
}
impl
    ForeignTryFrom<(
        proto::RecurringPaymentServiceGetQrRequest,
        Connectors,
        &MaskedMetadata,
    )> for PaymentFlowData
{
    type Error = IntegrationError;
    fn foreign_try_from(
        (v, connectors, metadata): (
            proto::RecurringPaymentServiceGetQrRequest,
            Connectors,
            &MaskedMetadata,
        ),
    ) -> Result<Self> {
        let connector_customer = v
            .state
            .as_ref()
            .and_then(|s| s.connector_customer_id.clone());
        let mut common = Self::foreign_try_from((
            proto::PaymentServiceGetRequest {
                merchant_transaction_id: v.merchant_request_id.clone(),
                merchant_request_id: v.merchant_request_id,
                connector_transaction_id: v.connector_recurring_payment_id,
                state: v.state,
                test_mode: v.test_mode,
                connector_feature_data: v.connector_feature_data,
                ..Default::default()
            },
            connectors,
            metadata,
        ))?;
        common.payment_method = PaymentMethod::BankTransfer;
        common.status = AttemptStatus::Pending;
        common.connector_customer = connector_customer;
        Ok(common)
    }
}
pub fn generate_recurring_qr_response(
    data: RouterDataV2<
        crate::connector_flow::GenerateQr,
        PaymentFlowData,
        crate::connector_types::GenerateQrRequestData,
        PaymentsResponseData,
    >,
) -> std::result::Result<proto::RecurringPaymentServiceConsentResponse, Report<ConnectorError>> {
    generate_consent_response(data, |r| &r.connector_mandate_id, false)
}
