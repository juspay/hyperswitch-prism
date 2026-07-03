use common_enums::enums;
use common_utils::{ext_traits::ValueExt, request::Method};
use domain_types::{
    connector_flow::{
        Authorize, Capture, CreateConnectorCustomer, PaymentMethodToken, RSync, Refund,
        RepeatPayment, Void,
    },
    connector_types::{
        ConnectorCustomerData, ConnectorCustomerResponse, MandateReference, MandateReferenceId,
        PaymentFlowData, PaymentMethodTokenResponse, PaymentMethodTokenizationData, PaymentVoidData,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData, RefundFlowData,
        RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData, ResponseId,
    },
    payment_method_data::{
        ApplePayPaymentData, BankDebitData, BankRedirectData, GiftCardData, GpayTokenizationData,
        PaymentMethodData, PaymentMethodDataTypes, WalletData,
    },
    router_data::{ConnectorSpecificConfig, PaysafePaymentMethodDetails},
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
                context: Default::default(),
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
    let non_empty =
        |value: Option<Secret<String>>| value.filter(|v| !v.peek().trim().is_empty());
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

/// Whether this Authorize invocation is the SECOND leg of a redirect-APM payment:
/// the shopper has returned from the Paysafe hosted redirect (`redirect_response`
/// present) and/or the caller echoed back the leg-1 payment-handle token via
/// `connector_feature_data`. The settle leg POSTs `v1/payments` with the payable
/// handle token — mirrors hyperswitch's CompleteAuthorize flow. Shared by the
/// Authorize URL selector and the Authorize request builder so both agree.
pub(crate) fn is_paysafe_apm_settle_leg<
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

/// Build a Paysafe payment-handle body for a redirect APM (Skrill, Interac
/// e-Transfer, paysafecard) directly from the Authorize request. Mirrors the
/// payment-handle body created by the PaymentMethodToken flow, but sourced from
/// `PaymentsAuthorizeData` so the Authorize response can return the redirect link
/// (matching hyperswitch's single Authorize -> paymenthandles behaviour).
fn build_paysafe_redirect_handle_request<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    router_data: &RouterDataV2<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<T>,
        PaymentsResponseData,
    >,
) -> Result<PaysafeSetupMandateRequest<T>, error_stack::Report<IntegrationError>> {
    let auth = PaysafeAuthType::try_from(&router_data.connector_config)?;
    let account_id = auth
        .account_id
        .ok_or(IntegrationError::InvalidConnectorConfig {
            config: "account_id",
            context: Default::default(),
        })?;

    let currency = router_data.request.currency;
    let amount = router_data.request.minor_amount;

    let (payment_method, payment_type, account_id, profile) = match &router_data
        .request
        .payment_method_data
    {
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
                    context: Default::default(),
                })?;
            let skrill_account_id = account_id.get_skrill_account_id(currency)?;
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
                    context: Default::default(),
                })?;
            // Interac REQUIRES an accountId for CAD (unlike Skrill).
            let account_id = account_id.get_interac_account_id(currency)?;
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
                        context: Default::default(),
                    })?,
                last_name: router_data
                    .resource_common_data
                    .get_optional_billing_last_name()
                    .ok_or(IntegrationError::MissingRequiredField {
                        field_name: "billing_last_name",
                        context: Default::default(),
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
            )
        }
        _ => {
            return Err(IntegrationError::NotImplemented(
                "Only Skrill, Interac e-Transfer, and paysafecard are supported as redirect payment methods for the Paysafe Authorize flow".to_string(),
                Default::default(),
            )
            .into())
        }
    };

    let billing_details = create_paysafe_billing_details(&router_data.resource_common_data)?;

    // Paysafe requires return_links to build the customer redirect.
    let redirect_url = router_data.resource_common_data.get_return_url().ok_or(
        IntegrationError::MissingRequiredField {
            field_name: "return_url",
            context: Default::default(),
        },
    )?;

    // On successful redirect completion Paysafe must send the customer to the
    // complete_authorize_url so hyperswitch runs CompleteAuthorize (settling the
    // payment handle into a payment). Routing on_completed to the plain return_url
    // only triggers a PSync, which finds no settled payment yet and fails. Falls
    // back to the return_url when complete_authorize_url is absent.
    let complete_authorize_url = router_data
        .request
        .complete_authorize_url
        .clone()
        .unwrap_or_else(|| redirect_url.clone());

    let return_links = Some(vec![
        ReturnLink {
            rel: LinkType::Default,
            href: redirect_url.clone(),
            method: Method::Get.to_string(),
        },
        ReturnLink {
            rel: LinkType::OnCompleted,
            href: complete_authorize_url,
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

    Ok(PaysafeSetupMandateRequest {
        merchant_ref_num: router_data
            .resource_common_data
            .connector_request_reference_id
            .clone(),
        amount,
        // Redirect APMs omit settleWithAuth on the payment-handle body.
        settle_with_auth: None,
        payment_method,
        currency_code: currency,
        payment_type,
        transaction_type: TransactionType::Payment,
        return_links,
        account_id,
        three_ds: None,
        profile,
        billing_details,
    })
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
                context: Default::default(),
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
                context: Default::default(),
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
                    let account_id = account_id.get_no_three_ds_account_id(currency)?;
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
                            context: Default::default(),
                        })?;
                    let account_type = bank_type
                        .as_ref()
                        .map(PaysafeAchAccountType::try_from)
                        .transpose()?
                        .ok_or(IntegrationError::MissingRequiredField {
                            field_name: "bank_type (ach.accountType)",
                            context: Default::default(),
                        })?;
                    let ach = PaysafeAch {
                        account_holder_name,
                        account_number: account_number.clone(),
                        routing_number: routing_number.clone(),
                        account_type,
                    };
                    let account_id = account_id.get_ach_account_id(currency)?;
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
                                context: Default::default(),
                            }
                            .into())
                        }
                    };

                    let expiration_month = decrypted_data
                        .get_expiry_month()
                        .change_context(IntegrationError::MissingRequiredField {
                            field_name: "google_pay_decrypted_data.card_exp_month",
                            context: Default::default(),
                        })?
                        .peek()
                        .parse::<u8>()
                        .map_err(|_| {
                            IntegrationError::InvalidDataFormat {
                                field_name: "google_pay_decrypted_data.card_exp_month",
                                context: Default::default(),
                            }
                        })?;

                    let expiration_year = decrypted_data
                        .get_four_digit_expiry_year()
                        .change_context(IntegrationError::MissingRequiredField {
                            field_name: "google_pay_decrypted_data.card_exp_year",
                            context: Default::default(),
                        })?
                        .peek()
                        .parse::<u16>()
                        .map_err(|_| {
                            IntegrationError::InvalidDataFormat {
                                field_name: "google_pay_decrypted_data.card_exp_year",
                                context: Default::default(),
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
                        message_expiration: "9999999999999".to_string(),
                        payment_method_details,
                    };

                    let google_pay_payment_token = PaysafeGooglePayPaymentToken {
                        api_version: 2,
                        api_version_minor: 0,
                        payment_method_data: PaysafeGooglePayPaymentMethodData {
                            pm_type: "CARD".to_string(),
                            description: google_pay_data.description.clone(),
                            info: PaysafeGooglePayCardInfo {
                                card_network: google_pay_data.info.card_network.clone(),
                                card_details: google_pay_data.info.card_details.clone(),
                            },
                            tokenization_data: PaysafeGooglePayTokenizationData {
                                token_type: "PAYMENT_GATEWAY".to_string(),
                                decrypted_token,
                            },
                        },
                    };

                    let account_id = account_id.get_no_three_ds_account_id(currency)?;
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
                                    context: Default::default(),
                                })?;
                            PaysafeApplePayPaymentData::Encrypted(
                                serde_json::from_str(decoded_token.peek()).change_context(
                                    IntegrationError::InvalidDataFormat {
                                        field_name: "apple_pay.payment_data",
                                        context: Default::default(),
                                    },
                                )?,
                            )
                        }
                        ApplePayPaymentData::Decrypted(decrypted) => {
                            let expiry_year = decrypted
                                .get_two_digit_expiry_year()
                                .change_context(IntegrationError::InvalidDataFormat {
                                    field_name: "apple_pay.application_expiration_year",
                                    context: Default::default(),
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
                                        currency_code: currency.to_string(),
                                        transaction_amount: Some(amount),
                                        cardholder_name: None,
                                        device_manufacturer_identifier: Some(
                                            "Apple".to_string(),
                                        ),
                                        payment_data_type: Some("3DSecure".to_string()),
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
                        label: apple_pay_data.payment_method.display_name.clone(),
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
                    let is_encrypted = matches!(
                        &apple_pay_data.payment_data,
                        ApplePayPaymentData::Encrypted(_)
                    );
                    let account_id = account_id
                        .get_apple_pay_account_id(currency, is_encrypted)
                        .or_else(|_| account_id.get_no_three_ds_account_id(currency))?;
                    (
                        PaysafePaymentMethod::ApplePay { apple_pay },
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
                            context: Default::default(),
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
                    let skrill_account_id = account_id.get_skrill_account_id(currency)?;
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
                            context: Default::default(),
                        })?;
                    let interac_etransfer = PaysafeInterac { consumer_id };
                    // Interac REQUIRES an accountId for CAD (unlike Skrill). Resolve from the
                    // interac CAD metadata slot; gracefully errors if unprovisioned.
                    let account_id = account_id.get_interac_account_id(currency)?;
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
                context: Default::default(),
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
                context: Default::default(),
            })?;

        let payment_handle_token: Secret<String> = match &router_data.request.payment_method_data {
            PaymentMethodData::PaymentMethodToken(t) => t.token.clone(),
            _ => paysafe_feature_data_handle_token(&router_data.resource_common_data)
                .ok_or(IntegrationError::MissingRequiredField {
                    field_name: "payment_method_token",
                    context: IntegrationErrorContext {
                        suggested_action: Some("Obtain a Paysafe payment_handle_token via PaymentMethodService.Tokenize before authorizing.".to_string()),
                        doc_url: Some("https://developer.paysafe.com/en/payments/payment-handles/create-payment-handle/".to_string()),
                        additional_context: Some("Paysafe requires a payment handle token. Pass it via PaymentMethodData::PaymentMethodToken or connector_feature_data metadata.".to_string()),
                    },
                })?,
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

        // For ACH, use the ach account_id; for wallets (GooglePay), always use no_three_ds
        // because the PaymentMethodToken (payment handle) was created under the no_three_ds
        // account. For cards, branch on is_three_ds(). Redirect-APM settles (the second
        // Authorize leg, mirroring hyperswitch's CompleteAuthorize) send NO accountId:
        // the payment handle already carries its account binding, and re-specifying a
        // card/ach account for e.g. an INTERAC_ETRANSFER handle is rejected by Paysafe
        // with error 5068.
        let is_wallet = matches!(
            router_data.resource_common_data.payment_method,
            enums::PaymentMethod::Wallet
        );

        let account_id = match (
            is_paysafe_redirect_apm(&router_data.request.payment_method_data),
            is_ach,
            is_wallet || !router_data.resource_common_data.is_three_ds(),
        ) {
            (true, _, _) => None,
            (false, true, _) => Some(account_id.get_ach_account_id(router_data.request.currency)?),
            (false, false, true) => {
                // Wallets (GooglePay) always use no_three_ds account because the payment
                // handle (created in PaymentMethodToken flow) uses no_three_ds account.
                // Non-3DS card payments also use no_three_ds.
                Some(account_id.get_no_three_ds_account_id(router_data.request.currency)?)
            }
            (false, false, false) => {
                Some(account_id.get_three_ds_account_id(router_data.request.currency)?)
            }
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
        // Redirect APMs (Skrill, Interac e-Transfer, paysafecard) create a payment
        // handle on the FIRST Authorize leg (mirrors hyperswitch): they carry no
        // pre-created token and must surface a customer redirect. On the SECOND leg
        // (shopper returned from the redirect / handle token echoed back) and for
        // every other payment method, settle the handle token via v1/payments —
        // mirrors hyperswitch's CompleteAuthorize.
        match (
            is_paysafe_redirect_apm(&item.router_data.request.payment_method_data),
            is_paysafe_apm_settle_leg(&item.router_data),
        ) {
            (true, false) => {
                let handle_request = build_paysafe_redirect_handle_request(&item.router_data)?;
                Ok(Self::PaymentHandle(Box::new(handle_request)))
            }
            _ => {
                let payments_request = PaysafePaymentsRequest::try_from(item)?;
                Ok(Self::Payment(Box::new(payments_request)))
            }
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

                // Prefer a customer-facing redirect link (rel contains "redirect"); fall
                // back to the first link, matching hyperswitch's links.first() behaviour.
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
                    context: Default::default(),
                }
                .into());
            }
        };

        let raw_connector_mandate_id = mandate_data.get_connector_mandate_id().ok_or(
            IntegrationError::MissingRequiredField {
                field_name: "connector_mandate_id",
                context: Default::default(),
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
                            context: Default::default(),
                        })?
                        .parse_value("PaysafeMandateMetadata")
                        .change_context(IntegrationError::RequestEncodingFailed {
                            context: Default::default(),
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
                context: Default::default(),
            })?
            .get_no_three_ds_account_id(router_data.request.currency)?;

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
                    context: Default::default(),
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
