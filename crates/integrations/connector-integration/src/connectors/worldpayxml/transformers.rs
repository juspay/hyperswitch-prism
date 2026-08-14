use std::fmt::Debug;

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD_ENGINE, Engine};
use common_enums::{AttemptStatus, CaptureMethod, RefundStatus};
use domain_types::{
    connector_flow::{
        Authorize, Capture, PSync, RSync, Refund, RepeatPayment, SetupMandate, Void, VoidPC,
    },
    connector_types::{
        MandateReference, MandateReferenceId, PaymentFlowData, PaymentVoidData,
        PaymentsAuthorizeData, PaymentsCancelPostCaptureData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, RepeatPaymentData, ResponseId, SetupMandateRequestData,
    },
    payment_method_data::{
        Card, GpayTokenizationData, PaymentMethodData, PaymentMethodDataTypes, WalletData,
    },
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
};
use error_stack::{Report, ResultExt};
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::Serialize;

use super::{
    requests::{self, WorldpayxmlAction},
    responses::{self, WorldpayxmlLastEvent},
    WorldpayxmlRouterData,
};
use crate::types::ResponseRouterData;
use domain_types::errors::ConnectorError;
use domain_types::errors::IntegrationError;

const API_VERSION: &str = "1.4";

#[derive(Debug, Clone)]
pub struct WorldpayxmlAuthType {
    pub api_username: Secret<String>,
    pub api_password: Secret<String>,
    pub merchant_code: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for WorldpayxmlAuthType {
    type Error = Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Worldpayxml {
                api_username,
                api_password,
                merchant_code,
                ..
            } => Ok(Self {
                api_username: api_username.to_owned(),
                api_password: api_password.to_owned(),
                merchant_code: merchant_code.to_owned(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType {
                context: Default::default(),
            }
            .into()),
        }
    }
}

// Helper function to get currency exponent

const DEFAULT_PAYMENT_DESCRIPTION: &str = "Payment";

// Helper function to get payment method XML element
fn get_worldpayxml_payment_method<T>(
    payment_method_data: &PaymentMethodData<T>,
    card: &Card<T>,
    billing_address: Option<&requests::WorldpayxmlBillingAddress>,
) -> Result<requests::WorldpayxmlPaymentMethod, Report<IntegrationError>>
where
    T: PaymentMethodDataTypes,
{
    match payment_method_data {
        PaymentMethodData::Card(_) => {
            let formatted_year = crate::utils::pad_expiry_year_to_four_digits(&card.card_exp_year);

            let card_holder_name = crate::utils::build_card_holder_name(
                &card.card_holder_name,
                billing_address.and_then(|b| b.address.first_name.clone()),
                billing_address.and_then(|b| b.address.last_name.clone()),
            )
            .map(crate::utils::normalize_cardholder_name);

            let card_data = requests::WorldpayxmlCard {
                card_number: Secret::new(card.card_number.peek().to_string()),
                expiry_date: requests::WorldpayxmlExpiryDate {
                    date: requests::WorldpayxmlDate {
                        month: card.card_exp_month.clone(),
                        year: formatted_year,
                    },
                },
                card_holder_name,
                cvc: Some(card.card_cvc.clone()),
            };

            match card.card_network.as_ref() {
                Some(network) => match network {
                    common_enums::CardNetwork::Visa => {
                        Ok(requests::WorldpayxmlPaymentMethod::Visa(card_data))
                    }
                    common_enums::CardNetwork::Mastercard => {
                        Ok(requests::WorldpayxmlPaymentMethod::Ecmc(card_data))
                    }
                    _ => Ok(requests::WorldpayxmlPaymentMethod::Card(card_data)),
                },
                None => Ok(requests::WorldpayxmlPaymentMethod::Card(card_data)),
            }
        }
        _ => Err(IntegrationError::NotSupported {
            message: "Selected payment method".to_string(),
            connector: "worldpayxml",
            context: Default::default(),
        }
        .into()),
    }
}

/// Builds the Worldpay payment method element for an Apple Pay or Google Pay wallet.
///
/// Wallet data that arrives already decrypted goes over `EMVCO_TOKEN-SSL` as a network token
/// (PAN + cryptogram), which is the only shape Worldpay will register against a stored-credential
/// agreement. Data that arrives still encrypted — the connector decryption flow, where Worldpay
/// decrypts at its end — is forwarded on the wallet-specific element instead.
///
/// `customer_name` is only carried on the Google Pay elements; Apple Pay's decrypted token has no
/// cardholder name associated with it, matching how hyperswitch builds these requests.
fn get_worldpayxml_wallet_payment_method(
    wallet_data: &WalletData,
    customer_name: Option<Secret<String>>,
) -> Result<requests::WorldpayxmlPaymentMethod, Report<IntegrationError>> {
    match wallet_data {
        WalletData::ApplePay(apple_pay_data) => {
            match apple_pay_data
                .payment_data
                .get_decrypted_apple_pay_payment_data_optional()
            {
                Some(decrypt_data) => Ok(requests::WorldpayxmlPaymentMethod::EmvcoToken(
                    requests::WorldpayxmlEmvcoTokenData {
                        token_type: requests::WorldpayxmlEmvcoTokenType::Applepay,
                        token_number: decrypt_data.application_primary_account_number.clone(),
                        expiry_date: requests::WorldpayxmlExpiryDate {
                            date: requests::WorldpayxmlDate {
                                month: decrypt_data.get_expiry_month(),
                                year: decrypt_data.get_four_digit_expiry_year(),
                            },
                        },
                        card_holder_name: None,
                        cryptogram: Some(
                            decrypt_data.payment_data.online_payment_cryptogram.clone(),
                        ),
                        eci_indicator: decrypt_data.payment_data.eci_indicator.clone(),
                    },
                )),
                None => {
                    let encrypted_data = apple_pay_data
                        .payment_data
                        .get_encrypted_apple_pay_payment_data_mandatory()
                        .change_context(IntegrationError::MissingRequiredField {
                            field_name: "apple_pay_encrypted_data",
                            context: Default::default(),
                        })?;

                    let decoded_data = BASE64_STANDARD_ENGINE
                        .decode(encrypted_data)
                        .change_context(IntegrationError::InvalidDataFormat {
                            field_name: "apple_pay_encrypted_data",
                            context: Default::default(),
                        })?;

                    let apple_pay_token: requests::WorldpayxmlApplePayData =
                        serde_json::from_slice(&decoded_data).change_context(
                            IntegrationError::InvalidDataFormat {
                                field_name: "apple_pay_token_json",
                                context: Default::default(),
                            },
                        )?;

                    Ok(requests::WorldpayxmlPaymentMethod::ApplePay(
                        apple_pay_token,
                    ))
                }
            }
        }
        WalletData::GooglePay(google_pay_data) => match &google_pay_data.tokenization_data {
            GpayTokenizationData::Decrypted(decrypt_data) => {
                let expiry_date = requests::WorldpayxmlExpiryDate {
                    date: requests::WorldpayxmlDate {
                        month: decrypt_data.card_exp_month.clone(),
                        year: decrypt_data.get_four_digit_expiry_year().change_context(
                            IntegrationError::MissingRequiredField {
                                field_name: "google_pay_decrypted_data.card_exp_year",
                                context: Default::default(),
                            },
                        )?,
                    },
                };

                match &decrypt_data.cryptogram {
                    Some(cryptogram) => Ok(requests::WorldpayxmlPaymentMethod::EmvcoToken(
                        requests::WorldpayxmlEmvcoTokenData {
                            token_type: requests::WorldpayxmlEmvcoTokenType::Googlepay,
                            token_number: decrypt_data.application_primary_account_number.clone(),
                            expiry_date,
                            card_holder_name: customer_name.clone(),
                            cryptogram: Some(cryptogram.clone()),
                            eci_indicator: decrypt_data.eci_indicator.clone(),
                        },
                    )),
                    // Without a cryptogram there is nothing token-specific left to send, so the
                    // decrypted PAN goes over as a plain card.
                    None => Ok(requests::WorldpayxmlPaymentMethod::Card(
                        requests::WorldpayxmlCard {
                            card_number: Secret::new(
                                decrypt_data
                                    .application_primary_account_number
                                    .peek()
                                    .to_string(),
                            ),
                            expiry_date,
                            card_holder_name: customer_name.clone(),
                            cvc: None,
                        },
                    )),
                }
            }
            GpayTokenizationData::Encrypted(encrypted_token) => {
                let parsed_token: requests::WorldpayxmlGooglePayData = serde_json::from_str(
                    &encrypted_token.token,
                )
                .change_context(IntegrationError::InvalidDataFormat {
                    field_name: "google_pay_token_json",
                    context: Default::default(),
                })?;

                Ok(requests::WorldpayxmlPaymentMethod::PayWithGoogle(
                    parsed_token,
                ))
            }
        },
        _ => Err(IntegrationError::NotSupported {
            message: "Selected wallet".to_string(),
            connector: "worldpayxml",
            context: Default::default(),
        }
        .into()),
    }
}

/// Worldpay scopes the tokens it issues to a single shopper, which is the scope Hyperswitch
/// stores as the connector mandate id.
const TOKEN_SCOPE_SHOPPER: &str = "shopper";

/// Number of decimal places Worldpay expects for the order currency.
fn get_worldpayxml_exponent(currency: common_enums::Currency) -> String {
    if currency.is_three_decimal_currency() {
        "3".to_string()
    } else if currency.is_zero_decimal_currency() {
        "0".to_string()
    } else {
        "2".to_string()
    }
}

/// Builds the `<billingAddress>` element from the flow's billing address, when one was supplied.
fn get_worldpayxml_billing_address(
    resource_common_data: &PaymentFlowData,
) -> Option<requests::WorldpayxmlBillingAddress> {
    resource_common_data
        .address
        .get_payment_billing()
        .and_then(|billing| {
            let telephone_number = billing.phone.as_ref().and_then(|phone| {
                phone
                    .get_number_with_country_code()
                    .or_else(|_| phone.get_number())
                    .ok()
            });
            billing
                .address
                .as_ref()
                .map(|addr| requests::WorldpayxmlBillingAddress {
                    address: requests::WorldpayxmlAddress {
                        first_name: addr.first_name.clone(),
                        last_name: addr.last_name.clone(),
                        address1: addr.line1.clone(),
                        address2: addr.line2.clone(),
                        address3: addr.line3.clone(),
                        postal_code: addr.zip.clone(),
                        city: addr.city.clone().map(|c| c.expose()),
                        state: addr.state.clone(),
                        country_code: addr.country,
                        telephone_number,
                    },
                })
        })
}

/// Resolves the `authenticatedShopperID` Worldpay ties shopper-scoped tokens to.
///
/// Requests that create or spend such a token cannot work without it, so those callers pass
/// `is_required` and get a missing-field error rather than a connector-side rejection.
fn get_worldpayxml_authenticated_shopper_id(
    resource_common_data: &PaymentFlowData,
    is_required: bool,
) -> Result<Option<Secret<String>>, Report<IntegrationError>> {
    match resource_common_data.connector_customer.clone() {
        Some(connector_customer) => Ok(Some(Secret::new(connector_customer))),
        None if is_required => Err(IntegrationError::MissingRequiredField {
            field_name: "connector_customer_id",
            context: Default::default(),
        }
        .into()),
        None => Ok(None),
    }
}

fn get_worldpayxml_mandate_type(
    mit_category: Option<common_enums::MitCategory>,
) -> requests::WorldpayxmlMandateType {
    match mit_category {
        Some(common_enums::MitCategory::Installment) => {
            requests::WorldpayxmlMandateType::Instalment
        }
        Some(common_enums::MitCategory::Recurring) => requests::WorldpayxmlMandateType::Recurring,
        Some(common_enums::MitCategory::Unscheduled)
        | Some(common_enums::MitCategory::Resubmission)
        | None => requests::WorldpayxmlMandateType::Unscheduled,
    }
}

// Authorize flow transformers
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        WorldpayxmlRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for requests::WorldpayxmlPaymentsRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: WorldpayxmlRouterData<
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
        let auth = WorldpayxmlAuthType::try_from(&router_data.connector_config)?;

