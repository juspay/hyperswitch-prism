use common_utils::{pii, request::Method, FloatMajorUnit};
use domain_types::{
    connector_flow::{self, Authorize, RepeatPayment, SetupMandate},
    connector_types::{
        MandateReference, MandateReferenceId, PaymentFlowData, PaymentVoidData,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData,
        ResponseId, SetupMandateRequestData,
    },
    errors::{ConnectorError, IntegrationError, ResponseTransformationErrorContext},
    payment_method_data::{self, PaymentMethodData, PaymentMethodDataTypes, RawCardNumber},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
    utils,
};
use error_stack::ResultExt;
use hyperswitch_masking::Secret;
use serde::{Deserialize, Serialize};

use crate::{connectors::dlocal::DlocalRouterData, types::ResponseRouterData};

#[derive(Debug, Default, Eq, PartialEq, Serialize)]
pub struct Payer {
    pub name: Secret<String>,
    pub email: pii::Email,
    pub document: Secret<String>,
}

#[derive(Debug, Default, Eq, Clone, PartialEq, Serialize, Deserialize)]
pub struct Card<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize> {
    pub holder_name: Option<Secret<String>>,
    pub number: RawCardNumber<T>,
    pub cvv: Secret<String>,
    pub expiration_month: Secret<String>,
    pub expiration_year: Secret<String>,
    pub capture: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub save: Option<bool>,
}

#[derive(Debug, Default, Eq, PartialEq, Serialize)]
pub struct ThreeDSecureReqData {
    pub force: bool,
}

#[derive(Debug, Serialize, Default, Deserialize, Clone, Eq, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum PaymentMethodId {
    #[default]
    Card,
    #[serde(untagged)]
    Other(String),
}

#[derive(Debug, Serialize, Default, Deserialize, Clone, Eq, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum PaymentMethodFlow {
    #[default]
    Direct,
    #[serde(rename = "REDIRECT")]
    Redirect,
}

