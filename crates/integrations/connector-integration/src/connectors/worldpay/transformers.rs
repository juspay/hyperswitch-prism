use std::collections::HashMap;

use common_enums as enums;
use common_utils::{ext_traits::OptionExt, pii, types::MinorUnit, CustomResult};
use domain_types::{
    connector_flow::{Authorize, Capture, IncrementalAuthorization, Void},
    connector_types::{
        MandateIds, MandateReference, MandateReferenceId, PaymentFlowData, PaymentVoidData,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsIncrementalAuthorizationData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, RepeatPaymentData, ResponseId,
    },
    errors::{ConnectorError, IntegrationError},
    payment_method_data::{
        PaymentMethodData, PaymentMethodDataTypes, RawCardNumber,
        WalletData as WalletDataPaymentMethod,
    },
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
    utils,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::{connectors::worldpay::WorldpayRouterData, types::ResponseRouterData};

// Define ForeignTryFrom trait locally
pub trait ForeignTryFrom<T>: Sized {
    type Error;
    fn foreign_try_from(value: T) -> Result<Self, Self::Error>;
}

use super::requests::*;
use super::response::*;

// Form field keys
const FORM_FIELD_JWT: &str = "JWT";
const FORM_FIELD_BIN: &str = "Bin";

// Metadata keys
const METADATA_LINK_DATA: &str = "link_data";
const METADATA_3DS_STAGE: &str = "3ds_stage";
const METADATA_3DS_VERSION: &str = "3ds_version";
const METADATA_ECI: &str = "eci";
const METADATA_AUTH_APPLIED: &str = "authentication_applied";
const METADATA_DDC_REFERENCE: &str = "device_data_collection";

// 3DS stage values
const STAGE_DDC: &str = "ddc";
const STAGE_CHALLENGE: &str = "challenge";

// HAL link relation for the incremental-authorization action exposed by the
// Access Worldpay Card Payments API. The trailing segment of the link's href
// is the linkData used as `connector_authorization_id` for subsequent calls.
const LINK_KEY_INCREASE_AUTHORIZED_AMOUNT: &str = "cardPayments:increaseAuthorizedAmount";

/// Metadata object extracted from connector_feature_data
/// Contains Worldpay-specific merchant configuration
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct WorldpayConnectorMetadataObject {
    pub merchant_name: Option<Secret<String>>,
}

impl TryFrom<Option<&pii::SecretSerdeValue>> for WorldpayConnectorMetadataObject {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(meta_data: Option<&pii::SecretSerdeValue>) -> Result<Self, Self::Error> {
        let metadata: Self =
            crate::utils::to_connector_meta_from_secret::<Self>(meta_data.cloned())
                .change_context(IntegrationError::InvalidConnectorConfig {
                    config: "connector_feature_data",
                    context: Default::default(),
                })?;
        Ok(metadata)
    }
}

fn fetch_payment_instrument<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    payment_method: PaymentMethodData<T>,
    billing_address: Option<&domain_types::payment_address::Address>,
) -> CustomResult<PaymentInstrument<T>, IntegrationError> {
    match payment_method {
        PaymentMethodData::Card(card) => {
            // Extract expiry month and year using helper functions
            let expiry_month_i8 = card.get_expiry_month_as_i8()?;
            let expiry_year_4_digit = card.get_expiry_year_4_digit();
            let expiry_year: i32 = expiry_year_4_digit
                .peek()
                .parse::<i32>()
                .change_context(IntegrationError::RequestEncodingFailed {
                    context: Default::default(),
                })?;

            Ok(PaymentInstrument::Card(CardPayment {
                raw_card_details: RawCardDetails {
                    payment_type: PaymentType::Plain,
                    expiry_date: ExpiryDate {
                        month: expiry_month_i8,
                        year: Secret::new(expiry_year)
},
                    card_number: card.card_number
},
                cvc: card.card_cvc,
                card_holder_name: billing_address
                    .and_then(|address| address.get_optional_full_name()),
                billing_address: billing_address
                    .and_then(|addr| addr.address.clone())
                    .and_then(|address| {
                        match (address.line1, address.city, address.zip, address.country) {
                            (Some(address1), Some(city), Some(postal_code), Some(country_code)) => {
                                Some(BillingAddress {
                                    address1,
                                    address2: address.line2,
                                    address3: address.line3,
                                    city,
                                    state: address.state,
                                    postal_code,
                                    country_code
})
                            }
                            _ => None
}
                    })
}))
        }
        PaymentMethodData::CardDetailsForNetworkTransactionId(raw_card_details) => {
            // Extract expiry month and year using helper functions
            let expiry_month_i8 = raw_card_details.get_expiry_month_as_i8()?;
            let expiry_year_4_digit = raw_card_details.get_expiry_year_4_digit();
            let expiry_year: i32 = expiry_year_4_digit
                .peek()
                .parse::<i32>()
                .change_context(IntegrationError::RequestEncodingFailed {
                    context: Default::default(),
                })?;

            Ok(PaymentInstrument::RawCardForNTI(RawCardDetails {
                payment_type: PaymentType::Plain,
                expiry_date: ExpiryDate {
                    month: expiry_month_i8,
                    year: Secret::new(expiry_year)
},
                card_number: RawCardNumber(raw_card_details.card_number)
}))
        }
        PaymentMethodData::MandatePayment => {
            Err(IntegrationError::not_implemented(
                "MandatePayment should not be used in Authorize flow - use RepeatPayment flow for MIT transactions".to_string()
            ).into())
        }
        PaymentMethodData::Wallet(wallet) => match wallet {
            WalletDataPaymentMethod::GooglePay(data) => {
                Ok(PaymentInstrument::Googlepay(WalletPayment {
                    payment_type: PaymentType::Encrypted,
                    wallet_token: Secret::new(
                        data.tokenization_data
                            .get_encrypted_google_pay_token()
                            .change_context(IntegrationError::MissingRequiredField {
                                field_name: "gpay wallet_token",
                context: Default::default()
                            })?,
                    ),
                    ..WalletPayment::default()
                }))
            }
            WalletDataPaymentMethod::ApplePay(data) => {
                Ok(PaymentInstrument::Applepay(WalletPayment {
                    payment_type: PaymentType::Encrypted,
                    wallet_token: data.get_applepay_decoded_payment_data()?,
                    ..WalletPayment::default()
                }))
            }
            WalletDataPaymentMethod::AliPayQr(_)
            | WalletDataPaymentMethod::AliPayRedirect(_)
            | WalletDataPaymentMethod::AliPayHkRedirect(_)
            | WalletDataPaymentMethod::AmazonPayRedirect(_)
            | WalletDataPaymentMethod::MomoRedirect(_)
            | WalletDataPaymentMethod::KakaoPayRedirect(_)
            | WalletDataPaymentMethod::GoPayRedirect(_)
            | WalletDataPaymentMethod::GcashRedirect(_)
            | WalletDataPaymentMethod::ApplePayRedirect(_)
            | WalletDataPaymentMethod::ApplePayThirdPartySdk(_)
            | WalletDataPaymentMethod::DanaRedirect {}
            | WalletDataPaymentMethod::GooglePayRedirect(_)
            | WalletDataPaymentMethod::GooglePayThirdPartySdk(_)
            | WalletDataPaymentMethod::MbWayRedirect(_)
            | WalletDataPaymentMethod::MobilePayRedirect(_)
            | WalletDataPaymentMethod::PaypalRedirect(_)
            | WalletDataPaymentMethod::PaypalSdk(_)
            | WalletDataPaymentMethod::Paze(_)
            | WalletDataPaymentMethod::SamsungPay(_)
            | WalletDataPaymentMethod::TwintRedirect {}
            | WalletDataPaymentMethod::VippsRedirect {}
            | WalletDataPaymentMethod::TouchNGoRedirect(_)
            | WalletDataPaymentMethod::WeChatPayRedirect(_)
            | WalletDataPaymentMethod::CashappQr(_)
            | WalletDataPaymentMethod::SwishQr(_)
            | WalletDataPaymentMethod::WeChatPayQr(_)
            | WalletDataPaymentMethod::Mifinity(_)
            | WalletDataPaymentMethod::RevolutPay(_)
            | WalletDataPaymentMethod::BluecodeRedirect {}
            | WalletDataPaymentMethod::MbWay(_)
            | WalletDataPaymentMethod::Satispay(_)
            | WalletDataPaymentMethod::Wero(_)
            | WalletDataPaymentMethod::LazyPayRedirect(_)
            | WalletDataPaymentMethod::PhonePeRedirect(_)
            | WalletDataPaymentMethod::BillDeskRedirect(_)
            | WalletDataPaymentMethod::CashfreeRedirect(_)
            | WalletDataPaymentMethod::PayURedirect(_)
            | WalletDataPaymentMethod::EaseBuzzRedirect(_) => {
                Err(IntegrationError::not_implemented(
                    utils::get_unimplemented_payment_method_error_message("worldpay"),
                )
                .into())
            }
        },
        PaymentMethodData::PayLater(_)
        | PaymentMethodData::BankRedirect(_)
        | PaymentMethodData::BankDebit(_)
        | PaymentMethodData::BankTransfer(_)
        | PaymentMethodData::Crypto(_)
        | PaymentMethodData::Reward
        | PaymentMethodData::RealTimePayment(_)
        | PaymentMethodData::MobilePayment(_)
        | PaymentMethodData::Upi(_)
        | PaymentMethodData::Voucher(_)
        | PaymentMethodData::CardRedirect(_)
        | PaymentMethodData::GiftCard(_)
        | PaymentMethodData::OpenBanking(_)
        | PaymentMethodData::PaymentMethodToken(_)
        | PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)
        | PaymentMethodData::NetworkToken(_) => Err(IntegrationError::not_implemented(
            utils::get_unimplemented_payment_method_error_message("worldpay"),
        )
        .into())
}
}