        // Determine if manual capture
        let is_manual_capture = router_data.request.capture_method == Some(CaptureMethod::Manual)
            || router_data.request.capture_method == Some(CaptureMethod::ManualMultiple);

        // Extract billing address first (needed for payment method)
        let billing_address = get_worldpayxml_billing_address(&router_data.resource_common_data);

        // Get payment method
        let payment_method = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card) => get_worldpayxml_payment_method(
                &router_data.request.payment_method_data,
                card,
                billing_address.as_ref(),
            )?,
            PaymentMethodData::Wallet(wallet_data) => {
                let customer_name = router_data
                    .request
                    .customer_name
                    .clone()
                    .map(|name| crate::utils::normalize_cardholder_name(Secret::new(name)));

                get_worldpayxml_wallet_payment_method(wallet_data, customer_name)?
            }
            _ => {
                return Err(IntegrationError::NotSupported {
                    message: "Selected payment method".to_string(),
                    connector: "worldpayxml",
                    context: Default::default(),
                }
                .into())
            }
        };

        let is_cit_mandate_payment = router_data.request.is_customer_initiated_mandate_payment();

        // A customer-initiated mandate setup must be flagged to Worldpay as the first transaction
        // of a stored-credential agreement, otherwise later merchant-initiated payments against it
        // are declined by the scheme.
        let stored_credentials =
            is_cit_mandate_payment.then(|| requests::WorldpayxmlStoredCredentials {
                usage: requests::WorldpayxmlUsageType::First,
                customer_initiated_reason: Some(get_worldpayxml_mandate_type(
                    router_data.request.mit_category.clone(),
                )),
                merchant_initiated_reason: None,
                scheme_transaction_identifier: None,
            });

        // Ask Worldpay to issue a payment token on the customer-initiated transaction; without it
        // there is nothing for a later merchant-initiated payment to be charged against.
        let create_token = is_cit_mandate_payment.then(|| requests::WorldpayxmlCreateToken {
            token_scope: TOKEN_SCOPE_SHOPPER.to_string(),
            token_event_reference: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
        });

        let authenticated_shopper_id = get_worldpayxml_authenticated_shopper_id(
            &router_data.resource_common_data,
            is_cit_mandate_payment,
        )?;

        // Convert amount using the connector's amount converter
        let converted_amount = super::WorldpayxmlAmountConvertor::convert(
            router_data.request.minor_amount,
            router_data.request.currency,
        )?;

        Ok(Self {
            version: API_VERSION.to_string(),
            merchant_code: auth.merchant_code,
            submit: requests::WorldpayxmlSubmit {
                order: requests::WorldpayxmlOrder {
                    order_code: router_data
                        .resource_common_data
                        .connector_request_reference_id
                        .clone(),
                    capture_delay: if is_manual_capture {
                        "OFF".to_string()
                    } else {
                        "0".to_string().to_string()
                    },
                    description: router_data
                        .resource_common_data
                        .description
                        .clone()
                        .unwrap_or_else(|| DEFAULT_PAYMENT_DESCRIPTION.to_string()),
                    amount: requests::WorldpayxmlAmount {
                        value: converted_amount,
                        currency_code: router_data.request.currency,
                        exponent: get_worldpayxml_exponent(router_data.request.currency),
                    },
                    payment_details: requests::WorldpayxmlPaymentDetails {
                        action: Some(if is_manual_capture {
                            WorldpayxmlAction::Authorise
                        } else {
                            WorldpayxmlAction::Sale
                        }),
                        payment_method,
                        stored_credentials,
                    },
                    shopper: requests::WorldpayxmlShopper {
                        shopper_email_address: router_data.request.email.clone(),
                        authenticated_shopper_id,
                        browser: router_data
                            .request
                            .browser_info
                            .as_ref()
                            .map(|browser_info| requests::WorldpayxmlBrowser {
                                accept_header: browser_info.accept_header.clone(),
                                user_agent_header: browser_info.user_agent.clone(),
                                http_accept_language: browser_info.accept_language.clone(),
                                time_zone: browser_info.time_zone,
                                browser_language: browser_info.language.clone(),
                                browser_java_enabled: browser_info.java_enabled,
                                browser_java_script_enabled: browser_info.java_script_enabled,
                                browser_colour_depth: browser_info.color_depth.map(u32::from),
                                browser_screen_height: browser_info.screen_height,
                                browser_screen_width: browser_info.screen_width,
                            }),
                    },
                    billing_address,
                    create_token,
                },
            },
        })
    }
}

