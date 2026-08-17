use base64::Engine;
use common_enums::{AttemptStatus, Currency, RefundStatus};
use common_utils::{
    crypto::{self, SignMessage},
    date_time,
    pii::Email,
    types::{AmountConvertor, FloatMajorUnit, FloatMajorUnitForConnector},
};
use domain_types::{
    connector_flow::{Authorize, PSync, RSync, Refund},
    connector_types::{
        EventType, PaymentFlowData, PaymentWebhookReference, PaymentsAuthorizeData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, ResponseId, WebhookDetailsResponse, WebhookResourceReference,
    },
    errors,
    payment_method_data::{CardRedirectData, PaymentMethodData, PaymentMethodDataTypes},
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use error_stack::ResultExt;
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};
use time::{format_description::well_known::Iso8601, OffsetDateTime};

use super::{crypto as card_crypto, BoostRouterData};

pub const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

/// BCPG's `paymentMethod` value for the card-redirect flow (section 3.2.1).
const BOOST_PAYMENT_METHOD_CARD: &str = "card";

const FIELD_PROBE_FROZEN_TIMESTAMP_ENV_VAR: &str = "UCS_FIELD_PROBE_FROZEN_TIMESTAMP";

fn stable_created_timestamp() -> Result<String, error_stack::Report<errors::IntegrationError>> {
    if let Ok(frozen) = std::env::var(FIELD_PROBE_FROZEN_TIMESTAMP_ENV_VAR) {
        return Ok(frozen);
    }

    let ts = date_time::date_as_yyyymmddthhmmssmmmz().change_context(
        errors::IntegrationError::RequestEncodingFailed {
            context: errors::IntegrationErrorContext {
                suggested_action: None,
                doc_url: None,
                additional_context: Some(
                    "Failed to format the current UTC timestamp into BCPG's required \
                     `created` field format (ISO 8601 with milliseconds, e.g. \
                     2024-04-08T07:57:47.051Z)."
                        .to_string(),
                ),
            },
        },
    )?;
    Ok(match ts.find('.') {
        Some(dot_idx) => format!("{}.000Z", &ts[..dot_idx]),
        None => ts,
    })
}

/// Parse `created` (as returned by [`stable_created_timestamp`]) back into
/// epoch seconds, for BCPG's AES-GCM AAD (integration guideline section
/// 4.2.5: `merchantId|referenceId|createdEpochSeconds`). Derived by parsing
/// the already-generated `created` string rather than taking a second,
/// independent `now()` call — the doc requires `created` and the AAD's
/// epoch-seconds value to be from the exact same instant, and two
/// independent clock reads risk differing by a fraction of a second (the
/// same class of bug `stable_created_timestamp` above already guards
/// against for header/body signature consistency).
fn parse_epoch_seconds(
    iso_timestamp: &str,
) -> Result<i64, error_stack::Report<errors::IntegrationError>> {
    OffsetDateTime::parse(iso_timestamp, &Iso8601::DEFAULT)
        .change_context(errors::IntegrationError::RequestEncodingFailed {
            context: errors::IntegrationErrorContext {
                suggested_action: None,
                doc_url: None,
                additional_context: Some(format!(
                    "Failed to parse Boost's `created` timestamp ({iso_timestamp}) back into \
                     epoch seconds for the AES-GCM AAD used in the Direct Card Payment flow."
                )),
            },
        })
        .map(|dt| dt.unix_timestamp())
}

// =============================================================================
// AUTH
// =============================================================================

#[derive(Debug, Clone)]
pub struct BoostAuthType {
    pub client_id: Secret<String>,
    pub merchant_secret: Secret<String>,
    /// Only required for Authorize's Direct Card Payment path; PSync/Refund/
    /// RSync never read this.
    pub public_key: Option<Secret<String>>,
}