#[derive(Default, Debug, Serialize, PartialEq)]
pub struct DlocalPaymentsRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub amount: FloatMajorUnit,
    pub currency: common_enums::Currency,
    pub country: common_enums::CountryAlpha2,
    pub payment_method_id: PaymentMethodId,
    pub payment_method_flow: PaymentMethodFlow,
    pub payer: Payer,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card: Option<Card<T>>,
    pub order_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub three_dsecure: Option<ThreeDSecureReqData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub callback_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notification_url: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        DlocalRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for DlocalPaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: DlocalRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let email = item.router_data.request.get_email()?;
        let address = item
            .router_data
            .resource_common_data
            .get_billing_address()?;
        let country = *address.get_country()?;
        let name = address.get_full_name()?;
        let amount = utils::convert_amount(
            item.connector.amount_converter,
            item.router_data.request.minor_amount,
            item.router_data.request.currency,
        )?;
        let order_id = item
            .router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();
        let callback_url = Some(item.router_data.request.get_router_return_url()?);
        let description = item.router_data.resource_common_data.description.clone();

        match item.router_data.request.payment_method_data {
            PaymentMethodData::Card(ref ccard) => {
                let should_capture = matches!(
                    item.router_data.request.capture_method,
                    Some(common_enums::CaptureMethod::Automatic)
                        | Some(common_enums::CaptureMethod::SequentialAutomatic)
                );
                let payment_request = Self {
                    amount,
                    currency: item.router_data.request.currency,
                    payment_method_id: PaymentMethodId::Card,
                    payment_method_flow: PaymentMethodFlow::Direct,
                    country,
                    payer: Payer {
                        name,
                        email,
                        // dLocal requires a payer document (tax ID) for Latin American markets.
                        // The hyperswitch reference uses `get_customer_document_details()` to pull
                        // the real customer document; UCS does not yet surface this PII on the
                        // Authorize request, so a country-specific sandbox-valid placeholder is
                        // used here. Production flows should pass the real customer document.
                        document: get_doc_from_currency(country.to_string()),
                    },
                    card: Some(Card {
                        holder_name: ccard.card_holder_name.clone(),
                        number: ccard.card_number.clone(),
                        cvv: ccard.card_cvc.clone(),
                        expiration_month: ccard.card_exp_month.clone(),
                        expiration_year: ccard.card_exp_year.clone(),
                        capture: should_capture.to_string(),
                        // `save` is None for regular Authorize flow — card tokenization
                        // is handled separately via SetupMandate with `save: Some(true)`.
                        save: None,
                    }),
                    order_id,
                    three_dsecure: match item.router_data.resource_common_data.auth_type {
                        common_enums::AuthenticationType::ThreeDs => {
                            Some(ThreeDSecureReqData { force: true })
                        }
                        common_enums::AuthenticationType::NoThreeDs => None,
                    },
                    callback_url,
                    description,
                    notification_url: None,
                };
                Ok(payment_request)
            }
            PaymentMethodData::BankDebit(ref bank_debit_data) => {
                let (payment_method_id, _notification_url) =
                    get_bank_debit_payment_method_id(bank_debit_data, country)?;
                let webhook_url = item.router_data.request.webhook_url.clone();
                let payment_request = Self {
                    amount,
                    currency: item.router_data.request.currency,
                    payment_method_id,
                    payment_method_flow: PaymentMethodFlow::Redirect,
                    country,
                    payer: Payer {
                        name,
                        email,
                        // Same placeholder rationale as the card Authorize branch above —
                        // UCS has not yet plumbed the real customer document through.
                        document: get_doc_from_currency(country.to_string()),
                    },
                    card: None,
                    order_id,
                    three_dsecure: None,
                    callback_url,
                    description,
                    notification_url: webhook_url,
                };
                Ok(payment_request)
            }
            PaymentMethodData::CardRedirect(_)
            | PaymentMethodData::Wallet(_)
            | PaymentMethodData::PayLater(_)
            | PaymentMethodData::BankRedirect(_)
            | PaymentMethodData::BankTransfer(_)
            | PaymentMethodData::Crypto(_)
            | PaymentMethodData::MandatePayment
            | PaymentMethodData::Reward
            | PaymentMethodData::RealTimePayment(_)
            | PaymentMethodData::MobilePayment(_)
            | PaymentMethodData::Upi(_)
            | PaymentMethodData::Voucher(_)
            | PaymentMethodData::GiftCard(_)
            | PaymentMethodData::OpenBanking(_)
            | PaymentMethodData::PaymentMethodToken(_)
            | PaymentMethodData::NetworkToken(_)
            | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
            | PaymentMethodData::CardDetailsForNetworkTransactionId(_) => {
                Err(IntegrationError::not_implemented(
                    crate::utils::get_unimplemented_payment_method_error_message("Dlocal"),
                ))?
            }
        }
    }
}

#[derive(Default, Debug, Serialize, PartialEq)]
pub struct DlocalPaymentsCaptureRequest {
    pub authorization_id: Secret<String>,
    pub amount: FloatMajorUnit,
    pub currency: String,
    pub order_id: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        DlocalRouterData<
            RouterDataV2<
                connector_flow::Capture,
                PaymentFlowData,
                PaymentsCaptureData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for DlocalPaymentsCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: DlocalRouterData<
            RouterDataV2<
                connector_flow::Capture,
                PaymentFlowData,
                PaymentsCaptureData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let amount = utils::convert_amount(
            item.connector.amount_converter,
            item.router_data.request.minor_amount_to_capture,
            item.router_data.request.currency,
        )?;

        Ok(Self {
            authorization_id: Secret::new(
                item.router_data
                    .request
                    .connector_transaction_id
                    .get_connector_transaction_id()
                    .change_context(IntegrationError::MissingConnectorTransactionID {
                        context: Default::default(),
                    })?,
            ),
            amount,
            currency: item.router_data.request.currency.to_string(),
            order_id: item
                .router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
        })
    }
}
// RepeatPayment (MIT) flow types

#[derive(Debug, Serialize, Clone, Eq, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StoredCredentialType {
    CardOnFile,
    Subscription,
    UnscheduledCardOnFile,
    Installments,
}

