use common_enums::enums;
use common_utils::{ext_traits::ValueExt, request::Method};
use domain_types::{
    connector_flow::{
        Authenticate, Authorize, Capture, CreateConnectorCustomer, PaymentMethodToken,
        PreAuthenticate, RSync, Refund, RepeatPayment, Void,
    },
    connector_types::{
        ConnectorCustomerData, ConnectorCustomerResponse, MandateReference, MandateReferenceId,
        PaymentFlowData, PaymentMethodTokenResponse, PaymentMethodTokenizationData,
        PaymentVoidData, PaymentsAuthenticateData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsPreAuthenticateData, PaymentsResponseData, RefundFlowData, RefundSyncData,
        RefundsData, RefundsResponseData, RepeatPaymentData, ResponseId,
    },
    payment_method_data::{
        ApplePayPaymentData, BankDebitData, BankRedirectData, GiftCardData, GpayTokenizationData,
        PaymentMethodData, PaymentMethodDataTypes, WalletData,
    },
    router_data::{
        ConnectorSpecificConfig, PaysafeAccountKind, PaysafeApplePayFlow,
        PaysafePaymentMethodDetails,
    },
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use error_stack::ResultExt;
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::connectors::paysafe::PaysafeRouterData;
use crate::types::ResponseRouterData;
use domain_types::errors::ConnectorError;
use domain_types::errors::{IntegrationError, IntegrationErrorContext};

pub use super::requests::*;
pub use super::responses::*;

// Wire-protocol constants (values fixed by the Paysafe / wallet-network specs).

/// `deviceManufacturerIdentifier` Paysafe expects in Apple Pay decryptedData.
const APPLE_PAY_DEVICE_MANUFACTURER_ID: &str = "Apple";
/// `paymentDataType` for an Apple Pay 3-D Secure cryptogram payload.
const APPLE_PAY_PAYMENT_DATA_TYPE: &str = "3DSecure";
/// Google Pay `paymentMethodData.type` — always CARD for gateway tokens.
const GOOGLE_PAY_PM_TYPE: &str = "CARD";
/// Google Pay `tokenizationData.type` for gateway (non-direct) tokenization.
const GOOGLE_PAY_TOKEN_TYPE: &str = "PAYMENT_GATEWAY";
/// Placeholder epoch-millis expiry for the reconstructed Google Pay decrypted
/// token: hyperswitch drops the original message_expiration before forwarding
/// (see issue #11684 referenced below), so we send a far-future value that
/// Paysafe accepts instead of failing the payment.
const GOOGLE_PAY_MESSAGE_EXPIRATION_MS: &str = "9999999999999";

// Auth Type

#[derive(Debug, Clone)]
pub struct PaysafeAuthType {
    pub username: Secret<String>,
    pub password: Secret<String>,
    pub account_id: Option<PaysafePaymentMethodDetails>,
}

impl TryFrom<&ConnectorSpecificConfig> for PaysafeAuthType {
    type Error = IntegrationError;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Paysafe {
                username,
                password,
                account_id,
                ..
            } => Ok(Self {
                username: username.clone(),
                password: password.clone(),
                account_id: account_id.clone(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Paysafe requires ConnectorSpecificConfig::Paysafe with username/password (Basic auth) and the per-method account_id map."
                            .to_string(),
                    ),
                    ..Default::default()
                },
            }),
        }
    }
}

// Mandate Metadata

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PaysafeMandateMetadata {
    pub initial_transaction_id: String,
}

/// Self-contained mandate reference encoded into `connector_mandate_id`.
///
/// The gRPC recurring path cannot carry `mandate_metadata` (the proto
/// `ConnectorMandateReferenceId` has no metadata field and the Charge ->
/// RepeatPaymentData conversion hardcodes it to `None`), so the CIT Authorize
/// response encodes BOTH the reusable payment-handle token and the initial
/// transaction id (Paysafe payment `id`) into `connector_mandate_id`. The MIT
/// RepeatPayment request decodes both back out. Older bare-token values are
/// still handled via the `mandate_metadata` fallback.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PaysafeMandateReference {
    pub payment_handle_token: String,
    pub initial_transaction_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaysafeMeta {
    pub payment_handle_token: Secret<String>,
}

// Helper Functions

fn create_paysafe_billing_details(
    resource_common_data: &PaymentFlowData,
) -> Result<Option<PaysafeBillingDetails>, error_stack::Report<IntegrationError>> {
    let billing_address = resource_common_data.get_billing_address()?;
    // Paysafe rejects optional strings that are present but empty (error 5068:
    // "size must be between 1 and 50 where leading and trailing spaces are
    // omitted"), so blank values must be omitted rather than sent as "".
    let non_empty = |value: Option<Secret<String>>| value.filter(|v| !v.peek().trim().is_empty());
    // Only send billing details if billing mandatory fields are available
    if let (Some(zip), Some(country), Some(state)) = (
        resource_common_data.get_optional_billing_zip(),
        resource_common_data.get_optional_billing_country(),
        billing_address.to_state_code_as_optional()?,
    ) {
        Ok(Some(PaysafeBillingDetails {
            nick_name: non_empty(resource_common_data.get_optional_billing_first_name()),
            street: non_empty(resource_common_data.get_optional_billing_line1()),
            street2: non_empty(resource_common_data.get_optional_billing_line2()),
            city: non_empty(resource_common_data.get_optional_billing_city()),
            zip,
            country,
            state,
        }))
    } else {
        Ok(None)
    }
}

/// Whether this payment method is a Paysafe redirect APM that must create a payment
/// handle (and surface a customer redirect) in the Authorize flow rather than
/// settling a pre-created handle token. Shared by the Authorize URL selector and the
/// Authorize request builder so both agree on the routing.
pub(crate) fn is_paysafe_redirect_apm<T: PaymentMethodDataTypes>(
    payment_method_data: &PaymentMethodData<T>,
) -> bool {
    match payment_method_data {
        PaymentMethodData::Wallet(WalletData::Skrill(_))
        | PaymentMethodData::BankRedirect(BankRedirectData::Interac { .. }) => true,
        PaymentMethodData::GiftCard(gift_card_data) => {
            matches!(gift_card_data.as_ref(), GiftCardData::PaySafeCard {})
        }
        _ => false,
    }
}

/// Extract the Paysafe payment-handle token echoed back by the caller via
/// `connector_feature_data` (serialized [`PaysafeMeta`]). The Authorize leg-1
/// response returns this token in `connectorFeatureData`; the caller passes it
/// back on the settle leg. Shared by the settle request builder and the
/// second-leg detector so both read the token identically.
pub(crate) fn paysafe_feature_data_handle_token(
    resource_common_data: &PaymentFlowData,
) -> Option<Secret<String>> {
    resource_common_data
        .connector_feature_data
        .as_ref()
        .and_then(|metadata_value| {
            metadata_value
                .clone()
                .parse_value::<PaysafeMeta>("PaysafeMeta")
                .ok()
        })
        .map(|meta| meta.payment_handle_token)
}

/// Second leg of a redirect APM payment (shopper returned, or leg-1 handle echoed back).
pub(crate) fn is_paysafe_settle_leg<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    router_data: &RouterDataV2<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<T>,
        PaymentsResponseData,
    >,
) -> bool {
    is_paysafe_redirect_apm(&router_data.request.payment_method_data)
        && (router_data.request.redirect_response.is_some()
            || paysafe_feature_data_handle_token(&router_data.resource_common_data).is_some())
}

/// Leg 1: create a payment handle so a redirect APM returns its hosted-page link.
pub(crate) fn is_paysafe_handle_creation_leg<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    router_data: &RouterDataV2<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<T>,
        PaymentsResponseData,
    >,
) -> bool {
    is_paysafe_redirect_apm(&router_data.request.payment_method_data)
        && !is_paysafe_settle_leg(router_data)
}

/// Build a Paysafe payment-handle body for a redirect APM (Skrill, Interac
/// e-Transfer, paysafecard) directly from the Authorize request. Mirrors the
/// payment-handle body created by the PaymentMethodToken flow, but sourced from
/// `PaymentsAuthorizeData` so the Authorize response can return the redirect link
/// (matching hyperswitch's single Authorize -> paymenthandles behaviour). Kept as a
/// `TryFrom` (consistent with the sibling request builders) so the Authorize dispatch
/// can call `PaysafeSetupMandateRequest::try_from(&router_data)`.
impl<'a, T>
    TryFrom<
        &'a RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
    > for PaysafeSetupMandateRequest<T>