impl TryFrom<(enums::PaymentMethod, Option<enums::PaymentMethodType>)> for PaymentMethod {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        src: (enums::PaymentMethod, Option<enums::PaymentMethodType>),
    ) -> Result<Self, Self::Error> {
        match (src.0, src.1) {
            (enums::PaymentMethod::Card, _) => Ok(Self::Card),
            (enums::PaymentMethod::Wallet, pmt) => {
                let pm = pmt.ok_or(IntegrationError::MissingRequiredField {
                    field_name: "payment_method_type",
                    context: Default::default(),
                })?;
                match pm {
                    enums::PaymentMethodType::ApplePay => Ok(Self::ApplePay),
                    enums::PaymentMethodType::GooglePay => Ok(Self::GooglePay),
                    _ => Err(IntegrationError::not_implemented(
                        utils::get_unimplemented_payment_method_error_message("worldpay"),
                    )
                    .into()),
                }
            }
            _ => Err(IntegrationError::not_implemented(
                utils::get_unimplemented_payment_method_error_message("worldpay"),
            )
            .into()),
        }
    }
}

// Helper function to create ThreeDS request for RouterDataV2
fn create_three_ds_request<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    router_data: &RouterDataV2<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<T>,
        PaymentsResponseData,
    >,
    is_mandate_payment: bool,
) -> Result<Option<ThreeDSRequest>, error_stack::Report<IntegrationError>> {
    match (
        &router_data.resource_common_data.auth_type,
        &router_data.request.payment_method_data,
    ) {
        // 3DS for NTI flow
        (_, PaymentMethodData::CardDetailsForNetworkTransactionId(_)) => Ok(None),
        // 3DS for regular payments
        (enums::AuthenticationType::ThreeDs, _) => {
            let browser_info = router_data.request.browser_info.as_ref().ok_or(
                IntegrationError::MissingRequiredField {
                    field_name: "browser_info",
                    context: Default::default(),
                },
            )?;

            let accept_header = browser_info
                .accept_header
                .clone()
                .get_required_value("accept_header")
                .change_context(IntegrationError::MissingRequiredField {
                    field_name: "accept_header",
                    context: Default::default(),
                })?;

            let user_agent_header = browser_info
                .user_agent
                .clone()
                .get_required_value("user_agent")
                .change_context(IntegrationError::MissingRequiredField {
                    field_name: "user_agent",
                    context: Default::default(),
                })?;

            let channel = Some(ThreeDSRequestChannel::Browser);

            Ok(Some(ThreeDSRequest {
                three_ds_type: THREE_DS_TYPE.to_string(),
                mode: THREE_DS_MODE.to_string(),
                device_data: ThreeDSRequestDeviceData {
                    accept_header,
                    user_agent_header,
                    browser_language: browser_info.language.clone(),
                    browser_screen_width: browser_info.screen_width,
                    browser_screen_height: browser_info.screen_height,
                    browser_color_depth: browser_info.color_depth.map(|depth| depth.to_string()),
                    time_zone: browser_info.time_zone.map(|tz| tz.to_string()),
                    browser_java_enabled: browser_info.java_enabled,
                    browser_javascript_enabled: browser_info.java_script_enabled,
                    channel,
                },
                challenge: ThreeDSRequestChallenge {
                    return_url: router_data.request.get_complete_authorize_url()?,
                    preference: if is_mandate_payment {
                        Some(THREE_DS_CHALLENGE_PREFERENCE.to_string())
                    } else {
                        None
                    },
                },
            }))
        }
        // Non 3DS
        _ => Ok(None),
    }
}