// SetupMandate flow transformers
//
// A mandate setup is submitted as an ordinary order that additionally asks Worldpay to create a
// payment token, and flags the transaction to the scheme as the first of a stored-credential
// agreement. The order amount is whatever the caller asked for, which is normally zero.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        WorldpayxmlRouterData<
            RouterDataV2<
                SetupMandate,
                PaymentFlowData,
                SetupMandateRequestData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for requests::WorldpayxmlSetupMandateRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: WorldpayxmlRouterData<
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
        let auth = WorldpayxmlAuthType::try_from(&router_data.connector_config)?;

        let is_manual_capture = matches!(
            router_data.request.capture_method,
            Some(CaptureMethod::Manual) | Some(CaptureMethod::ManualMultiple)
        );

        let billing_address = get_worldpayxml_billing_address(&router_data.resource_common_data);

        let payment_method = match &router_data.request.payment_method_data {
            PaymentMethodData::Card(card) => get_worldpayxml_payment_method(
                &router_data.request.payment_method_data,
                card,
                billing_address.as_ref(),
            )?,
            PaymentMethodData::Wallet(wallet_data) => {
                let customer_name = router_data
                    .request
                    .customer_name
                    .clone()
                    .map(|name| crate::utils::normalize_cardholder_name(Secret::new(name)));

                get_worldpayxml_wallet_payment_method(wallet_data, customer_name)?
            }
            _ => {
                return Err(IntegrationError::NotSupported {
                    message: "Selected payment method".to_string(),
                    connector: "worldpayxml",
                    context: Default::default(),
                }
                .into())
            }
        };

        let authenticated_shopper_id =
            get_worldpayxml_authenticated_shopper_id(&router_data.resource_common_data, true)?;

        let converted_amount = super::WorldpayxmlAmountConvertor::convert(
            router_data
                .request
                .minor_amount
                .unwrap_or_else(common_utils::types::MinorUnit::zero),
            router_data.request.currency,
        )?;

        Ok(Self {
            version: API_VERSION.to_string(),
            merchant_code: auth.merchant_code,
            submit: requests::WorldpayxmlSubmit {
                order: requests::WorldpayxmlOrder {
                    order_code: router_data
                        .resource_common_data
                        .connector_request_reference_id
                        .clone(),
                    capture_delay: if is_manual_capture {
                        "OFF".to_string()
                    } else {
                        "0".to_string()
                    },
                    description: router_data
                        .resource_common_data
                        .description
                        .clone()
                        .unwrap_or_else(|| DEFAULT_PAYMENT_DESCRIPTION.to_string()),
                    amount: requests::WorldpayxmlAmount {
                        value: converted_amount,
                        currency_code: router_data.request.currency,
                        exponent: get_worldpayxml_exponent(router_data.request.currency),
                    },
                    payment_details: requests::WorldpayxmlPaymentDetails {
                        action: Some(if is_manual_capture {
                            WorldpayxmlAction::Authorise
                        } else {
                            WorldpayxmlAction::Sale
                        }),
                        payment_method,
                        stored_credentials: Some(requests::WorldpayxmlStoredCredentials {
                            usage: requests::WorldpayxmlUsageType::First,
                            customer_initiated_reason: Some(get_worldpayxml_mandate_type(
                                router_data.request.mit_category.clone(),
                            )),
                            merchant_initiated_reason: None,
                            scheme_transaction_identifier: None,
                        }),
                    },
                    shopper: requests::WorldpayxmlShopper {
                        shopper_email_address: router_data.request.email.clone(),
                        authenticated_shopper_id,
                        browser: router_data
                            .request
                            .browser_info
                            .as_ref()
                            .map(|browser_info| requests::WorldpayxmlBrowser {
                                accept_header: browser_info.accept_header.clone(),
                                user_agent_header: browser_info.user_agent.clone(),
                                http_accept_language: browser_info.accept_language.clone(),
                                time_zone: browser_info.time_zone,
                                browser_language: browser_info.language.clone(),
                                browser_java_enabled: browser_info.java_enabled,
                                browser_java_script_enabled: browser_info.java_script_enabled,
                                browser_colour_depth: browser_info.color_depth.map(u32::from),
                                browser_screen_height: browser_info.screen_height,
                                browser_screen_width: browser_info.screen_width,
                            }),
                    },
                    billing_address,
                    create_token: Some(requests::WorldpayxmlCreateToken {
                        token_scope: TOKEN_SCOPE_SHOPPER.to_string(),
                        token_event_reference: router_data
                            .resource_common_data
                            .connector_request_reference_id
                            .clone(),
                    }),
                },
            },
        })
    }
}