impl From<Option<common_enums::MitCategory>> for StoredCredentialType {
    fn from(category: Option<common_enums::MitCategory>) -> Self {
        match category {
            Some(common_enums::MitCategory::Recurring) => Self::Subscription,
            Some(common_enums::MitCategory::Installment) => Self::Installments,
            Some(common_enums::MitCategory::Unscheduled) => Self::UnscheduledCardOnFile,
            Some(common_enums::MitCategory::Resubmission) => Self::UnscheduledCardOnFile,
            None => Self::CardOnFile,
        }
    }
}

#[derive(Debug, Serialize, Clone, Eq, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StoredCredentialUsage {
    Used,
}

#[derive(Debug, Serialize)]
pub struct DlocalRepeatPaymentCard {
    pub card_id: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capture: Option<String>,
    pub stored_credential_type: StoredCredentialType,
    pub stored_credential_usage: StoredCredentialUsage,
}

#[derive(Debug, Serialize)]
pub struct DlocalRepeatPaymentRequest {
    pub amount: FloatMajorUnit,
    pub currency: common_enums::Currency,
    pub country: common_enums::CountryAlpha2,
    pub payment_method_id: PaymentMethodId,
    pub payment_method_flow: PaymentMethodFlow,
    pub payer: Payer,
    pub card: DlocalRepeatPaymentCard,
    pub order_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notification_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        DlocalRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for DlocalRepeatPaymentRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: DlocalRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Extract connector_mandate_id (card_id from a prior CIT)
        let card_id = match &router_data.request.mandate_reference {
            MandateReferenceId::ConnectorMandateId(connector_mandate_ref) => connector_mandate_ref
                .get_connector_mandate_id()
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "connector_mandate_id",
                    context: Default::default(),
                })?,
            MandateReferenceId::NetworkMandateId(_)
            | MandateReferenceId::NetworkTokenWithNTI(_) => {
                Err(error_stack::report!(IntegrationError::NotSupported {
                    message: "Network mandate ID not supported for repeat payments in dlocal"
                        .to_string(),
                    connector: "Dlocal",
                    context: Default::default(),
                }))?
            }
        };

        let address = router_data.resource_common_data.get_billing_address()?;
        let country = *address.get_country()?;
        let name = address.get_full_name()?;

        let email = router_data.request.get_email()?;

        let amount = utils::convert_amount(
            item.connector.amount_converter,
            router_data.request.minor_amount,
            router_data.request.currency,
        )?;

        let order_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        let should_capture = matches!(
            router_data.request.capture_method,
            Some(common_enums::CaptureMethod::Automatic)
                | Some(common_enums::CaptureMethod::SequentialAutomatic)
                | None
        );

        let stored_credential_type =
            StoredCredentialType::from(router_data.request.mit_category.clone());

        Ok(Self {
            amount,
            currency: router_data.request.currency,
            country,
            payment_method_id: PaymentMethodId::Card,
            payment_method_flow: PaymentMethodFlow::Direct,
            payer: Payer {
                name,
                email,
                document: get_doc_from_currency(country.to_string()),
            },
            card: DlocalRepeatPaymentCard {
                card_id: Secret::new(card_id),
                capture: Some(should_capture.to_string()),
                stored_credential_type,
                stored_credential_usage: StoredCredentialUsage::Used,
            },
            order_id,
            notification_url: router_data.request.webhook_url.clone(),
            description: router_data.resource_common_data.description.clone(),
        })
    }
}

// RepeatPayment response - reuses DlocalPaymentsResponse (generic TryFrom covers this)
pub type DlocalRepeatPaymentResponse = DlocalPaymentsResponse;

// SetupMandate (CIT) flow: tokenize a card by sending a zero-amount payment with
// `card.save=true`. dLocal's response includes a `card` object containing the
// saved `card_id` which is later used in RepeatPayment (MIT) flow.