// Helper function to get settlement info for RouterDataV2
fn get_settlement_info<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    router_data: &RouterDataV2<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<T>,
        PaymentsResponseData,
    >,
    amount: MinorUnit,
) -> Option<AutoSettlement> {
    match router_data.request.capture_method.unwrap_or_default() {
        _ if amount == MinorUnit::zero() => None,
        enums::CaptureMethod::Automatic | enums::CaptureMethod::SequentialAutomatic => {
            Some(AutoSettlement { auto: true })
        }
        enums::CaptureMethod::Manual | enums::CaptureMethod::ManualMultiple => {
            Some(AutoSettlement { auto: false })
        }
        _ => None,
    }
}

// Dangling helper function to determine token and agreement settings
fn get_token_and_agreement<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    payment_method_data: &PaymentMethodData<T>,
    setup_future_usage: Option<enums::FutureUsage>,
    off_session: Option<bool>,
    mandate_ids: Option<MandateIds>,
) -> (Option<TokenCreation>, Option<CustomerAgreement>) {
    match (payment_method_data, setup_future_usage, off_session) {
        // CIT - Setup for future usage (creates token for future MIT via RepeatPayment)
        (PaymentMethodData::Card(_), Some(enums::FutureUsage::OffSession), _) => (
            Some(TokenCreation {
                token_type: TokenCreationType::Worldpay,
            }),
            Some(CustomerAgreement {
                agreement_type: CustomerAgreementType::Subscription,
                stored_card_usage: Some(StoredCardUsageType::First),
                scheme_reference: None,
            }),
        ),
        // NTI with raw card data
        (PaymentMethodData::CardDetailsForNetworkTransactionId(_), _, _) => (
            None,
            mandate_ids.and_then(|mandate_ids| {
                mandate_ids
                    .mandate_reference_id
                    .and_then(|mandate_id| match mandate_id {
                        MandateReferenceId::NetworkMandateId(network_transaction_id) => {
                            Some(CustomerAgreement {
                                agreement_type: CustomerAgreementType::Unscheduled,
                                scheme_reference: Some(network_transaction_id.into()),
                                stored_card_usage: None,
                            })
                        }
                        _ => None,
                    })
            }),
        ),
        _ => (None, None),
    }
}

// Implementation for WorldpayAuthorizeRequest using abstracted request
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        WorldpayRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for WorldpayAuthorizeRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: WorldpayRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = WorldpayAuthType::try_from(&item.router_data.connector_config)?;

        let merchant_name = auth
            .merchant_name
            .ok_or(IntegrationError::InvalidConnectorConfig {
                config: "connector_config.merchant_name",
                context: Default::default(),
            })?;

        let is_mandate_payment = item.router_data.request.is_mandate_payment();
        let three_ds = create_three_ds_request(&item.router_data, is_mandate_payment)?;

        let (token_creation, customer_agreement) = get_token_and_agreement(
            &item.router_data.request.payment_method_data,
            item.router_data.request.setup_future_usage,
            item.router_data.request.off_session,
            item.router_data.request.mandate_id.clone(),
        );

        Ok(Self {
            instruction: Instruction {
                settlement: get_settlement_info(
                    &item.router_data,
                    item.router_data.request.minor_amount,
                ),
                method: PaymentMethod::try_from((
                    item.router_data.resource_common_data.payment_method,
                    item.router_data.request.payment_method_type,
                ))?,
                payment_instrument: fetch_payment_instrument(
                    item.router_data.request.payment_method_data.clone(),
                    item.router_data.resource_common_data.get_optional_billing(),
                )?,
                narrative: InstructionNarrative {
                    line1: merchant_name.expose(),
                },
                value: PaymentValue {
                    amount: item.router_data.request.minor_amount,
                    currency: item.router_data.request.currency,
                },
                debt_repayment: None,
                three_ds,
                token_creation,
                customer_agreement,
            },
            merchant: Merchant {
                entity: WorldpayAuthType::try_from(&item.router_data.connector_config)?.entity_id,
                ..Default::default()
            },
            transaction_reference: item
                .router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            customer: None,
        })
    }
}