// RepeatPayment flow transformers
//
// A merchant-initiated payment replays the stored-credential agreement: the Worldpay token
// issued on the customer-initiated transaction is submitted over `TOKEN-SSL`, and the scheme
// transaction identifier from that original transaction chains the two together. No `action`
// is sent — Worldpay derives it from the order's capture delay, matching hyperswitch.
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        WorldpayxmlRouterData<
            RouterDataV2<
                RepeatPayment,
                PaymentFlowData,
                RepeatPaymentData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for requests::WorldpayxmlRepeatPaymentRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: WorldpayxmlRouterData<
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
        let auth = WorldpayxmlAuthType::try_from(&router_data.connector_config)?;

        let connector_mandate = match &router_data.request.mandate_reference {
            MandateReferenceId::ConnectorMandateId(connector_mandate) => connector_mandate,
            MandateReferenceId::NetworkMandateId(_)
            | MandateReferenceId::NetworkTokenWithNTI(_) => {
                return Err(IntegrationError::NotSupported {
                    message: "Merchant initiated payment without a connector mandate id"
                        .to_string(),
                    connector: "worldpayxml",
                    context: Default::default(),
                }
                .into())
            }
        };

        let payment_token_id = connector_mandate.get_connector_mandate_id().ok_or(
            IntegrationError::MissingRequiredField {
                field_name: "connector_mandate_id",
                context: Default::default(),
            },
        )?;

        let is_manual_capture = !router_data.request.is_auto_capture();

        let authenticated_shopper_id =
            get_worldpayxml_authenticated_shopper_id(&router_data.resource_common_data, true)?;

        let converted_amount = super::WorldpayxmlAmountConvertor::convert(
            router_data.request.minor_amount,
            router_data.request.currency,
        )?;

        Ok(Self {
            version: API_VERSION.to_string(),
            merchant_code: auth.merchant_code,
            submit: requests::WorldpayxmlSubmit {
                order: requests::WorldpayxmlOrder {
                    order_code: router_data
                        .resource_common_data
                        .connector_request_reference_id
                        .clone(),
                    capture_delay: if is_manual_capture {
                        "OFF".to_string()
                    } else {
                        "0".to_string()
                    },
                    description: router_data
                        .resource_common_data
                        .description
                        .clone()
                        .unwrap_or_else(|| DEFAULT_PAYMENT_DESCRIPTION.to_string()),
                    amount: requests::WorldpayxmlAmount {
                        value: converted_amount,
                        currency_code: router_data.request.currency,
                        exponent: get_worldpayxml_exponent(router_data.request.currency),
                    },
                    payment_details: requests::WorldpayxmlPaymentDetails {
                        action: None,
                        payment_method: requests::WorldpayxmlPaymentMethod::TokenSsl(
                            requests::WorldpayxmlTokenData {
                                token_scope: Secret::new(TOKEN_SCOPE_SHOPPER.to_string()),
                                payment_token_id: Secret::new(payment_token_id),
                            },
                        ),
                        stored_credentials: Some(requests::WorldpayxmlStoredCredentials {
                            usage: requests::WorldpayxmlUsageType::Used,
                            customer_initiated_reason: None,
                            merchant_initiated_reason: Some(get_worldpayxml_mandate_type(
                                router_data.request.mit_category.clone(),
                            )),
                            // Only sent when the customer-initiated transaction reported one;
                            // Worldpay accepts the merchant-initiated payment without it.
                            scheme_transaction_identifier: connector_mandate
                                .get_connector_mandate_request_reference_id()
                                .map(Secret::new),
                        }),
                    },
                    shopper: requests::WorldpayxmlShopper {
                        shopper_email_address: router_data.request.email.clone(),
                        authenticated_shopper_id,
                        browser: None,
                    },
                    billing_address: get_worldpayxml_billing_address(
                        &router_data.resource_common_data,
                    ),
                    create_token: None,
                },
            },
        })
    }
}

// Capture flow transformers
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        WorldpayxmlRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for requests::WorldpayxmlCaptureRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: WorldpayxmlRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = WorldpayxmlAuthType::try_from(&router_data.connector_config)?;

        // Extract connector_transaction_id from request
        let connector_transaction_id = router_data
            .request
            .connector_transaction_id
            .get_connector_transaction_id()
            .change_context(IntegrationError::MissingConnectorTransactionID {
                context: Default::default(),
            })?;

        // Convert amount using the connector's amount converter
        let converted_amount = super::WorldpayxmlAmountConvertor::convert(
            router_data.request.minor_amount_to_capture,
            router_data.request.currency,
        )?;

        Ok(Self {
            version: API_VERSION.to_string(),
            merchant_code: auth.merchant_code,
            modify: requests::WorldpayxmlModify {
                order_modification: requests::WorldpayxmlOrderModification {
                    order_code: connector_transaction_id.clone(),
                    capture: requests::WorldpayxmlCapture {
                        amount: requests::WorldpayxmlAmount {
                            value: converted_amount,
                            currency_code: router_data.request.currency,
                            exponent: get_worldpayxml_exponent(router_data.request.currency),
                        },
                    },
                },
            },
        })
    }
}

// Void flow transformers
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        WorldpayxmlRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for requests::WorldpayxmlVoidRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: WorldpayxmlRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = WorldpayxmlAuthType::try_from(&router_data.connector_config)?;

        // Extract connector_transaction_id from request
        let connector_transaction_id = router_data.request.connector_transaction_id.clone();

        Ok(Self {
            version: API_VERSION.to_string(),
            merchant_code: auth.merchant_code,
            modify: requests::WorldpayxmlVoidModify {
                order_modification: requests::WorldpayxmlVoidOrderModification {
                    order_code: connector_transaction_id,
                    cancel: requests::WorldpayxmlCancel {},
                },
            },
        })
    }
}

// Refund flow transformers
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        WorldpayxmlRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for requests::WorldpayxmlRefundRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: WorldpayxmlRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = WorldpayxmlAuthType::try_from(&router_data.connector_config)?;

        // Extract connector_transaction_id from request
        let connector_transaction_id = router_data.request.connector_transaction_id.clone();

        // Convert refund amount using the connector's amount converter
        let converted_amount = super::WorldpayxmlAmountConvertor::convert(
            router_data.request.minor_refund_amount,
            router_data.request.currency,
        )?;

        Ok(Self {
            version: API_VERSION.to_string(),
            merchant_code: auth.merchant_code,
            modify: requests::WorldpayxmlRefundModify {
                order_modification: requests::WorldpayxmlRefundOrderModification {
                    order_code: connector_transaction_id,
                    refund: requests::WorldpayxmlRefund {
                        amount: requests::WorldpayxmlAmount {
                            value: converted_amount,
                            currency_code: router_data.request.currency,
                            exponent: get_worldpayxml_exponent(router_data.request.currency),
                        },
                    },
                },
            },
        })
    }
}

// PSync flow transformers
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        WorldpayxmlRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for requests::WorldpayxmlPSyncRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: WorldpayxmlRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = WorldpayxmlAuthType::try_from(&router_data.connector_config)?;

        // Extract connector_transaction_id from request
        let connector_transaction_id = router_data
            .request
            .connector_transaction_id
            .get_connector_transaction_id()
            .change_context(IntegrationError::MissingConnectorTransactionID {
                context: Default::default(),
            })?;

        Ok(Self {
            version: API_VERSION.to_string(),
            merchant_code: auth.merchant_code,
            inquiry: requests::WorldpayxmlInquiry {
                order_inquiry: requests::WorldpayxmlOrderInquiry {
                    order_code: connector_transaction_id,
                },
            },
        })
    }
}

// RSync flow transformers - REUSE PSync request structure via type alias
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        WorldpayxmlRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    > for requests::WorldpayxmlRSyncRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: WorldpayxmlRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = WorldpayxmlAuthType::try_from(&router_data.connector_config)?;

        // Extract connector_refund_id from request
        // This could be either the connector_refund_id OR the original connector_transaction_id
        let order_code = router_data.request.connector_refund_id.clone();

        Ok(Self {
            version: API_VERSION.to_string(),
            merchant_code: auth.merchant_code,
            inquiry: requests::WorldpayxmlInquiry {
                order_inquiry: requests::WorldpayxmlOrderInquiry { order_code },
            },
        })
    }
}