where
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        router_data: &'a RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = PaysafeAuthType::try_from(&router_data.connector_config)?;
        let account_id = auth
        .account_id
        .ok_or(IntegrationError::InvalidConnectorConfig {
            config: "account_id",
            context: IntegrationErrorContext {
                additional_context: Some(
                    "Paysafe redirect APMs need the account_id map in the connector config (skrill/interac slots) to resolve the processing account."
                        .to_string(),
                ),
                ..Default::default()
            },
        })?;

        let currency = router_data.request.currency;
        let amount = router_data.request.minor_amount;

        // Resolved before the match: both the returnLinks and `threeDs.merchantUrl` need it.
        let redirect_url = router_data.resource_common_data.get_return_url().ok_or(
        IntegrationError::MissingRequiredField {
            field_name: "return_url",
            context: IntegrationErrorContext {
                additional_context: Some(
                    "Paysafe payment handles that surface a customer redirect (redirect APMs, card + 3DS) need a return_url: it builds the returnLinks the shopper is sent back to, and is the mandatory threeDs.merchantUrl for 3DS."
                        .to_string(),
                ),
                ..Default::default()
            },
        },
    )?;

        // The 3DS ACS returns to complete_authorize_url so HS runs CompleteAuthorize (settle);
        // the plain return_url would only PSync. Falls back to return_url.
        let complete_authorize_url = router_data
            .request
            .complete_authorize_url
            .clone()
            .unwrap_or_else(|| redirect_url.clone());

        let (payment_method, payment_type, account_id, profile, settle_with_auth, three_ds) = match &router_data
        .request
        .payment_method_data
    {
        // Cards never create a payment handle in the Authorize leg, so no card
        // account-kind (3DS vs no-3DS) is ever chosen here. This builder's sole
        // caller is gated by `is_paysafe_handle_creation_leg`, whose
        // `is_paysafe_redirect_apm` check admits only Skrill / Interac / paysafecard.
        // A card + 3DS mints its `threeDs` handle in the PreAuthenticate flow; a
        // no-3DS card mints via the PaymentMethodToken flow and settles through
        // `PaysafePaymentsRequest`, which resolves the account by `is_three_ds()`.
        // Reject explicitly so the account-kind decision can never be assumed here.
        PaymentMethodData::Card(_) => {
            return Err(IntegrationError::NotImplemented(
                "Paysafe: cards do not create a payment handle in the Authorize leg (card + 3DS uses PreAuthenticate; no-3DS card uses PaymentMethodToken + settle)"
                    .to_string(),
                Default::default(),
            )
            .into());
        }
        PaymentMethodData::Wallet(WalletData::Skrill(_)) => {
            // Skrill consumer id is the billing email (mandatory). The FMA carries a
            // dedicated SKRILL processing account per currency; Paysafe requires its
            // accountId on the payment handle when the FMA has multiple accounts
            // (sending the card accountId instead returns error 5068). Mirror
            // hyperswitch: resolve from the skrill metadata slot.
            let consumer_id = router_data
                .resource_common_data
                .get_optional_billing_email()
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "email",
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "Skrill payment handles require the billing email as the Skrill consumerId."
                                .to_string(),
                        ),
                        ..Default::default()
                    },
                })?;
            let skrill_account_id = account_id.get_account_id(PaysafeAccountKind::Skrill, currency)?;
            let country_code = router_data.resource_common_data.get_optional_billing_country();
            (
                PaysafePaymentMethod::Skrill {
                    skrill: PaysafeSkrill {
                        consumer_id,
                        country_code,
                    },
                },
                PaysafePaymentType::Skrill,
                Some(skrill_account_id),
                None,
                None,
                None,
            )
        }
        PaymentMethodData::BankRedirect(BankRedirectData::Interac { email, .. }) => {
            // Interac e-Transfer consumer id: prefer the variant email, else billing.
            let consumer_id = email
                .clone()
                .or_else(|| {
                    router_data
                        .resource_common_data
                        .get_optional_billing_email()
                })
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "email",
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "Interac e-Transfer requires a consumer email: pass it in the interac payment_method_data or as billing email."
                                .to_string(),
                        ),
                        ..Default::default()
                    },
                })?;
            // Interac REQUIRES an accountId for CAD (unlike Skrill).
            let account_id = account_id.get_account_id(PaysafeAccountKind::Interac, currency)?;
            // Paysafe REQUIRES a consumer profile on the INTERAC_ETRANSFER payment
            // handle. Mirror hyperswitch: firstName, lastName and email are all
            // mandatory, sourced from billing details. profile is set ONLY for
            // Interac (never for Skrill/paysafecard/card/wallet).
            let profile = Some(PaysafeProfile {
                first_name: router_data
                    .resource_common_data
                    .get_optional_billing_first_name()
                    .ok_or(IntegrationError::MissingRequiredField {
                        field_name: "billing_first_name",
                        context: IntegrationErrorContext {
                            additional_context: Some(
                                "Paysafe requires a consumer profile (firstName) on INTERAC_ETRANSFER payment handles."
                                    .to_string(),
                            ),
                            ..Default::default()
                        },
                    })?,
                last_name: router_data
                    .resource_common_data
                    .get_optional_billing_last_name()
                    .ok_or(IntegrationError::MissingRequiredField {
                        field_name: "billing_last_name",
                        context: IntegrationErrorContext {
                            additional_context: Some(
                                "Paysafe requires a consumer profile (lastName) on INTERAC_ETRANSFER payment handles."
                                    .to_string(),
                            ),
                            ..Default::default()
                        },
                    })?,
                email: consumer_id.clone(),
            });
            (
                PaysafePaymentMethod::InteracEtransfer {
                    interac_etransfer: PaysafeInterac { consumer_id },
                },
                PaysafePaymentType::InteracEtransfer,
                Some(account_id),
                profile,
                None,
                None,
            )
        }
        PaymentMethodData::GiftCard(gift_card_data)
            if matches!(gift_card_data.as_ref(), GiftCardData::PaySafeCard {}) =>
        {
            // paysafecard consumerId is the merchant customer id (mandatory),
            // NOT the billing email. paysafecard restricts consumerId to a
            // limited character set (alphanumeric + a few specials), so a raw
            // email containing '@' is rejected. Mirror hyperswitch, which sources
            // this field from get_customer_id() (id_type::CustomerId) and reserves
            // billing email for Skrill/Interac only. Mirror Skrill: omit accountId.
            let consumer_id = router_data.resource_common_data.get_customer_id()?;
            (
                PaysafePaymentMethod::Paysafecard {
                    paysafecard: PaysafePaysafecard { consumer_id },
                },
                PaysafePaymentType::Paysafecard,
                None,
                None,
                None,
                None,
            )
        }
        _ => {
            return Err(IntegrationError::NotImplemented(
                "Only card + 3DS, Skrill, Interac e-Transfer, and paysafecard create a payment handle in the Paysafe Authorize flow".to_string(),
                Default::default(),
            )
            .into())
        }
    };

        let billing_details = create_paysafe_billing_details(&router_data.resource_common_data)?;

        // Success links (on_completed, and `default` for card+3DS since the ACS may return there)
        // point at complete_authorize_url so HS settles; on_failed/on_cancelled stay on return_url.
        let default_return_url = if three_ds.is_some() {
            complete_authorize_url.clone()
        } else {
            redirect_url.clone()
        };
        let return_links = Some(vec![
            ReturnLink {
                rel: LinkType::Default,
                href: default_return_url,
                method: Method::Get.to_string(),
            },
            ReturnLink {
                rel: LinkType::OnCompleted,
                href: complete_authorize_url.clone(),
                method: Method::Get.to_string(),
            },
            ReturnLink {
                rel: LinkType::OnFailed,
                href: redirect_url.clone(),
                method: Method::Get.to_string(),
            },
            ReturnLink {
                rel: LinkType::OnCancelled,
                href: redirect_url,
                method: Method::Get.to_string(),
            },
        ]);

        Ok(Self {
            merchant_ref_num: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            amount,
            // Cards send settleWithAuth; redirect APMs omit it on the payment-handle body.
            settle_with_auth,
            payment_method,
            currency_code: currency,
            payment_type,
            transaction_type: TransactionType::Payment,
            return_links,
            account_id,
            three_ds,
            profile,
            billing_details,
        })
    }
}