// RepeatPayment request transformer - for MIT (Merchant Initiated Transactions)
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        WorldpayRouterData<
            RouterDataV2<
                domain_types::connector_flow::RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for WorldpayRepeatPaymentRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: WorldpayRouterData<
            RouterDataV2<
                domain_types::connector_flow::RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Extract merchant name from connector config
        let auth = WorldpayAuthType::try_from(&item.router_data.connector_config)?;

        let merchant_name = auth
            .merchant_name
            .ok_or(IntegrationError::InvalidConnectorConfig {
                config: "connector_config.merchant_name",
                context: Default::default(),
            })?;

        // Extract payment instrument from mandate_reference
        let payment_instrument = match &item.router_data.request.mandate_reference {
            MandateReferenceId::ConnectorMandateId(connector_mandate_ref) => {
                let href = connector_mandate_ref.get_connector_mandate_id().ok_or(
                    IntegrationError::MissingRequiredField {
                        field_name: "connector_mandate_id",
                        context: Default::default(),
                    },
                )?;

                PaymentInstrument::CardToken(CardToken {
                    payment_type: PaymentType::Token,
                    href,
                    cvc: None,
                })
            }
            MandateReferenceId::NetworkMandateId(_network_txn_id) => {
                // NTI flow would need raw card details, which RepeatPayment doesn't have
                return Err(IntegrationError::not_implemented(
                    "NetworkMandateId not supported in RepeatPayment".to_string(),
                )
                .into());
            }
            MandateReferenceId::NetworkTokenWithNTI(_) => {
                return Err(IntegrationError::not_implemented(
                    "NetworkTokenWithNTI not supported in RepeatPayment yet".to_string(),
                )
                .into());
            }
        };

        // Determine settlement from capture_method
        let settlement = match item.router_data.request.capture_method {
            Some(enums::CaptureMethod::Automatic)
            | Some(enums::CaptureMethod::SequentialAutomatic)
            | None => Some(AutoSettlement { auto: true }),
            Some(enums::CaptureMethod::Manual) | Some(enums::CaptureMethod::ManualMultiple) => {
                Some(AutoSettlement { auto: false })
            }
            _ => None,
        };

        Ok(Self {
            instruction: Instruction {
                settlement,
                method: PaymentMethod::Card, // RepeatPayment is always card-based
                payment_instrument,
                narrative: InstructionNarrative {
                    line1: merchant_name.expose(),
                },
                value: PaymentValue {
                    amount: item.router_data.request.minor_amount,
                    currency: item.router_data.request.currency,
                },
                debt_repayment: None,
                three_ds: None,       // MIT transactions don't require 3DS
                token_creation: None, // No new token creation for repeat payments
                customer_agreement: Some(CustomerAgreement {
                    agreement_type: CustomerAgreementType::Subscription,
                    stored_card_usage: Some(StoredCardUsageType::Subsequent), // CRITICAL: MIT indicator
                    scheme_reference: None,
                }),
            },
            merchant: Merchant {
                entity: WorldpayAuthType::try_from(&item.router_data.connector_config)?.entity_id,
                ..Default::default()
            },
            transaction_reference: item
                .router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            customer: None,
        })
    }
}

pub struct WorldpayAuthType {
    pub(super) api_key: Secret<String>,
    pub(super) entity_id: Secret<String>,
    pub(super) merchant_name: Option<Secret<String>>,
}

impl TryFrom<&ConnectorSpecificConfig> for WorldpayAuthType {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Worldpay {
                username,
                password,
                entity_id,
                merchant_name,
                ..
            } => {
                let auth_key = format!("{}:{}", username.peek(), password.peek());
                let auth_header = format!(
                    "Basic {}",
                    base64::Engine::encode(&base64::engine::general_purpose::STANDARD, auth_key)
                );
                Ok(Self {
                    api_key: Secret::new(auth_header),
                    entity_id: entity_id.clone(),
                    merchant_name: merchant_name.clone(),
                })
            }
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            })?,
        }
    }
}

impl From<PaymentOutcome> for enums::AttemptStatus {
    fn from(item: PaymentOutcome) -> Self {
        match item {
            PaymentOutcome::Authorized => Self::Authorized,
            PaymentOutcome::SentForSettlement => Self::Charged,
            PaymentOutcome::ThreeDsDeviceDataRequired => Self::DeviceDataCollectionPending,
            PaymentOutcome::ThreeDsAuthenticationFailed => Self::AuthenticationFailed,
            PaymentOutcome::ThreeDsChallenged => Self::AuthenticationPending,
            PaymentOutcome::SentForCancellation => Self::VoidInitiated,
            PaymentOutcome::SentForPartialRefund | PaymentOutcome::SentForRefund => {
                Self::AutoRefunded
            }
            PaymentOutcome::Refused | PaymentOutcome::FraudHighRisk => Self::Failure,
            PaymentOutcome::ThreeDsUnavailable => Self::AuthenticationFailed,
        }
    }
}

impl From<PaymentOutcome> for enums::RefundStatus {
    fn from(item: PaymentOutcome) -> Self {
        match item {
            PaymentOutcome::SentForPartialRefund | PaymentOutcome::SentForRefund => Self::Success,
            PaymentOutcome::Refused
            | PaymentOutcome::FraudHighRisk
            | PaymentOutcome::Authorized
            | PaymentOutcome::SentForSettlement
            | PaymentOutcome::ThreeDsDeviceDataRequired
            | PaymentOutcome::ThreeDsAuthenticationFailed
            | PaymentOutcome::ThreeDsChallenged
            | PaymentOutcome::SentForCancellation
            | PaymentOutcome::ThreeDsUnavailable => Self::Failure,
        }
    }
}

impl From<&EventType> for enums::AttemptStatus {
    fn from(value: &EventType) -> Self {
        match value {
            EventType::SentForAuthorization => Self::Authorizing,
            EventType::SentForSettlement => Self::Charged,
            EventType::Settled => Self::Charged,
            EventType::Authorized => Self::Authorized,
            EventType::Refused
            | EventType::SettlementFailed
            | EventType::Expired
            | EventType::Cancelled => Self::Failure,
            EventType::SentForRefund | EventType::RefundFailed | EventType::Refunded => {
                Self::Charged
            }
            EventType::Error | EventType::Unknown => Self::Pending,
        }
    }
}

impl From<EventType> for enums::RefundStatus {
    fn from(value: EventType) -> Self {
        match value {
            EventType::Refunded | EventType::SentForRefund => Self::Success,
            EventType::RefundFailed => Self::Failure,
            EventType::Authorized
            | EventType::Cancelled
            | EventType::Settled
            | EventType::Refused
            | EventType::Error
            | EventType::SentForSettlement
            | EventType::SentForAuthorization
            | EventType::SettlementFailed
            | EventType::Expired
            | EventType::Unknown => Self::Pending,
        }
    }
}

// Add the TryFrom implementation that the macro system expects
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<WorldpayPaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<WorldpayPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Extract amount before moving item to pass for correct status determination
        let amount = item.router_data.request.minor_amount;
        // Use the existing ForeignTryFrom implementation
        Self::foreign_try_from((item, None, amount))
    }
}