#[derive(Debug, Serialize)]
pub struct DlocalSetupMandateRequest<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
> {
    pub amount: FloatMajorUnit,
    pub currency: common_enums::Currency,
    pub country: common_enums::CountryAlpha2,
    pub payment_method_id: PaymentMethodId,
    pub payment_method_flow: PaymentMethodFlow,
    pub payer: Payer,
    pub card: Card<T>,
    pub order_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub three_dsecure: Option<ThreeDSecureReqData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub callback_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notification_url: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        DlocalRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for DlocalSetupMandateRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: DlocalRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        let email = router_data.request.get_email()?;
        let address = router_data.resource_common_data.get_billing_address()?;
        let country = *address.get_country()?;
        let name = address.get_full_name()?;

        // For SetupMandate (CIT), dLocal requires a non-zero authorization amount
        // alongside `card.save = true` to tokenize the card. dLocal rejects
        // amounts <= 1.00 with code 5016 "Amount too low", so callers must
        // provide an appropriate verify amount (e.g. 5.00 BRL). The request
        // runs with `capture: false` so funds are released immediately after
        // the authorization.
        let minor_amount =
            router_data
                .request
                .minor_amount
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "minor_amount",
                    context: Default::default(),
                })?;
        let amount = utils::convert_amount(
            item.connector.amount_converter,
            minor_amount,
            router_data.request.currency,
        )?;

        let order_id = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();
        let callback_url = router_data.request.router_return_url.clone();
        let description = router_data.resource_common_data.description.clone();

        match &router_data.request.payment_method_data {
            PaymentMethodData::Card(ccard) => Ok(Self {
                amount,
                currency: router_data.request.currency,
                payment_method_id: PaymentMethodId::Card,
                payment_method_flow: PaymentMethodFlow::Direct,
                country,
                payer: Payer {
                    name,
                    email,
                    // dLocal requires a payer document (tax ID) for Latin American markets.
                    // The hyperswitch reference uses `get_customer_document_details()` to pull
                    // the real customer document; UCS does not yet surface this PII on the
                    // SetupMandate request, so a country-specific sandbox-valid placeholder is
                    // used here. Production flows should pass the real customer document.
                    document: get_doc_from_currency(country.to_string()),
                },
                card: Card {
                    holder_name: ccard.card_holder_name.clone(),
                    number: ccard.card_number.clone(),
                    cvv: ccard.card_cvc.clone(),
                    expiration_month: ccard.card_exp_month.clone(),
                    expiration_year: ccard.card_exp_year.clone(),
                    // Setup mandate is always a verify/no-capture operation
                    capture: "false".to_string(),
                    save: Some(true),
                },
                order_id,
                three_dsecure: None,
                callback_url,
                description,
                notification_url: router_data.request.webhook_url.clone(),
            }),
            _ => Err(error_stack::report!(IntegrationError::NotSupported {
                message: crate::utils::get_unimplemented_payment_method_error_message("Dlocal"),
                connector: "Dlocal",
                context: Default::default(),
            }))?,
        }
    }
}