impl TryFrom<&ConnectorSpecificConfig> for BoostAuthType {
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Boost {
                client_id,
                merchant_secret,
                public_key,
                ..
            } => Ok(Self {
                client_id: client_id.to_owned(),
                merchant_secret: merchant_secret.to_owned(),
                public_key: public_key.to_owned(),
            }),
            _ => Err(error_stack::report!(
                errors::IntegrationError::FailedToObtainAuthType {
                    context: errors::IntegrationErrorContext {
                        suggested_action: Some(
                            "Configure this merchant account's Boost connector with a \
                             client_id and merchant_secret (ConnectorSpecificConfig::Boost)."
                                .to_string(),
                        ),
                        doc_url: None,
                        additional_context: Some(
                            "The connector_config passed to BoostAuthType::try_from was not \
                             the ConnectorSpecificConfig::Boost variant — either a different \
                             connector's config was routed to Boost, or Boost's client_id/ \
                             merchant_secret were never configured for this merchant account."
                                .to_string(),
                        ),
                    }
                }
            )),
        }
    }
}

impl BoostAuthType {
    /// BCPG Message Level Security (integration guideline section 4.1):
    /// Basic Auth whose password is a per-request HMAC-SHA256 signature over
    /// `method + path + body`, keyed by the merchant secret. `path` excludes the
    /// `/gateway` server prefix (i.e. it's whatever comes after the connector's
    /// configured base_url).
    pub fn build_signed_auth_header(
        &self,
        method: &str,
        path: &str,
        body: &str,
    ) -> Result<String, error_stack::Report<errors::IntegrationError>> {
        let raw = format!("{method}{path}{body}");
        let signature = crypto::HmacSha256
            .sign_message(self.merchant_secret.peek().as_bytes(), raw.as_bytes())
            .change_context(errors::IntegrationError::RequestEncodingFailed {
                context: errors::IntegrationErrorContext {
                    suggested_action: None,
                    doc_url: None,
                    additional_context: Some(format!(
                        "HMAC-SHA256 signing failed while building the Basic-Auth password \
                         for a {method} request to Boost's {path} — this is BCPG's \
                         Message Level Security scheme (integration guideline section 4.1): \
                         Base64(HmacSHA256(method + path + body, merchantSecret))."
                    )),
                },
            })
            .attach_printable("Failed to HMAC-sign the BCPG request")?;
        let password = BASE64_ENGINE.encode(signature);
        let token = BASE64_ENGINE.encode(format!("{}:{}", self.client_id.peek(), password));
        Ok(format!("Basic {token}"))
    }
}

// =============================================================================
// COMMON ERROR RESPONSE (doc section 3.8)
// =============================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BoostErrorResponse {
    pub timestamp: Option<String>,
    pub status: Option<String>,
    pub error: Option<String>,
    pub message: Option<String>,
}

// =============================================================================
// SHARED DATA TYPES
// =============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct BoostCustomer {
    #[serde(rename = "fullName", skip_serializing_if = "Option::is_none")]
    pub full_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<Email>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<String>,
}

/// Payment status enum (doc section 3.3.1 — Retrieve Payment Details).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BoostPaymentStatus {
    PendingPaymentMethod,
    PendingConfirmation,
    /// Observed in BCPG's live staging responses; not documented in the
    /// integration guideline (which only lists `pending_confirmation`), but
    /// semantically identical — BCPG's docs and its actual API have drifted.
    AwaitingConfirmation,
    Canceled,
    Expired,
    Processing,
    Failed,
    Denied,
    Error,
    Succeeded,
    /// Catch-all for any status value BCPG returns that isn't one of the above
    #[serde(other)]
    Unknown,
}

impl From<BoostPaymentStatus> for AttemptStatus {
    fn from(status: BoostPaymentStatus) -> Self {
        match status {
            BoostPaymentStatus::Succeeded => Self::Charged,
            BoostPaymentStatus::PendingPaymentMethod => Self::PaymentMethodAwaited,
            BoostPaymentStatus::PendingConfirmation | BoostPaymentStatus::AwaitingConfirmation => {
                Self::ConfirmationAwaited
            }
            BoostPaymentStatus::Processing => Self::Pending,
            BoostPaymentStatus::Canceled => Self::Voided,
            BoostPaymentStatus::Expired => Self::Expired,
            BoostPaymentStatus::Failed | BoostPaymentStatus::Denied | BoostPaymentStatus::Error => {
                Self::Failure
            }
            BoostPaymentStatus::Unknown => Self::Pending,
        }
    }
}