// Helper function to map lastEvent to AttemptStatus
fn map_worldpayxml_authorize_status(
    last_event: &WorldpayxmlLastEvent,
    is_auto_capture: bool,
    previous_status: Option<&AttemptStatus>,
) -> AttemptStatus {
    match last_event {
        WorldpayxmlLastEvent::Authorised => {
            if is_auto_capture {
                AttemptStatus::Pending
            } else {
                // Check if we're in CaptureInitiated or VoidInitiated state
                match previous_status {
                    Some(AttemptStatus::CaptureInitiated) => AttemptStatus::CaptureInitiated,
                    Some(AttemptStatus::VoidInitiated) => AttemptStatus::VoidInitiated,
                    _ => AttemptStatus::Authorized,
                }
            }
        }
        WorldpayxmlLastEvent::Refused => AttemptStatus::Failure,
        WorldpayxmlLastEvent::Cancelled => AttemptStatus::Voided,
        WorldpayxmlLastEvent::Captured
        | WorldpayxmlLastEvent::Settled
        | WorldpayxmlLastEvent::SettledByMerchant => AttemptStatus::Charged,
        WorldpayxmlLastEvent::SentForAuthorisation => AttemptStatus::Authorizing,
        WorldpayxmlLastEvent::SentForRefund
        | WorldpayxmlLastEvent::SentForFastRefund
        | WorldpayxmlLastEvent::RefundRequested
        | WorldpayxmlLastEvent::RefundReceived
        | WorldpayxmlLastEvent::QueryRequired => AttemptStatus::Pending,
        WorldpayxmlLastEvent::Refunded | WorldpayxmlLastEvent::RefundedByMerchant => {
            AttemptStatus::Charged
        }
        WorldpayxmlLastEvent::CancelReceived => AttemptStatus::VoidInitiated,
        WorldpayxmlLastEvent::RefundFailed => AttemptStatus::Failure,
        WorldpayxmlLastEvent::Expired => AttemptStatus::Failure,
        WorldpayxmlLastEvent::Error => AttemptStatus::Failure,

        WorldpayxmlLastEvent::PushRequested
        | WorldpayxmlLastEvent::PushPending
        | WorldpayxmlLastEvent::PushApproved
        | WorldpayxmlLastEvent::PushRefused => AttemptStatus::Failure,
        WorldpayxmlLastEvent::Unknown => retain_previous_attempt_status(previous_status),
    }
}

/// Maps `lastEvent` for a mandate setup, where an authorisation already completes the flow —
/// there is no capture to wait for on a (usually zero-amount) verification order.
fn map_worldpayxml_setup_mandate_status(
    last_event: &WorldpayxmlLastEvent,
    previous_status: Option<&AttemptStatus>,
) -> AttemptStatus {
    match last_event {
        WorldpayxmlLastEvent::Authorised
        | WorldpayxmlLastEvent::Captured
        | WorldpayxmlLastEvent::Settled
        | WorldpayxmlLastEvent::SettledByMerchant => AttemptStatus::Charged,
        WorldpayxmlLastEvent::SentForAuthorisation => AttemptStatus::Authorizing,
        WorldpayxmlLastEvent::Cancelled => AttemptStatus::Voided,
        WorldpayxmlLastEvent::Refused
        | WorldpayxmlLastEvent::SentForRefund
        | WorldpayxmlLastEvent::SentForFastRefund
        | WorldpayxmlLastEvent::Refunded
        | WorldpayxmlLastEvent::RefundRequested
        | WorldpayxmlLastEvent::RefundedByMerchant
        | WorldpayxmlLastEvent::RefundReceived
        | WorldpayxmlLastEvent::RefundFailed
        | WorldpayxmlLastEvent::CancelReceived
        | WorldpayxmlLastEvent::QueryRequired
        | WorldpayxmlLastEvent::Expired
        | WorldpayxmlLastEvent::Error
        | WorldpayxmlLastEvent::PushRequested
        | WorldpayxmlLastEvent::PushPending
        | WorldpayxmlLastEvent::PushApproved
        | WorldpayxmlLastEvent::PushRefused => AttemptStatus::Failure,
        WorldpayxmlLastEvent::Unknown => retain_previous_attempt_status(previous_status),
    }
}

/// Worldpay keeps introducing journey events. An unrecognised one says nothing about the order, so
/// hold the status we already had rather than guessing at a terminal state.
fn retain_previous_attempt_status(previous_status: Option<&AttemptStatus>) -> AttemptStatus {
    let status = previous_status.copied().unwrap_or_default();
    tracing::warn!(
        retained_status = ?status,
        "worldpayxml: unknown lastEvent received; retaining previous attempt status"
    );
    status
}

/// Builds the mandate reference Hyperswitch stores for later merchant-initiated payments.
///
/// The Worldpay payment token is what the merchant-initiated payment is charged against, and the
/// scheme transaction identifier chains it back to the customer-initiated transaction.
fn get_worldpayxml_mandate_reference(
    order_status: &responses::WorldpayxmlOrderStatus,
    payment: &responses::WorldpayxmlPayment,
) -> Option<Box<MandateReference>> {
    order_status.token.as_ref().map(|token| {
        Box::new(MandateReference {
            connector_mandate_id: Some(token.token_details.payment_token_id.peek().to_string()),
            payment_method_id: None,
            mandate_metadata: None,
            connector_mandate_request_reference_id: payment
                .scheme_response
                .as_ref()
                .map(|scheme_response| scheme_response.transaction_identifier.clone()),
        })
    })
}

// Helper function to map lastEvent to RefundStatus
fn map_worldpayxml_refund_status(
    last_event: &WorldpayxmlLastEvent,
    previous_status: RefundStatus,
) -> RefundStatus {
    match last_event {
        WorldpayxmlLastEvent::Refunded | WorldpayxmlLastEvent::RefundedByMerchant => {
            RefundStatus::Success
        }
        WorldpayxmlLastEvent::RefundFailed => RefundStatus::Failure,
        WorldpayxmlLastEvent::Unknown => {
            // An unrecognised event says nothing about the refund, so hold the status we already
            // had rather than reporting a terminal one.
            tracing::warn!(
                retained_status = ?previous_status,
                "worldpayxml: unknown lastEvent received; retaining previous refund status"
            );
            previous_status
        }
        _ => RefundStatus::Pending, // Refund is still in flight
    }
}

// Response transformers - Authorize
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<responses::WorldpayxmlAuthorizeResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<responses::WorldpayxmlAuthorizeResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // Check for top-level error first
        if let Some(error) = &response.reply.error {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::Failure,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    code: error.code.clone(),
                    message: error.message.clone(),
                    reason: Some(error.message.clone()),
                    status_code: item.http_code,
                    attempt_status: Some(FlowStatus::Payment(AttemptStatus::Failure)),
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                }),
                ..router_data.clone()
            });
        }

        // Extract order status
        let order_status = response.reply.order_status.as_ref().ok_or(
            crate::utils::response_deserialization_fail(item.http_code, "worldpayxml: response body did not match the expected format; confirm API version and connector documentation."),
        )?;

        // Check for error in order status
        if let Some(error) = &order_status.error {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::Failure,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    code: error.code.clone(),
                    message: error.message.clone(),
                    reason: Some(error.message.clone()),
                    status_code: item.http_code,
                    attempt_status: Some(FlowStatus::Payment(AttemptStatus::Failure)),
                    connector_transaction_id: Some(order_status.order_code.clone()),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                }),
                ..router_data.clone()
            });
        }

        // Extract payment details
        let payment = order_status.payment.as_ref().ok_or(
            crate::utils::response_deserialization_fail(item.http_code, "worldpayxml: response body did not match the expected format; confirm API version and connector documentation."),
        )?;

        // Determine if auto-capture
        let is_auto_capture = router_data.request.capture_method != Some(CaptureMethod::Manual)
            && router_data.request.capture_method != Some(CaptureMethod::ManualMultiple);

        // Map status from lastEvent
        let status = map_worldpayxml_authorize_status(
            &payment.last_event,
            is_auto_capture,
            Some(&router_data.resource_common_data.status),
        );

        // Build success response
        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(order_status.order_code.clone()),
            redirection_data: None,
            mandate_reference: get_worldpayxml_mandate_reference(order_status, payment),
            connector_metadata: None,
            network_txn_id: payment
                .authorisation_id
                .as_ref()
                .and_then(|auth_id| auth_id.id.clone()),
            network_txn_link_id: None,
            connector_response_reference_id: Some(order_status.order_code.clone()),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
            splits: None,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(payments_response_data),
            ..router_data.clone()
        })
    }
}

