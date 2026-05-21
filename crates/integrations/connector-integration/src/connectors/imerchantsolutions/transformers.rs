use common_enums::{self, AttemptStatus, CaptureMethod, CountryAlpha2, Currency, RefundStatus};
use common_utils::{consts, errors::ParsingError, pii, types::MinorUnit};
use domain_types::{
    connector_flow::{Authorize, Capture, RSync, Refund, RepeatPayment, Void},
    connector_types::{
        EventType, MandateReference, PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData,
        PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData, RefundFlowData,
        RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData, ResponseId,
    },
    errors,
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes, RawCardNumber},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_request_types::SyncRequestType,
    utils::is_payment_failure,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, ExposeOptionInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::{
    connectors::{
        imerchantsolutions::ImerchantsolutionsRouterData, jpmorgan::JpmorganResourceData,
    },
    types::ResponseRouterData,
    utils::{self, is_manual_capture},
};

const IMERCHANTSOLUTIONS: &str = "imerchantsolutions";

pub struct ImerchantsolutionsAuthType {
    pub(super) api_key: Secret<String>,
    pub(super) merchant_id: Option<Secret<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImerchantsolutionsErrorResponse {
    pub error: String,
    pub message: Option<String>,
    pub code: Option<String>,
    pub suggestion: Option<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for ImerchantsolutionsAuthType {
    type Error = error_stack::Report<errors::IntegrationError>;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Imerchantsolutions {
                api_key,
                merchant_id,
                ..
            } => {
                let is_platform_key = api_key.clone().expose().starts_with("pk_");
                if is_platform_key {
                    if merchant_id.is_none() {
                        return Err(errors::IntegrationError::FailedToObtainAuthType {
                            context: errors::IntegrationErrorContext {
                                suggested_action: Some("Provide `merchant_id` when using a platform API key (prefix `pk_`).".to_string()),
                                doc_url: Some("https://imerchantsolutions.com/docs/partners#authentication".to_string()),
                                additional_context: Some("Received platform API key (prefix: `pk_`) but `merchant_id` was None.".to_string()),
                            },
                        }
                        .into());
                    } else {
                        return Ok(Self {
                            api_key: api_key.to_owned(),
                            merchant_id: merchant_id.clone(),
                        });
                    }
                }
                Ok(Self {
                    api_key: api_key.to_owned(),
                    merchant_id: None,
                })
            }
            _ => Err(errors::IntegrationError::FailedToObtainAuthType {
                context: errors::IntegrationErrorContext {
                    suggested_action: Some("Provide AuthType as HeaderKey".to_string()),
                    doc_url: Some("https://imerchantsolutions.com/docs#authentication".to_string()),
                    additional_context: Some(
                        "Provided AuthType is incorrect. AuthType should be HeaderKey.".to_string(),
                    ),
                },
            }
            .into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImerchantsolutionsPaymentsRequestData<T: PaymentMethodDataTypes> {
    amount: MinorUnit,
    currency: Currency,
    reference: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    card: Option<CardDetails<T>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shopper_email: Option<pii::Email>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shopper_name: Option<ShopperName>,
    #[serde(skip_serializing_if = "Option::is_none")]
    telephone_number: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    billing: Option<AddressDetails>,
    #[serde(skip_serializing_if = "Option::is_none")]
    delivery_address: Option<AddressDetails>,
    manual_capture: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    capture_delay_hours: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shopper_reference: Option<String>,
    store_payment_method: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    shopper_interaction: Option<ShopperInteraction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stored_payment_method_id: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    recurring_processing_model: Option<RecurringProcessingModel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scheme_transaction_id: Option<Secret<String>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ImerchantsolutionsMetadata {
    capture_delay_hours: Option<u32>,
}

fn get_imerchantsolutions_metadata(
    metadata: Option<serde_json::Value>,
) -> error_stack::Result<ImerchantsolutionsMetadata, errors::IntegrationError> {
    metadata
        .map(|meta| {
            serde_json::from_value::<ImerchantsolutionsMetadata>(meta).change_context(
                errors::IntegrationError::InvalidDataFormat {
                    field_name: "connector_metadata",
                    context: errors::IntegrationErrorContext {
                        suggested_action: None,
                        doc_url: None,
                        additional_context: Some(
                            "Failed to deserialize Imerchantsolutions metadata".to_string(),
                        ),
                    },
                },
            )
        })
        .transpose()
        .map(|opt| opt.unwrap_or_default())
}

#[derive(Debug, Clone, Serialize, PartialEq)]
struct CardDetails<T: PaymentMethodDataTypes> {
    number: RawCardNumber<T>,
    cvv: Secret<String>,
    expiry_month: Secret<String>,
    expiry_year: Secret<String>,
    holder: Option<Secret<String>>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct ShopperName {
    first_name: Option<Secret<String>>,
    last_name: Option<Secret<String>>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct AddressDetails {
    address: Option<Secret<String>>,
    city: Option<Secret<String>>,
    state: Option<Secret<String>>,
    postal_code: Option<Secret<String>>,
    country: Option<CountryAlpha2>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
enum ShopperInteraction {
    Moto,
    Ecommerce,
    #[serde(rename = "ContAuth")]
    ContinuedAuthentication,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
enum RecurringProcessingModel {
    UnscheduledCardOnFile,
    CardOnFile,
}

fn get_imerchantsolutions_capture_delay_hours(
    imerchantsolutions_metadata: ImerchantsolutionsMetadata,
    is_auto_capture: bool,
) -> Result<Option<u32>, errors::IntegrationError> {
    let metadata_capture_delay = imerchantsolutions_metadata.capture_delay_hours;

    if is_auto_capture {
        match metadata_capture_delay {
            Some(0) | None => Ok(metadata_capture_delay),
            Some(_) => Err(errors::IntegrationError::InvalidDataFormat {
                field_name: "metadata.capture_delay_hours",
                context: errors::IntegrationErrorContext {
                    suggested_action: Some(
                        "Remove `capture_delay_hours` or set it to 0 when using auto-capture."
                            .to_string(),
                    ),
                    doc_url: Some(
                        "https://imerchantsolutions.com/docs/api#post--payments".to_string(),
                    ),
                    additional_context: Some(
                        "Positive `capture_delay_hours` does not enable delayed auto-capture. \
                            For immediate capture, omit this field or set it to 0."
                            .to_string(),
                    ),
                },
            }),
        }
    } else {
        match metadata_capture_delay {
            Some(0) => {
                Err(errors::IntegrationError::InvalidDataFormat {
                    field_name: "metadata.capture_delay_hours",
                    context: errors::IntegrationErrorContext {
                        suggested_action: Some("Use a positive integer for `capture_delay_hours` or omit it for manual capture.".to_string()),
                        doc_url: Some("https://imerchantsolutions.com/docs/api#post--payments".to_string()),
                        additional_context: Some(
                            "`capture_delay_hours = 0` does not enable manual capture. \
                            To enable manual capture, provide a positive value or use `manualCapture: true`.".to_string()
                        ),
                    },
                })
            }
            Some(_) | None => Ok(metadata_capture_delay),
        }
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ImerchantsolutionsRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for ImerchantsolutionsPaymentsRequestData<T>
{
    type Error = error_stack::Report<errors::IntegrationError>;
    fn try_from(
        item: ImerchantsolutionsRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        match &item.router_data.request.payment_method_data {
            PaymentMethodData::Card(ref card_data) => {
                let card = Some(CardDetails {
                    number: card_data.card_number.clone(),
                    cvv: card_data.card_cvc.clone(),
                    expiry_month: card_data.get_card_expiry_month_2_digit()?,
                    expiry_year: card_data.get_expiry_year_4_digit(),
                    holder: card_data.get_optional_cardholder_name(),
                });
                let shopper_email = item
                    .router_data
                    .resource_common_data
                    .get_optional_billing_email()
                    .or_else(|| item.router_data.request.get_optional_email());
                let shopper_name = Some(ShopperName {
                    first_name: item
                        .router_data
                        .resource_common_data
                        .get_optional_billing_first_name(),
                    last_name: item
                        .router_data
                        .resource_common_data
                        .get_optional_billing_last_name(),
                });
                let billing = Some(AddressDetails {
                    address: item
                        .router_data
                        .resource_common_data
                        .get_optional_billing_line1(),
                    city: item
                        .router_data
                        .resource_common_data
                        .get_optional_billing_city(),
                    state: item
                        .router_data
                        .resource_common_data
                        .get_optional_billing_state(),
                    postal_code: item
                        .router_data
                        .resource_common_data
                        .get_optional_billing_zip(),
                    country: item
                        .router_data
                        .resource_common_data
                        .get_optional_billing_country(),
                });
                let delivery_address = Some(AddressDetails {
                    address: item
                        .router_data
                        .resource_common_data
                        .get_optional_shipping_line1(),
                    city: item
                        .router_data
                        .resource_common_data
                        .get_optional_shipping_city(),
                    state: item
                        .router_data
                        .resource_common_data
                        .get_optional_shipping_state(),
                    postal_code: item
                        .router_data
                        .resource_common_data
                        .get_optional_shipping_zip(),
                    country: item
                        .router_data
                        .resource_common_data
                        .get_optional_shipping_country(),
                });
                let imerchantsolutions_metadata = get_imerchantsolutions_metadata(
                    item.router_data.request.metadata.clone().expose_option(),
                )?;
                let capture_delay_hours = get_imerchantsolutions_capture_delay_hours(
                    imerchantsolutions_metadata,
                    item.router_data.request.is_auto_capture(),
                )?;
                let shopper_reference = match item.router_data.request.get_customer_id() {
                    Ok(customer_id) => Some(customer_id.get_string_repr().to_string()),
                    Err(err) => {
                        if item
                            .router_data
                            .request
                            .is_customer_initiated_mandate_payment()
                        {
                            return Err(err);
                        } else {
                            None
                        }
                    }
                };
                let store_payment_method = item
                    .router_data
                    .request
                    .is_customer_initiated_mandate_payment();
                let recurring_processing_model = if item.router_data.request.setup_future_usage
                    == Some(common_enums::FutureUsage::OffSession)
                {
                    Some(RecurringProcessingModel::CardOnFile)
                } else {
                    None
                };
                Ok(Self {
                    amount: item.router_data.request.amount,
                    currency: item.router_data.request.currency,
                    reference: item
                        .router_data
                        .resource_common_data
                        .connector_request_reference_id
                        .clone(),
                    card,
                    shopper_email,
                    shopper_name,
                    telephone_number: item
                        .router_data
                        .resource_common_data
                        .get_optional_billing_phone_number(),
                    billing,
                    delivery_address,
                    manual_capture: is_manual_capture(item.router_data.request.capture_method),
                    capture_delay_hours,
                    shopper_reference,
                    store_payment_method,
                    shopper_interaction: None,
                    stored_payment_method_id: None,
                    recurring_processing_model,
                    scheme_transaction_id: None,
                })
            }
            PaymentMethodData::CardRedirect(_)
            | PaymentMethodData::Wallet(_)
            | PaymentMethodData::BankRedirect(_)
            | PaymentMethodData::BankTransfer(_)
            | PaymentMethodData::BankDebit(_)
            | PaymentMethodData::GiftCard(_)
            | PaymentMethodData::Voucher(_)
            | PaymentMethodData::PayLater(_)
            | PaymentMethodData::Crypto(_)
            | PaymentMethodData::MandatePayment
            | PaymentMethodData::Reward
            | PaymentMethodData::RealTimePayment(_)
            | PaymentMethodData::Upi(_)
            | PaymentMethodData::OpenBanking(_)
            | PaymentMethodData::CardDetailsForNetworkTransactionId(_)
            | PaymentMethodData::NetworkToken(_)
            | PaymentMethodData::MobilePayment(_)
            | PaymentMethodData::PaymentMethodToken(_)
            | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_) => {
                Err(errors::IntegrationError::NotImplemented(
                    utils::get_unimplemented_payment_method_error_message("Imerchantsolutions"),
                    Default::default(),
                ))?
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImerchantsolutionsPaymentsResponseData {
    payment_id: String,
    psp_reference: String,
    merchant_reference: Option<String>,
    amount: AmountDetails,
    result_code: ResultCode,
    status: ImerchantsolutionsPaymentStatus,
    additional_data: Option<AdditionalData>,
    capture_mode: Option<CaptureMode>,
    capture_delay_hours: Option<i32>,
    message: Option<String>,
    shopper_interaction: Option<ShopperInteraction>,
    stored_payment_method_id: Option<Secret<String>>,
    scheme_transaction_id: Option<Secret<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct AmountDetails {
    value: MinorUnit,
    currency: Currency,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
enum ResultCode {
    Authorised,
    Refused,
    Pending,
    Error,
    Cancelled,
    RedirectShopper,
    ChallengeShopper,
    IdentifyShopper,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum ImerchantsolutionsPaymentStatus {
    #[serde(alias = "AUTHORISED")]
    Authorised,
    Authorized,
    #[serde(rename = "pending_3ds")]
    Pending3ds,
    Cancelled,
    #[serde(alias = "PENDING_CAPTURE")]
    PendingCapture,
    PartiallyCaptured,
    Captured,
    Pending,
    Refused,
    Failed,
    PartiallyRefunded,
    Refunded,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct AdditionalData {
    card_summary: Option<String>,
    card_bin: Option<String>,
    #[serde(rename = "recurring.recurringDetailReference")]
    recurring_detail_reference: Option<Secret<String>>,
    #[serde(rename = "recurring.storedPaymentMethodId")]
    stored_payment_method_id: Option<Secret<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CaptureMode {
    Auto,
    Manual,
}

impl<F, T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<ImerchantsolutionsPaymentsResponseData, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;
    fn try_from(
        item: ResponseRouterData<ImerchantsolutionsPaymentsResponseData, Self>,
    ) -> Result<Self, Self::Error> {
        let status = item.response.status.clone().into();

        if is_payment_failure(status) {
            let error_response = ErrorResponse {
                code: consts::NO_ERROR_CODE.to_string(),
                message: consts::NO_ERROR_MESSAGE.to_string(),
                reason: None,
                status_code: item.http_code,
                attempt_status: Some(status),
                connector_transaction_id: Some(item.response.psp_reference),
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
            };

            Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data
                },
                response: Err(error_response),
                ..item.router_data
            })
        } else {
            let additional_data = item.response.additional_data.as_ref();

            let connector_mandate_id = additional_data
                .and_then(|data| data.stored_payment_method_id.clone())
                .or(item.response.stored_payment_method_id);

            let connector_mandate_request_reference_id = additional_data.and_then(|data| {
                data.recurring_detail_reference
                    .as_ref()
                    .map(|id| id.clone().expose())
            });

            let mandate_reference = connector_mandate_id
                .map(|mandate_id| MandateReference {
                    connector_mandate_id: Some(mandate_id.expose()),
                    payment_method_id: None,
                    connector_mandate_request_reference_id,
                })
                .unwrap_or(MandateReference {
                    connector_mandate_id: None,
                    payment_method_id: None,
                    connector_mandate_request_reference_id: None,
                });
            Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    minor_amount_capturable: Some(item.response.amount.value),
                    ..item.router_data.resource_common_data
                },
                response: Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(
                        item.response.psp_reference.clone(),
                    ),
                    redirection_data: None,
                    mandate_reference: Some(Box::new(mandate_reference)),
                    connector_metadata: None,
                    network_txn_id: None,
                    connector_response_reference_id: Some(item.response.payment_id.clone()),
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                }),
                ..item.router_data
            })
        }
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ImerchantsolutionsRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for ImerchantsolutionsPaymentsRequestData<T>
{
    type Error = error_stack::Report<errors::IntegrationError>;
    fn try_from(
        item: ImerchantsolutionsRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let amount = item.router_data.request.minor_amount;
        let currency = item.router_data.request.currency;
        let reference = item
            .router_data
            .resource_common_data
            .connector_request_reference_id();
        let shopper_email = item
            .router_data
            .resource_common_data
            .get_optional_billing_email()
            .or_else(|| item.router_data.request.get_optional_email());
        let shopper_name = Some(ShopperName {
            first_name: item
                .router_data
                .resource_common_data
                .get_optional_billing_first_name(),
            last_name: item
                .router_data
                .resource_common_data
                .get_optional_billing_last_name(),
        });
        let telephone_number = item
            .router_data
            .resource_common_data
            .get_optional_billing_phone_number();
        let billing = Some(AddressDetails {
            address: item
                .router_data
                .resource_common_data
                .get_optional_billing_line1(),
            city: item
                .router_data
                .resource_common_data
                .get_optional_billing_city(),
            state: item
                .router_data
                .resource_common_data
                .get_optional_billing_state(),
            postal_code: item
                .router_data
                .resource_common_data
                .get_optional_billing_zip(),
            country: item
                .router_data
                .resource_common_data
                .get_optional_billing_country(),
        });
        let delivery_address = Some(AddressDetails {
            address: item
                .router_data
                .resource_common_data
                .get_optional_shipping_line1(),
            city: item
                .router_data
                .resource_common_data
                .get_optional_shipping_city(),
            state: item
                .router_data
                .resource_common_data
                .get_optional_shipping_state(),
            postal_code: item
                .router_data
                .resource_common_data
                .get_optional_shipping_zip(),
            country: item
                .router_data
                .resource_common_data
                .get_optional_shipping_country(),
        });
        let imerchantsolutions_metadata = get_imerchantsolutions_metadata(
            item.router_data.request.metadata.clone().expose_option(),
        )?;
        let capture_delay_hours = get_imerchantsolutions_capture_delay_hours(
            imerchantsolutions_metadata,
            item.router_data.request.is_auto_capture(),
        )?;
        let shopper_reference = match item.router_data.resource_common_data.get_customer_id() {
            Ok(customer_id) => Some(customer_id.get_string_repr().to_string()),
            Err(err) => return Err(err),
        };
        let stored_payment_method_id =
            item.router_data
                .request
                .connector_mandate_id()
                .ok_or_else(|| {
                    errors::IntegrationError::MissingRequiredField {
                    field_name: "connector_mandate_id",
                    context: errors::IntegrationErrorContext {
                        suggested_action: Some(
                            "Pass a valid `connector_mandate_id` for recurring or mandate payments."
                                .to_string(),
                        ),
                        doc_url: Some(
                            "https://imerchantsolutions.com/docs/api#post--payments"
                                .to_string(),
                        ),
                        additional_context: Some(
                            "Expected connector mandate ID not found".to_string(),
                        ),
                    },
                }
                })?;
        Ok(Self {
            amount,
            currency,
            reference,
            card: None,
            shopper_email,
            shopper_name,
            telephone_number,
            billing,
            delivery_address,
            manual_capture: is_manual_capture(item.router_data.request.capture_method),
            capture_delay_hours,
            shopper_reference,
            store_payment_method: false,
            shopper_interaction: Some(ShopperInteraction::ContinuedAuthentication),
            stored_payment_method_id: Some(Secret::new(stored_payment_method_id)),
            recurring_processing_model: Some(RecurringProcessingModel::UnscheduledCardOnFile),
            scheme_transaction_id: None,
        })
    }
}

impl<F, T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<ImerchantsolutionsPaymentsResponseData, Self>>
    for RouterDataV2<F, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;
    fn try_from(
        item: ResponseRouterData<ImerchantsolutionsPaymentsResponseData, Self>,
    ) -> Result<Self, Self::Error> {
        let status = item.response.status.clone().into();

        if is_payment_failure(status) {
            let error_response = ErrorResponse {
                code: consts::NO_ERROR_CODE.to_string(),
                message: consts::NO_ERROR_MESSAGE.to_string(),
                reason: None,
                status_code: item.http_code,
                attempt_status: Some(status),
                connector_transaction_id: Some(item.response.psp_reference),
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
            };

            Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..item.router_data.resource_common_data
                },
                response: Err(error_response),
                ..item.router_data
            })
        } else {
            Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    minor_amount_capturable: Some(item.response.amount.value),
                    ..item.router_data.resource_common_data
                },
                response: Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(
                        item.response.psp_reference.clone(),
                    ),
                    redirection_data: None,
                    mandate_reference: None,
                    connector_metadata: None,
                    network_txn_id: None,
                    connector_response_reference_id: Some(item.response.payment_id.clone()),
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                }),
                ..item.router_data
            })
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ImerchantsolutionsPaymentSyncResponse {
    ImerchantsolutionsPSyncResponse(ImerchantsolutionsPSyncResponseData),
    ImerchantsolutionsWebhookResponse(Box<ImerchantsolutionsWebhookData>),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImerchantsolutionsPSyncResponseData {
    payment_id: String,
    psp_reference: String,
    merchant_reference: Option<String>,
    authorized_amount: Option<MinorUnit>,
    total_captured: Option<MinorUnit>,
    remaining_amount: Option<MinorUnit>,
    capture_closed: Option<bool>,
    captures: Vec<Captures>,
    currency: Currency,
    status: ImerchantsolutionsPaymentStatus,
    capture_mode: CaptureMode,
    captured_at: Option<String>,
    can_capture: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct Captures {
    amount: MinorUnit,
    currency: Currency,
    psp_reference: String,
    captured_at: Option<String>,
    #[serde(rename = "final")]
    final_capture: Option<bool>,
}

#[derive(Clone, Debug)]
struct CaptureWithStatus<'a> {
    capture: &'a Captures,
    status: &'a ImerchantsolutionsPaymentStatus,
    psp_reference: &'a String,
}

enum CaptureStatus {
    Pending,
    CaptureFailed,
    Charged,
}

impl<'a> utils::MultipleCaptureSyncResponse for CaptureWithStatus<'a> {
    fn get_connector_capture_id(&self) -> String {
        self.capture.psp_reference.clone()
    }

    // Connector does not provide per-capture status.
    // We derive capture status from overall payment status.
    // This assumes uniform outcome across all captures.
    fn get_capture_attempt_status(&self) -> AttemptStatus {
        let capture_status: CaptureStatus = self.status.clone().into();
        capture_status.into()
    }

    fn is_capture_response(&self) -> bool {
        true
    }

    fn get_connector_reference_id(&self) -> Option<String> {
        Some(self.psp_reference.clone())
    }

    fn get_amount_captured(&self) -> Result<Option<MinorUnit>, error_stack::Report<ParsingError>> {
        Ok(Some(self.capture.amount))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImerchantsolutionsWebhookData {
    #[serde(rename = "type")]
    pub event_type: ImerchantsolutionsWebhookEventType,
    pub payment_id: String,
    pub psp_reference: String,
    pub original_reference: Option<String>,
    pub reference: Option<String>,
    pub merchant_reference: Option<String>,
    pub status: ImerchantsolutionsWebhookStatus,
    pub reason: Option<String>,
    pub error: Option<String>,
    pub amount: Option<MinorUnit>,
    pub captured_amount: Option<MinorUnit>,
    pub total_captured: Option<MinorUnit>,
    refunded_amount: Option<MinorUnit>,
    total_refunded: Option<MinorUnit>,
    currency: Currency,
    processor: Option<String>,
    card_last4: Option<String>,
    card_brand: Option<String>,
    customer_email: Option<pii::Email>,
    partner_id: Option<Secret<String>>,
    merchant_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ImerchantsolutionsWebhookEventType {
    #[serde(rename = "payment.completed")]
    PaymentCompleted,
    #[serde(rename = "payment.cancelled")]
    PaymentCancelled,
    #[serde(rename = "payment.failed")]
    PaymentFailed,
    #[serde(rename = "payment.refunded")]
    PaymentRefunded,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ImerchantsolutionsWebhookStatus {
    Authorized,
    PartiallyCaptured,
    Captured,
    PartiallyRefunded,
    Refunded,
    Cancelled,
    Failed,
    Refused,
}

impl<F> TryFrom<ResponseRouterData<ImerchantsolutionsPaymentSyncResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;
    fn try_from(
        item: ResponseRouterData<ImerchantsolutionsPaymentSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;

        let is_multiple_capture_psync_flow = match router_data.request.sync_type {
            SyncRequestType::MultipleCaptureSync => true,
            SyncRequestType::SinglePaymentSync => false,
        };

        match response {
            ImerchantsolutionsPaymentSyncResponse::ImerchantsolutionsPSyncResponse(response) => {
                let status = response.status.clone().into();

                let amount_captured = response
                    .total_captured
                    .map(|minor_amount| minor_amount.get_amount_as_i64());

                if is_payment_failure(status) {
                    let error_response = ErrorResponse {
                        code: consts::NO_ERROR_CODE.to_string(),
                        message: consts::NO_ERROR_MESSAGE.to_string(),
                        reason: None,
                        status_code: http_code,
                        attempt_status: Some(status),
                        connector_transaction_id: Some(response.psp_reference),
                        network_advice_code: None,
                        network_decline_code: None,
                        network_error_message: None,
                    };

                    Ok(Self {
                        resource_common_data: PaymentFlowData {
                            status,
                            ..router_data.resource_common_data
                        },
                        response: Err(error_response),
                        ..router_data
                    })
                } else if is_multiple_capture_psync_flow {
                    let wrapped_captures: Vec<CaptureWithStatus<'_>> = response
                        .captures
                        .iter()
                        .map(|c| CaptureWithStatus {
                            capture: c,
                            status: &response.status,
                            psp_reference: &response.psp_reference,
                        })
                        .collect();

                    let capture_sync_response_list =
                        utils::construct_captures_response_hashmap(wrapped_captures)
                            .change_context(utils::response_handling_fail_for_connector(
                                http_code,
                                IMERCHANTSOLUTIONS,
                            ))?;

                    Ok(Self {
                        resource_common_data: PaymentFlowData {
                            status: response.status.clone().into(),
                            amount_captured,
                            minor_amount_captured: response.total_captured,
                            minor_amount_capturable: response.remaining_amount,
                            ..router_data.resource_common_data
                        },
                        response: Ok(PaymentsResponseData::MultipleCaptureResponse {
                            capture_sync_response_list,
                            status_code: http_code,
                        }),
                        ..router_data
                    })
                } else {
                    Ok(Self {
                        resource_common_data: PaymentFlowData {
                            status,
                            amount_captured,
                            minor_amount_captured: response.total_captured,
                            minor_amount_capturable: response.remaining_amount,
                            ..router_data.resource_common_data
                        },
                        response: Ok(PaymentsResponseData::TransactionResponse {
                            resource_id: ResponseId::ConnectorTransactionId(
                                response.psp_reference.clone(),
                            ),
                            redirection_data: None,
                            mandate_reference: None,
                            connector_metadata: None,
                            network_txn_id: None,
                            connector_response_reference_id: Some(response.payment_id.clone()),
                            incremental_authorization_allowed: None,
                            status_code: http_code,
                        }),
                        ..router_data
                    })
                }
            }
            ImerchantsolutionsPaymentSyncResponse::ImerchantsolutionsWebhookResponse(response) => {
                let status = response.status.clone().into();

                if is_payment_failure(status) {
                    let error_response = ErrorResponse {
                        code: consts::NO_ERROR_CODE.to_string(),
                        message: response
                            .error
                            .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string()),
                        reason: response.reason,
                        status_code: http_code,
                        attempt_status: Some(status),
                        connector_transaction_id: Some(response.psp_reference),
                        network_advice_code: None,
                        network_decline_code: None,
                        network_error_message: None,
                    };

                    Ok(Self {
                        resource_common_data: PaymentFlowData {
                            status,
                            ..router_data.resource_common_data
                        },
                        response: Err(error_response),
                        ..router_data
                    })
                } else {
                    let (minor_amount_captured, minor_amount_capturable) = match status {
                        AttemptStatus::Authorized => (None, response.amount),
                        AttemptStatus::Charged => (response.amount, None),
                        AttemptStatus::PartialCharged => {
                            let captured = response.total_captured;

                            let capturable = response
                                .amount
                                .zip(captured)
                                .map(|(total, captured)| total - captured);

                            (captured, capturable)
                        }
                        _ => (None, None),
                    };

                    Ok(Self {
                        resource_common_data: PaymentFlowData {
                            status,
                            amount_captured: minor_amount_captured
                                .map(|minor_amount| minor_amount.get_amount_as_i64()),
                            minor_amount_captured,
                            minor_amount_capturable,
                            ..router_data.resource_common_data
                        },
                        response: Ok(PaymentsResponseData::TransactionResponse {
                            resource_id: ResponseId::ConnectorTransactionId(
                                response.psp_reference.clone(),
                            ),
                            redirection_data: None,
                            mandate_reference: None,
                            connector_metadata: None,
                            network_txn_id: None,
                            connector_response_reference_id: Some(response.payment_id.clone()),
                            incremental_authorization_allowed: None,
                            status_code: http_code,
                        }),
                        ..router_data
                    })
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImerchantsolutionsVoidRequestData {
    psp_reference: String,
    reason: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ImerchantsolutionsRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for ImerchantsolutionsVoidRequestData
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: ImerchantsolutionsRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            psp_reference: item.router_data.request.connector_transaction_id,
            reason: item.router_data.request.cancellation_reason.clone(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImerchantsolutionsVoidResponseData {
    success: bool,
    psp_reference: String,
    original_reference: String,
    status: ImerchantsolutionsVoidStatus,
    message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum ImerchantsolutionsVoidStatus {
    Received,
    Cancelled,
}

impl TryFrom<ResponseRouterData<ImerchantsolutionsVoidResponseData, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<ImerchantsolutionsVoidResponseData, Self>,
    ) -> Result<Self, Self::Error> {
        let status = match item.response.status {
            ImerchantsolutionsVoidStatus::Received => AttemptStatus::VoidInitiated,
            ImerchantsolutionsVoidStatus::Cancelled => AttemptStatus::Voided,
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    item.response.original_reference.clone(),
                ),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(item.response.psp_reference.clone()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImerchantsolutionsCaptureRequestData {
    psp_reference: String,
    amount: MinorUnit,
    currency: Currency,
    #[serde(rename = "final")]
    final_capture: bool,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ImerchantsolutionsRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for ImerchantsolutionsCaptureRequestData
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: ImerchantsolutionsRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let psp_reference = item
            .router_data
            .request
            .connector_transaction_id
            .get_connector_transaction_id()
            .change_context(errors::IntegrationError::MissingConnectorTransactionID {
                context: errors::IntegrationErrorContext {
                    suggested_action: None,
                    doc_url: Some(
                        "https://imerchantsolutions.com/docs/api#post--payments-capture"
                            .to_string(),
                    ),
                    additional_context: Some(
                        "Expected connector transaction ID not found".to_string(),
                    ),
                },
            })?;

        let final_capture = matches!(
            item.router_data.request.capture_method.unwrap_or_default(),
            CaptureMethod::Manual
        );

        Ok(Self {
            psp_reference,
            amount: item.router_data.request.minor_amount_to_capture,
            currency: item.router_data.request.currency,
            final_capture,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImerchantsolutionsCaptureResponseData {
    success: bool,
    psp_reference: String,
    original_reference: String,
    captured_amount: Option<MinorUnit>,
    total_captured: Option<MinorUnit>,
    currency: Currency,
    status: ImerchantsolutionsCaptureStatus,
    capture_closed: Option<bool>,
    final_capture: Option<bool>,
    remainder_released: Option<RemainderReleased>,
    message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum ImerchantsolutionsCaptureStatus {
    Received,
    PartiallyCaptured,
    Captured,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum RemainderReleased {
    Attempted,
    Mock,
    Skipped,
}

impl TryFrom<ResponseRouterData<ImerchantsolutionsCaptureResponseData, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<ImerchantsolutionsCaptureResponseData, Self>,
    ) -> Result<Self, Self::Error> {
        let status = item.response.status.into();

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(
                    item.response.original_reference.clone(),
                ),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                connector_response_reference_id: Some(item.response.psp_reference.clone()),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImerchantsolutionsRefundRequestData {
    psp_reference: String,
    amount: MinorUnit,
    currency: Currency,
    reference: Option<String>,
    reason: Option<String>,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        ImerchantsolutionsRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for ImerchantsolutionsRefundRequestData
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: ImerchantsolutionsRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            psp_reference: item.router_data.request.connector_transaction_id.clone(),
            amount: item.router_data.request.minor_refund_amount,
            currency: item.router_data.request.currency,
            reference: Some(item.router_data.request.refund_id.clone()),
            reason: item.router_data.request.reason,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImerchantsolutionsRefundResponseData {
    success: bool,
    psp_reference: String,
    original_reference: String,
    refunded_amount: MinorUnit,
    total_refunded: MinorUnit,
    currency: Currency,
    status: ImerchantsolutionsRefundStatus,
    message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum ImerchantsolutionsRefundStatus {
    Received,
    PartiallyRefunded,
    Refunded,
}

impl TryFrom<ResponseRouterData<ImerchantsolutionsRefundResponseData, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<ImerchantsolutionsRefundResponseData, Self>,
    ) -> Result<Self, Self::Error> {
        let refund_status = item.response.status.into();

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.psp_reference.to_string(),
                refund_status,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ImerchantsolutionsRefundSyncResponse {
    ImerchantsolutionsRsyncResponse(ImerchantsolutionsRsyncResponseData),
    ImerchantsolutionsWebhookResponse(Box<ImerchantsolutionsWebhookData>),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImerchantsolutionsRsyncResponseData {
    payment_id: String,
    psp_reference: String,
    merchant_reference: Option<String>,
    payment_amount: Option<MinorUnit>,
    total_captured: Option<MinorUnit>,
    total_refunded: Option<MinorUnit>,
    remaining_amount: Option<MinorUnit>,
    currency: Currency,
    status: ImerchantsolutionsRefundStatus,
    can_refund: bool,
    refunds: Vec<Refunds>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct Refunds {
    psp_reference: String,
    amount: MinorUnit,
    currency: Currency,
    reason: Option<String>,
    created_at: Option<String>,
}

impl TryFrom<ResponseRouterData<ImerchantsolutionsRefundSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;
    fn try_from(
        item: ResponseRouterData<ImerchantsolutionsRefundSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let ResponseRouterData {
            response,
            router_data,
            http_code,
        } = item;

        let connector_refund_id = router_data.request.connector_refund_id.clone();

        match response {
            ImerchantsolutionsRefundSyncResponse::ImerchantsolutionsRsyncResponse(response) => {
                let refund_status = response.status.clone().into();

                Ok(Self {
                    response: Ok(RefundsResponseData {
                        connector_refund_id,
                        refund_status,
                        status_code: http_code,
                    }),
                    ..router_data
                })
            }
            ImerchantsolutionsRefundSyncResponse::ImerchantsolutionsWebhookResponse(response) => {
                let refund_status =
                    RefundStatus::try_from(response.status.clone()).map_err(|err| {
                        err.change_context(utils::response_handling_fail_for_connector(
                            http_code,
                            IMERCHANTSOLUTIONS,
                        ))
                        .attach_printable(format!("Invalid refund status: {:?}", response.status))
                    })?;

                if utils::is_refund_failure(refund_status) {
                    let error_response = Err(ErrorResponse {
                        status_code: http_code,
                        code: consts::NO_ERROR_CODE.to_string(),
                        message: response
                            .reason
                            .clone()
                            .unwrap_or_else(|| consts::NO_ERROR_MESSAGE.to_string()),
                        reason: response.reason,
                        attempt_status: None,
                        connector_transaction_id: response.original_reference,
                        network_advice_code: None,
                        network_decline_code: None,
                        network_error_message: None,
                    });

                    Ok(Self {
                        response: error_response,
                        ..router_data
                    })
                } else {
                    Ok(Self {
                        response: Ok(RefundsResponseData {
                            connector_refund_id,
                            refund_status,
                            status_code: http_code,
                        }),
                        ..router_data
                    })
                }
            }
        }
    }
}

pub trait ForeignTryFrom<F>: Sized {
    type Error;

    fn foreign_try_from(from: F) -> Result<Self, Self::Error>;
}

impl From<ImerchantsolutionsPaymentStatus> for AttemptStatus {
    fn from(item: ImerchantsolutionsPaymentStatus) -> Self {
        match item {
            ImerchantsolutionsPaymentStatus::Authorised
            | ImerchantsolutionsPaymentStatus::Authorized
            | ImerchantsolutionsPaymentStatus::PendingCapture => Self::Authorized,

            ImerchantsolutionsPaymentStatus::Pending3ds => Self::AuthenticationPending,

            ImerchantsolutionsPaymentStatus::Cancelled => Self::Voided,

            ImerchantsolutionsPaymentStatus::PartiallyCaptured => Self::PartialCharged,

            ImerchantsolutionsPaymentStatus::Captured
            | ImerchantsolutionsPaymentStatus::PartiallyRefunded
            | ImerchantsolutionsPaymentStatus::Refunded => Self::Charged,

            ImerchantsolutionsPaymentStatus::Pending => Self::Pending,

            ImerchantsolutionsPaymentStatus::Refused | ImerchantsolutionsPaymentStatus::Failed => {
                Self::Failure
            }
        }
    }
}

impl From<ImerchantsolutionsWebhookStatus> for AttemptStatus {
    fn from(item: ImerchantsolutionsWebhookStatus) -> Self {
        match item {
            ImerchantsolutionsWebhookStatus::Authorized => Self::Authorized,

            ImerchantsolutionsWebhookStatus::PartiallyCaptured => Self::PartialCharged,

            ImerchantsolutionsWebhookStatus::Cancelled => Self::Voided,

            ImerchantsolutionsWebhookStatus::Captured
            | ImerchantsolutionsWebhookStatus::PartiallyRefunded
            | ImerchantsolutionsWebhookStatus::Refunded => Self::Charged,

            ImerchantsolutionsWebhookStatus::Failed | ImerchantsolutionsWebhookStatus::Refused => {
                Self::Failure
            }
        }
    }
}

impl From<ImerchantsolutionsCaptureStatus> for AttemptStatus {
    fn from(capture_status: ImerchantsolutionsCaptureStatus) -> Self {
        match capture_status {
            ImerchantsolutionsCaptureStatus::Received => Self::CaptureInitiated,

            ImerchantsolutionsCaptureStatus::PartiallyCaptured => Self::PartialCharged,

            ImerchantsolutionsCaptureStatus::Captured => Self::Charged,
        }
    }
}

impl From<ImerchantsolutionsPaymentStatus> for CaptureStatus {
    fn from(status: ImerchantsolutionsPaymentStatus) -> Self {
        match status {
            ImerchantsolutionsPaymentStatus::Authorised
            | ImerchantsolutionsPaymentStatus::Authorized
            | ImerchantsolutionsPaymentStatus::PendingCapture
            | ImerchantsolutionsPaymentStatus::Pending3ds
            | ImerchantsolutionsPaymentStatus::Pending => Self::Pending,

            ImerchantsolutionsPaymentStatus::Cancelled
            | ImerchantsolutionsPaymentStatus::Refused
            | ImerchantsolutionsPaymentStatus::Failed => Self::CaptureFailed,

            ImerchantsolutionsPaymentStatus::PartiallyCaptured
            | ImerchantsolutionsPaymentStatus::Captured
            | ImerchantsolutionsPaymentStatus::PartiallyRefunded
            | ImerchantsolutionsPaymentStatus::Refunded => Self::Charged,
        }
    }
}

impl From<CaptureStatus> for AttemptStatus {
    fn from(status: CaptureStatus) -> Self {
        match status {
            CaptureStatus::Pending => Self::Pending,

            CaptureStatus::CaptureFailed => Self::CaptureFailed,

            CaptureStatus::Charged => Self::Charged,
        }
    }
}

impl From<ImerchantsolutionsRefundStatus> for RefundStatus {
    fn from(status: ImerchantsolutionsRefundStatus) -> Self {
        match status {
            ImerchantsolutionsRefundStatus::Received => Self::Pending,
            ImerchantsolutionsRefundStatus::PartiallyRefunded
            | ImerchantsolutionsRefundStatus::Refunded => Self::Success,
        }
    }
}

impl TryFrom<ImerchantsolutionsWebhookStatus> for RefundStatus {
    type Error = error_stack::Report<errors::WebhookError>;

    fn try_from(status: ImerchantsolutionsWebhookStatus) -> Result<Self, Self::Error> {
        match status {
            ImerchantsolutionsWebhookStatus::PartiallyRefunded
            | ImerchantsolutionsWebhookStatus::Refunded => Ok(Self::Success),

            ImerchantsolutionsWebhookStatus::Failed | ImerchantsolutionsWebhookStatus::Refused => {
                Ok(Self::Failure)
            }

            ImerchantsolutionsWebhookStatus::Authorized
            | ImerchantsolutionsWebhookStatus::Cancelled
            | ImerchantsolutionsWebhookStatus::PartiallyCaptured
            | ImerchantsolutionsWebhookStatus::Captured => {
                Err(errors::WebhookError::WebhookBodyDecodingFailed.into())
            }
        }
    }
}

impl
    ForeignTryFrom<(
        ImerchantsolutionsWebhookEventType,
        ImerchantsolutionsWebhookStatus,
    )> for EventType
{
    type Error = error_stack::Report<errors::WebhookError>;

    fn foreign_try_from(
        (event_type, status): (
            ImerchantsolutionsWebhookEventType,
            ImerchantsolutionsWebhookStatus,
        ),
    ) -> Result<Self, Self::Error> {
        match event_type {
            ImerchantsolutionsWebhookEventType::PaymentCompleted => match status {
                ImerchantsolutionsWebhookStatus::Authorized => {
                    Ok(Self::PaymentIntentAuthorizationSuccess)
                }
                ImerchantsolutionsWebhookStatus::PartiallyCaptured => {
                    Ok(Self::PaymentIntentPartiallyFunded)
                }
                ImerchantsolutionsWebhookStatus::Captured => Ok(Self::PaymentIntentCaptureSuccess),
                ImerchantsolutionsWebhookStatus::Cancelled
                | ImerchantsolutionsWebhookStatus::Failed
                | ImerchantsolutionsWebhookStatus::Refused
                | ImerchantsolutionsWebhookStatus::PartiallyRefunded
                | ImerchantsolutionsWebhookStatus::Refunded => {
                    Err(errors::WebhookError::WebhookBodyDecodingFailed.into())
                }
            },
            ImerchantsolutionsWebhookEventType::PaymentCancelled => {
                Ok(Self::PaymentIntentCancelled)
            }
            ImerchantsolutionsWebhookEventType::PaymentFailed => Ok(Self::PaymentIntentFailure),
            ImerchantsolutionsWebhookEventType::PaymentRefunded => Ok(Self::RefundSuccess),
        }
    }
}