/// Reversal status enum (doc sections 3.2.2/3.2.3/3.3.3 — PaymentReversal). The
/// last three values are FPX-only per the doc but are still accepted here since
/// they're part of BCPG's single shared reversal status vocabulary.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BoostReversalStatus {
    Pending,
    Succeeded,
    Failed,
    PendingApproval,
    Expired,
    Denied,
    /// Catch-all for any status value BCPG returns that isn't one of the above.
    #[serde(other)]
    Unknown,
}

impl From<BoostReversalStatus> for RefundStatus {
    fn from(status: BoostReversalStatus) -> Self {
        match status {
            BoostReversalStatus::Succeeded => Self::Success,
            BoostReversalStatus::Pending | BoostReversalStatus::PendingApproval => Self::Pending,
            BoostReversalStatus::Failed
            | BoostReversalStatus::Expired
            | BoostReversalStatus::Denied => Self::Failure,
            BoostReversalStatus::Unknown => Self::Pending,
        }
    }
}

// =============================================================================
// AUTHORIZE (Payment Init — card, hosted/3DS redirect)
// =============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct BoostPaymentInitRequest {
    #[serde(rename = "referenceId")]
    pub reference_id: String,
    pub amount: FloatMajorUnit,
    pub currency: Currency,
    pub created: String,
    pub description: String,
    #[serde(rename = "paymentMethod", skip_serializing_if = "Option::is_none")]
    pub payment_method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer: Option<BoostCustomer>,
    #[serde(rename = "returnUrl", skip_serializing_if = "Option::is_none")]
    pub return_url: Option<String>,
    #[serde(rename = "callbackUrl", skip_serializing_if = "Option::is_none")]
    pub callback_url: Option<String>,
    #[serde(rename = "encryptedCardDetails", skip_serializing_if = "Option::is_none")]
    pub encrypted_card_details: Option<BoostEncryptedCardDetails>,
}

/// `EncryptedCardDetails` (integration guideline section 3.7.9) — built by
/// [`crate::connectors::boost::crypto::encrypt_card`].
#[derive(Debug, Clone, Serialize)]
pub struct BoostEncryptedCardDetails {
    #[serde(rename = "encryptedKey")]
    pub encrypted_key: String,
    pub iv: String,
    pub ciphertext: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        BoostRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for BoostPaymentInitRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: BoostRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // BCPG supports two ways to get card data (integration guideline section
        // 2.3.1 / 4.2): the hosted/3DS redirect flow, where the customer enters
        // card details on BCPG's own page (PaymentMethodData::CardRedirect, no
        // `encryptedCardDetails` in the body) — and the Direct Card Payment flow,
        // where the merchant collects raw card data and must client-side encrypt
        // it (RSA-OAEP + AES-256-GCM) into `encryptedCardDetails` before sending
        // it (PaymentMethodData::Card). Both send paymentMethod="card"; only the
        // presence of `encryptedCardDetails` differs.
        let card_data = match &item.router_data.request.payment_method_data {
            PaymentMethodData::CardRedirect(CardRedirectData::CardRedirect {}) => None,
            PaymentMethodData::Card(card) => Some(card),
            _ => {
                return Err(error_stack::report!(
                    errors::IntegrationError::NotImplemented(
                        "Boost only supports Card and CardRedirect payment_method_data for \
                         Authorize — Card for the Direct Card Payment flow (client-side \
                         encrypted, integration guideline section 4.2), CardRedirect for the \
                         hosted/3DS redirect flow where BCPG collects card details on its own \
                         page; other payment method data types are out of scope for this \
                         connector implementation"
                            .to_string(),
                        errors::IntegrationErrorContext {
                            suggested_action: Some(
                                "Route this payment with either PaymentMethodData::Card (raw \
                                 card data — encrypted client-side and sent as \
                                 encryptedCardDetails) or \
                                 PaymentMethodData::CardRedirect(CardRedirectData::CardRedirect{}) \
                                 (BCPG collects card details on its own hosted page after \
                                 redirect)."
                                    .to_string(),
                            ),
                            doc_url: None,
                            additional_context: Some(format!(
                                "Unsupported payment_method_data variant for Boost Authorize: {:?}",
                                item.router_data.request.payment_method_data
                            )),
                        },
                    )
                ));
            }
        };