/// PreAuthenticate leg for card + 3DS: mint a payment handle with the `threeDs` object so Paysafe
/// returns the ACS challenge redirect; the handle token is settled on the follow-up Authorize.
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PaysafeRouterData<
            RouterDataV2<
                PreAuthenticate,
                PaymentFlowData,
                PaymentsPreAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for PaysafeSetupMandateRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: PaysafeRouterData<
            RouterDataV2<
                PreAuthenticate,
                PaymentFlowData,
                PaymentsPreAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = PaysafeAuthType::try_from(&router_data.connector_config)?;
        let account_id_map = auth.account_id.ok_or(IntegrationError::InvalidConnectorConfig {
            config: "account_id",
            context: IntegrationErrorContext {
                additional_context: Some(
                    "Paysafe card + 3DS needs the account_id map in the connector config (card.three_ds slot) to resolve the 3DS processing account."
                        .to_string(),
                ),
                ..Default::default()
            },
        })?;

        let currency =
            router_data
                .request
                .currency
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "currency",
                    context: Default::default(),
                })?;
        let amount = router_data.request.amount;

        // The ACS return must land on continue_redirection_url (…/redirect/complete/) so HS runs
        // CompleteAuthorize and settles; router_return_url only PSyncs. Falls back to return_url.
        let redirect_url = router_data
            .request
            .continue_redirection_url
            .as_ref()
            .map(|url| url.to_string())
            .or_else(|| router_data.resource_common_data.get_return_url())
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "continue_redirection_url",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Paysafe card + 3DS PreAuthenticate needs the complete_authorize continuation URL (continue_redirection_url) or a return_url: it is the mandatory threeDs.merchantUrl and builds the returnLinks the shopper is sent back to."
                            .to_string(),
                    ),
                    ..Default::default()
                },
            })?;

        let req_card = match &router_data.request.payment_method_data {
            Some(PaymentMethodData::Card(req_card)) => req_card,
            _ => {
                return Err(IntegrationError::NotImplemented(
                    "Paysafe PreAuthenticate only supports card + 3DS".to_string(),
                    Default::default(),
                )
                .into())
            }
        };

        let card = PaysafeCard {
            card_num: req_card.card_number.clone(),
            card_expiry: PaysafeCardExpiry {
                month: req_card.card_exp_month.clone(),
                year: req_card.get_expiry_year_4_digit(),
            },
            // Paysafe rejects an empty-string cvv; omit it instead.
            cvv: if req_card.card_cvc.peek().is_empty() {
                None
            } else {
                Some(req_card.card_cvc.clone())
            },
            holder_name: req_card.card_holder_name.clone().or_else(|| {
                router_data
                    .resource_common_data
                    .get_optional_billing_full_name()
            }),
        };
        // Paysafe rejects a `threeDs` body on a non-3DS account (error 5040); use 3DS account.
        let account_id =
            account_id_map.get_account_id(PaysafeAccountKind::CardThreeDs, currency)?;
        let three_ds = ThreeDs {
            merchant_url: redirect_url.clone(),
            // UCS has no client-platform signal; BROWSER is the correct channel here.
            device_channel: DeviceChannel::Browser,
            message_category: ThreeDsMessageCategory::Payment,
            authentication_purpose: ThreeDsAuthenticationPurpose::PaymentTransaction,
            requestor_challenge_preference: ThreeDsChallengePreference::ChallengeMandated,
        };
        let settle_with_auth = Some(matches!(
            router_data.request.capture_method,
            Some(enums::CaptureMethod::Automatic) | None
        ));

        let billing_details = create_paysafe_billing_details(&router_data.resource_common_data)?;

        // All return links use return_url: HS runs the settle Authorize on return regardless.
        let return_links = Some(vec![
            ReturnLink {
                rel: LinkType::Default,
                href: redirect_url.clone(),
                method: Method::Get.to_string(),
            },
            ReturnLink {
                rel: LinkType::OnCompleted,
                href: redirect_url.clone(),
                method: Method::Get.to_string(),
            },
            ReturnLink {
                rel: LinkType::OnFailed,
                href: redirect_url.clone(),
                method: Method::Get.to_string(),
            },
            ReturnLink {
                rel: LinkType::OnCancelled,
                href: redirect_url,
                method: Method::Get.to_string(),
            },
        ]);

        Ok(Self {
            merchant_ref_num: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            amount,
            settle_with_auth,
            payment_method: PaysafePaymentMethod::Card { card },
            currency_code: currency,
            payment_type: PaysafePaymentType::Card,
            transaction_type: TransactionType::Payment,
            return_links,
            account_id: Some(account_id),
            three_ds: Some(three_ds),
            profile: None,
            billing_details,
        })
    }
}

// Status Mapping Functions

pub fn get_paysafe_payment_status(
    status: PaysafePaymentStatus,
    capture_method: Option<enums::CaptureMethod>,
) -> enums::AttemptStatus {
    match status {
        PaysafePaymentStatus::Completed => match capture_method {
            Some(enums::CaptureMethod::Manual) => enums::AttemptStatus::Authorized,
            Some(enums::CaptureMethod::Automatic) | None => enums::AttemptStatus::Charged,
            Some(enums::CaptureMethod::SequentialAutomatic)
            | Some(enums::CaptureMethod::ManualMultiple)
            | Some(enums::CaptureMethod::Scheduled) => enums::AttemptStatus::Unresolved,
        },
        PaysafePaymentStatus::Failed => enums::AttemptStatus::Failure,
        PaysafePaymentStatus::Pending | PaysafePaymentStatus::Processing => {
            enums::AttemptStatus::Pending
        }
        PaysafePaymentStatus::Cancelled => enums::AttemptStatus::Voided,
    }
}

impl TryFrom<PaysafePaymentHandleStatus> for enums::AttemptStatus {
    type Error = ConnectorError;
    fn try_from(item: PaysafePaymentHandleStatus) -> Result<Self, Self::Error> {
        match item {
            PaysafePaymentHandleStatus::Completed => Ok(Self::Authorized),
            PaysafePaymentHandleStatus::Failed
            | PaysafePaymentHandleStatus::Expired
            | PaysafePaymentHandleStatus::Error => Ok(Self::Failure),
            PaysafePaymentHandleStatus::Initiated => Ok(Self::AuthenticationPending),
            PaysafePaymentHandleStatus::Payable | PaysafePaymentHandleStatus::Processing => {
                Ok(Self::Pending)
            }
        }
    }
}

impl From<PaysafeSettlementStatus> for enums::AttemptStatus {
    fn from(item: PaysafeSettlementStatus) -> Self {
        match item {
            PaysafeSettlementStatus::Completed
            | PaysafeSettlementStatus::Pending
            | PaysafeSettlementStatus::Processing => Self::Charged,
            PaysafeSettlementStatus::Failed => Self::Failure,
            PaysafeSettlementStatus::Cancelled => Self::Voided,
        }
    }
}

impl From<PaysafeVoidStatus> for enums::AttemptStatus {
    fn from(item: PaysafeVoidStatus) -> Self {
        match item {
            PaysafeVoidStatus::Completed
            | PaysafeVoidStatus::Pending
            | PaysafeVoidStatus::Processing => Self::Voided,
            PaysafeVoidStatus::Failed => Self::Failure,
            PaysafeVoidStatus::Cancelled => Self::Voided,
        }
    }
}

impl From<PaysafeRefundStatus> for enums::RefundStatus {
    fn from(item: PaysafeRefundStatus) -> Self {
        match item {
            PaysafeRefundStatus::Completed => Self::Success,
            PaysafeRefundStatus::Failed | PaysafeRefundStatus::Cancelled => Self::Failure,
            PaysafeRefundStatus::Pending | PaysafeRefundStatus::Processing => Self::Pending,
        }
    }
}

impl TryFrom<&enums::BankType> for PaysafeAchAccountType {
    type Error = IntegrationError;
    fn try_from(bank_type: &enums::BankType) -> Result<Self, Self::Error> {
        match bank_type {
            enums::BankType::Checking => Ok(Self::Checking),
            enums::BankType::Savings => Ok(Self::Savings),
            _ => Err(IntegrationError::NotImplemented(
                format!(
                    "Bank type {:?} is not supported for ACH bank debit",
                    bank_type
                ),
                Default::default(),
            )),
        }
    }
}

// CreateConnectorCustomer Flow - Request
//
// Registers a Paysafe customer (POST v1/customers) so a reusable MULTI_USE
// payment handle can later be minted under v1/customers/{customerId}/paymenthandles
// for card-on-file recurring (MIT). Mirrors hyperswitch's PaysafeCustomerDetails:
// merchantCustomerId is mandatory; name/email/phone are optional and sourced from
// the customer request and billing details.
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PaysafeRouterData<
            RouterDataV2<
                CreateConnectorCustomer,
                PaymentFlowData,
                ConnectorCustomerData,
                ConnectorCustomerResponse,
            >,
            T,
        >,
    > for PaysafeCustomerRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: PaysafeRouterData<
            RouterDataV2<
                CreateConnectorCustomer,
                PaymentFlowData,
                ConnectorCustomerData,
                ConnectorCustomerResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let customer_data = &router_data.request;

        // merchantCustomerId is the merchant's stable customer identifier and is
        // required by Paysafe.
        let merchant_customer_id = customer_data
            .customer_id
            .as_ref()
            .map(|id| id.peek().to_string())
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "customer_id",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Paysafe customer profiles require merchantCustomerId; pass the merchant customer id in the CreateConnectorCustomer request."
                            .to_string(),
                    ),
                    ..Default::default()
                },
            })?;

        let email = customer_data
            .email
            .as_ref()
            .map(|email| email.peek().clone())
            .or_else(|| {
                router_data
                    .resource_common_data
                    .get_optional_billing_email()
            });

        let phone = customer_data.phone.clone().or_else(|| {
            router_data
                .resource_common_data
                .get_optional_billing_phone_number()
        });

        Ok(Self {
            merchant_customer_id,
            first_name: router_data
                .resource_common_data
                .get_optional_billing_first_name(),
            last_name: router_data
                .resource_common_data
                .get_optional_billing_last_name(),
            email,
            phone,
        })
    }
}

// CreateConnectorCustomer Flow - Response

