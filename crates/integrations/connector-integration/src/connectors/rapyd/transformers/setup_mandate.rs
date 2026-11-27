use std::fmt::Debug;

use common_utils::{pii::Email, types::MinorUnit};
use domain_types::{
    connector_flow::{PSync, SetupMandate},
    connector_types::{
        MandateReference, PaymentFlowData, PaymentsResponseData, PaymentsSyncData, ResponseId,
        SetupMandateRequestData,
    },
    errors::{ConnectorError, IntegrationError},
    payment_method_data::{PaymentMethodData, PaymentMethodDataTypes},
    router_data::ErrorResponse,
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use url::Url;

use super::super::RapydRouterData;
use super::{
    build_external_3ds_options, resolve_cardholder_name, Address, NextAction, PaymentFields,
    RapydClientDetails, RapydExternal3DsOptions, RapydPSyncResponse, RapydPaymentMethodType,
    RapydPaymentPurpose, RapydPurposeFlow, RapydResponsePaymentMethodData, Status,
};
use crate::{types::ResponseRouterData, utils};

const SETUP_PREFIX: &str = "rapyd_setup_v1_";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SetupMode {
    #[serde(rename = "hosted_3ds")]
    Hosted3ds,
    #[serde(rename = "external_3ds")]
    External3ds,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupMarker {
    pub ucs_setup_attempt: String,
    pub ucs_setup_mode: SetupMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ucs_setup_eci: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ucs_setup_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupState {
    pub reference: String,
    pub customer_id: String,
    pub mode: SetupMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eci: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

fn setup_reference(attempt: &str) -> Result<String, error_stack::Report<IntegrationError>> {
    if attempt.is_empty() || attempt == "IRRELEVANT_ATTEMPT_ID" {
        return Err(IntegrationError::MissingRequiredField {
            field_name: "x-resource-id",
            context: utils::integration_ctx(
                "Rapyd card verification needs a unique caller attempt identifier",
                "Send the payment attempt ID in x-resource-id.",
            ),
        }
        .into());
    }
    Ok(format!(
        "{SETUP_PREFIX}{}",
        hex::encode(ring::digest::digest(&ring::digest::SHA256, attempt.as_bytes()).as_ref())
    ))
}

#[derive(Debug, Serialize)]
pub struct SetupCard<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> {
    #[serde(rename = "type")]
    pm_type: RapydPaymentMethodType,
    fields: PaymentFields<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    address: Option<Address>,
    #[serde(skip_serializing_if = "Option::is_none")]
    client_details: Option<RapydClientDetails>,
    complete_payment_url: String,
    error_payment_url: String,
    metadata: SetupMarker,
}

#[derive(Debug, Serialize)]
pub struct SetupOptions {
    #[serde(rename = "3d_required", skip_serializing_if = "Option::is_none")]
    required: Option<bool>,
    #[serde(flatten)]
    external: Option<RapydExternal3DsOptions>,
    currency: common_enums::Currency,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum RapydSetupMandateRequest<
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
> {
    Create {
        name: Secret<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        email: Option<Email>,
        payment_method: SetupCard<T>,
        payment_method_options: SetupOptions,
    },
    Add {
        #[serde(flatten)]
        payment_method: SetupCard<T>,
        payment_method_options: SetupOptions,
    },
}

fn setup_options<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>(
    request: &SetupMandateRequestData<T>,
    common: &PaymentFlowData,
) -> Result<(SetupMode, SetupOptions), error_stack::Report<IntegrationError>> {
    let (mode, required, external) = match request.authentication_data.as_ref() {
        Some(authentication) => {
            let options = build_external_3ds_options(authentication)?;
            if authentication.trans_status != Some(common_enums::TransactionStatus::Success)
                || options.cavv.peek().is_empty()
                || options.eci.as_deref().is_none_or(str::is_empty)
                || options.ds_trans_id.as_deref().is_none_or(str::is_empty)
            {
                return Err(IntegrationError::NotSupported {
                    message: "Incomplete or unsuccessful external 3DS verification".into(),
                    connector: "rapyd",
                    context: utils::integration_ctx(
                        "Rapyd tokenization requires completed 3DS authentication",
                        "Send successful external 3DS data with CAVV, ECI, version, and directory-server transaction ID.",
                    ),
                }.into());
            }
            (SetupMode::External3ds, None, Some(options))
        }
        None if common.auth_type == common_enums::AuthenticationType::ThreeDs => {
            (SetupMode::Hosted3ds, Some(true), None)
        }
        None => {
            return Err(IntegrationError::NotSupported {
                message: "Card tokenization without 3DS".into(),
                connector: "rapyd",
                context: utils::integration_ctx(
                    "Rapyd requires 3DS during card tokenization",
                    "Use hosted 3DS or provide completed external 3DS authentication.",
                ),
            }
            .into())
        }
    };
    Ok((
        mode,
        SetupOptions {
            required,
            external,
            currency: request.currency,
        },
    ))
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        RapydRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for RapydSetupMandateRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: RapydRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let data = item.router_data;
        let request = &data.request;
        let money =
            request
                .money
                .as_ref()
                .ok_or_else(|| IntegrationError::MissingRequiredField {
                    field_name: "minor_amount",
                    context: utils::integration_ctx(
                        "The setup amount is missing",
                        "Send an explicit zero amount.",
                    ),
                })?;
        if money.amount() != MinorUnit::zero() {
            return Err(IntegrationError::NotSupported {
                message: "Nonzero setup amount".into(),
                connector: "rapyd",
                context: utils::integration_ctx(
                    "SetupMandate only verifies a card",
                    "Use Authorize for a nonzero payment.",
                ),
            }
            .into());
        }
        let reference = setup_reference(&data.resource_common_data.attempt_id)?;
        let (mode, payment_method_options) = setup_options(request, &data.resource_common_data)?;
        let PaymentMethodData::Card(card) = &request.payment_method_data else {
            return Err(IntegrationError::NotImplemented(
                "payment_method for rapyd SetupMandate".into(),
                utils::integration_ctx("This setup method is not supported", "Use a card."),
            )
            .into());
        };
        let name = resolve_cardholder_name(&data.resource_common_data, &card.card_holder_name)
            .ok_or_else(|| IntegrationError::MissingRequiredField {
                field_name: "card.card_holder_name",
                context: utils::integration_ctx(
                    "The cardholder name is missing",
                    "Send the cardholder or billing name.",
                ),
            })?;
        let purpose = RapydPaymentPurpose::try_from((
            request.mit_category.as_ref(),
            RapydPurposeFlow::SetupMandate,
        ))?;
        let return_url = request.get_router_return_url()?;
        let payment_method = SetupCard {
            pm_type: RapydPaymentMethodType::try_from((&data.connector_config, card))?,
            fields: PaymentFields {
                number: card.card_number.clone(),
                expiration_month: card.card_exp_month.clone(),
                expiration_year: card.card_exp_year.clone(),
                name,
                cvv: card.card_cvc.clone(),
                recurrence_type: Some(purpose.recurrence_type),
            },
            address: data
                .resource_common_data
                .get_optional_billing()
                .map(Address::from),
            client_details: request.browser_info.as_ref().map(RapydClientDetails::from),
            complete_payment_url: return_url.clone(),
            error_payment_url: return_url,
            metadata: SetupMarker {
                ucs_setup_attempt: reference,
                ucs_setup_mode: mode,
                ucs_setup_eci: payment_method_options
                    .external
                    .as_ref()
                    .and_then(|options| options.eci.clone()),
                ucs_setup_version: payment_method_options
                    .external
                    .as_ref()
                    .map(|options| options.three_ds_version.clone()),
            },
        };
        if data
            .resource_common_data
            .connector_customer
            .as_deref()
            .is_some_and(|id| !id.is_empty())
        {
            Ok(Self::Add {
                payment_method,
                payment_method_options,
            })
        } else {
            let name = request
                .customer_name
                .as_ref()
                .filter(|name| !name.is_empty())
                .ok_or_else(|| IntegrationError::MissingRequiredField {
                    field_name: "customer.name",
                    context: utils::integration_ctx(
                        "The customer name is missing",
                        "Send the customer name.",
                    ),
                })?;
            Ok(Self::Create {
                name: Secret::new(name.clone()),
                email: request.email.clone(),
                payment_method,
                payment_method_options,
            })
        }
    }
}

pub fn sync_state(
    request: &PaymentsSyncData,
) -> Result<Option<SetupState>, error_stack::Report<IntegrationError>> {
    let id = request.get_connector_transaction_id()?;
    if !id.starts_with(SETUP_PREFIX) {
        return Ok(None);
    }
    let state = request
        .connector_feature_data
        .as_ref()
        .and_then(|metadata| metadata.peek().get("rapyd_setup"))
        .ok_or_else(|| IntegrationError::MissingRequiredField {
            field_name: "connector_feature_data.rapyd_setup",
            context: utils::integration_ctx(
                "The card verification state is missing",
                "Preserve connector_feature_data from SetupRecurring.",
            ),
        })?;
    let state: SetupState = serde_json::from_value(state.clone()).change_context(
        IntegrationError::RequestEncodingFailed {
            context: utils::integration_ctx(
                "The card verification state is invalid",
                "Send the stored SetupRecurring state unchanged.",
            ),
        },
    )?;
    if state.reference != id
        || !state.customer_id.starts_with("cus_")
        || state.mode == SetupMode::Unknown
        || (state.mode == SetupMode::External3ds
            && (state.eci.as_deref().is_none_or(str::is_empty)
                || state.version.as_deref().is_none_or(str::is_empty)))
    {
        return Err(IntegrationError::RequestEncodingFailed {
            context: utils::integration_ctx(
                "The setup identifier and state do not match",
                "Send the stored setup identifier and state together.",
            ),
        }
        .into());
    }
    Ok(Some(state))
}

pub fn setup_sync_url(
    request: &PaymentsSyncData,
    base: &str,
) -> Result<Option<String>, error_stack::Report<IntegrationError>> {
    Ok(sync_state(request)?.map(|state| {
        format!(
            "{base}/v1/payments?customer={}&limit=100",
            urlencoding::encode(&state.customer_id)
        )
    }))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupAuthentication {
    pub result: Option<String>,
    pub eci: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SetupPaymentStatus {
    #[serde(rename = "ACT")]
    Active,
    #[serde(rename = "ERR")]
    Error,
    #[serde(rename = "CAN")]
    Canceled,
    #[serde(rename = "EXP")]
    Expired,
    #[serde(rename = "REV")]
    Reversed,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupPayment {
    pub id: String,
    pub amount: common_utils::FloatMajorUnit,
    pub original_amount: Option<common_utils::FloatMajorUnit>,
    pub customer_token: Option<String>,
    pub status: SetupPaymentStatus,
    pub next_action: Option<NextAction>,
    pub payment_method: Option<Secret<String>>,
    pub payment_method_data: Option<RapydResponsePaymentMethodData>,
    pub currency_code: Option<common_enums::Currency>,
    pub authentication_result: Option<SetupAuthentication>,
    #[serde(default)]
    pub metadata: serde_json::Value,
    pub failure_code: Option<String>,
    pub failure_message: Option<String>,
    pub error_code: Option<String>,
}

impl SetupPayment {
    pub fn completion_status(
        &self,
        mode: SetupMode,
        eci: Option<&str>,
        version: Option<&str>,
    ) -> common_enums::AttemptStatus {
        use common_enums::AttemptStatus;
        if matches!(
            self.status,
            SetupPaymentStatus::Error
                | SetupPaymentStatus::Canceled
                | SetupPaymentStatus::Expired
                | SetupPaymentStatus::Reversed
        ) || self
            .failure_code
            .as_deref()
            .is_some_and(|value| !value.is_empty())
            || self
                .failure_message
                .as_deref()
                .is_some_and(|value| !value.is_empty())
            || self
                .error_code
                .as_deref()
                .is_some_and(|value| !value.is_empty())
        {
            return AttemptStatus::Failure;
        }
        if !matches!(self.status, SetupPaymentStatus::Active)
            || self
                .payment_method
                .as_ref()
                .is_none_or(|card| !card.peek().starts_with("card_"))
        {
            return AttemptStatus::AuthenticationPending;
        }
        let Some(authentication) = self.authentication_result.as_ref() else {
            return AttemptStatus::AuthenticationPending;
        };
        let verified = match mode {
            SetupMode::Hosted3ds => {
                matches!(
                    self.next_action,
                    Some(NextAction::PendingCapture | NextAction::NotApplicable)
                ) && authentication.result.as_deref() == Some("A")
                    && matches!(authentication.eci.as_deref(), Some("05" | "02"))
                    && authentication
                        .version
                        .as_deref()
                        .is_some_and(|value| !value.is_empty())
            }
            SetupMode::External3ds => {
                self.next_action == Some(NextAction::NotApplicable)
                    && matches!(authentication.result.as_deref(), None | Some("A"))
                    && eci.is_some()
                    && authentication.eci.as_deref() == eci
                    && version.is_some()
                    && authentication.version.as_deref() == version
            }
            SetupMode::Unknown => false,
        };
        if verified {
            AttemptStatus::Charged
        } else {
            AttemptStatus::AuthenticationPending
        }
    }
}

pub struct SetupWebhook {
    reference: String,
    status: common_enums::AttemptStatus,
    card: Option<String>,
    network: Option<String>,
    amount: Option<common_utils::types::Money>,
    error_code: Option<String>,
    error_message: Option<String>,
}

impl SetupWebhook {
    pub fn event_type(&self) -> domain_types::connector_types::EventType {
        use common_enums::AttemptStatus;
        use domain_types::connector_types::EventType;
        match self.status {
            AttemptStatus::Charged => EventType::PaymentIntentSuccess,
            AttemptStatus::Failure => EventType::PaymentIntentFailure,
            _ => EventType::PaymentIntentProcessing,
        }
    }

    pub fn reference(&self) -> String {
        self.reference.clone()
    }

    pub fn into_response(
        self,
        raw_body: &[u8],
    ) -> domain_types::connector_types::WebhookDetailsResponse {
        let verified = self.status == common_enums::AttemptStatus::Charged;
        domain_types::connector_types::WebhookDetailsResponse {
            raw_connector_status: None,
            amount: self.amount,
            captured_money: None,
            resource_id: Some(ResponseId::ConnectorTransactionId(self.reference)),
            status: self.status,
            connector_response_reference_id: None,
            connector_request_reference_id: None,
            mandate_reference: verified.then_some(self.card).flatten().map(|card| {
                Box::new(MandateReference {
                    connector_mandate_id: Some(card),
                    payment_method_id: None,
                    mandate_metadata: None,
                    connector_mandate_request_reference_id: None,
                })
            }),
            network_txn_id: verified.then_some(self.network).flatten(),
            error_code: self.error_code,
            error_reason: self.error_message.clone(),
            error_message: self.error_message,
            raw_connector_response: Some(String::from_utf8_lossy(raw_body).into_owned()),
            status_code: 200,
            response_headers: None,
            payment_method_update: None,
            sender_payment_instrument_id: None,
            connector_returned_payment_method_details: None,
        }
    }
}

pub fn setup_webhook(
    event: &super::RapydIncomingWebhook,
) -> Result<Option<SetupWebhook>, error_stack::Report<domain_types::errors::WebhookError>> {
    use super::RapydWebhookEventType;
    use domain_types::errors::WebhookError;
    if !matches!(
        event.webhook_type,
        RapydWebhookEventType::PaymentUpdated
            | RapydWebhookEventType::PaymentSucceeded
            | RapydWebhookEventType::PaymentCompleted
            | RapydWebhookEventType::PaymentCaptured
            | RapydWebhookEventType::PaymentFailed
            | RapydWebhookEventType::PaymentCanceled
            | RapydWebhookEventType::PaymentExpired
            | RapydWebhookEventType::CardAddedSuccessfully
            | RapydWebhookEventType::CardAddedFailure
    ) {
        return Ok(None);
    }
    let value = event.data.peek();
    let Some(metadata) = value
        .get("metadata")
        .filter(|metadata| metadata.get("ucs_setup_attempt").is_some())
    else {
        return Ok(None);
    };
    let marker: SetupMarker = serde_json::from_value(metadata.clone())
        .change_context(WebhookError::WebhookBodyDecodingFailed)?;
    let valid_reference = marker
        .ucs_setup_attempt
        .strip_prefix(SETUP_PREFIX)
        .is_some_and(|hash| hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()));
    if !valid_reference
        || marker.ucs_setup_mode == SetupMode::Unknown
        || (marker.ucs_setup_mode == SetupMode::External3ds
            && (marker.ucs_setup_eci.as_deref().is_none_or(str::is_empty)
                || marker
                    .ucs_setup_version
                    .as_deref()
                    .is_none_or(str::is_empty)))
    {
        return Err(WebhookError::WebhookBodyDecodingFailed.into());
    }
    let (status, card, network, amount, error_code, error_message) = match event.webhook_type {
        RapydWebhookEventType::CardAddedFailure | RapydWebhookEventType::CardAddedSuccessfully => {
            let card: SetupCardResponse = serde_json::from_value(value.clone())
                .change_context(WebhookError::WebhookBodyDecodingFailed)?;
            let failed = matches!(event.webhook_type, RapydWebhookEventType::CardAddedFailure)
                || card
                    .failure_code
                    .as_deref()
                    .is_some_and(|code| !code.is_empty())
                || card
                    .failure_message
                    .as_deref()
                    .is_some_and(|message| !message.is_empty());
            let status = if failed {
                common_enums::AttemptStatus::Failure
            } else if marker.ucs_setup_mode == SetupMode::External3ds
                && card.next_action == Some(NextAction::NotApplicable)
                && card.redirect_url.as_deref().is_none_or(str::is_empty)
                && card.id.as_deref().is_some_and(|id| id.starts_with("card_"))
            {
                common_enums::AttemptStatus::Charged
            } else {
                common_enums::AttemptStatus::AuthenticationPending
            };
            (
                status,
                card.id,
                card.network_reference_id.map(|id| id.expose()),
                None,
                card.failure_code,
                card.failure_message,
            )
        }
        _ => {
            let payment: SetupPayment = serde_json::from_value(value.clone())
                .change_context(WebhookError::WebhookBodyDecodingFailed)?;
            if payment.amount != common_utils::FloatMajorUnit::zero()
                || payment.original_amount != Some(common_utils::FloatMajorUnit::zero())
                || payment
                    .customer_token
                    .as_deref()
                    .is_none_or(|id| !id.starts_with("cus_"))
            {
                return Err(WebhookError::WebhookBodyDecodingFailed.into());
            }
            let status = match event.webhook_type {
                RapydWebhookEventType::PaymentFailed
                | RapydWebhookEventType::PaymentCanceled
                | RapydWebhookEventType::PaymentExpired => common_enums::AttemptStatus::Failure,
                _ => payment.completion_status(
                    marker.ucs_setup_mode,
                    marker.ucs_setup_eci.as_deref(),
                    marker.ucs_setup_version.as_deref(),
                ),
            };
            let amount = payment
                .currency_code
                .map(|currency| {
                    domain_types::utils::convert_back_amount_to_money_for_webhook(
                        &common_utils::types::FloatMajorUnitForConnector,
                        payment.amount,
                        currency,
                    )
                })
                .transpose()?;
            (
                status,
                payment.payment_method.map(|id| id.expose()),
                payment
                    .payment_method_data
                    .and_then(|method| method.network_reference_id)
                    .map(|id| id.expose()),
                amount,
                payment.failure_code,
                payment.failure_message,
            )
        }
    };
    let failed = status == common_enums::AttemptStatus::Failure;
    Ok(Some(SetupWebhook {
        reference: marker.ucs_setup_attempt,
        status,
        card,
        network,
        amount,
        error_code: failed.then(|| {
            error_code
                .filter(|code| !code.is_empty())
                .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.into())
        }),
        error_message: failed.then(|| {
            error_message
                .filter(|message| !message.is_empty())
                .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.into())
        }),
    }))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupSyncResponse {
    pub status: Status,
    pub data: Option<Vec<SetupPayment>>,
}

pub fn map_sync(
    item: ResponseRouterData<
        RapydPSyncResponse,
        RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
    >,
    state: SetupState,
) -> Result<
    RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
    error_stack::Report<ConnectorError>,
> {
    let mut data = item.router_data;
    let RapydPSyncResponse::Setup(response) = item.response else {
        return Err(utils::response_handling_fail(
            item.http_code,
            "Expected the setup verification payment list",
        )
        .into());
    };
    if response.status.status.as_deref() != Some("SUCCESS")
        || !response.status.error_code.is_empty()
    {
        data.response = Err(response.status.to_error_response(item.http_code));
        return Ok(data);
    }
    let payments = response.data.ok_or_else(|| {
        utils::response_handling_fail(item.http_code, "Missing setup verification payment list")
    })?;
    // A full page can hide another matching attempt. Never choose an arbitrary match.
    if payments.len() >= 100 {
        return Err(utils::response_handling_fail(
            item.http_code,
            "Rapyd verification payment list is truncated; completion is unresolved",
        )
        .into());
    }
    let mut matches = payments.into_iter().filter(|payment| {
        payment
            .metadata
            .get("ucs_setup_attempt")
            .and_then(serde_json::Value::as_str)
            == Some(state.reference.as_str())
    });
    let payment = matches.next();
    if matches.next().is_some() {
        return Err(utils::response_handling_fail(
            item.http_code,
            "Multiple verification payments match the setup attempt",
        )
        .into());
    }
    let Some(payment) = payment else {
        data.resource_common_data.status = common_enums::AttemptStatus::AuthenticationPending;
        data.response = Ok(transaction_response(
            &state,
            data.resource_common_data.status,
            None,
            None,
            None,
            item.http_code,
        )?);
        return Ok(data);
    };
    let marker: SetupMarker = serde_json::from_value(payment.metadata.clone()).change_context(
        utils::response_handling_fail(item.http_code, "Invalid setup verification metadata"),
    )?;
    if payment.customer_token.as_deref() != Some(state.customer_id.as_str())
        || marker.ucs_setup_mode != state.mode
        || marker.ucs_setup_eci != state.eci
        || marker.ucs_setup_version != state.version
        || payment.amount != common_utils::FloatMajorUnit::zero()
        || payment.original_amount != Some(common_utils::FloatMajorUnit::zero())
        // Rapyd can use a different currency for its generated zero verification.
        || payment.currency_code.is_none()
    {
        return Err(utils::response_handling_fail(
            item.http_code,
            "Verification payment does not match the stored setup context",
        )
        .into());
    }
    let status =
        payment.completion_status(state.mode, state.eci.as_deref(), state.version.as_deref());
    data.resource_common_data.status = status;
    data.response = if status == common_enums::AttemptStatus::Failure {
        Err(ErrorResponse {
            code: payment
                .failure_code
                .filter(|code| !code.is_empty())
                .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.into()),
            message: payment
                .failure_message
                .clone()
                .filter(|message| !message.is_empty())
                .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.into()),
            reason: payment.failure_message,
            status_code: item.http_code,
            connector_transaction_id: Some(state.reference.clone()),
            ..Default::default()
        })
    } else {
        Ok(transaction_response(
            &state,
            status,
            payment.payment_method.map(|id| id.expose()),
            None,
            payment
                .payment_method_data
                .and_then(|method| method.network_reference_id)
                .map(|id| id.expose()),
            item.http_code,
        )?)
    };
    Ok(data)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupCardResponse {
    pub id: Option<String>,
    pub next_action: Option<NextAction>,
    pub redirect_url: Option<String>,
    #[serde(default, deserialize_with = "super::deserialize_network_reference_id")]
    pub network_reference_id: Option<Secret<String>>,
    pub failure_code: Option<String>,
    pub failure_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupCards {
    pub data: Vec<SetupCardResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupCustomerResponse {
    #[serde(flatten)]
    pub card: SetupCardResponse,
    pub payment_methods: Option<SetupCards>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RapydSetupMandateResponse {
    pub status: Status,
    pub data: Option<SetupCustomerResponse>,
}

pub fn transaction_response(
    state: &SetupState,
    status: common_enums::AttemptStatus,
    card: Option<String>,
    redirect: Option<String>,
    network: Option<String>,
    http_code: u16,
) -> Result<PaymentsResponseData, error_stack::Report<ConnectorError>> {
    let redirect = redirect
        .filter(|url| !url.is_empty())
        .map(|url| {
            Url::parse(&url)
                .map(|url| {
                    Box::new(RedirectForm::from((
                        url,
                        common_utils::request::Method::Get,
                    )))
                })
                .change_context(utils::response_handling_fail(
                    http_code,
                    "Invalid Rapyd verification redirect",
                ))
        })
        .transpose()?;
    Ok(PaymentsResponseData::TransactionResponse {
        resource_id: ResponseId::ConnectorTransactionId(state.reference.clone()),
        redirection_data: redirect,
        mandate_reference: if status == common_enums::AttemptStatus::Charged {
            card.map(|card| {
                Box::new(MandateReference {
                    connector_mandate_id: Some(card),
                    payment_method_id: None,
                    connector_mandate_request_reference_id: None,
                    mandate_metadata: None,
                })
            })
        } else {
            None
        },
        connector_metadata: Some(serde_json::json!({"rapyd_setup": state})),
        network_txn_id: (status == common_enums::AttemptStatus::Charged)
            .then_some(network)
            .flatten(),
        network_txn_link_id: None,
        connector_response_reference_id: None,
        incremental_authorization_allowed: None,
        status_code: http_code,
        splits: None,
        payment_account_reference: None,
    })
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<RapydSetupMandateResponse, Self>>
    for RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<RapydSetupMandateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let mut data = item.router_data;
        if item.response.status.status.as_deref() != Some("SUCCESS")
            || !item.response.status.error_code.is_empty()
        {
            data.response = Err(item.response.status.to_error_response(item.http_code));
            return Ok(data);
        }
        let response = item.response.data.ok_or_else(|| {
            utils::response_handling_fail(item.http_code, "Missing Rapyd customer/card response")
        })?;
        let (customer_id, card) = match data
            .resource_common_data
            .connector_customer
            .clone()
            .filter(|id| !id.is_empty())
        {
            Some(customer) => (customer, response.card),
            None => {
                let customer = response
                    .card
                    .id
                    .filter(|id| id.starts_with("cus_"))
                    .ok_or_else(|| {
                        utils::response_handling_fail(item.http_code, "Missing Rapyd customer ID")
                    })?;
                let mut cards = response
                    .payment_methods
                    .filter(|cards| cards.data.len() == 1)
                    .ok_or_else(|| {
                        utils::response_handling_fail(
                            item.http_code,
                            "Expected one registered card",
                        )
                    })?
                    .data;
                let mut card = cards.remove(0);
                card.network_reference_id = card
                    .network_reference_id
                    .or(response.card.network_reference_id);
                (customer, card)
            }
        };
        let (mode, options) = setup_options(&data.request, &data.resource_common_data)
            .change_context(utils::response_handling_fail(
                item.http_code,
                "Invalid setup authentication context",
            ))?;
        let state = SetupState {
            reference: setup_reference(&data.resource_common_data.attempt_id).change_context(
                utils::response_handling_fail(item.http_code, "Missing setup attempt context"),
            )?,
            customer_id: customer_id.clone(),
            mode,
            eci: options
                .external
                .as_ref()
                .and_then(|options| options.eci.clone()),
            version: options.external.map(|options| options.three_ds_version),
        };
        let failed = card
            .failure_code
            .as_deref()
            .is_some_and(|code| !code.is_empty())
            || card
                .failure_message
                .as_deref()
                .is_some_and(|message| !message.is_empty());
        let status = if failed {
            common_enums::AttemptStatus::Failure
        } else if mode == SetupMode::External3ds
            && card.next_action == Some(NextAction::NotApplicable)
            && card.redirect_url.as_deref().is_none_or(str::is_empty)
            && card.id.as_deref().is_some_and(|id| id.starts_with("card_"))
        {
            common_enums::AttemptStatus::Charged
        } else {
            common_enums::AttemptStatus::AuthenticationPending
        };
        data.response = if failed {
            Err(ErrorResponse {
                code: card
                    .failure_code
                    .filter(|code| !code.is_empty())
                    .unwrap_or_else(|| common_utils::consts::NO_ERROR_CODE.into()),
                message: card
                    .failure_message
                    .clone()
                    .filter(|message| !message.is_empty())
                    .unwrap_or_else(|| common_utils::consts::NO_ERROR_MESSAGE.into()),
                reason: card.failure_message,
                status_code: item.http_code,
                connector_transaction_id: Some(state.reference),
                ..Default::default()
            })
        } else {
            Ok(transaction_response(
                &state,
                status,
                card.id,
                card.redirect_url,
                card.network_reference_id.map(|id| id.expose()),
                item.http_code,
            )?)
        };
        data.resource_common_data.status = status;
        data.resource_common_data.connector_customer = Some(customer_id);
        Ok(data)
    }
}