// RepeatPayment response transformer
impl<
        T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize + Serialize,
    > TryFrom<ResponseRouterData<WorldpayPaymentsResponse, Self>>
    for RouterDataV2<
        domain_types::connector_flow::RepeatPayment,
        PaymentFlowData,
        RepeatPaymentData<T>,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<WorldpayPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Extract amount before moving item to pass for correct status determination
        let amount = item.router_data.request.minor_amount;
        // Use the existing ForeignTryFrom implementation
        Self::foreign_try_from((item, None, amount))
    }
}

impl<F, T>
    ForeignTryFrom<(
        ResponseRouterData<WorldpayPaymentsResponse, Self>,
        Option<String>,
        MinorUnit,
    )> for RouterDataV2<F, PaymentFlowData, T, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn foreign_try_from(
        item: (
            ResponseRouterData<WorldpayPaymentsResponse, Self>,
            Option<String>,
            MinorUnit,
        ),
    ) -> Result<Self, Self::Error> {
        let (router_data, optional_correlation_id, amount) = item;
        let (description, redirection_data, mandate_reference, network_txn_id, error) = router_data
            .response
            .other_fields
            .as_ref()
            .map(|other_fields| match other_fields {
                WorldpayPaymentResponseFields::AuthorizedResponse(res) => (
                    res.description.clone(),
                    None,
                    res.token.as_ref().map(|mandate_token| MandateReference {
                        connector_mandate_id: Some(mandate_token.href.clone().expose()),
                        payment_method_id: Some(mandate_token.token_id.clone()),
                        connector_mandate_request_reference_id: None,
                    }),
                    res.scheme_reference.clone(),
                    None,
                ),
                WorldpayPaymentResponseFields::DDCResponse(res) => {
                    let link_data = res
                        .actions
                        .supply_ddc_data
                        .href
                        .split('/')
                        .nth_back(1)
                        .map(|s| s.to_string());
                    (
                        None,
                        Some(RedirectForm::WorldpayDDCForm {
                            endpoint: res.device_data_collection.url.clone(),
                            method: common_utils::request::Method::Post,
                            collection_id: link_data,
                            form_fields: HashMap::from([
                                (
                                    FORM_FIELD_BIN.to_string(),
                                    res.device_data_collection.bin.clone().expose(),
                                ),
                                (
                                    FORM_FIELD_JWT.to_string(),
                                    res.device_data_collection.jwt.clone().expose(),
                                ),
                            ]),
                        }),
                        None,
                        None,
                        None,
                    )
                }
                WorldpayPaymentResponseFields::ThreeDsChallenged(res) => (
                    None,
                    Some(RedirectForm::Form {
                        endpoint: res.challenge.url.to_string(),
                        method: common_utils::request::Method::Post,
                        form_fields: HashMap::from([(
                            FORM_FIELD_JWT.to_string(),
                            res.challenge.jwt.clone().expose(),
                        )]),
                    }),
                    None,
                    None,
                    None,
                ),
                WorldpayPaymentResponseFields::RefusedResponse(res) => (
                    None,
                    None,
                    None,
                    None,
                    Some((
                        res.refusal_code.clone(),
                        res.refusal_description.clone(),
                        res.advice
                            .as_ref()
                            .and_then(|advice_code| advice_code.code.clone()),
                    )),
                ),
                WorldpayPaymentResponseFields::FraudHighRisk(_) => (None, None, None, None, None),
            })
            .unwrap_or((None, None, None, None, None));
        let worldpay_status = router_data.response.outcome.clone();
        let optional_error_message = match worldpay_status {
            PaymentOutcome::ThreeDsAuthenticationFailed => {
                Some("3DS authentication failed from issuer".to_string())
            }
            PaymentOutcome::ThreeDsUnavailable => {
                Some("3DS authentication unavailable from issuer".to_string())
            }
            PaymentOutcome::FraudHighRisk => Some("Transaction marked as high risk".to_string()),
            _ => None,
        };
        let status = if amount == MinorUnit::zero() && worldpay_status == PaymentOutcome::Authorized
        {
            enums::AttemptStatus::Charged
        } else {
            enums::AttemptStatus::from(worldpay_status.clone())
        };

        // Extract linkData for 3DS flows and store in metadata with stage indicator
        let connector_metadata = match &router_data.response.other_fields {
            Some(WorldpayPaymentResponseFields::DDCResponse(res)) => res
                .actions
                .supply_ddc_data
                .href
                .split('/')
                .nth_back(1)
                .map(|link_data| {
                    let mut metadata = serde_json::Map::new();
                    metadata.insert(
                        METADATA_LINK_DATA.to_string(),
                        serde_json::Value::String(link_data.to_string()),
                    );
                    metadata.insert(
                        METADATA_3DS_STAGE.to_string(),
                        serde_json::Value::String(STAGE_DDC.to_string()),
                    );
                    serde_json::Value::Object(metadata)
                }),
            Some(WorldpayPaymentResponseFields::ThreeDsChallenged(res)) => res
                .actions
                .complete_three_ds_challenge
                .href
                .split('/')
                .nth_back(1)
                .map(|link_data| {
                    let mut metadata = serde_json::Map::new();
                    metadata.insert(
                        METADATA_LINK_DATA.to_string(),
                        serde_json::Value::String(link_data.to_string()),
                    );
                    metadata.insert(
                        METADATA_3DS_STAGE.to_string(),
                        serde_json::Value::String(STAGE_CHALLENGE.to_string()),
                    );
                    serde_json::Value::Object(metadata)
                }),
            _ => None,
        };

        let response = match (optional_error_message, error) {
            (None, None) => Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::foreign_try_from((
                    router_data.response,
                    optional_correlation_id.clone(),
                    router_data.http_code,
                ))?,
                redirection_data: redirection_data.map(Box::new),
                mandate_reference: mandate_reference.map(Box::new),
                connector_metadata,
                network_txn_id: network_txn_id.map(|id| id.expose()),
                connector_response_reference_id: optional_correlation_id.clone(),
                incremental_authorization_allowed: None,
                status_code: router_data.http_code,
            }),
            (Some(reason), _) => Err(ErrorResponse {
                code: worldpay_status.to_string(),
                message: reason.clone(),
                reason: Some(reason),
                status_code: router_data.http_code,
                attempt_status: Some(status),
                connector_transaction_id: optional_correlation_id,
                network_advice_code: None,
                network_decline_code: None,
                network_error_message: None,
            }),
            (_, Some((code, message, advice_code))) => Err(ErrorResponse {
                code: code.clone(),
                message: message.clone(),
                reason: Some(message.clone()),
                status_code: router_data.http_code,
                attempt_status: Some(status),
                connector_transaction_id: optional_correlation_id,
                network_advice_code: advice_code,
                // Access Worldpay returns a raw response code in the refusalCode field (if enabled) containing the unmodified response code received either directly from the card scheme for Worldpay-acquired transactions, or from third party acquirers.
                // You can use raw response codes to inform your retry logic. A rawCode is only returned if specifically requested.
                network_decline_code: Some(code),
                network_error_message: Some(message),
            }),
        };
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                description,
                ..router_data.router_data.resource_common_data
            },
            response,
            ..router_data.router_data
        })
    }
}