// SetupMandate response — adds a `card` object with the saved `card_id` on top of
// the standard payment response fields. Per dLocal's Save-Card API the saved token
// is returned as `card.card_id` (same field consumed by the RepeatPayment/MIT flow
// above, see `DlocalRepeatPaymentCard`).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DlocalSetupMandateCardData {
    pub card_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DlocalSetupMandateResponse {
    pub status: DlocalPaymentStatus,
    pub id: String,
    pub three_dsecure: Option<ThreeDSecureResData>,
    pub order_id: Option<String>,
    pub redirect_url: Option<url::Url>,
    pub card: Option<DlocalSetupMandateCardData>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<DlocalSetupMandateResponse, Self>>
    for RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<DlocalSetupMandateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // For failed payments (Rejected/Cancelled), return ErrorResponse instead
        // of TransactionResponse to capture the failure details.
        if matches!(
            item.response.status,
            DlocalPaymentStatus::Rejected | DlocalPaymentStatus::Cancelled
        ) {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: common_enums::AttemptStatus::from(item.response.status.clone()),
                    ..item.router_data.resource_common_data
                },
                response: Err(ErrorResponse {
                    code: format!("{:?}", item.response.status),
                    message: format!(
                        "SetupMandate failed with status: {:?}",
                        item.response.status
                    ),
                    reason: Some(format!(
                        "dLocal returned {:?} status for SetupMandate request",
                        item.response.status
                    )),
                    status_code: item.http_code,
                    attempt_status: Some(common_enums::AttemptStatus::from(
                        item.response.status.clone(),
                    )),
                    connector_transaction_id: Some(item.response.id.clone()),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                }),
                ..item.router_data
            });
        }

        let redirection_data = item
            .response
            .three_dsecure
            .and_then(|three_secure_data| three_secure_data.redirect_url)
            .or(item.response.redirect_url)
            .map(|redirect_url| RedirectForm::from((redirect_url, Method::Get)));

        // Extract the saved card identifier from the response. Per dLocal's
        // Save-Card API contract the token is returned on `card.card_id`.
        let saved_card_id = item.response.card.as_ref().and_then(|c| c.card_id.clone());

        // SetupMandate only succeeds if dLocal hands us a tokenised card_id —
        // otherwise the downstream RepeatPayment (MIT) flow has nothing to
        // reference. Fail fast on AUTHORIZED / PAID responses that are missing
        // the token rather than silently completing with `mandate_reference = None`.
        let is_successful = matches!(
            item.response.status,
            DlocalPaymentStatus::Authorized | DlocalPaymentStatus::Paid
        );
        if is_successful && saved_card_id.is_none() {
            return Err(ConnectorError::UnexpectedResponseError {
                context: ResponseTransformationErrorContext {
                    http_status_code: Some(item.http_code),
                    additional_context: Some(
                        "dLocal SetupMandate succeeded but response is missing `card.card_id` — \
                         cannot build MandateReference for downstream RepeatPayment (MIT) flow."
                            .to_string(),
                    ),
                },
            }
            .into());
        }

        let mandate_reference = saved_card_id.map(|card_id| MandateReference {
            connector_mandate_id: Some(card_id),
            payment_method_id: None,
            connector_mandate_request_reference_id: None,
        });

        let response = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
            redirection_data: redirection_data.map(Box::new),
            mandate_reference: mandate_reference.map(Box::new),
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: item.response.order_id.clone(),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: common_enums::AttemptStatus::from(item.response.status),
                ..item.router_data.resource_common_data
            },
            response: Ok(response),
            ..item.router_data
        })
    }
}

// Auth Struct
pub struct DlocalAuthType {
    pub(super) x_login: Secret<String>,
    pub(super) x_trans_key: Secret<String>,
    pub(super) secret: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for DlocalAuthType {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        if let ConnectorSpecificConfig::Dlocal {
            x_login,
            x_trans_key,
            secret,
            ..
        } = auth_type
        {
            Ok(Self {
                x_login: x_login.to_owned(),
                x_trans_key: x_trans_key.to_owned(),
                secret: secret.to_owned(),
            })
        } else {
            Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
            .into())
        }
    }
}
#[derive(Debug, Clone, Eq, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum DlocalPaymentStatus {
    Authorized,
    Paid,
    Cancelled,
    #[default]
    Pending,
    Rejected,
}

impl From<DlocalPaymentStatus> for common_enums::AttemptStatus {
    fn from(item: DlocalPaymentStatus) -> Self {
        match item {
            DlocalPaymentStatus::Authorized => Self::Authorized,
            DlocalPaymentStatus::Paid => Self::Charged,
            DlocalPaymentStatus::Pending => Self::Pending,
            DlocalPaymentStatus::Cancelled => Self::Voided,
            DlocalPaymentStatus::Rejected => Self::Failure,
        }
    }
}

#[derive(Eq, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThreeDSecureResData {
    pub redirect_url: Option<url::Url>,
}

#[derive(Debug, Default, Eq, Clone, PartialEq, Serialize, Deserialize)]
pub struct DlocalPaymentsResponse {
    status: DlocalPaymentStatus,
    id: String,
    three_dsecure: Option<ThreeDSecureResData>,
    order_id: Option<String>,
    redirect_url: Option<url::Url>,
}