        let amount = item
            .connector
            .amount_converter
            .convert(
                item.router_data.request.minor_amount,
                item.router_data.request.currency,
            )
            .change_context(errors::IntegrationError::AmountConversionFailed {
                context: errors::IntegrationErrorContext {
                    suggested_action: None,
                    doc_url: None,
                    additional_context: Some(format!(
                        "Failed to convert minor_amount {} {} to Boost's FloatMajorUnit \
                         (unquoted JSON decimal, e.g. 1.00) for the Authorize request.",
                        item.router_data.request.minor_amount.get_amount_as_i64(),
                        item.router_data.request.currency
                    )),
                },
            })
            .attach_printable("Failed to convert amount for Boost")?;

        let created = stable_created_timestamp()
            .attach_printable("Failed to format created timestamp for Boost")?;

        let description = item
            .router_data
            .resource_common_data
            .description
            .clone()
            .unwrap_or_else(|| "Payment".to_string());

        let email = item.router_data.request.email.clone();
        let full_name = item.router_data.request.customer_name.clone();
        let customer = if email.is_some() || full_name.is_some() {
            Some(BoostCustomer {
                full_name: full_name.clone(),
                email,
                phone: None,
            })
        } else {
            None
        };

        let reference_id = item
            .router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();

        // Only the Direct Card Payment flow (PaymentMethodData::Card) needs
        // encryption; CardRedirect never touches the public_key or crypto at all.
        let encrypted_card_details = match card_data {
            None => None,
            Some(card) => {
                let created_epoch_seconds = parse_epoch_seconds(&created).attach_printable(
                    "Failed to derive epoch seconds from `created` for Boost's AAD",
                )?;

                let auth = BoostAuthType::try_from(&item.router_data.connector_config)
                    .attach_printable(
                        "Failed to obtain Boost auth config while encrypting card data",
                    )?;
                let public_key = auth.public_key.as_ref().ok_or_else(|| {
                    error_stack::report!(errors::IntegrationError::InvalidConnectorConfig {
                        config: "boost.public_key",
                        context: errors::IntegrationErrorContext {
                            suggested_action: Some(
                                "Configure this merchant account's Boost connector with a \
                                 public_key (base64-encoded DER SPKI RSA public key, from \
                                 GET /v1/payments/card-encryption-key) to accept Direct Card \
                                 Payment (raw card) payments. This is only required for the \
                                 Card path of Authorize; CardRedirect, PSync, Refund, and \
                                 RSync all work without it."
                                    .to_string(),
                            ),
                            doc_url: None,
                            additional_context: Some(
                                "Boost's public_key is not configured for this merchant \
                                 account — required for the Direct Card Payment flow's \
                                 client-side encryption."
                                    .to_string(),
                            ),
                        },
                    })
                })?;

                // BCPG's AAD `merchantId` (integration guideline section 4.2.5). Using
                // client_id here, consistent with how this has been tested against
                // BCPG's staging so far. NOTE: BCPG has never explicitly confirmed
                // this is the correct value for the AAD — it's an open question from
                // the Boost/redBus integration email thread that was never answered.
                // If BCPG starts rejecting these requests with a decryption/signature
                // failure, this is the first thing to re-verify with BCPG.
                let merchant_id = auth.client_id.peek();

                let card_holder_name = card
                    .card_holder_name
                    .as_ref()
                    .map(|name| name.peek().to_owned())
                    .or_else(|| full_name.clone())
                    .unwrap_or_default();

                let encrypted = card_crypto::encrypt_card(
                    card.card_number.peek(),
                    card.card_exp_month.peek(),
                    card.card_exp_year.peek(),
                    card.card_cvc.peek(),
                    &card_holder_name,
                    public_key,
                    merchant_id,
                    &reference_id,
                    created_epoch_seconds,
                )
                .attach_printable(
                    "Failed to encrypt card data for Boost's Direct Card Payment flow",
                )?;

                Some(BoostEncryptedCardDetails {
                    encrypted_key: encrypted.encrypted_key,
                    iv: encrypted.iv,
                    ciphertext: encrypted.ciphertext,
                })
            }
        };