// Note: Old RouterData TryFrom implementations removed as we're using RouterDataV2
// The following implementations are kept for compatibility with existing response processing
// Steps 100-109: TryFrom implementations for Capture flow
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        WorldpayRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for WorldpayPartialRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: WorldpayRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Always include value field for both full and partial captures (same as Hyperswitch)
        // Worldpay's /partialSettlements endpoint requires the value field
        // Replace underscores with hyphens as Worldpay only accepts alphanumeric and hyphens
        Ok(Self {
            reference: item
                .router_data
                .resource_common_data
                .connector_request_reference_id
                .replace('_', "-"),
            value: PaymentValue {
                amount: item.router_data.request.minor_amount_to_capture,
                currency: item.router_data.request.currency,
            },
        })
    }
}

impl TryFrom<ResponseRouterData<WorldpayPaymentsResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<WorldpayPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = enums::AttemptStatus::from(item.response.outcome.clone());
        let response = Ok(PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::foreign_try_from((
                item.response.clone(),
                None,
                item.http_code,
            ))?,
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: None,
            incremental_authorization_allowed: None,
            status_code: item.http_code,
        });

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            response,
            ..item.router_data
        })
    }
}

impl<F>
    TryFrom<(
        &RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>,
        MinorUnit,
    )> for WorldpayPartialRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        req: (
            &RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>,
            MinorUnit,
        ),
    ) -> Result<Self, Self::Error> {
        let (item, amount) = req;
        Ok(Self {
            reference: item.request.refund_id.replace('_', "-"),
            value: PaymentValue {
                amount,
                currency: item.request.currency,
            },
        })
    }
}

impl TryFrom<WorldpayWebhookEventType> for WorldpayEventResponse {
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(event: WorldpayWebhookEventType) -> Result<Self, Self::Error> {
        Ok(Self {
            last_event: event.event_details.event_type,
            links: None,
        })
    }
}

// Step 80-84: TryFrom implementations for PSync flow
impl<F> TryFrom<ResponseRouterData<WorldpayEventResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<WorldpayEventResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = enums::AttemptStatus::from(&item.response.last_event);
        let response = Ok(PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(
                item.router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
            ),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: None,
            incremental_authorization_allowed: None,
            status_code: item.http_code,
        });

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            response,
            ..item.router_data
        })
    }
}

// Steps 85-94: TryFrom implementations for Refund flow
impl<F, T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        WorldpayRouterData<RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    > for WorldpayPartialRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: WorldpayRouterData<
            RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            reference: item.router_data.request.refund_id.replace('_', "-"),
            value: PaymentValue {
                amount: item.router_data.request.minor_refund_amount,
                currency: item.router_data.request.currency,
            },
        })
    }
}

impl<F> TryFrom<ResponseRouterData<WorldpayPaymentsResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<WorldpayPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let refund_status = enums::RefundStatus::from(item.response.outcome.clone());
        let response = Ok(RefundsResponseData {
            connector_refund_id: item.router_data.request.refund_id.clone(),
            refund_status,
            status_code: item.http_code,
        });

        Ok(Self {
            resource_common_data: RefundFlowData {
                ..item.router_data.resource_common_data
            },
            response,
            ..item.router_data
        })
    }
}

// Steps 95-99: TryFrom implementations for RSync flow
impl<F> TryFrom<ResponseRouterData<WorldpayEventResponse, Self>>
    for RouterDataV2<F, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<WorldpayEventResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let refund_status = enums::RefundStatus::from(item.response.last_event);
        let response = Ok(RefundsResponseData {
            connector_refund_id: item
                .router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            refund_status,
            status_code: item.http_code,
        });

        Ok(Self {
            resource_common_data: RefundFlowData {
                ..item.router_data.resource_common_data
            },
            response,
            ..item.router_data
        })
    }
}

// Steps 110-119: TryFrom implementations for Void flow
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        WorldpayRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for ()
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        _item: WorldpayRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        // Void request has empty body
        Ok(())
    }
}

impl TryFrom<ResponseRouterData<WorldpayPaymentsResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<WorldpayPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = enums::AttemptStatus::from(item.response.outcome.clone());
        let response = Ok(PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::foreign_try_from((
                item.response.clone(),
                None,
                item.http_code,
            ))?,
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            connector_response_reference_id: None,
            incremental_authorization_allowed: None,
            status_code: item.http_code,
        });

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            response,
            ..item.router_data
        })
    }
}

impl ForeignTryFrom<(WorldpayPaymentsResponse, Option<String>, u16)> for ResponseIdStr {
    type Error = error_stack::Report<ConnectorError>;
    fn foreign_try_from(
        item: (WorldpayPaymentsResponse, Option<String>, u16),
    ) -> Result<Self, Self::Error> {
        get_resource_id(item.0, item.1, |id| Self { id }, item.2)
    }
}