// Response transformers - SetupMandate
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<responses::WorldpayxmlSetupMandateResponse, Self>>
    for RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<T>,
        PaymentsResponseData,
    >
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<responses::WorldpayxmlSetupMandateResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        if let Some(error) = &response.reply.error {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::Failure,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    code: error.code.clone(),
                    message: error.message.clone(),
                    reason: Some(error.message.clone()),
                    status_code: item.http_code,
                    attempt_status: Some(FlowStatus::Payment(AttemptStatus::Failure)),
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                }),
                ..router_data.clone()
            });
        }

        let order_status = response.reply.order_status.as_ref().ok_or(
            crate::utils::response_deserialization_fail(item.http_code, "worldpayxml: response body did not match the expected format; confirm API version and connector documentation."),
        )?;

        if let Some(error) = &order_status.error {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::Failure,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    code: error.code.clone(),
                    message: error.message.clone(),
                    reason: Some(error.message.clone()),
                    status_code: item.http_code,
                    attempt_status: Some(FlowStatus::Payment(AttemptStatus::Failure)),
                    connector_transaction_id: Some(order_status.order_code.clone()),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                }),
                ..router_data.clone()
            });
        }

        let payment = order_status.payment.as_ref().ok_or(
            crate::utils::response_deserialization_fail(item.http_code, "worldpayxml: response body did not match the expected format; confirm API version and connector documentation."),
        )?;

        let status = map_worldpayxml_setup_mandate_status(
            &payment.last_event,
            Some(&router_data.resource_common_data.status),
        );

        if status == AttemptStatus::Failure {
            let return_code = payment.iso8583_return_code.as_ref();
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    code: return_code.map_or_else(
                        || common_utils::consts::NO_ERROR_CODE.to_string(),
                        |code| code.code.clone(),
                    ),
                    message: return_code.map_or_else(
                        || common_utils::consts::NO_ERROR_MESSAGE.to_string(),
                        |code| code.description.clone(),
                    ),
                    reason: return_code.map(|code| code.description.clone()),
                    status_code: item.http_code,
                    attempt_status: Some(FlowStatus::Payment(status)),
                    connector_transaction_id: Some(order_status.order_code.clone()),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                }),
                ..router_data.clone()
            });
        }

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(order_status.order_code.clone()),
            redirection_data: None,
            mandate_reference: get_worldpayxml_mandate_reference(order_status, payment),
            connector_metadata: None,
            network_txn_id: payment
                .authorisation_id
                .as_ref()
                .and_then(|auth_id| auth_id.id.clone()),
            network_txn_link_id: None,
            connector_response_reference_id: Some(order_status.order_code.clone()),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
            splits: None,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(payments_response_data),
            ..router_data.clone()
        })
    }
}

// Response transformers - RepeatPayment
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<responses::WorldpayxmlRepeatPaymentResponse, Self>>
    for RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData<T>, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<responses::WorldpayxmlRepeatPaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        if let Some(error) = &response.reply.error {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::Failure,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    code: error.code.clone(),
                    message: error.message.clone(),
                    reason: Some(error.message.clone()),
                    status_code: item.http_code,
                    attempt_status: Some(FlowStatus::Payment(AttemptStatus::Failure)),
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                }),
                ..router_data.clone()
            });
        }

        let order_status = response.reply.order_status.as_ref().ok_or(
            crate::utils::response_deserialization_fail(item.http_code, "worldpayxml: response body did not match the expected format; confirm API version and connector documentation."),
        )?;

        if let Some(error) = &order_status.error {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::Failure,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    code: error.code.clone(),
                    message: error.message.clone(),
                    reason: Some(error.message.clone()),
                    status_code: item.http_code,
                    attempt_status: Some(FlowStatus::Payment(AttemptStatus::Failure)),
                    connector_transaction_id: Some(order_status.order_code.clone()),
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                }),
                ..router_data.clone()
            });
        }

        let payment = order_status.payment.as_ref().ok_or(
            crate::utils::response_deserialization_fail(item.http_code, "worldpayxml: response body did not match the expected format; confirm API version and connector documentation."),
        )?;

        let status = map_worldpayxml_authorize_status(
            &payment.last_event,
            router_data.request.is_auto_capture(),
            Some(&router_data.resource_common_data.status),
        );

        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(order_status.order_code.clone()),
            redirection_data: None,
            mandate_reference: get_worldpayxml_mandate_reference(order_status, payment),
            connector_metadata: None,
            network_txn_id: payment
                .authorisation_id
                .as_ref()
                .and_then(|auth_id| auth_id.id.clone()),
            network_txn_link_id: None,
            connector_response_reference_id: Some(order_status.order_code.clone()),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
            splits: None,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(payments_response_data),
            ..router_data.clone()
        })
    }
}

// Response transformers - Capture
impl TryFrom<ResponseRouterData<responses::WorldpayxmlCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<responses::WorldpayxmlCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // Check for top-level error first
        if let Some(error) = &response.reply.error {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::CaptureFailed,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    code: error.code.clone(),
                    message: error.message.clone(),
                    reason: Some(error.message.clone()),
                    status_code: item.http_code,
                    attempt_status: Some(FlowStatus::Payment(AttemptStatus::CaptureFailed)),
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                }),
                ..router_data.clone()
            });
        }

        // Extract ok response
        let ok_response = response.reply.ok.as_ref().ok_or(
            crate::utils::response_deserialization_fail(item.http_code, "worldpayxml: response body did not match the expected format; confirm API version and connector documentation."),
        )?;

        // Extract captureReceived
        let capture_received = &ok_response.capture_received;

        // Build success response
        // Status is CaptureInitiated (capture confirmed but not yet processed)
        // Actual completion must be verified via PSync
        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(capture_received.order_code.clone()),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            network_txn_link_id: None,
            connector_response_reference_id: Some(capture_received.order_code.clone()),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
            splits: None,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: AttemptStatus::CaptureInitiated,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(payments_response_data),
            ..router_data.clone()
        })
    }
}

// Response transformers - Void
impl TryFrom<ResponseRouterData<responses::WorldpayxmlVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<responses::WorldpayxmlVoidResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // Check for top-level error first
        if let Some(error) = &response.reply.error {
            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::VoidFailed,
                    ..router_data.resource_common_data.clone()
                },
                response: Err(ErrorResponse {
                    code: error.code.clone(),
                    message: error.message.clone(),
                    reason: Some(error.message.clone()),
                    status_code: item.http_code,
                    attempt_status: Some(FlowStatus::Payment(AttemptStatus::VoidFailed)),
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                }),
                ..router_data.clone()
            });
        }

        // Extract ok response
        let ok_response = response.reply.ok.as_ref().ok_or(
            crate::utils::response_deserialization_fail(item.http_code, "worldpayxml: response body did not match the expected format; confirm API version and connector documentation."),
        )?;

        // Extract cancelReceived
        let cancel_received = &ok_response.cancel_received;

        // Build success response
        // Status is VoidInitiated (cancellation confirmed but not yet processed)
        // Actual completion must be verified via PSync
        let payments_response_data = PaymentsResponseData::TransactionResponse {
            resource_id: ResponseId::ConnectorTransactionId(cancel_received.order_code.clone()),
            redirection_data: None,
            mandate_reference: None,
            connector_metadata: None,
            network_txn_id: None,
            network_txn_link_id: None,
            connector_response_reference_id: Some(cancel_received.order_code.clone()),
            incremental_authorization_allowed: None,
            status_code: item.http_code,
            splits: None,
        };

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status: AttemptStatus::VoidInitiated,
                ..router_data.resource_common_data.clone()
            },
            response: Ok(payments_response_data),
            ..router_data.clone()
        })
    }
}