impl TryFrom<ResponseRouterData<PaysafeCustomerResponse, Self>>
    for RouterDataV2<
        CreateConnectorCustomer,
        PaymentFlowData,
        ConnectorCustomerData,
        ConnectorCustomerResponse,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PaysafeCustomerResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(ConnectorCustomerResponse {
                connector_customer_id: item.response.id,
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PaysafeRouterData<
            RouterDataV2<
                PaymentMethodToken,
                PaymentFlowData,
                PaymentMethodTokenizationData<T>,
                PaymentMethodTokenResponse,
            >,
            T,
        >,
    > for PaysafePaymentMethodTokenRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: PaysafeRouterData<
            RouterDataV2<
                PaymentMethodToken,
                PaymentFlowData,
                PaymentMethodTokenizationData<T>,
                PaymentMethodTokenResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        let auth = PaysafeAuthType::try_from(&item.router_data.connector_config)?;
        let account_id = auth
            .account_id
            .ok_or(IntegrationError::InvalidConnectorConfig {
                config: "account_id",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Paysafe Tokenize needs the account_id map (card no_three_ds / ach / apple_pay slots) to pick the processing account for the payment handle."
                            .to_string(),
                    ),
                    ..Default::default()
                },
            })?;

        let currency = router_data.request.currency;
        let amount = router_data.request.amount;

        let (payment_method, payment_type, account_id) =
            match &router_data.request.payment_method_data {
                PaymentMethodData::Card(req_card) => {
                    let card = PaysafeCard {
                        card_num: req_card.card_number.clone(),
                        card_expiry: PaysafeCardExpiry {
                            month: req_card.card_exp_month.clone(),
                            year: req_card.get_expiry_year_4_digit(),
                        },
                        cvv: if req_card.card_cvc.peek().is_empty() {
                            None
                        } else {
                            Some(req_card.card_cvc.clone())
                        },
                        holder_name: req_card.card_holder_name.clone().or_else(|| {
                            router_data
                                .resource_common_data
                                .get_optional_billing_full_name()
                        }),
                    };
                    let account_id = account_id.get_account_id(PaysafeAccountKind::CardNoThreeDs, currency)?;
                    (
                        PaysafePaymentMethod::Card { card },
                        PaysafePaymentType::Card,
                        Some(account_id),
                    )
                }
                PaymentMethodData::BankDebit(BankDebitData::AchBankDebit {
                    account_number,
                    routing_number,
                    bank_account_holder_name,
                    bank_type,
                    ..
                }) => {
                    let account_holder_name = bank_account_holder_name
                        .clone()
                        .or_else(|| {
                            router_data
                                .resource_common_data
                                .get_optional_billing_full_name()
                        })
                        .ok_or(IntegrationError::MissingRequiredField {
                            field_name: "bank_account_holder_name",
                            context: IntegrationErrorContext {
                                additional_context: Some(
                                    "Paysafe ACH requires the account holder name; provide bank_account_holder_name or a billing full name."
                                        .to_string(),
                                ),
                                ..Default::default()
                            },
                        })?;
                    let account_type = bank_type
                        .as_ref()
                        .map(PaysafeAchAccountType::try_from)
                        .transpose()?
                        .ok_or(IntegrationError::MissingRequiredField {
                            field_name: "bank_type (ach.accountType)",
                            context: IntegrationErrorContext {
                                additional_context: Some(
                                    "Paysafe ACH requires accountType (CHECKING/SAVINGS) mapped from the bank_debit bank_type."
                                        .to_string(),
                                ),
                                ..Default::default()
                            },
                        })?;
                    let ach = PaysafeAch {
                        account_holder_name,
                        account_number: account_number.clone(),
                        routing_number: routing_number.clone(),
                        account_type,
                    };
                    let account_id = account_id.get_account_id(PaysafeAccountKind::Ach, currency)?;
                    (
                        PaysafePaymentMethod::Ach { ach },
                        PaysafePaymentType::Ach,
                        Some(account_id),
                    )
                }
                PaymentMethodData::Wallet(WalletData::GooglePay(google_pay_data)) => {
                    let decrypted_data = match &google_pay_data.tokenization_data {
                        GpayTokenizationData::Decrypted(d) => d,
                        GpayTokenizationData::Encrypted(_) => {
                            return Err(IntegrationError::MissingRequiredField {
                                field_name: "google_pay.tokenization_data (decrypted)",
                                context: IntegrationErrorContext {
                                    additional_context: Some(
                                        "Paysafe Google Pay expects a pre-decrypted token (GpayTokenizationData::Decrypted); encrypted Google Pay tokens are not forwarded."
                                            .to_string(),
                                    ),
                                    ..Default::default()
                                },
                            }
                            .into())
                        }
                    };

                    let expiration_month = decrypted_data
                        .get_expiry_month()
                        .change_context(IntegrationError::MissingRequiredField {
                            field_name: "google_pay_decrypted_data.card_exp_month",
                            context: IntegrationErrorContext {
                                additional_context: Some(
                                    "Paysafe Google Pay decrypted tokens must carry the PAN expiration month."
                                        .to_string(),
                                ),
                                ..Default::default()
                            },
                        })?
                        .peek()
                        .parse::<u8>()
                        .map_err(|_| {
                            IntegrationError::InvalidDataFormat {
                                field_name: "google_pay_decrypted_data.card_exp_month",
                                context: IntegrationErrorContext {
                                    additional_context: Some(
                                        "Google Pay PAN expiration month must be a numeric MM value."
                                            .to_string(),
                                    ),
                                    ..Default::default()
                                },
                            }
                        })?;

                    let expiration_year = decrypted_data
                        .get_four_digit_expiry_year()
                        .change_context(IntegrationError::MissingRequiredField {
                            field_name: "google_pay_decrypted_data.card_exp_year",
                            context: IntegrationErrorContext {
                                additional_context: Some(
                                    "Paysafe Google Pay decrypted tokens must carry the PAN expiration year."
                                        .to_string(),
                                ),
                                ..Default::default()
                            },
                        })?
                        .peek()
                        .parse::<u16>()
                        .map_err(|_| {
                            IntegrationError::InvalidDataFormat {
                                field_name: "google_pay_decrypted_data.card_exp_year",
                                context: IntegrationErrorContext {
                                    additional_context: Some(
                                        "Google Pay PAN expiration year must be a numeric YYYY value."
                                            .to_string(),
                                    ),
                                    ..Default::default()
                                },
                            }
                        })?;

                    let pan = Secret::new(
                        decrypted_data
                            .application_primary_account_number
                            .get_card_no(),
                    );

                    let auth_method = if decrypted_data.cryptogram.is_some() {
                        PaysafeGooglePayAuthMethod::Cryptogram3Ds
                    } else {
                        PaysafeGooglePayAuthMethod::PanOnly
                    };

                    let payment_method_details = PaysafeGooglePayPaymentMethodDetails {
                        auth_method,
                        pan,
                        expiration_month,
                        expiration_year,
                        cryptogram: decrypted_data.cryptogram.clone(),
                    };

                    // TODO(https://github.com/juspay/hyperswitch/issues/11684): HS parses
                    // message_id and message_expiration from the decrypted GPay payload
                    // internally but drops them before forwarding to UCS via GPayPredecryptData.
                    // Until HS propagates these fields, we fall back to a random UUID for
                    // message_id (losing Paysafe's replay-detection guarantee) and a far-future
                    // placeholder for message_expiration.
                    let decrypted_token = PaysafeGooglePayDecryptedToken {
                        message_id: uuid::Uuid::new_v4().to_string(),
                        message_expiration: GOOGLE_PAY_MESSAGE_EXPIRATION_MS.to_string(),
                        payment_method_details,
                    };

                    let google_pay_payment_token = PaysafeGooglePayPaymentToken {
                        api_version: 2,
                        api_version_minor: 0,
                        payment_method_data: PaysafeGooglePayPaymentMethodData {
                            pm_type: GOOGLE_PAY_PM_TYPE.to_string(),
                            description: google_pay_data.description.clone(),
                            info: PaysafeGooglePayCardInfo {
                                card_network: google_pay_data.info.card_network.clone(),
                                card_details: google_pay_data.info.card_details.clone(),
                            },
                            tokenization_data: PaysafeGooglePayTokenizationData {
                                token_type: GOOGLE_PAY_TOKEN_TYPE.to_string(),
                                decrypted_token,
                            },
                        },
                    };

                    let account_id = account_id.get_account_id(PaysafeAccountKind::CardNoThreeDs, currency)?;
                    (
                        PaysafePaymentMethod::GooglePay {
                            google_pay: PaysafeGooglePay {
                                google_pay_payment_token,
                            },
                        },
                        PaysafePaymentType::Card,
                        Some(account_id),
                    )
                }
                PaymentMethodData::Wallet(WalletData::ApplePay(apple_pay_data)) => {
                let payment_data = match &apple_pay_data.payment_data {
                        ApplePayPaymentData::Encrypted(_) => {
                            let decoded_token = apple_pay_data
                                .get_applepay_decoded_payment_data()
                                .change_context(IntegrationError::InvalidDataFormat {
                                    field_name: "apple_pay.payment_data",
                                    context: IntegrationErrorContext {
                                        additional_context: Some(
                                            "Apple Pay payment_data must be a base64-encoded PKPaymentToken."
                                                .to_string(),
                                        ),
                                        ..Default::default()
                                    },
                                })?;
                            PaysafeApplePayPaymentData::Encrypted(
                                serde_json::from_str(decoded_token.peek()).change_context(
                                    IntegrationError::InvalidDataFormat {
                                        field_name: "apple_pay.payment_data",
                                        context: IntegrationErrorContext {
                                            additional_context: Some(
                                                "Decoded Apple Pay payment_data is not valid PKPaymentToken JSON."
                                                    .to_string(),
                                            ),
                                            ..Default::default()
                                        },
                                    },
                                )?,
                            )
                        }
                        ApplePayPaymentData::Decrypted(decrypted) => {
                            let expiry_year = decrypted
                                .get_two_digit_expiry_year()
                                .change_context(IntegrationError::InvalidDataFormat {
                                    field_name: "apple_pay.application_expiration_year",
                                    context: IntegrationErrorContext {
                                        additional_context: Some(
                                            "Apple Pay decrypted expiration year must reduce to two digits (YY) for Paysafe's YYMM applicationExpirationDate."
                                                .to_string(),
                                        ),
                                        ..Default::default()
                                    },
                                })?;
                            let application_expiration_date = Secret::new(format!(
                                "{}{:0>2}",
                                expiry_year.peek(),
                                decrypted.get_expiry_month().peek()
                            ));
                            PaysafeApplePayPaymentData::Decrypted(
                                PaysafeApplePayDecryptedDataWrapper {
                                    decrypted_data: PaysafeApplePayDecryptedData {
                                        application_primary_account_number: Secret::new(
                                            decrypted
                                                .application_primary_account_number
                                                .get_card_no(),
                                        ),
                                        application_expiration_date,
                                        // Numeric ISO 4217 code (e.g. "840"), per Paysafe's
                                        // decryptedData contract — NOT the alphabetic code.
                                        currency_code: currency.iso_4217().to_string(),
                                        transaction_amount: Some(amount),
                                        cardholder_name: None,
                                        device_manufacturer_identifier: Some(
                                            APPLE_PAY_DEVICE_MANUFACTURER_ID.to_string(),
                                        ),
                                        payment_data_type: Some(
                                            APPLE_PAY_PAYMENT_DATA_TYPE.to_string(),
                                        ),
                                        payment_data: PaysafeApplePayDecryptedPaymentData {
                                            online_payment_cryptogram: decrypted
                                                .payment_data
                                                .online_payment_cryptogram
                                                .clone(),
                                            eci_indicator: decrypted
                                                .payment_data
                                                .eci_indicator
                                                .clone(),
                                        },
                                    },
                                },
                            )
                        }
                    };

                    let apple_pay = PaysafeApplePay {
                        label: None,
                        request_billing_address: Some(false),
                        apple_pay_payment_token: PaysafeApplePayPaymentToken {
                            token: PaysafeApplePayToken {
                                payment_data,
                                payment_method: PaysafeApplePayPaymentMethod {
                                    display_name: apple_pay_data
                                        .payment_method
                                        .display_name
                                        .clone(),
                                    network: apple_pay_data.payment_method.network.clone(),
                                    pm_type: apple_pay_data.payment_method.pm_type.clone(),
                                },
                                transaction_identifier: apple_pay_data
                                    .transaction_identifier
                                    .clone(),
                            },
                            billing_contact: Some(PaysafeApplePayBillingContact {
                                address_lines: vec![
                                    router_data
                                        .resource_common_data
                                        .get_optional_billing_line1(),
                                    router_data
                                        .resource_common_data
                                        .get_optional_billing_line2(),
                                ],
                                postal_code: router_data
                                    .resource_common_data
                                    .get_billing_zip()
                                    .change_context(IntegrationError::MissingRequiredField {
                                        field_name: "billing.address.zip",
                                        context: IntegrationErrorContext {
                                            additional_context: Some(
                                                "Paysafe Apple Pay billingContact requires the billing zip (hyperswitch parity)."
                                                    .to_string(),
                                            ),
                                            ..Default::default()
                                        },
                                    })?,
                                country_code: router_data
                                    .resource_common_data
                                    .get_billing_country()
                                    .change_context(IntegrationError::MissingRequiredField {
                                        field_name: "billing.address.country",
                                        context: IntegrationErrorContext {
                                            additional_context: Some(
                                                "Paysafe Apple Pay billingContact requires the billing country (hyperswitch parity)."
                                                    .to_string(),
                                            ),
                                            ..Default::default()
                                        },
                                    })?,
                                administrative_area: None,
                                country: None,
                                family_name: None,
                                given_name: None,
                                locality: None,
                                phonetic_family_name: None,
                                phonetic_given_name: None,
                                sub_administrative_area: None,
                                sub_locality: None,
                            }),
                        },
                    };

                    // Apple Pay uses a DEDICATED Paysafe processing account,
                    // distinct from the card account. Mirror hyperswitch by
                    // selecting the apple_pay slot for the active flow: `encrypt`
                    // for the encrypted PKPaymentToken flow, `decrypt` for the
                    // pre-decrypted flow.
                    //
                    // Fallback: hyperswitch hard-requires the apple_pay account,
                    // but sandboxes without an apple_pay slot provisioned work
                    // using the card `no_three_ds` account. To avoid a regression
                    // while enabling parity once provisioned, prefer the apple_pay
                    // account and gracefully fall back to the card no_three_ds
                    // account when the apple_pay slot is absent.
                    let flow = match &apple_pay_data.payment_data {
                        ApplePayPaymentData::Encrypted(_) => PaysafeApplePayFlow::Encrypt,
                        _ => PaysafeApplePayFlow::Decrypt,
                    };
                    let account_id = account_id
                        .get_account_id(PaysafeAccountKind::ApplePay(flow), currency)
                        .or_else(|_| account_id.get_account_id(PaysafeAccountKind::CardNoThreeDs, currency))?;
                    (
                        PaysafePaymentMethod::ApplePay {
                            apple_pay: Box::new(apple_pay),
                        },
                        PaysafePaymentType::Card,
                        Some(account_id),
                    )
                }
                PaymentMethodData::Wallet(WalletData::Skrill(_)) => {
                    // Skrill consumer id is the billing email. It is mandatory.
                    let consumer_id = router_data
                        .resource_common_data
                        .get_optional_billing_email()
                        .ok_or(IntegrationError::MissingRequiredField {
                            field_name: "email",
                            context: IntegrationErrorContext {
                                additional_context: Some(
                                    "Skrill payment handles require the billing email as the Skrill consumerId."
                                        .to_string(),
                                ),
                                ..Default::default()
                            },
                        })?;
                    let skrill = PaysafeSkrill {
                        consumer_id,
                        country_code: router_data
                            .resource_common_data
                            .get_optional_billing_country(),
                    };
                    // The FMA carries a dedicated SKRILL processing account per currency;
                    // Paysafe requires its accountId on the payment handle when the FMA
                    // has multiple accounts (sending the card accountId instead returns
                    // error 5068). Mirror hyperswitch: resolve from the skrill slot.
                    let skrill_account_id = account_id.get_account_id(PaysafeAccountKind::Skrill, currency)?;
                    (
                        PaysafePaymentMethod::Skrill { skrill },
                        PaysafePaymentType::Skrill,
                        Some(skrill_account_id),
                    )
                }
                PaymentMethodData::BankRedirect(BankRedirectData::Interac { email, .. }) => {
                    // Interac e-Transfer consumer id: prefer the variant email, else billing
                    // email. Mandatory.
                    let consumer_id = email
                        .clone()
                        .or_else(|| {
                            router_data
                                .resource_common_data
                                .get_optional_billing_email()
                        })
                        .ok_or(IntegrationError::MissingRequiredField {
                            field_name: "email",
                            context: IntegrationErrorContext {
                                additional_context: Some(
                                    "Interac e-Transfer requires a consumer email: pass it in the interac payment_method_data or as billing email."
                                        .to_string(),
                                ),
                                ..Default::default()
                            },
                        })?;
                    let interac_etransfer = PaysafeInterac { consumer_id };
                    // Interac REQUIRES an accountId for CAD (unlike Skrill). Resolve from the
                    // interac CAD metadata slot; gracefully errors if unprovisioned.
                    let account_id = account_id.get_account_id(PaysafeAccountKind::Interac, currency)?;
                    (
                        PaysafePaymentMethod::InteracEtransfer { interac_etransfer },
                        PaysafePaymentType::InteracEtransfer,
                        Some(account_id),
                    )
                }
                PaymentMethodData::GiftCard(gift_card_data) => match gift_card_data.as_ref() {
                    GiftCardData::PaySafeCard {} => {
                        // paysafecard consumerId is the merchant customer id, NOT the
                        // billing email. paysafecard restricts consumerId to a limited
                        // character set, so a raw email ('@') is rejected. Mirror
                        // hyperswitch: source from get_customer_id() (id_type::CustomerId),
                        // reserving billing email for Skrill/Interac only.
                        let consumer_id = router_data.resource_common_data.get_customer_id()?;
                        let paysafecard = PaysafePaysafecard { consumer_id };
                        // Mirror Skrill: omit accountId entirely for paysafecard.
                        (
                            PaysafePaymentMethod::Paysafecard { paysafecard },
                            PaysafePaymentType::Paysafecard,
                            None,
                        )
                    }
                    GiftCardData::Givex(_) => {
                        return Err(IntegrationError::NotImplemented(
                            "Givex gift cards are not supported for Paysafe".to_string(),
                            Default::default(),
                        )
                        .into())
                    }
                },
                _ => {
                    return Err(IntegrationError::NotImplemented("Only card, ACH, GooglePay, ApplePay, Skrill, Interac, and Paysafecard payment methods are supported for PaymentMethodToken"
                            .to_string() , Default::default())
                    .into())
                }
            };

        // For ACH payments, Paysafe requires settleWithAuth to be true.
        // For Card (including GooglePay which maps to Card), settle based on capture_method.
        // For Skrill (redirect wallet), the verified payment-handle body omits settleWithAuth.
        let settle_with_auth = match payment_type {
            PaysafePaymentType::Ach => Some(true),
            PaysafePaymentType::Card => Some(matches!(
                router_data.request.capture_method,
                Some(enums::CaptureMethod::Automatic) | None
            )),
            PaysafePaymentType::Skrill => None,
            PaysafePaymentType::InteracEtransfer => None,
            PaysafePaymentType::Paysafecard => None,
        };

        let billing_details = create_paysafe_billing_details(&router_data.resource_common_data)?;

        // Paysafe requires return_links even for no-3DS flows
        let redirect_url = router_data.resource_common_data.get_return_url().ok_or(
            IntegrationError::MissingRequiredField {
                field_name: "return_url",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Paysafe payment handles need a return_url to build the returnLinks the shopper is sent back to."
                            .to_string(),
                    ),
                    ..Default::default()
                },
            },
        )?;

        let return_links = Some(vec![
            ReturnLink {
                rel: LinkType::Default,
                href: redirect_url.clone(),
                method: Method::Get.to_string(),
            },
            ReturnLink {
                rel: LinkType::OnCompleted,
                href: redirect_url.clone(),
                method: Method::Get.to_string(),
            },
            ReturnLink {
                rel: LinkType::OnFailed,
                href: redirect_url.clone(),
                method: Method::Get.to_string(),
            },
            ReturnLink {
                rel: LinkType::OnCancelled,
                href: redirect_url,
                method: Method::Get.to_string(),
            },
        ]);

        Ok(Self {
            merchant_ref_num: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            amount,
            settle_with_auth,
            payment_method,
            currency_code: currency,
            payment_type,
            transaction_type: TransactionType::Payment,
            return_links,
            account_id,
            three_ds: None, // No 3DS for PaymentMethodToken
            profile: None,
            billing_details,
        })
    }
}