impl ForeignTryFrom<(WorldpayPaymentsResponse, Option<String>, u16)> for ResponseId {
    type Error = error_stack::Report<ConnectorError>;
    fn foreign_try_from(
        item: (WorldpayPaymentsResponse, Option<String>, u16),
    ) -> Result<Self, Self::Error> {
        get_resource_id(item.0, item.1, Self::ConnectorTransactionId, item.2)
    }
}

// Authentication flow implementations

// PreAuthenticate request transformer (for 3dsDeviceData/DDC)
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        WorldpayRouterData<
            RouterDataV2<
                domain_types::connector_flow::PreAuthenticate,
                PaymentFlowData,
                domain_types::connector_types::PaymentsPreAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for WorldpayPreAuthenticateRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: WorldpayRouterData<
            RouterDataV2<
                domain_types::connector_flow::PreAuthenticate,
                PaymentFlowData,
                domain_types::connector_types::PaymentsPreAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let params = item
            .router_data
            .request
            .redirect_response
            .as_ref()
            .and_then(|redirect_response| redirect_response.params.as_ref())
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "redirect_response.params",
                context: Default::default(),
            })?;

        let parsed_request = serde_urlencoded::from_str::<Self>(params.peek()).change_context(
            IntegrationError::BodySerializationFailed {
                context: Default::default(),
            },
        )?;

        Ok(parsed_request)
    }
}

// PostAuthenticate request transformer (for 3dsChallenges)
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        WorldpayRouterData<
            RouterDataV2<
                domain_types::connector_flow::PostAuthenticate,
                PaymentFlowData,
                domain_types::connector_types::PaymentsPostAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for WorldpayPostAuthenticateRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: WorldpayRouterData<
            RouterDataV2<
                domain_types::connector_flow::PostAuthenticate,
                PaymentFlowData,
                domain_types::connector_types::PaymentsPostAuthenticateData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let params = item
            .router_data
            .request
            .redirect_response
            .as_ref()
            .and_then(|redirect_response| redirect_response.params.as_ref())
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "redirect_response.params",
                context: Default::default(),
            })?;

        let parsed_request = serde_urlencoded::from_str::<Self>(params.peek()).change_context(
            IntegrationError::BodySerializationFailed {
                context: Default::default(),
            },
        )?;

        Ok(parsed_request)
    }
}

// Response implementations for authentication flows

// PreAuthenticate response transformer
impl<T> TryFrom<ResponseRouterData<WorldpayPaymentsResponse, Self>>
    for RouterDataV2<
        domain_types::connector_flow::PreAuthenticate,
        PaymentFlowData,
        domain_types::connector_types::PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    >
where
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<WorldpayPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = enums::AttemptStatus::from(item.response.outcome.clone());
        let (redirection_data, connector_response_reference_id) =
            extract_redirection_data(&item.response)?;
        let _connector_metadata = extract_three_ds_metadata(&item.response);

        let response = Ok(PaymentsResponseData::PreAuthenticateResponse {
            redirection_data: redirection_data.map(Box::new),
            connector_response_reference_id,
            status_code: item.http_code,
            authentication_data: None,
        });

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            response,
            ..item.router_data
        })
    }
}

// PostAuthenticate response transformer
impl<T> TryFrom<ResponseRouterData<WorldpayPaymentsResponse, Self>>
    for RouterDataV2<
        domain_types::connector_flow::PostAuthenticate,
        PaymentFlowData,
        domain_types::connector_types::PaymentsPostAuthenticateData<T>,
        PaymentsResponseData,
    >
where
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<WorldpayPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = enums::AttemptStatus::from(item.response.outcome.clone());
        let (_redirection_data, connector_response_reference_id) =
            extract_redirection_data(&item.response)?;
        let _connector_metadata = extract_three_ds_metadata(&item.response);

        let response = Ok(PaymentsResponseData::PostAuthenticateResponse {
            authentication_data: None,
            connector_response_reference_id,
            status_code: item.http_code,
        });

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            response,
            ..item.router_data
        })
    }
}

fn extract_redirection_data(
    response: &WorldpayPaymentsResponse,
) -> Result<(Option<RedirectForm>, Option<String>), error_stack::Report<ConnectorError>> {
    match &response.other_fields {
        Some(WorldpayPaymentResponseFields::ThreeDsChallenged(challenged)) => {
            let redirect_form = RedirectForm::Form {
                endpoint: challenged.challenge.url.to_string(),
                method: common_utils::request::Method::Post,
                form_fields: HashMap::from([(
                    "JWT".to_string(),
                    challenged.challenge.jwt.clone().expose(),
                )]),
            };
            Ok((
                Some(redirect_form),
                Some(challenged.challenge.reference.clone()),
            ))
        }
        Some(WorldpayPaymentResponseFields::DDCResponse(ddc)) => {
            let link_data = ddc
                .actions
                .supply_ddc_data
                .href
                .split('/')
                .nth_back(1)
                .map(|s| s.to_string());
            let redirect_form = RedirectForm::WorldpayDDCForm {
                endpoint: ddc.device_data_collection.url.clone(),
                method: common_utils::request::Method::Post,
                collection_id: link_data,
                form_fields: HashMap::from([
                    (
                        FORM_FIELD_BIN.to_string(),
                        ddc.device_data_collection.bin.clone().expose(),
                    ),
                    (
                        FORM_FIELD_JWT.to_string(),
                        ddc.device_data_collection.jwt.clone().expose(),
                    ),
                ]),
            };
            Ok((
                Some(redirect_form),
                Some(METADATA_DDC_REFERENCE.to_string()),
            ))
        }
        _ => Ok((None, None)),
    }
}