// Response transformers - PSync
impl TryFrom<ResponseRouterData<responses::WorldpayxmlTransactionResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<responses::WorldpayxmlTransactionResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Match on the response enum to handle both XML and JSON formats
        match &item.response {
            responses::WorldpayxmlTransactionResponse::Payment(xml_response) => {
                // Process XML response (same structure as Authorize)
                let response = xml_response.as_ref();

                // Check for top-level error first
                if let Some(error) = &response.reply.error {
                    return Ok(Self {
                        resource_common_data: PaymentFlowData {
                            status: AttemptStatus::Failure,
                            ..router_data.resource_common_data.clone()
                        },
                        response: Err(ErrorResponse {
                            code: error.code.clone(),
                            message: error.message.clone(),
                            reason: Some(error.message.clone()),
                            status_code: item.http_code,
                            attempt_status: Some(FlowStatus::Payment(AttemptStatus::Failure)),
                            connector_transaction_id: None,
                            network_decline_code: None,
                            network_advice_code: None,
                            network_error_message: None,
                            typed_connector_response: None,
                            raw_connector_response: None,
                            raw_connector_request: None,
                            typed_connector_request: None,
                        }),
                        ..router_data.clone()
                    });
                }

                // Extract order status
                let order_status = response.reply.order_status.as_ref().ok_or(
                    crate::utils::response_deserialization_fail(
                        item.http_code,
                    "worldpayxml: response body did not match the expected format; confirm API version and connector documentation."),
                )?;

                // Special handling: If error exists but payment is None, return current status (don't fail)
                if let Some(error) = &order_status.error {
                    if order_status.payment.is_none() {
                        // Error exists but no payment data - return current status as Pending
                        let payments_response_data = PaymentsResponseData::TransactionResponse {
                            resource_id: ResponseId::ConnectorTransactionId(
                                order_status.order_code.clone(),
                            ),
                            redirection_data: None,
                            mandate_reference: None,
                            connector_metadata: None,
                            network_txn_id: None,
                            network_txn_link_id: None,
                            connector_response_reference_id: Some(order_status.order_code.clone()),
                            incremental_authorization_allowed: None,
                            status_code: item.http_code,
                            splits: None,
                        };

                        return Ok(Self {
                            resource_common_data: PaymentFlowData {
                                status: AttemptStatus::Pending,
                                ..router_data.resource_common_data.clone()
                            },
                            response: Ok(payments_response_data),
                            ..router_data.clone()
                        });
                    }

                    // Error exists with payment data - fail the payment
                    return Ok(Self {
                        resource_common_data: PaymentFlowData {
                            status: AttemptStatus::Failure,
                            ..router_data.resource_common_data.clone()
                        },
                        response: Err(ErrorResponse {
                            code: error.code.clone(),
                            message: error.message.clone(),
                            reason: Some(error.message.clone()),
                            status_code: item.http_code,
                            attempt_status: Some(FlowStatus::Payment(AttemptStatus::Failure)),
                            connector_transaction_id: Some(order_status.order_code.clone()),
                            network_decline_code: None,
                            network_advice_code: None,
                            network_error_message: None,
                            typed_connector_response: None,
                            raw_connector_response: None,
                            raw_connector_request: None,
                            typed_connector_request: None,
                        }),
                        ..router_data.clone()
                    });
                }

                // Extract payment details
                let payment = order_status.payment.as_ref().ok_or(
                    crate::utils::response_deserialization_fail(
                        item.http_code,
                    "worldpayxml: response body did not match the expected format; confirm API version and connector documentation."),
                )?;

                // Determine if auto-capture from request data
                let is_auto_capture = router_data.request.capture_method
                    != Some(CaptureMethod::Manual)
                    && router_data.request.capture_method != Some(CaptureMethod::ManualMultiple);

                // Map status from lastEvent - reuse the helper function
                let status = map_worldpayxml_authorize_status(
                    &payment.last_event,
                    is_auto_capture,
                    Some(&router_data.resource_common_data.status),
                );

                // Build success response
                let payments_response_data = PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(
                        order_status.order_code.clone(),
                    ),
                    redirection_data: None,
                    mandate_reference: None,
                    connector_metadata: None,
                    network_txn_id: payment
                        .authorisation_id
                        .as_ref()
                        .and_then(|auth_id| auth_id.id.clone()),
                    network_txn_link_id: None,
                    connector_response_reference_id: Some(order_status.order_code.clone()),
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                    splits: None,
                };

                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status,
                        ..router_data.resource_common_data.clone()
                    },
                    response: Ok(payments_response_data),
                    ..router_data.clone()
                })
            }
            responses::WorldpayxmlTransactionResponse::Webhook(webhook_response) => {
                // Process order-notification body
                let order_code = webhook_response.order_code.clone();

                // Determine if auto-capture from request data
                let is_auto_capture = router_data.request.capture_method
                    != Some(CaptureMethod::Manual)
                    && router_data.request.capture_method != Some(CaptureMethod::ManualMultiple);

                // Map status from PaymentStatus
                let status = map_worldpayxml_authorize_status(
                    &webhook_response.payment_status,
                    is_auto_capture,
                    Some(&router_data.resource_common_data.status),
                );

                // Build success response
                let payments_response_data = PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(order_code.clone()),
                    redirection_data: None,
                    mandate_reference: None,
                    connector_metadata: None,
                    network_txn_id: None,
                    network_txn_link_id: None,
                    connector_response_reference_id: Some(order_code),
                    incremental_authorization_allowed: None,
                    status_code: item.http_code,
                    splits: None,
                };

                Ok(Self {
                    resource_common_data: PaymentFlowData {
                        status,
                        ..router_data.resource_common_data.clone()
                    },
                    response: Ok(payments_response_data),
                    ..router_data.clone()
                })
            }
        }
    }
}

// Response transformers - Refund
impl TryFrom<ResponseRouterData<responses::WorldpayxmlRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<responses::WorldpayxmlRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // Check for top-level error first
        if let Some(error) = &response.reply.error {
            return Ok(Self {
                response: Err(ErrorResponse {
                    code: error.code.clone(),
                    message: error.message.clone(),
                    reason: Some(error.message.clone()),
                    status_code: item.http_code,
                    attempt_status: None,
                    connector_transaction_id: None,
                    network_decline_code: None,
                    network_advice_code: None,
                    network_error_message: None,
                    typed_connector_response: None,
                    raw_connector_response: None,
                    raw_connector_request: None,
                    typed_connector_request: None,
                }),
                ..router_data.clone()
            });
        }

        // Extract ok response
        let ok_response = response.reply.ok.as_ref().ok_or(
            crate::utils::response_deserialization_fail(item.http_code, "worldpayxml: response body did not match the expected format; confirm API version and connector documentation."),
        )?;

        // Extract refundReceived
        let refund_received = &ok_response.refund_received;

        // Build success response
        // Status is Pending (refund initiated but not completed)
        // Actual completion must be verified via RSync
        let refunds_response_data = RefundsResponseData {
            connector_refund_id: refund_received.order_code.clone(),
            refund_status: RefundStatus::Pending,
            status_code: item.http_code,
            acquirer_reference_number: None,
        };

        Ok(Self {
            response: Ok(refunds_response_data),
            ..router_data.clone()
        })
    }
}