// PaymentMethodToken (No-3DS) Flow - Response

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<PaysafePaymentMethodTokenResponse, Self>>
    for RouterDataV2<
        PaymentMethodToken,
        PaymentFlowData,
        PaymentMethodTokenizationData<T>,
        PaymentMethodTokenResponse,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PaysafePaymentMethodTokenResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = enums::AttemptStatus::try_from(item.response.status)?;

        let mut router_data = item.router_data;
        router_data.resource_common_data.status = status;

        // Return the payment_handle_token as the payment method token
        Ok(Self {
            response: Ok(PaymentMethodTokenResponse {
                token: item.response.payment_handle_token.peek().to_string(),
            }),
            ..router_data
        })
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PaysafeRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for PaysafePaymentsRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: PaysafeRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let amount = router_data.request.minor_amount;

        let auth = PaysafeAuthType::try_from(&item.router_data.connector_config)?;
        let account_id = auth
            .account_id
            .ok_or(IntegrationError::InvalidConnectorConfig {
                config: "account_id",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Paysafe Authorize needs the account_id map to resolve the card three_ds/no_three_ds processing account."
                            .to_string(),
                    ),
                    ..Default::default()
                },
            })?;

        // Prefer the PreAuthenticate handle from authentication_data (3DS); else the tokenize
        // PaymentMethodToken (no-3DS); else connector_feature_data (redirect APMs).
        let payment_handle_token: Secret<String> = if let Some(token) =
            paysafe_authentication_data_handle_token(
                router_data.request.authentication_data.as_ref(),
            ) {
            token
        } else {
            match &router_data.request.payment_method_data {
                PaymentMethodData::PaymentMethodToken(t) => t.token.clone(),
                _ => paysafe_feature_data_handle_token(&router_data.resource_common_data)
                    .ok_or(IntegrationError::MissingRequiredField {
                        field_name: "payment_method_token",
                        context: IntegrationErrorContext {
                            suggested_action: Some("Obtain a Paysafe payment_handle_token via PaymentMethodService.Tokenize before authorizing.".to_string()),
                            doc_url: Some("https://developer.paysafe.com/en/payments/payment-handles/create-payment-handle/".to_string()),
                            additional_context: Some("Paysafe requires a payment handle token. Pass it via PaymentMethodData::PaymentMethodToken, authentication_data (3DS), or connector_feature_data metadata.".to_string()),
                        },
                    })?,
            }
        };

        let customer_ip = router_data
            .request
            .get_browser_info()
            .ok()
            .and_then(|browser_info| browser_info.ip_address)
            .map(|ip| Secret::new(ip.to_string()));

        // Determine if this is an ACH payment based on payment_method
        let is_ach = matches!(
            router_data.resource_common_data.payment_method,
            enums::PaymentMethod::BankDebit
        );

        // For ACH payments, Paysafe requires settleWithAuth to be true
        let settle_with_auth = if is_ach {
            true
        } else {
            matches!(
                router_data.request.capture_method,
                Some(enums::CaptureMethod::Automatic) | None
            )
        };

        // Hyperswitch parity (verified via shadow-mode body comparison): only CARD
        // settles carry an accountId (three_ds/no_three_ds by auth type). Every other
        // payment method — wallets, ACH, redirect-APM settle legs — sends NO accountId:
        // the payment handle already carries its account binding, and re-specifying an
        // account (e.g. for an INTERAC_ETRANSFER handle) is rejected by Paysafe with
        // error 5068.
        // Match on the payment-method enum (not payment_method_data) because the settle
        // leg carries PaymentMethodData::PaymentMethodToken for cards too.
        let account_id = match router_data.resource_common_data.payment_method {
            enums::PaymentMethod::Card => {
                if router_data.resource_common_data.is_three_ds() {
                    Some(account_id.get_account_id(
                        PaysafeAccountKind::CardThreeDs,
                        router_data.request.currency,
                    )?)
                } else {
                    Some(account_id.get_account_id(
                        PaysafeAccountKind::CardNoThreeDs,
                        router_data.request.currency,
                    )?)
                }
            }
            _ => None,
        };

        Ok(Self {
            merchant_ref_num: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            payment_handle_token,
            amount,
            settle_with_auth,
            currency_code: router_data.request.currency,
            customer_ip,
            // CIT (first mandate payment): register the initial transaction of the
            // stored-credential series so the subsequent MIT — which references
            // initialTransactionId — is accepted/scored correctly by Paysafe.
            // Mirrors hyperswitch's storedCredential {type: ADHOC, occurrence: INITIAL}.
            // One-off (non-mandate) payments send no storedCredential.
            stored_credential: if router_data.request.is_customer_initiated_mandate_payment() {
                Some(PaysafeStoredCredential {
                    stored_credential_type: PaysafeStoredCredentialType::Adhoc,
                    occurrence: MandateOccurrence::Initial,
                    initial_transaction_id: None,
                })
            } else {
                None
            },
            account_id,
        })
    }
}

