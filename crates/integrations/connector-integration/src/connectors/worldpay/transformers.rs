use std::collections::HashMap;

use common_enums as enums;
use common_utils::{ext_traits::OptionExt, pii, types::MinorUnit, CustomResult};
use domain_types::{
    connector_flow::{Authorize, Capture, Void},
    connector_types::{
        MandateIds, MandateReference, MandateReferenceId, PaymentFlowData, PaymentVoidData,
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData,
        ResponseId,
    },
    errors::ConnectorError,
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

/// Metadata object extracted from connector_feature_data
/// Contains Worldpay-specific merchant configuration
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct WorldpayConnectorMetadataObject {
    pub merchant_name: Option<Secret<String>>,
}

impl TryFrom<Option<&pii::SecretSerdeValue>> for WorldpayConnectorMetadataObject {
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(meta_data: Option<&pii::SecretSerdeValue>) -> Result<Self, Self::Error> {
        let metadata: Self =
            crate::utils::to_connector_meta_from_secret::<Self>(meta_data.cloned())
                .change_context(ConnectorError::InvalidConnectorConfig {
                    config: "connector_feature_data",
                })?;
        Ok(metadata)
    }
}

fn fetch_payment_instrument<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    payment_method: PaymentMethodData<T>,
    billing_address: Option<&domain_types::payment_address::Address>,
) -> CustomResult<PaymentInstrument<T>, ConnectorError> {
    match payment_method {
        PaymentMethodData::Card(card) => {
            // Extract expiry month and year using helper functions
            let expiry_month_i8 = card.get_expiry_month_as_i8()?;
            let expiry_year_4_digit = card.get_expiry_year_4_digit();
            let expiry_year: i32 = expiry_year_4_digit
                .peek()
                .parse::<i32>()
                .change_context(ConnectorError::ResponseDeserializationFailed)?;

            Ok(PaymentInstrument::Card(CardPayment {
                raw_card_details: RawCardDetails {
                    payment_type: PaymentType::Plain,
                    expiry_date: ExpiryDate {
                        month: expiry_month_i8,
                        year: Secret::new(expiry_year),
                    },
                    card_number: card.card_number,
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
                                    country_code,
                                })
                            }
                            _ => None,
                        }
                    }),
            }))
        }
        PaymentMethodData::CardDetailsForNetworkTransactionId(raw_card_details) => {
            // Extract expiry month and year using helper functions
            let expiry_month_i8 = raw_card_details.get_expiry_month_as_i8()?;
            let expiry_year_4_digit = raw_card_details.get_expiry_year_4_digit();
            let expiry_year: i32 = expiry_year_4_digit
                .peek()
                .parse::<i32>()
                .change_context(ConnectorError::ResponseDeserializationFailed)?;

            Ok(PaymentInstrument::RawCardForNTI(RawCardDetails {
                payment_type: PaymentType::Plain,
                expiry_date: ExpiryDate {
                    month: expiry_month_i8,
                    year: Secret::new(expiry_year),
                },
                card_number: RawCardNumber(raw_card_details.card_number),
            }))
        }
        PaymentMethodData::MandatePayment => {
            Err(ConnectorError::NotImplemented(
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
                            .change_context(ConnectorError::MissingRequiredField {
                                field_name: "gpay wallet_token",
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
            | WalletDataPaymentMethod::Wero(_) => {
                Err(ConnectorError::NotImplemented(
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
        | PaymentMethodData::CardToken(_)
        | PaymentMethodData::NetworkToken(_) => Err(ConnectorError::NotImplemented(
            utils::get_unimplemented_payment_method_error_message("worldpay"),
        )
        .into()),
    }
}

impl TryFrom<(enums::PaymentMethod, Option<enums::PaymentMethodType>)> for PaymentMethod {
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        src: (enums::PaymentMethod, Option<enums::PaymentMethodType>),
    ) -> Result<Self, Self::Error> {
        match (src.0, src.1) {
            (enums::PaymentMethod::Card, _) => Ok(Self::Card),
            (enums::PaymentMethod::Wallet, pmt) => {
                let pm = pmt.ok_or(ConnectorError::MissingRequiredField {
                    field_name: "payment_method_type",
                })?;
                match pm {
                    enums::PaymentMethodType::ApplePay => Ok(Self::ApplePay),
                    enums::PaymentMethodType::GooglePay => Ok(Self::GooglePay),
                    _ => Err(ConnectorError::NotImplemented(
                        utils::get_unimplemented_payment_method_error_message("worldpay"),
                    )
                    .into()),
                }
            }
            _ => Err(ConnectorError::NotImplemented(
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
) -> Result<Option<ThreeDSRequest>, error_stack::Report<ConnectorError>> {
    match (
        &router_data.resource_common_data.auth_type,
        &router_data.request.payment_method_data,
    ) {
        // 3DS for NTI flow
        (_, PaymentMethodData::CardDetailsForNetworkTransactionId(_)) => Ok(None),
        // 3DS for regular payments
        (enums::AuthenticationType::ThreeDs, _) => {
            let browser_info = router_data.request.browser_info.as_ref().ok_or(
                ConnectorError::MissingRequiredField {
                    field_name: "browser_info",
                },
            )?;

            let accept_header = browser_info
                .accept_header
                .clone()
                .get_required_value("accept_header")
                .change_context(ConnectorError::MissingRequiredField {
                    field_name: "accept_header",
                })?;

            let user_agent_header = browser_info
                .user_agent
                .clone()
                .get_required_value("user_agent")
                .change_context(ConnectorError::MissingRequiredField {
                    field_name: "user_agent",
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
    type Error = error_stack::Report<ConnectorError>;
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
        // Route to APM request builder for BankDebit payments
        if let PaymentMethodData::BankDebit(ref bank_debit_data) =
            item.router_data.request.payment_method_data
        {
            return build_apm_request(&item.router_data, bank_debit_data);
        }

        let auth = WorldpayAuthType::try_from(&item.router_data.connector_config)?;

        let merchant_name = auth
            .merchant_name
            .ok_or(ConnectorError::InvalidConnectorConfig {
                config: "connector_config.merchant_name",
            })?;

        let is_mandate_payment = item.router_data.request.is_mandate_payment();
        let three_ds = create_three_ds_request(&item.router_data, is_mandate_payment)?;

        let (token_creation, customer_agreement) = get_token_and_agreement(
            &item.router_data.request.payment_method_data,
            item.router_data.request.setup_future_usage,
            item.router_data.request.off_session,
            item.router_data.request.mandate_id.clone(),
        );

        Ok(WorldpayAuthorizeRequest::Card(WorldpayCardPaymentRequest {
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
        }))
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
    type Error = error_stack::Report<ConnectorError>;
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
            .ok_or(ConnectorError::InvalidConnectorConfig {
                config: "connector_config.merchant_name",
            })?;

        // Extract payment instrument from mandate_reference
        let payment_instrument = match &item.router_data.request.mandate_reference {
            MandateReferenceId::ConnectorMandateId(connector_mandate_ref) => {
                let href = connector_mandate_ref.get_connector_mandate_id().ok_or(
                    ConnectorError::MissingRequiredField {
                        field_name: "connector_mandate_id",
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
                return Err(ConnectorError::NotImplemented(
                    "NetworkMandateId not supported in RepeatPayment".to_string(),
                )
                .into());
            }
            MandateReferenceId::NetworkTokenWithNTI(_) => {
                return Err(ConnectorError::NotImplemented(
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

        Ok(WorldpayRepeatPaymentRequest::Card(
            WorldpayCardPaymentRequest {
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
                    entity: WorldpayAuthType::try_from(&item.router_data.connector_config)?
                        .entity_id,
                    ..Default::default()
                },
                transaction_reference: item
                    .router_data
                    .resource_common_data
                    .connector_request_reference_id
                    .clone(),
                customer: None,
            },
        ))
    }
}

/// Build an APM payment request for BankDebit payments (ACH, SEPA)
fn build_apm_request<
    T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize,
>(
    router_data: &RouterDataV2<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<T>,
        PaymentsResponseData,
    >,
    bank_debit_data: &domain_types::payment_method_data::BankDebitData,
) -> Result<WorldpayAuthorizeRequest<T>, error_stack::Report<ConnectorError>> {
    let auth = WorldpayAuthType::try_from(&router_data.connector_config)?;
    let narrative_line1 = auth
        .merchant_name
        .map(|m| m.expose())
        .unwrap_or_else(|| "Payment".to_string());

    let billing = router_data.resource_common_data.get_optional_billing();
    let billing_address_details = billing.and_then(|b| b.address.as_ref());

    let apm_billing_address = billing_address_details.map(|addr| {
        // Worldpay requires ISO 3166-2 format for state (e.g., "US-OH" not "OH")
        let state = addr.state.as_ref().map(|s| {
            let state_code = utils::convert_us_state_to_code(s.peek());
            match addr.country {
                Some(country) => format!("{}-{}", country, state_code),
                None => state_code,
            }
        });
        ApmBillingAddress {
            address1: addr.line1.as_ref().map(|s| s.peek().to_string()),
            address2: addr.line2.as_ref().map(|s| s.peek().to_string()),
            address3: addr.line3.as_ref().map(|s| s.peek().to_string()),
            city: addr.city.as_ref().map(|s| s.peek().to_string()),
            postal_code: addr.zip.as_ref().map(|s| s.peek().to_string()),
            state,
            country_code: addr.country.map(|c| c.to_string()),
        }
    });

    let (method, payment_instrument, customer, customer_agreement) = match bank_debit_data {
        domain_types::payment_method_data::BankDebitData::AchBankDebit {
            account_number,
            routing_number,
            bank_type,
            ..
        } => {
            let account_type = bank_type.as_ref().map(|bt| match bt {
                enums::BankType::Checking => "checking".to_string(),
                enums::BankType::Savings => "savings".to_string(),
            });

            let first_name = billing.and_then(|b| {
                b.address
                    .as_ref()
                    .and_then(|a| a.first_name.as_ref().map(|s| s.peek().to_string()))
            });
            let last_name = billing.and_then(|b| {
                b.address
                    .as_ref()
                    .and_then(|a| a.last_name.as_ref().map(|s| s.peek().to_string()))
            });
            let email = router_data
                .request
                .email
                .as_ref()
                .map(|e| e.peek().to_string());

            (
                ApmMethod::Ach,
                ApmPaymentInstrument {
                    instrument_type: "direct".to_string(),
                    account_type,
                    account_number: Some(account_number.clone()),
                    routing_number: Some(routing_number.clone()),
                    iban: None,
                    swift_bic: None,
                    account_holder_name: None, // ACH does not use accountHolderName; name goes in customer object
                    language: None,
                    billing_address: apm_billing_address,
                },
                ApmCustomer {
                    first_name,
                    last_name,
                    email,
                },
                None, // No customer agreement for ACH
            )
        }
        domain_types::payment_method_data::BankDebitData::SepaBankDebit {
            iban,
            bank_account_holder_name,
        } => {
            let email = router_data
                .request
                .email
                .as_ref()
                .map(|e| e.peek().to_string());

            // Generate a mandate ID from the transaction reference
            let mandate_id = format!(
                "M-{}",
                router_data
                    .resource_common_data
                    .connector_request_reference_id
            );

            (
                ApmMethod::Sepa,
                ApmPaymentInstrument {
                    instrument_type: "direct".to_string(),
                    account_type: None,
                    account_number: None,
                    routing_number: None,
                    iban: Some(iban.clone()),
                    swift_bic: None,
                    account_holder_name: bank_account_holder_name.clone(),
                    language: Some("en".to_string()),
                    billing_address: apm_billing_address,
                },
                ApmCustomer {
                    first_name: None,
                    last_name: None,
                    email,
                },
                Some(ApmCustomerAgreement {
                    agreement_type: "oneTime".to_string(),
                    mandate: ApmMandate {
                        mandate_type: "oneTime".to_string(),
                        mandate_id,
                    },
                }),
            )
        }
        _ => {
            return Err(ConnectorError::NotImplemented(
                utils::get_unimplemented_payment_method_error_message("worldpay"),
            )
            .into());
        }
    };

    Ok(WorldpayAuthorizeRequest::Apm(WorldpayApmPaymentRequest {
        transaction_reference: router_data
            .resource_common_data
            .connector_request_reference_id
            .clone(),
        order_reference: None,
        merchant: ApmMerchant {
            entity: auth.entity_id,
        },
        instruction: ApmInstruction {
            method,
            value: PaymentValue {
                amount: router_data.request.minor_amount,
                currency: router_data.request.currency,
            },
            narrative: InstructionNarrative {
                line1: narrative_line1,
            },
            payment_instrument,
            customer,
            customer_agreement,
        },
    }))
}

pub struct WorldpayAuthType {
    pub(super) api_key: Secret<String>,
    pub(super) entity_id: Secret<String>,
    pub(super) merchant_name: Option<Secret<String>>,
}

impl TryFrom<&ConnectorSpecificConfig> for WorldpayAuthType {
    type Error = error_stack::Report<ConnectorError>;
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
            _ => Err(ConnectorError::FailedToObtainAuthType)?,
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
            PaymentOutcome::SentForAuthorization => Self::Pending,
            PaymentOutcome::Pending => Self::Pending,
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
            | PaymentOutcome::ThreeDsUnavailable
            | PaymentOutcome::SentForAuthorization
            | PaymentOutcome::Pending => Self::Failure,
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
    type Error = error_stack::Report<ConnectorError>;
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
            resource_id: ResponseId::foreign_try_from((item.response.clone(), None))?,
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
    type Error = error_stack::Report<ConnectorError>;
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
    type Error = error_stack::Report<ConnectorError>;
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
    type Error = error_stack::Report<ConnectorError>;
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
    type Error = error_stack::Report<ConnectorError>;
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
            resource_id: ResponseId::foreign_try_from((item.response.clone(), None))?,
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

impl ForeignTryFrom<(WorldpayPaymentsResponse, Option<String>)> for ResponseIdStr {
    type Error = error_stack::Report<ConnectorError>;
    fn foreign_try_from(
        item: (WorldpayPaymentsResponse, Option<String>),
    ) -> Result<Self, Self::Error> {
        get_resource_id(item.0, item.1, |id| Self { id })
    }
}

impl ForeignTryFrom<(WorldpayPaymentsResponse, Option<String>)> for ResponseId {
    type Error = error_stack::Report<ConnectorError>;
    fn foreign_try_from(
        item: (WorldpayPaymentsResponse, Option<String>),
    ) -> Result<Self, Self::Error> {
        get_resource_id(item.0, item.1, Self::ConnectorTransactionId)
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
    type Error = error_stack::Report<ConnectorError>;
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
            .ok_or(ConnectorError::ResponseDeserializationFailed)?;

        let parsed_request = serde_urlencoded::from_str::<Self>(params.peek())
            .change_context(ConnectorError::ResponseDeserializationFailed)?;

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
    type Error = error_stack::Report<ConnectorError>;
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
            .ok_or(ConnectorError::ResponseDeserializationFailed)?;

        let parsed_request = serde_urlencoded::from_str::<Self>(params.peek())
            .change_context(ConnectorError::ResponseDeserializationFailed)?;

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