// Response transformers - RSync (REUSE PSync response structure via type alias)
impl TryFrom<ResponseRouterData<responses::WorldpayxmlRsyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<responses::WorldpayxmlRsyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;

        // Match on the response enum to handle both XML and JSON formats (same as PSync)
        match &item.response {
            responses::WorldpayxmlTransactionResponse::Payment(xml_response) => {
                // Process XML response
                let response = xml_response.as_ref();

                // Check for top-level error first
                if let Some(error) = &response.reply.error {
                    return Ok(Self {
                        response: Err(crate::utils::build_error_response(
                            error.code.clone(),
                            error.message.clone(),
                            item.http_code,
                            None,
                        )),
                        ..router_data.clone()
                    });
                }

                // Extract order status
                let order_status = response.reply.order_status.as_ref().ok_or(
                    crate::utils::response_deserialization_fail(
                        item.http_code,
                    "worldpayxml: response body did not match the expected format; confirm API version and connector documentation."),
                )?;

                // Special handling: If error exists but payment is None, return Pending (don't fail)
                if let Some(_error) = &order_status.error {
                    if order_status.payment.is_none() {
                        // Error exists but no payment data - return current status as Pending
                        let refunds_response_data = RefundsResponseData {
                            connector_refund_id: order_status.order_code.clone(),
                            refund_status: RefundStatus::Pending,
                            status_code: item.http_code,
                            acquirer_reference_number: None,
                        };

                        return Ok(Self {
                            response: Ok(refunds_response_data),
                            ..router_data.clone()
                        });
                    }
                }

                // Extract payment details
                let payment = order_status.payment.as_ref().ok_or(
                    crate::utils::response_deserialization_fail(
                        item.http_code,
                    "worldpayxml: response body did not match the expected format; confirm API version and connector documentation."),
                )?;

                // Map status from lastEvent using refund status mapping
                let refund_status = map_worldpayxml_refund_status(
                    &payment.last_event,
                    router_data.request.refund_status,
                );

                // Check if refund failed and extract error details from ISO8583ReturnCode
                if refund_status == RefundStatus::Failure {
                    if let Some(return_code) = &payment.iso8583_return_code {
                        return Ok(Self {
                            response: Err(ErrorResponse {
                                code: return_code.code.clone(),
                                message: return_code.description.clone(),
                                reason: Some(return_code.description.clone()),
                                status_code: item.http_code,
                                attempt_status: None,
                                connector_transaction_id: Some(order_status.order_code.clone()),
                                network_decline_code: None,
                                network_advice_code: None,
                                network_error_message: None,
                                typed_connector_response: None,
                                raw_connector_response: None,
                                raw_connector_request: None,
                                typed_connector_request: None,
                            }),
                            ..router_data.clone()
                        });
                    }
                }

                // Build success response
                let refunds_response_data = RefundsResponseData {
                    connector_refund_id: order_status.order_code.clone(),
                    refund_status,
                    status_code: item.http_code,
                    acquirer_reference_number: None,
                };

                Ok(Self {
                    response: Ok(refunds_response_data),
                    ..router_data.clone()
                })
            }
            responses::WorldpayxmlTransactionResponse::Webhook(webhook_response) => {
                // Process order-notification body
                let order_code = webhook_response.order_code.clone();

                // Map status from PaymentStatus using refund status mapping
                let refund_status = map_worldpayxml_refund_status(
                    &webhook_response.payment_status,
                    router_data.request.refund_status,
                );

                // Build success response
                let refunds_response_data = RefundsResponseData {
                    connector_refund_id: order_code,
                    refund_status,
                    status_code: item.http_code,
                    acquirer_reference_number: None,
                };

                Ok(Self {
                    response: Ok(refunds_response_data),
                    ..router_data.clone()
                })
            }
        }
    }
}

// ===== VOID POST CAPTURE TRANSFORMERS =====
// Uses <cancelOrRefund/> element which acts as cancel if pre-settlement, refund if post-settlement
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        WorldpayxmlRouterData<
            RouterDataV2<
                VoidPC,
                PaymentFlowData,
                PaymentsCancelPostCaptureData,
                PaymentsResponseData,
            >,
            T,
        >,
    > for requests::WorldpayxmlVoidPCRequest
{
    type Error = Report<IntegrationError>;

    fn try_from(
        item: WorldpayxmlRouterData<
            RouterDataV2<
                VoidPC,
                PaymentFlowData,
                PaymentsCancelPostCaptureData,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = WorldpayxmlAuthType::try_from(&router_data.connector_config)?;

        // connector_transaction_id is a String directly in PaymentsCancelPostCaptureData
        let order_code = router_data.request.connector_transaction_id.clone();

        Ok(Self {
            version: API_VERSION.to_string(),
            merchant_code: auth.merchant_code,
            modify: requests::WorldpayxmlVoidPCModify {
                order_modification: requests::WorldpayxmlVoidPCOrderModification {
                    order_code,
                    cancel_or_refund: requests::WorldpayxmlCancelOrRefund {},
                },
            },
        })
    }
}

// Response transformer - VoidPostCapture
impl TryFrom<ResponseRouterData<responses::WorldpayxmlVoidPCResponse, Self>>
    for RouterDataV2<VoidPC, PaymentFlowData, PaymentsCancelPostCaptureData, PaymentsResponseData>
{
    type Error = Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<responses::WorldpayxmlVoidPCResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // Map a top-level <error> reply to PostCaptureVoidResponse with Failed status,
        // surfacing the connector's error message via `description` (per WorldpayVantiv /
        // Payload convention for VoidPC).
        if let Some(error) = &response.reply.error {
            let payments_response_data = PaymentsResponseData::PostCaptureVoidResponse {
                post_capture_void_status: common_enums::PostCaptureVoidStatus::Failed,
                connector_reference_id: Some(router_data.request.connector_transaction_id.clone()),
                description: Some(error.message.clone()),
                status_code: item.http_code,
            };
            return Ok(Self {
                response: Ok(payments_response_data),
                ..router_data.clone()
            });
        }

        // Extract ok response
        let ok_response = response.reply.ok.as_ref().ok_or(
            crate::utils::response_deserialization_fail(item.http_code, "worldpayxml: response body did not match the expected format; confirm API version and connector documentation."),
        )?;

        // Extract order_code from ok element — WorldpayXML may return it as:
        // 1. An attribute on <ok> itself: <ok orderCode="..."/>
        // 2. Inside <cancelOrRefundReceived orderCode="..."/>
        // 3. Inside <cancelReceived orderCode="..."/>
        // 4. Or <ok/> with no orderCode (use connector_transaction_id as fallback)
        let order_code = ok_response
            .order_code
            .clone()
            .or_else(|| {
                ok_response
                    .cancel_or_refund_received
                    .as_ref()
                    .map(|r| r.order_code.clone())
            })
            .or_else(|| {
                ok_response
                    .cancel_received
                    .as_ref()
                    .map(|r| r.order_code.clone())
            })
            .unwrap_or_else(|| router_data.request.connector_transaction_id.clone());

        // Build success response
        // WorldpayXML returns cancelReceived/cancelOrRefundReceived synchronously,
        // so the void is confirmed as Succeeded immediately.
        let payments_response_data = PaymentsResponseData::PostCaptureVoidResponse {
            post_capture_void_status: common_enums::PostCaptureVoidStatus::Succeeded,
            connector_reference_id: Some(order_code.clone()),
            description: None,
            status_code: item.http_code,
        };

        Ok(Self {
            response: Ok(payments_response_data),
            ..router_data.clone()
        })
    }
}