// Authorize Flow - Request (redirect-APM aware dispatch)

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PaysafeRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for PaysafeAuthorizeRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: PaysafeRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        if is_paysafe_handle_creation_leg(&item.router_data) {
            let handle_request = PaysafeSetupMandateRequest::try_from(&item.router_data)?;
            Ok(Self::PaymentHandle(Box::new(handle_request)))
        } else {
            let payments_request = PaysafePaymentsRequest::try_from(item)?;
            Ok(Self::Payment(Box::new(payments_request)))
        }
    }
}

// Authorize Flow - Response

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<PaysafeAuthorizeResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PaysafeAuthorizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let http_code = item.http_code;
        let capture_method = item.router_data.request.capture_method;
        let mut router_data = item.router_data;

        let response_data = match item.response {
            // Non-redirect settlement (v1/payments): card no-3DS, Apple Pay, or a token
            // settlement. Behaves exactly as before.
            PaysafeAuthorizeResponse::Payment(response) => {
                let status = get_paysafe_payment_status(response.status, capture_method);

                // Store payment_handle_token for mandate if present. Encode both the
                // reusable payment-handle token and the initial transaction id (Paysafe
                // payment `id`) into connector_mandate_id, because the gRPC recurring path
                // cannot carry mandate_metadata. The MIT RepeatPayment request decodes both
                // back out.
                let mandate_reference = response.payment_handle_token.as_ref().map(|token| {
                    let connector_mandate_id = serde_json::to_string(&PaysafeMandateReference {
                        payment_handle_token: token.peek().to_string(),
                        initial_transaction_id: response.id.clone(),
                    })
                    .unwrap_or_else(|_| token.peek().to_string());
                    MandateReference {
                        connector_mandate_id: Some(connector_mandate_id),
                        payment_method_id: None,
                        connector_mandate_request_reference_id: None,
                        mandate_metadata: None,
                    }
                });

                router_data.resource_common_data.status = status;

                PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(response.id.clone()),
                    redirection_data: None,
                    mandate_reference: mandate_reference.map(Box::new),
                    connector_metadata: None,
                    network_txn_id: None,
                    network_txn_link_id: None,
                    connector_response_reference_id: Some(response.merchant_ref_num),
                    incremental_authorization_allowed: None,
                    status_code: http_code,
                    splits: None,
                }
            }
            // Redirect APM (v1/paymenthandles): Skrill, Interac e-Transfer, paysafecard.
            // Paysafe returns the handle INITIATED + a customer redirect link; surface it
            // as redirection_data and keep the handle token in connector_metadata so the
            // follow-up v1/payments settlement can locate it after the redirect returns.
            PaysafeAuthorizeResponse::PaymentHandle(response) => {
                let status = enums::AttemptStatus::try_from(response.status)?;

                // Prefer a redirect-rel link; else the first (HS links.first()).
                let redirection_data = response
                    .links
                    .as_ref()
                    .and_then(|links| {
                        links
                            .iter()
                            .find(|link| link.rel.to_lowercase().contains("redirect"))
                            .or_else(|| links.first())
                    })
                    .map(|link| {
                        Box::new(RedirectForm::Form {
                            endpoint: link.href.clone(),
                            method: Method::Get,
                            form_fields: Default::default(),
                        })
                    });

                let connector_metadata = Some(serde_json::json!(PaysafeMeta {
                    payment_handle_token: response.payment_handle_token.clone(),
                }));

                router_data.resource_common_data.status = status;

                PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::NoResponseId,
                    redirection_data,
                    mandate_reference: None,
                    connector_metadata,
                    network_txn_id: None,
                    network_txn_link_id: None,
                    connector_response_reference_id: Some(response.merchant_ref_num),
                    incremental_authorization_allowed: None,
                    status_code: http_code,
                    splits: None,
                }
            }
        };

        Ok(Self {
            response: Ok(response_data),
            ..router_data
        })
    }
}