impl<F, T> TryFrom<ResponseRouterData<DlocalPaymentsResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, T, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<DlocalPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Check for redirect URL from both 3DS (card) and top-level (bank transfer/APM) responses
        let redirection_data = item
            .response
            .three_dsecure
            .and_then(|three_secure_data| three_secure_data.redirect_url)
            .or(item.response.redirect_url)
            .map(|redirect_url| RedirectForm::from((redirect_url, Method::Get)));

        let response = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
            redirection_data: redirection_data.map(Box::new),
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: item.response.order_id.clone(),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
        };
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: common_enums::AttemptStatus::from(item.response.status),
                ..item.router_data.resource_common_data
            },
            response: Ok(response),
            ..item.router_data
        })
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DlocalPaymentsSyncResponse {
    status: DlocalPaymentStatus,
    id: String,
    order_id: Option<String>,
}

impl<F> TryFrom<ResponseRouterData<DlocalPaymentsSyncResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<DlocalPaymentsSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: common_enums::AttemptStatus::from(item.response.status),
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: item.response.order_id.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DlocalPaymentsCaptureResponse {
    status: DlocalPaymentStatus,
    id: String,
    order_id: Option<String>,
}

impl<F> TryFrom<ResponseRouterData<DlocalPaymentsCaptureResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<DlocalPaymentsCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: common_enums::AttemptStatus::from(item.response.status),
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: item.response.order_id.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

pub struct DlocalPaymentsCancelResponse {
    status: DlocalPaymentStatus,
    order_id: String,
}

impl<F> TryFrom<ResponseRouterData<DlocalPaymentsCancelResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<DlocalPaymentsCancelResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: common_enums::AttemptStatus::from(item.response.status),
                ..item.router_data.resource_common_data
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.order_id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(item.response.order_id.clone()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// REFUND :
#[derive(Default, Debug, Serialize)]
pub struct DlocalRefundRequest {
    pub amount: FloatMajorUnit,
    pub payment_id: String,
    pub currency: common_enums::Currency,
    pub id: String,
}

impl<F, T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<DlocalRouterData<RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>, T>>
    for DlocalRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: DlocalRouterData<
            RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let amount_to_refund = utils::convert_amount(
            item.connector.amount_converter,
            item.router_data.request.minor_refund_amount,
            item.router_data.request.currency,
        )?;

        Ok(Self {
            amount: amount_to_refund,
            payment_id: item.router_data.request.connector_transaction_id.clone(),
            currency: item.router_data.request.currency,
            id: item.router_data.request.refund_id.clone(),
        })
    }
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Default, Deserialize, Clone)]
#[serde(rename_all = "UPPERCASE")]
pub enum RefundStatus {
    Success,
    #[default]
    Pending,
    Rejected,
    Cancelled,
}