// Helper function to extract 3DS authentication metadata
fn extract_three_ds_metadata(response: &WorldpayPaymentsResponse) -> Option<serde_json::Value> {
    match &response.other_fields {
        Some(WorldpayPaymentResponseFields::RefusedResponse(refused)) => {
            // Check for 3DS data in refused response
            if let Some(three_ds) = &refused.three_ds {
                let mut metadata = serde_json::Map::new();
                if let Some(version) = &three_ds.version {
                    metadata.insert(
                        METADATA_3DS_VERSION.to_string(),
                        serde_json::Value::String(version.clone()),
                    );
                }
                if let Some(eci) = &three_ds.eci {
                    metadata.insert(
                        METADATA_ECI.to_string(),
                        serde_json::Value::String(eci.clone()),
                    );
                }
                if let Some(applied) = &three_ds.applied {
                    metadata.insert(
                        METADATA_AUTH_APPLIED.to_string(),
                        serde_json::Value::String(applied.clone()),
                    );
                }
                if !metadata.is_empty() {
                    return Some(serde_json::Value::Object(metadata));
                }
            }
            None
        }
        Some(WorldpayPaymentResponseFields::ThreeDsChallenged(challenged)) => {
            let mut metadata = serde_json::Map::new();
            metadata.insert(
                METADATA_3DS_VERSION.to_string(),
                serde_json::Value::String(challenged.authentication.version.clone()),
            );
            if let Some(eci) = &challenged.authentication.eci {
                metadata.insert(
                    METADATA_ECI.to_string(),
                    serde_json::Value::String(eci.clone()),
                );
            }
            // Extract linkData and stage for Authenticate response with 3DS challenge
            if let Some(link_data) = challenged
                .actions
                .complete_three_ds_challenge
                .href
                .split('/')
                .nth_back(1)
            {
                metadata.insert(
                    "link_data".to_string(),
                    serde_json::Value::String(link_data.to_string()),
                );
                metadata.insert(
                    "3ds_stage".to_string(),
                    serde_json::Value::String("challenge".to_string()),
                );
            }
            Some(serde_json::Value::Object(metadata))
        }
        Some(WorldpayPaymentResponseFields::DDCResponse(ddc)) => {
            // Extract linkData and stage for Authenticate response with DDC
            let mut metadata = serde_json::Map::new();
            if let Some(link_data) = ddc.actions.supply_ddc_data.href.split('/').nth_back(1) {
                metadata.insert(
                    "link_data".to_string(),
                    serde_json::Value::String(link_data.to_string()),
                );
                metadata.insert(
                    "3ds_stage".to_string(),
                    serde_json::Value::String("ddc".to_string()),
                );
            }
            if !metadata.is_empty() {
                Some(serde_json::Value::Object(metadata))
            } else {
                None
            }
        }
        _ => None,
    }
}

// Steps 120-129: TryFrom implementations for IncrementalAuthorization flow
// Access Worldpay endpoint: POST /payments/authorizations/incrementalAuthorizations/{linkData}
// Request body contains only { "value": { "amount": <minor>, "currency": "<ISO>" } }
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        WorldpayRouterData<
            RouterDataV2<
                IncrementalAuthorization,
                PaymentFlowData,
                PaymentsIncrementalAuthorizationData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for WorldpayIncrementalAuthRequest
{
    type Error = error_stack::Report<IntegrationError>;
    fn try_from(
        item: WorldpayRouterData<
            RouterDataV2<
                IncrementalAuthorization,
                PaymentFlowData,
                PaymentsIncrementalAuthorizationData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            value: PaymentValue {
                amount: item.router_data.request.minor_amount,
                currency: item.router_data.request.currency,
            },
        })
    }
}

impl TryFrom<ResponseRouterData<WorldpayIncrementalAuthResponse, Self>>
    for RouterDataV2<
        IncrementalAuthorization,
        PaymentFlowData,
        PaymentsIncrementalAuthorizationData,
        PaymentsResponseData,
    >
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<WorldpayIncrementalAuthResponse, Self>,
    ) -> Result<Self, Self::Error> {
        // Map Worldpay PaymentOutcome to AuthorizationStatus (success/processing/failure).
        // Refund-related outcomes (`sentForRefund`, `sentForPartialRefund`) are not part of
        // the documented outcome set for the Card Payments incremental-authorization endpoint,
        // but if the connector ever returns one on this flow it indicates the increase did
        // not take effect — treat as a terminal failure rather than Processing.
        let authorization_status = match &item.response.outcome {
            PaymentOutcome::Authorized | PaymentOutcome::SentForSettlement => {
                enums::AuthorizationStatus::Success
            }
            PaymentOutcome::Refused
            | PaymentOutcome::FraudHighRisk
            | PaymentOutcome::ThreeDsAuthenticationFailed
            | PaymentOutcome::ThreeDsUnavailable
            | PaymentOutcome::SentForCancellation
            | PaymentOutcome::SentForRefund
            | PaymentOutcome::SentForPartialRefund => enums::AuthorizationStatus::Failure,
            PaymentOutcome::ThreeDsDeviceDataRequired | PaymentOutcome::ThreeDsChallenged => {
                enums::AuthorizationStatus::Processing
            }
        };

        // connector_authorization_id is derived from the
        // `cardPayments:increaseAuthorizedAmount` action link's trailing
        // linkData segment. The original Authorize-flow connector_transaction_id
        // already represents this same linkData, so we fall back to it if the
        // incremental auth response does not expose a new one.
        let href = item.response.links.as_ref().and_then(|links| {
            links
                .get(LINK_KEY_INCREASE_AUTHORIZED_AMOUNT)
                .and_then(|v| v.get("href"))
                .and_then(|v| v.as_str())
                .map(str::to_string)
        });
        let connector_authorization_id = match href {
            Some(href) => {
                let encoded = href.rsplit_once('/').map(|(_, h)| h).unwrap_or(href.as_str());
                Some(
                    urlencoding::decode(encoded)
                        .map(|s| s.into_owned())
                        .change_context(crate::utils::response_handling_fail_for_connector(
                            item.http_code,
                            "worldpay",
                        ))?,
                )
            }
            None => Some(
                item.router_data
                    .request
                    .connector_transaction_id
                    .get_connector_transaction_id()
                    .change_context(crate::utils::response_handling_fail_for_connector(
                        item.http_code,
                        "worldpay",
                    ))?,
            ),
        };

        let response = Ok(PaymentsResponseData::IncrementalAuthorizationResponse {
            status: authorization_status,
            connector_authorization_id,
            status_code: item.http_code,
        });

        let attempt_status = enums::AttemptStatus::from(item.response.outcome);
        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: attempt_status,
                ..item.router_data.resource_common_data
            },
            response,
            ..item.router_data
        })
    }
}