/// Carry the Paysafe `paymentHandleToken` to the settle Authorize inside
/// `AuthenticationData.threeds_server_transaction_id` — the only field HS forwards on the
/// complete-authorize path, and free to reuse since Paysafe has no real 3DS-server txn id.
fn paysafe_handle_token_authentication_data(
    payment_handle_token: &Secret<String>,
) -> domain_types::router_request_types::AuthenticationData {
    domain_types::router_request_types::AuthenticationData {
        threeds_server_transaction_id: Some(payment_handle_token.peek().to_string()),
        trans_status: None,
        eci: None,
        cavv: None,
        ucaf_collection_indicator: None,
        message_version: None,
        ds_trans_id: None,
        acs_transaction_id: None,
        transaction_id: None,
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

/// Read the Paysafe `paymentHandleToken` stashed in `AuthenticationData.threeds_server_transaction_id`.
pub(crate) fn paysafe_authentication_data_handle_token(
    authentication_data: Option<&domain_types::router_request_types::AuthenticationData>,
) -> Option<Secret<String>> {
    authentication_data
        .and_then(|data| data.threeds_server_transaction_id.clone())
        .map(Secret::new)
}

/// PreAuthenticate response: surface the ACS redirect link as `redirection_data` and thread the
/// `paymentHandleToken` forward via `authentication_data` for the settle Authorize.
impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<PaysafeAuthorizeResponse, Self>>
    for RouterDataV2<
        PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PaysafeAuthorizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let http_code = item.http_code;
        let mut router_data = item.router_data;

        let response = match item.response {
            PaysafeAuthorizeResponse::PaymentHandle(response) => response,
            // v1/paymenthandles always returns a handle body; a payment body here is unexpected.
            PaysafeAuthorizeResponse::Payment(_) => {
                return Err(ConnectorError::unexpected_response_error_with_context(
                    http_code,
                    Some("Paysafe PreAuthenticate expected a payment-handle response".to_string()),
                )
                .into())
            }
        };

        let status = enums::AttemptStatus::try_from(response.status)?;

        // Prefer a redirect-rel link; else the first (HS links.first()).
        let redirection_data = response
            .links
            .as_ref()
            .and_then(|links| {
                links
                    .iter()
                    .find(|link| link.rel.to_lowercase().contains("redirect"))
                    .or_else(|| links.first())
            })
            .map(|link| {
                Box::new(RedirectForm::Form {
                    endpoint: link.href.clone(),
                    method: Method::Get,
                    form_fields: Default::default(),
                })
            });

        let authentication_data = Some(paysafe_handle_token_authentication_data(
            &response.payment_handle_token,
        ));

        router_data.resource_common_data.status = status;

        let response_data = PaymentsResponseData::PreAuthenticateResponse {
            resource_id: Some(ResponseId::NoResponseId),
            authentication_data,
            redirection_data,
            connector_response_reference_id: Some(response.merchant_ref_num),
            status_code: http_code,
        };

        Ok(Self {
            response: Ok(response_data),
            ..router_data
        })
    }
}

/// Authenticate is a body-less `GET /v1/paymenthandles?merchantRefNum=`.
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PaysafeRouterData<
            RouterDataV2<
                Authenticate,
                PaymentFlowData,
                PaymentsAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for PaysafeAuthenticateRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        _item: PaysafeRouterData<
            RouterDataV2<
                Authenticate,
                PaymentFlowData,
                PaymentsAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {})
    }
}

/// Authenticate re-fetch: recover the (now PAYABLE) handle's `paymentHandleToken` and thread it
/// forward via `authentication_data` + `connector_feature_data`. Read-only; the main Authorize settles.
impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<PaysafeAuthenticateResponse, Self>>
    for RouterDataV2<
        Authenticate,
        PaymentFlowData,
        PaymentsAuthenticateData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PaysafeAuthenticateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let http_code = item.http_code;
        let mut router_data = item.router_data;

        let unexpected = || {
            ConnectorError::unexpected_response_error_with_context(
                http_code,
                Some(
                    "Paysafe Authenticate expected a payment-handle body for the merchantRefNum"
                        .to_string(),
                ),
            )
        };

        let (status, payment_handle_token, handle_id, merchant_ref_num) = match &item.response {
            PaysafeSyncResponse::SinglePaymentHandle(handle) => (
                handle.status,
                &handle.payment_handle_token,
                &handle.id,
                &handle.merchant_ref_num,
            ),
            PaysafeSyncResponse::PaymentHandle(sync_response) => {
                let handle = sync_response
                    .payment_handles
                    .first()
                    .ok_or_else(unexpected)?;
                (
                    handle.status,
                    &handle.payment_handle_token,
                    &handle.id,
                    &handle.merchant_ref_num,
                )
            }
            PaysafeSyncResponse::SinglePayment(_) | PaysafeSyncResponse::Payments(_) => {
                return Err(unexpected().into())
            }
        };

        let status = enums::AttemptStatus::try_from(status)?;
        let authentication_data = Some(paysafe_handle_token_authentication_data(
            payment_handle_token,
        ));
        let connector_feature_data = Some(serde_json::json!(PaysafeMeta {
            payment_handle_token: payment_handle_token.clone(),
        }));

        router_data.resource_common_data.status = status;

        let response_data = PaymentsResponseData::AuthenticateResponse {
            resource_id: Some(ResponseId::ConnectorTransactionId(handle_id.clone())),
            redirection_data: None,
            authentication_data,
            connector_feature_data,
            connector_response_reference_id: Some(merchant_ref_num.clone()),
            status_code: http_code,
        };

        Ok(Self {
            response: Ok(response_data),
            ..router_data
        })
    }
}