impl From<RefundStatus> for common_enums::RefundStatus {
    fn from(item: RefundStatus) -> Self {
        match item {
            RefundStatus::Success => Self::Success,
            RefundStatus::Pending => Self::Pending,
            RefundStatus::Rejected | RefundStatus::Cancelled => Self::Failure,
        }
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct RefundResponse {
    pub id: String,
    pub status: RefundStatus,
}

impl<F> TryFrom<ResponseRouterData<RefundResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<RefundResponse, Self>) -> Result<Self, Self::Error> {
        let refund_status = common_enums::RefundStatus::from(item.response.status);
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.id,
                refund_status,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

impl<F> TryFrom<ResponseRouterData<RefundResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(item: ResponseRouterData<RefundResponse, Self>) -> Result<Self, Self::Error> {
        let refund_status = common_enums::RefundStatus::from(item.response.status);
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.id,
                refund_status,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

#[derive(Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct DlocalErrorResponse {
    pub code: i32,
    pub message: String,
    pub param: Option<String>,
}

/// Maps BankDebitData and country to the dLocal-specific payment_method_id.
/// dLocal uses country-specific payment method codes for bank transfer / direct debit payments.
/// For REDIRECT flow bank transfers, the payment_method_id depends on the country:
///   - BR: "IO" (Bank Transfer)
///   - AR: "IO" (Bank Transfer)
///   - MX: "SE" (SPEI)
///   - CL: "WP" (Webpay)
///   - CO: "PC" (PSE)
///   - PE: "BC" (BCP)
///
/// We use well-known codes per country from dLocal's payment methods API.
fn get_bank_debit_payment_method_id(
    bank_debit_data: &payment_method_data::BankDebitData,
    country: common_enums::CountryAlpha2,
) -> Result<(PaymentMethodId, Option<String>), error_stack::Report<IntegrationError>> {
    match bank_debit_data {
        payment_method_data::BankDebitData::SepaBankDebit { .. }
        | payment_method_data::BankDebitData::SepaGuaranteedBankDebit { .. }
        | payment_method_data::BankDebitData::AchBankDebit { .. } => {
            let method_id = get_bank_transfer_method_id_for_country(country)?;
            Ok((PaymentMethodId::Other(method_id), None))
        }
        payment_method_data::BankDebitData::BecsBankDebit { .. }
        | payment_method_data::BankDebitData::EftBankDebit { .. }
        | payment_method_data::BankDebitData::BacsBankDebit { .. } => {
            Err(error_stack::report!(IntegrationError::NotSupported {
                message: crate::utils::get_unimplemented_payment_method_error_message("Dlocal"),
                connector: "Dlocal",
                context: Default::default(),
            }))?
        }
    }
}

/// Returns a well-known bank transfer payment_method_id for the given country.
fn get_bank_transfer_method_id_for_country(
    country: common_enums::CountryAlpha2,
) -> Result<String, error_stack::Report<IntegrationError>> {
    match country {
        common_enums::CountryAlpha2::BR => Ok("IO".to_string()), // Bank Transfer
        common_enums::CountryAlpha2::AR => Ok("IO".to_string()), // Bank Transfer
        common_enums::CountryAlpha2::MX => Ok("SE".to_string()), // SPEI
        common_enums::CountryAlpha2::CL => Ok("WP".to_string()), // Webpay
        common_enums::CountryAlpha2::CO => Ok("PC".to_string()), // PSE
        common_enums::CountryAlpha2::PE => Ok("BC".to_string()), // BCP
        _ => Err(IntegrationError::NotSupported {
            message: format!("Bank debit is not supported for country: {country}"),
            connector: "Dlocal",
            context: Default::default(),
        }
        .into()),
    }
}

/// Returns a placeholder payer document (tax ID) for the given country.
///
/// dLocal requires a payer document for Latin American markets. These hardcoded
/// values are test/placeholder documents used when the actual customer document
/// is not provided in the request. In production, merchants should pass the real
/// customer document via the billing address or payer information.
///
/// The format varies by country:
/// - BR: CPF (11 digits) — Brazilian individual tax ID
/// - MX: CURP (18 chars) — Mexican unique population registry code
/// - AR: DNI (7-9 digits) — Argentine national identity document
/// - etc.
fn get_doc_from_currency(country: String) -> Secret<String> {
    let doc = match country.as_str() {
        "BR" => "91483309223",        // CPF (11 digits)
        "MX" => "BADD110313HCMLNS09", // CURP (18 chars)
        "AR" => "30682389",           // DNI (7-9 digits)
        "CL" => "12345678",           // CI/RUT (8-9 chars)
        "CO" => "1234567890",         // CC (6-11 digits)
        "PE" => "12345678",           // DNI (8 digits)
        "UY" => "12345678",           // CI (6-8 digits)
        "ZA" => "2001014800086",
        "BD" | "GT" | "HN" | "PK" | "SN" | "TH" => "1234567890001",
        "CR" | "SV" | "VN" => "123456789",
        "DO" | "NG" => "12345678901",
        "EG" => "12345678901112",
        "GH" | "ID" | "RW" | "UG" => "1234567890111123",
        "IN" => "NHSTP6374G",
        "CI" => "CA124356789",
        "JP" | "MY" | "PH" => "123456789012",
        "NI" => "1234567890111A",
        "TZ" => "12345678912345678900",
        _ => "12345678",
    };
    Secret::new(doc.to_string())
}