        Ok(Self {
            reference_id,
            amount,
            currency: item.router_data.request.currency,
            created,
            description,
            payment_method: Some(BOOST_PAYMENT_METHOD_CARD.to_string()),
            customer,
            return_url: item.router_data.request.router_return_url.clone(),
            callback_url: item.router_data.request.webhook_url.clone(),
            encrypted_card_details,
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BoostPaymentInitResponse {
    pub uuid: String,
    #[serde(rename = "paymentUrl")]
    pub payment_url: String,
}

impl<T: PaymentMethodDataTypes>
    TryFrom<crate::types::ResponseRouterData<BoostPaymentInitResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: crate::types::ResponseRouterData<BoostPaymentInitResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // The Init response carries no `status` field — the customer still has to
        // complete the redirect (3DS/OTP) before the payment is actually settled.
        let status = AttemptStatus::AuthenticationPending;

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.uuid),
                redirection_data: Some(Box::new(RedirectForm::Uri {
                    uri: item.response.payment_url,
                })),
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                splits: None,
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

// =============================================================================
// PSYNC (Retrieve Payment Details)
// =============================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BoostPaymentSyncResponse {
    pub uuid: String,
    pub status: BoostPaymentStatus,
}

impl TryFrom<crate::types::ResponseRouterData<BoostPaymentSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: crate::types::ResponseRouterData<BoostPaymentSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = AttemptStatus::from(item.response.status);

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.uuid),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                splits: None,
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

// =============================================================================
// REFUND (Create Reversal)
// =============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct BoostReversalRequest {
    pub uuid: String,
    #[serde(rename = "paymentUuid")]
    pub payment_uuid: String,
    pub amount: FloatMajorUnit,
    pub currency: Currency,
    pub created: String,
}