// RepeatPayment Flow - Request

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PaysafeRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for PaysafeRepeatPaymentRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: PaysafeRouterData<
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
        let amount = router_data.request.minor_amount;

        // Get mandate reference (carries the connector_mandate_id we issued at CIT time)
        let mandate_data = match &router_data.request.mandate_reference {
            MandateReferenceId::ConnectorMandateId(mandate_data) => mandate_data,
            MandateReferenceId::NetworkMandateId(_)
            | MandateReferenceId::NetworkTokenWithNTI(_) => {
                return Err(IntegrationError::MissingRequiredField {
                    field_name: "connector_mandate_id",
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "Paysafe MIT supports only connector mandates: pass the ConnectorMandateId issued by the CIT Authorize (network mandates are not supported)."
                                .to_string(),
                        ),
                        ..Default::default()
                    },
                }
                .into());
            }
        };

        let raw_connector_mandate_id = mandate_data.get_connector_mandate_id().ok_or(
            IntegrationError::MissingRequiredField {
                field_name: "connector_mandate_id",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Paysafe MIT requires the connector_mandate_id JSON ({payment_handle_token, initial_transaction_id}) returned by the CIT Authorize."
                            .to_string(),
                    ),
                    ..Default::default()
                },
            },
        )?;

        // Decode the connector_mandate_id. The CIT Authorize response encodes both the
        // reusable payment-handle token and the initial transaction id as JSON (because
        // the gRPC recurring path cannot carry mandate_metadata). For backward
        // compatibility, a bare (non-JSON) value is treated as the payment-handle token
        // and the initial transaction id is sourced from mandate_metadata instead.
        let (payment_handle_token, initial_transaction_id): (Secret<String>, String) =
            match serde_json::from_str::<PaysafeMandateReference>(&raw_connector_mandate_id) {
                Ok(decoded) => (
                    Secret::new(decoded.payment_handle_token),
                    decoded.initial_transaction_id,
                ),
                Err(_) => {
                    let mandate_metadata: PaysafeMandateMetadata = mandate_data
                        .get_mandate_metadata()
                        .ok_or(IntegrationError::MissingRequiredField {
                            field_name: "mandate_metadata",
                            context: IntegrationErrorContext {
                                additional_context: Some(
                                    "Bare (non-JSON) connector_mandate_id needs mandate_metadata carrying the initial_transaction_id for the Paysafe MIT."
                                        .to_string(),
                                ),
                                ..Default::default()
                            },
                        })?
                        .parse_value("PaysafeMandateMetadata")
                        .change_context(IntegrationError::RequestEncodingFailed {
                            context: IntegrationErrorContext {
                                additional_context: Some(
                                    "mandate_metadata did not parse as PaysafeMandateMetadata ({initial_transaction_id})."
                                        .to_string(),
                                ),
                                ..Default::default()
                            },
                        })?;
                    (
                        Secret::new(raw_connector_mandate_id),
                        mandate_metadata.initial_transaction_id,
                    )
                }
            };

        let customer_ip = router_data
            .request
            .browser_info
            .as_ref()
            .and_then(|browser_info| browser_info.ip_address.as_ref())
            .map(|ip| Secret::new(ip.to_string()));

        let settle_with_auth = matches!(
            router_data.request.capture_method,
            Some(enums::CaptureMethod::Automatic) | None
        );

        // MIT (Merchant Initiated Transaction) stored credential
        let stored_credential = Some(PaysafeStoredCredential {
            stored_credential_type: PaysafeStoredCredentialType::Topup,
            occurrence: MandateOccurrence::Subsequent,
            initial_transaction_id: Some(initial_transaction_id),
        });

        // Paysafe requires the processing accountId on the MIT settlement, just as
        // on the CIT. The reusable handle was vaulted under the card account, so the
        // MIT replays against the card no_three_ds account (MITs are never 3DS).
        // Mirrors hyperswitch, which sends the no_three_ds card account for
        // PaymentMethodData::MandatePayment.
        let auth = PaysafeAuthType::try_from(&router_data.connector_config)?;
        let account_id = auth
            .account_id
            .ok_or(IntegrationError::InvalidConnectorConfig {
                config: "account_id",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "Paysafe MIT needs the account_id map to resolve the card no_three_ds account the reusable handle was vaulted under."
                            .to_string(),
                    ),
                    ..Default::default()
                },
            })?
            .get_account_id(PaysafeAccountKind::CardNoThreeDs, router_data.request.currency)?;

        Ok(Self {
            merchant_ref_num: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            payment_handle_token,
            amount,
            settle_with_auth,
            currency_code: router_data.request.currency,
            customer_ip,
            stored_credential,
            account_id: Some(account_id),
        })
    }
}

// RepeatPayment Flow - Response

impl<
        T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize + Serialize,
    > TryFrom<ResponseRouterData<PaysafeRepeatPaymentResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PaysafeRepeatPaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = get_paysafe_payment_status(
            item.response.status,
            item.router_data.request.capture_method,
        );

        let mut router_data = item.router_data;
        router_data.resource_common_data.status = status;

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: Some(item.response.merchant_ref_num),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
            }),
            ..router_data
        })
    }
}

// PSync Flow - Response

impl TryFrom<ResponseRouterData<PaysafeSyncResponse, Self>>
    for RouterDataV2<
        domain_types::connector_flow::PSync,
        PaymentFlowData,
        domain_types::connector_types::PaymentsSyncData,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<PaysafeSyncResponse, Self>) -> Result<Self, Self::Error> {
        let (status, connector_transaction_id) = match &item.response {
            PaysafeSyncResponse::SinglePayment(payment_response) => {
                let status = get_paysafe_payment_status(
                    payment_response.status,
                    item.router_data.request.capture_method,
                );
                (status, Some(payment_response.id.clone()))
            }
            PaysafeSyncResponse::Payments(sync_response) => {
                let payment_response = sync_response.payments.first().ok_or_else(|| {
                    error_stack::Report::from(
                        crate::utils::response_deserialization_fail(
                            item.http_code,
                        "paysafe: response body did not match the expected format; confirm API version and connector documentation."),
                    )
                })?;
                let status = get_paysafe_payment_status(
                    payment_response.status,
                    item.router_data.request.capture_method,
                );
                (status, Some(payment_response.id.clone()))
            }
            PaysafeSyncResponse::SinglePaymentHandle(payment_handle_response) => {
                let status = enums::AttemptStatus::try_from(payment_handle_response.status)?;
                (status, Some(payment_handle_response.id.clone()))
            }
            PaysafeSyncResponse::PaymentHandle(sync_response) => {
                let payment_handle_response =
                    sync_response.payment_handles.first().ok_or_else(|| {
                        error_stack::Report::from(
                            crate::utils::response_deserialization_fail(
                                item.http_code,
                            "paysafe: response body did not match the expected format; confirm API version and connector documentation."),
                        )
                    })?;
                let status = enums::AttemptStatus::try_from(payment_handle_response.status)?;
                (status, Some(payment_handle_response.id.clone()))
            }
        };

        let mut router_data = item.router_data;
        router_data.resource_common_data.status = status;

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: connector_transaction_id
                    .map(ResponseId::ConnectorTransactionId)
                    .unwrap_or(ResponseId::NoResponseId),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
            }),
            ..router_data
        })
    }
}

// Capture Flow - Request

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PaysafeRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for PaysafeCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: PaysafeRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            merchant_ref_num: item
                .router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            amount: item.router_data.request.minor_amount_to_capture,
        })
    }
}

// Capture Flow - Response

impl TryFrom<ResponseRouterData<PaysafeCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PaysafeCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = enums::AttemptStatus::from(item.response.status);

        let mut router_data = item.router_data;
        router_data.resource_common_data.status = status;

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: Some(item.response.merchant_ref_num),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
            }),
            ..router_data
        })
    }
}

// Void Flow - Request

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PaysafeRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for PaysafeVoidRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: PaysafeRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let amount =
            item.router_data
                .request
                .amount
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "amount",
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "Paysafe refunds require an explicit amount; partial/full refund amount cannot be defaulted."
                                .to_string(),
                        ),
                        ..Default::default()
                    },
                })?;
        Ok(Self {
            merchant_ref_num: item
                .router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            amount,
        })
    }
}

// Void Flow - Response

impl TryFrom<ResponseRouterData<PaysafeVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<PaysafeVoidResponse, Self>) -> Result<Self, Self::Error> {
        let status = enums::AttemptStatus::from(item.response.status);

        let mut router_data = item.router_data;
        router_data.resource_common_data.status = status;

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::NoResponseId,
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: Some(item.response.merchant_ref_num),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
            }),
            ..router_data
        })
    }
}

// Refund Flow - Request

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        PaysafeRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for PaysafeRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: PaysafeRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            merchant_ref_num: item.router_data.request.refund_id.clone(),
            amount: item.router_data.request.minor_refund_amount,
        })
    }
}

// Refund Flow - Response

impl TryFrom<ResponseRouterData<PaysafeRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<PaysafeRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.id.clone(),
                refund_status: enums::RefundStatus::from(item.response.status),
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}

// RSync Flow - Response

impl TryFrom<ResponseRouterData<PaysafeRSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(item: ResponseRouterData<PaysafeRSyncResponse, Self>) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.id.clone(),
                refund_status: enums::RefundStatus::from(item.response.status),
                status_code: item.http_code,
            }),
            ..item.router_data
        })
    }
}