impl<F, T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<BoostRouterData<RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>, T>>
    for BoostReversalRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: BoostRouterData<RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    ) -> Result<Self, Self::Error> {
        let amount = item
            .connector
            .amount_converter
            .convert(
                item.router_data.request.minor_refund_amount,
                item.router_data.request.currency,
            )
            .change_context(errors::IntegrationError::AmountConversionFailed {
                context: errors::IntegrationErrorContext {
                    suggested_action: None,
                    doc_url: None,
                    additional_context: Some(format!(
                        "Failed to convert minor_refund_amount {} {} to Boost's \
                         FloatMajorUnit (unquoted JSON decimal, e.g. 1.00) for the reversal \
                         (refund/void) request against connector_transaction_id {}.",
                        item.router_data
                            .request
                            .minor_refund_amount
                            .get_amount_as_i64(),
                        item.router_data.request.currency,
                        item.router_data.request.connector_transaction_id
                    )),
                },
            })
            .attach_printable("Failed to convert refund amount for Boost")?;

        let created = stable_created_timestamp()
            .attach_printable("Failed to format created timestamp for Boost reversal")?;

        Ok(Self {
            uuid: item.router_data.request.refund_id.clone(),
            payment_uuid: item.router_data.request.connector_transaction_id.clone(),
            amount,
            currency: item.router_data.request.currency,
            created,
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BoostReversalResponse {
    pub uuid: String,
    pub status: BoostReversalStatus,
    #[serde(rename = "type")]
    pub reversal_type: Option<String>,
}

impl TryFrom<crate::types::ResponseRouterData<BoostReversalResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: crate::types::ResponseRouterData<BoostReversalResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let refund_status = RefundStatus::from(item.response.status);

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.uuid,
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            resource_common_data: RefundFlowData {
                status: refund_status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// RSYNC (Get Reversal Details)
// =============================================================================

impl TryFrom<crate::types::ResponseRouterData<BoostReversalResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: crate::types::ResponseRouterData<BoostReversalResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let refund_status = RefundStatus::from(item.response.status);

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.uuid,
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            resource_common_data: RefundFlowData {
                status: refund_status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// =============================================================================
// WEBHOOKS (tech spec "Webhook Events" section)
// =============================================================================
// BCPG POSTs a single payment-result payload to `callbackUrl` per outcome
// (success or failure) — there is no categorized event-type feed, just a
// `status` field that reuses the exact same vocabulary as the Retrieve
// Payment Details (PSync) endpoint, so `BoostPaymentStatus` and its existing
// `From<BoostPaymentStatus> for AttemptStatus` mapping are reused as-is
// rather than duplicating the status logic.

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BoostWebhookBody {
    pub timestamp: Option<String>,
    /// BCPG's own transaction id — same value returned as `uuid` on the
    /// Authorize (Init) and PSync responses.
    pub uuid: String,
    pub amount: Option<FloatMajorUnit>,
    pub currency: Option<Currency>,
    pub created: Option<String>,
    pub description: Option<String>,
    pub status: BoostPaymentStatus,
    #[serde(rename = "paymentMethod")]
    pub payment_method: Option<String>,
    /// The same merchant-generated reference sent as `referenceId` at
    /// Authorize and used to look the payment up again at PSync.
    #[serde(rename = "referenceId")]
    pub reference_id: String,
}

impl BoostWebhookBody {
    pub fn get_event_type(&self) -> EventType {
        match AttemptStatus::from(self.status.clone()) {
            AttemptStatus::Charged => EventType::PaymentIntentSuccess,
            AttemptStatus::Failure => EventType::PaymentIntentFailure,
            AttemptStatus::Voided => EventType::PaymentIntentCancelled,
            AttemptStatus::Expired => EventType::PaymentIntentExpired,
            AttemptStatus::PaymentMethodAwaited | AttemptStatus::ConfirmationAwaited => {
                EventType::PaymentActionRequired
            }
            _ => EventType::PaymentIntentProcessing,
        }
    }

    pub fn into_webhook_details_response(
        self,
        http_code: u16,
        raw_body: &[u8],
    ) -> WebhookDetailsResponse {
        let status = AttemptStatus::from(self.status);

        let minor_amount_captured = match (self.amount, self.currency) {
            (Some(amount), Some(currency)) => {
                match FloatMajorUnitForConnector.convert_back(amount, currency) {
                    Ok(minor_unit) => Some(minor_unit),
                    Err(err) => {
                        tracing::warn!(
                            error = ?err,
                            amount = ?amount,
                            currency = ?currency,
                            "Failed to convert Boost webhook amount to minor units; \
                             leaving amount_captured/minor_amount_captured unset"
                        );
                        None
                    }
                }
            }
            _ => None,
        };
        let amount_captured =
            minor_amount_captured.map(|minor_unit| minor_unit.get_amount_as_i64());

        WebhookDetailsResponse {
            resource_id: Some(ResponseId::ConnectorTransactionId(self.uuid.clone())),
            status,
            connector_response_reference_id: None,
            connector_request_reference_id: Some(self.reference_id),
            mandate_reference: None,
            // BCPG's documented webhook payload (tech spec "Webhook Events") carries
            // no error_code/error_message fields on failure — only `status`. Nothing
            // here to surface without fabricating it.
            error_code: None,
            error_message: None,
            error_reason: None,
            raw_connector_response: Some(String::from_utf8_lossy(raw_body).to_string()),
            status_code: http_code,
            response_headers: None,
            amount_captured,
            minor_amount_captured,
            network_txn_id: None,
            payment_method_update: None,
            sender_payment_instrument_id: None,
        }
    }

    pub fn into_webhook_event_reference(&self) -> WebhookResourceReference {
        WebhookResourceReference::Payment(PaymentWebhookReference {
            connector_transaction_id: Some(self.uuid.clone()),
            merchant_transaction_id: Some(self.reference_id.clone()),
        })
    }
}
