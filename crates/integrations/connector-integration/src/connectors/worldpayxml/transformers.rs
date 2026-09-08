use std::fmt::Debug;

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD_ENGINE, Engine};
use common_enums::{AttemptStatus, CaptureMethod, RefundStatus};
use domain_types::{
    connector_flow::{
        Authorize, Capture, PSync, PreAuthenticate, RSync, Refund, RepeatPayment, SetupMandate,
        Void, VoidPC,
    },
    connector_types::{
        ContinueRedirectionResponse, MandateReference, MandateReferenceId, PaymentFlowData,
        PaymentVoidData, PaymentsAuthorizeData, PaymentsCancelPostCaptureData, PaymentsCaptureData,
        PaymentsPreAuthenticateData, PaymentsResponseData, PaymentsSyncData, RecipientAccount,
        RecipientBankAccount, RecipientDetails, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, RepeatPaymentData, ResponseId, SetupMandateRequestData,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    payment_method_data::{
        Card, GpayTokenizationData, PaymentMethodData, PaymentMethodDataTypes, WalletData,
    },
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    router_response_types::{RedirectForm, Response},
};
use error_stack::{Report, ResultExt};
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use serde::Serialize;

use super::{
    requests::{self, WorldpayxmlAction},
    responses::{self, WorldpayxmlLastEvent},
    WorldpayxmlRouterData,
};
use crate::{types::ResponseRouterData, utils};
use common_utils::{errors::CustomResult, pii::SecretSerdeValue};

const API_VERSION: &str = "1.4";
const WORLDPAYXML_SUPPORTED_3DS_MAJOR_VERSION: u64 = 2;

/// `captureDelay` value that leaves the order uncaptured, for manual capture.
const CAPTURE_DELAY_MANUAL: &str = "OFF";
const CAPTURE_DELAY_AUTOMATIC: &str = "0";

const WORLDPAYXML_FUNDING_CATEGORY: &str = "PULL_FROM_CARD";
const WORLDPAYXML_ACCOUNT_TYPE_BANK_ROUTING: &str = "01";
const WORLDPAYXML_ACCOUNT_TYPE_IBAN: &str = "02";
const WORLDPAYXML_ACCOUNT_TYPE_CARD: &str = "03";
const WORLDPAYXML_ACCOUNT_TYPE_EMAIL: &str = "04";
const WORLDPAYXML_ACCOUNT_TYPE_PHONE: &str = "05";
const WORLDPAYXML_ACCOUNT_TYPE_BANK_BIC: &str = "06";
const WORLDPAYXML_ACCOUNT_TYPE_WALLET: &str = "07";
const WORLDPAYXML_ACCOUNT_TYPE_SOCIAL: &str = "08";

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

impl From<&common_enums::TransactionStatus> for requests::WorldpayxmlTransactionStatus {
    fn from(status: &common_enums::TransactionStatus) -> Self {
        match status {
            common_enums::TransactionStatus::Success => Self::Success,
            common_enums::TransactionStatus::Failure => Self::Failure,
            common_enums::TransactionStatus::VerificationNotPerformed => {
                Self::VerificationNotPerformed
            }
            common_enums::TransactionStatus::NotVerified => Self::NotVerified,
            common_enums::TransactionStatus::Rejected => Self::Rejected,
            common_enums::TransactionStatus::ChallengeRequired => Self::ChallengeRequired,
            common_enums::TransactionStatus::ChallengeRequiredDecoupledAuthentication => {
                Self::ChallengeRequiredDecoupledAuthentication
            }
            common_enums::TransactionStatus::InformationOnly => Self::InformationOnly,
        }
    }
}

fn get_worldpayxml_info_3d_secure(
    authentication_data: Option<&domain_types::router_request_types::AuthenticationData>,
) -> Result<Option<requests::WorldpayxmlExternalInfo3DSecure>, Report<IntegrationError>> {
    let Some(authentication_data) = authentication_data else {
        return Ok(None);
    };

    let has_external_authentication_result = authentication_data.ds_trans_id.is_some()
        || authentication_data.cavv.is_some()
        || authentication_data.eci.is_some()
        || authentication_data.trans_status.is_some();

    let Some(message_version) = authentication_data.message_version.as_ref() else {
        // AuthenticationData can also carry non-3DS-result metadata (for example, an exemption).
        // Since Worldpay exemption mapping is intentionally out of scope, omit that metadata.
        if !has_external_authentication_result {
            return Ok(None);
        }

        return Err(IntegrationError::MissingRequiredField {
            field_name: "authentication_data.message_version",
            context: IntegrationErrorContext {
                additional_context: Some(
                    "Worldpay XML needs the 3DS protocol version to submit externally authenticated payment data."
                        .to_string(),
                ),
                suggested_action: Some(
                    "Pass the external authentication result with a 2.x message_version."
                        .to_string(),
                ),
                ..Default::default()
            },
        }
        .into());
    };

    if message_version.get_major() != WORLDPAYXML_SUPPORTED_3DS_MAJOR_VERSION {
        return Err(IntegrationError::FlowNotSupported {
            flow: format!("External 3DS {} authorization", message_version),
            connector: "worldpayxml".to_string(),
            context: IntegrationErrorContext {
                additional_context: Some(
                    "The Worldpay XML external-authentication mapping implemented by this connector supports only 3DS2 results."
                        .to_string(),
                ),
                suggested_action: Some(
                    "Pass a completed 3DS2 authentication result with a 2.x message_version."
                        .to_string(),
                ),
                ..Default::default()
            },
        }
        .into());
    }

    if authentication_data
        .eci
        .as_ref()
        .is_some_and(|eci| eci.len() != 2 || !eci.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return Err(IntegrationError::InvalidDataFormat {
            field_name: "authentication_data.eci",
            context: IntegrationErrorContext {
                additional_context: Some(
                    "Worldpay XML expects ECI as exactly two ASCII digits (for example, 05)."
                        .to_string(),
                ),
                suggested_action: Some(
                    "Pass the scheme-provided numeric ECI instead of a semantic authentication status."
                        .to_string(),
                ),
                ..Default::default()
            },
        }
        .into());
    }

    Ok(Some(requests::WorldpayxmlExternalInfo3DSecure {
        three_ds_version: message_version.to_string(),
        ds_transaction_id: authentication_data.ds_trans_id.clone(),
        cavv: authentication_data.cavv.clone(),
        eci: authentication_data.eci.clone(),
        transaction_status: authentication_data
            .trans_status
            .as_ref()
            .map(requests::WorldpayxmlTransactionStatus::from),
    }))
}

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
            let formatted_year = utils::pad_expiry_year_to_four_digits(&card.card_exp_year);

            let card_holder_name = utils::build_card_holder_name(
                &card.card_holder_name,
                billing_address.and_then(|b| b.address.first_name.clone()),
                billing_address.and_then(|b| b.address.last_name.clone()),
            )
            .map(utils::normalize_cardholder_name);

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
                            context: IntegrationErrorContext {
                                additional_context: Some(
                                    "The Apple Pay payload carried neither decrypted token data \
                                     nor an encrypted payment token."
                                        .to_string(),
                                ),
                                suggested_action: Some(
                                    "Send the PKPaymentToken's paymentData, or decrypt the token \
                                     before forwarding it."
                                        .to_string(),
                                ),
                                ..Default::default()
                            },
                        })?;

                    let decoded_data = BASE64_STANDARD_ENGINE
                        .decode(encrypted_data)
                        .change_context(IntegrationError::InvalidDataFormat {
                            field_name: "apple_pay_encrypted_data",
                            context: IntegrationErrorContext {
                                additional_context: Some(
                                    "The Apple Pay payment token is not valid base64.".to_string(),
                                ),
                                suggested_action: Some(
                                    "Forward the wallet payload exactly as the Apple Pay SDK \
                                     produced it."
                                        .to_string(),
                                ),
                                ..Default::default()
                            },
                        })?;

                    let apple_pay_token: requests::WorldpayxmlApplePayData =
                        serde_json::from_slice(&decoded_data).change_context(
                            IntegrationError::InvalidDataFormat {
                                field_name: "apple_pay_token_json",
                                context: IntegrationErrorContext {
                                    additional_context: Some("The decoded Apple Pay payment token is not the JSON envelope Worldpay expects (header, signature, version, data).".to_string()),
                                    suggested_action: Some("Forward the wallet payload exactly as the Apple Pay SDK produced it.".to_string()),
                                    ..Default::default()
                                },
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
                                context: IntegrationErrorContext {
                                    additional_context: Some("The decrypted Google Pay expiry year could not be widened to four digits.".to_string()),
                                    suggested_action: Some("Check the expiry year on the decrypted token.".to_string()),
                                    ..Default::default()
                                },
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
                    // No cryptogram: nothing token-specific left, so send the PAN as a card.
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
                    context: IntegrationErrorContext {
                        additional_context: Some("The Google Pay token is not the JSON envelope Worldpay expects (protocolVersion, signature, signedMessage).".to_string()),
                        suggested_action: Some("Forward the wallet payload exactly as the Google Pay SDK produced it.".to_string()),
                        ..Default::default()
                    },
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

/// Number of decimal places Worldpay expects for the order currency.
fn get_worldpayxml_exponent(
    currency: common_enums::Currency,
) -> Result<String, Report<IntegrationError>> {
    currency
        .number_of_digits_after_decimal_point()
        .map(|digits| digits.to_string())
        .map_err(|err| {
            IntegrationError::InvalidDataFormat {
                field_name: "currency",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Use an ISO 4217 currency Worldpay accepts (e.g. GBP, USD, EUR)."
                            .to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(format!(
                        "Currency {currency:?} has no known minor-unit exponent: {err}"
                    )),
                },
            }
            .into()
        })
}

fn get_worldpayxml_billing_address(
    resource_common_data: &PaymentFlowData,
) -> Option<requests::WorldpayxmlBillingAddress> {
    let billing = resource_common_data.address.get_payment_billing()?;
    let telephone_number = billing.phone.as_ref().and_then(|phone| {
        phone
            .get_number_with_country_code()
            .or_else(|_| phone.get_number())
            .ok()
    });
    let addr = billing.address.as_ref()?;

    let address1 = addr.get_optional_line1()?;
    let postal_code = addr.get_optional_zip()?;
    let city = addr.get_optional_city()?.expose();
    let country_code = addr.get_optional_country()?;

    Some(requests::WorldpayxmlBillingAddress {
        address: requests::WorldpayxmlAddress {
            first_name: addr.get_optional_first_name(),
            last_name: addr.get_optional_last_name(),
            address1,
            address2: addr.get_optional_line2(),
            address3: addr.line3.clone(),
            postal_code,
            city,
            state: addr.state.clone(),
            country_code,
            telephone_number,
        },
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
            context: IntegrationErrorContext {
                additional_context: Some(
                    "Worldpay ties shopper-scoped tokens to authenticatedShopperID, so a \
                     customer-initiated mandate cannot be registered without it."
                        .to_string(),
                ),
                suggested_action: Some(
                    "Send customer.connector_customer_id on the request.".to_string(),
                ),
                ..Default::default()
            },
        }
        .into()),
        None => Ok(None),
    }
}

impl From<Option<common_enums::MitCategory>> for requests::WorldpayxmlMandateType {
    fn from(mit_category: Option<common_enums::MitCategory>) -> Self {
        match mit_category {
            Some(common_enums::MitCategory::Installment) => Self::Instalment,
            Some(common_enums::MitCategory::Recurring) => Self::Recurring,
            Some(common_enums::MitCategory::Unscheduled)
            | Some(common_enums::MitCategory::Resubmission)
            | None => Self::Unscheduled,
        }
    }
}

pub(crate) fn parse_worldpayxml_challenge_return(
    redirect_response: Option<&ContinueRedirectionResponse>,
) -> Option<responses::WorldpayxmlRedirectionResponse> {
    redirect_response
        .and_then(|redirect| redirect.payload.as_ref())
        .and_then(|payload| serde_json::from_value(payload.peek().clone()).ok())
}

fn parse_worldpayxml_ddc_return(
    redirect_response: Option<&ContinueRedirectionResponse>,
) -> Option<responses::WorldpayxmlDdcRedirectResponse> {
    redirect_response
        .and_then(|redirect| redirect.payload.as_ref())
        .and_then(|payload| serde_json::from_value(payload.peek().clone()).ok())
}

/// Worldpay pins a 3DS challenge to the machine that issued it; the `machine` cookie captured
/// from the challenge response must be replayed on the completion leg.
pub(crate) fn get_worldpayxml_cookie(
    connector_feature_data: Option<&SecretSerdeValue>,
) -> Result<String, IntegrationError> {
    connector_feature_data
        .and_then(|data| data.peek().get("cookie"))
        .and_then(|value| value.as_str())
        .map(|cookie| cookie.to_string())
        .ok_or(IntegrationError::MissingRequiredField {
            field_name: "connector_feature_data.cookie",
            context: IntegrationErrorContext {
                additional_context: Some(
                    "the machine cookie captured on the challenge leg was not persisted"
                        .to_string(),
                ),
                suggested_action: Some(
                    "replay the connector metadata from the challenge response on the completion call"
                        .to_string(),
                ),
                doc_url: None,
            },
        })
}

pub(crate) fn sign_worldpayxml_jwt<C: Serialize>(
    claims: &C,
    jwt_mac_key: &Secret<String>,
    http_code: u16,
) -> Result<String, Report<ConnectorError>> {
    let claims_map = match serde_json::to_value(claims) {
        Ok(serde_json::Value::Object(map)) => map,
        _ => {
            return Err(utils::response_handling_fail(
                http_code,
                "worldpayxml: failed to serialize the 3ds jwt claims.",
            )
            .into())
        }
    };
    let payload = josekit::jwt::JwtPayload::from_map(claims_map).change_context(
        utils::response_handling_fail(
            http_code,
            "worldpayxml: failed to build the 3ds jwt payload.",
        ),
    )?;
    let signer = josekit::jws::alg::hmac::HmacJwsAlgorithm::Hs256
        .signer_from_bytes(jwt_mac_key.peek().as_bytes())
        .change_context(utils::response_handling_fail(
            http_code,
            "worldpayxml: jwt_mac_key is not a valid HS256 key.",
        ))?;
    let mut header = josekit::jws::JwsHeader::new();
    header.set_algorithm("HS256");
    josekit::jwt::encode_with_signer(&payload, &header, &signer).change_context(
        utils::response_handling_fail(http_code, "worldpayxml: failed to sign the 3ds jwt."),
    )
}

/// Locally builds the Worldpayxml device-data-collection (DDC) response: no
/// outbound call is made, the shopper's browser talks to Cardinal directly.
pub(crate) fn handle_pre_authenticate_response<
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
>(
    data: &RouterDataV2<
        PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    >,
    _event_builder: Option<&mut common_utils::events::Event>,
    res: Response,
) -> CustomResult<
    RouterDataV2<
        PreAuthenticate,
        PaymentFlowData,
        PaymentsPreAuthenticateData<T>,
        PaymentsResponseData,
    >,
    ConnectorError,
> {
    // Fail before rendering the DDC page when the browser data the authorize leg
    // will need is absent, matching the hyperswitch PreAuthenticate validations.
    let browser_info = data
        .request
        .browser_info
        .as_ref()
        .ok_or(utils::response_handling_fail(
            res.status_code,
            "worldpayxml: browser_info is required for device data collection.",
        ))?;
    if browser_info.accept_header.is_none() {
        return Err(utils::response_handling_fail(
            res.status_code,
            "worldpayxml: browser_info.accept_header is required for device data collection.",
        )
        .into());
    }
    if browser_info.user_agent.is_none() {
        return Err(utils::response_handling_fail(
            res.status_code,
            "worldpayxml: browser_info.user_agent is required for device data collection.",
        )
        .into());
    }

    let bin = match &data.request.payment_method_data {
        Some(PaymentMethodData::Card(card)) => {
            card.card_number.peek().chars().take(6).collect::<String>()
        }
        Some(PaymentMethodData::Wallet(WalletData::GooglePay(gpay))) => {
            match &gpay.tokenization_data {
                GpayTokenizationData::Decrypted(decrypt_data) => decrypt_data
                    .application_primary_account_number
                    .get_card_isin(),
                GpayTokenizationData::Encrypted(_) => {
                    return Err(utils::response_handling_fail(
                        res.status_code,
                        "worldpayxml: device data collection needs the card bin; an encrypted google pay token does not carry it.",
                    )
                    .into())
                }
            }
        }
        _ => {
            return Err(utils::response_handling_fail(
                res.status_code,
                "worldpayxml: device data collection is only supported for cards and decrypted google pay.",
            )
            .into())
        }
    };

    let (iss, org_unit_id, jwt_mac_key) = match &data.connector_config {
        ConnectorSpecificConfig::Worldpayxml {
            issuer_id: Some(issuer_id),
            organizational_unit_id: Some(organizational_unit_id),
            jwt_mac_key: Some(jwt_mac_key),
            ..
        } => (
            issuer_id.clone(),
            organizational_unit_id.clone(),
            jwt_mac_key.clone(),
        ),
        _ => {
            return Err(utils::response_handling_fail(
                res.status_code,
                "worldpayxml: issuer_id, organizational_unit_id and jwt_mac_key must be configured in the connector metadata for 3ds.",
            )
            .into())
        }
    };
    let iat = u64::try_from(common_utils::date_time::now_unix_timestamp()).map_err(|_| {
        utils::response_handling_fail(
            res.status_code,
            "worldpayxml: system time is before the unix epoch.",
        )
    })?;
    let jwt = sign_worldpayxml_jwt(
        &requests::WorldpayxmlDdcJwt {
            jti: common_utils::fp_utils::generate_uuid_v4(),
            iat,
            iss,
            org_unit_id,
        },
        &jwt_mac_key,
        res.status_code,
    )?;

    let mut router_data = data.clone();
    router_data.resource_common_data.status = AttemptStatus::DeviceDataCollectionPending;
    router_data.response = Ok(PaymentsResponseData::PreAuthenticateResponse {
        resource_id: None,
        authentication_data: None,
        redirection_data: Some(Box::new(RedirectForm::WorldpayxmlDDCForm {
            bin,
            jwt: Secret::new(jwt),
        })),
        connector_response_reference_id: Some(
            data.resource_common_data
                .connector_request_reference_id
                .clone(),
        ),
        status_code: res.status_code,
    });
    Ok(router_data)
}

impl From<Secret<time::Date>> for requests::WorldpayxmlBirthDate {
    fn from(date_of_birth: Secret<time::Date>) -> Self {
        let date = date_of_birth.expose();
        Self {
            date: requests::WorldpayxmlFundingDate {
                day_of_month: Secret::new(date.day().to_string()),
                month: Secret::new(u8::from(date.month()).to_string()),
                year: Secret::new(date.year().to_string()),
            },
        }
    }
}

impl requests::WorldpayxmlFundingTransactionType {
    pub fn code(&self) -> &'static str {
        match self {
            Self::CreditCardBillRepayment => "CR",
            Self::GiftCardPurchase => "GP",
            Self::GiftCardPurchaseForAnother => "GO",
            Self::NonReloadablePrepaidCard => "PC",
            Self::ReloadablePrepaidCardOrAccount => "TP",
            Self::GamingChipsPurchase => "GC",
            Self::GamingStoredValueWallet => "GS",
            Self::GamingStagedDigitalWallet => "GW",
            Self::LiquidAndCryptoAssetsPurchase => "LC",
            Self::LiquidAndCryptoStoredValueWalletLoad => "LS",
            Self::StoredValueDigitalWalletLoad => "SW",
            Self::StoredValueDigitalWalletLoadNonSecurities => "WV",
            Self::SecuritiesStoredValueDigitalWalletLoad => "WO",
            Self::SecuritiesStagedDigitalWalletLoad => "WH",
            Self::SingleMerchantWalletLoad => "WS",
            Self::DebitCardLoad => "FC",
            Self::TransferToOwnDebitAccount => "TA",
            Self::FundsTransferMeToMe => "FM",
            Self::AccountToAccount => "AA",
            Self::BackToBackP2pWithoutWallet => "PB",
            Self::BackToBackP2pWithWallet => "PN",
            Self::AgentCashOut => "AC",
            Self::StagedDigitalWalletLoad => "WA",
            Self::StagedDigitalWalletPurchase => "WP",
            Self::BackToBackCardPurchase => "BB",
            Self::PayrollDisbursementFunding => "PD",
            Self::BusinessToConsumerDisbursement => "BD",
            Self::BusinessToBusinessInvoicePayment => "BT",
        }
    }
}

impl requests::WorldpayxmlPaymentPurpose {
    pub fn code(&self) -> &'static str {
        match self {
            Self::FamilySupport => "00",
            Self::RegularLabourTransfers => "01",
            Self::TravelAndTourism => "02",
            Self::Education => "03",
            Self::HospitalisationAndMedicalTreatment => "04",
            Self::EmergencyNeed => "05",
            Self::Savings => "06",
            Self::Gifts => "07",
            Self::Other => "08",
            Self::Salary => "09",
            Self::CrowdLending => "10",
            Self::CryptoCurrency => "11",
            Self::HighRiskSecurities => "16",
        }
    }
}

fn parse_worldpayxml_funding_transaction_type(
    raw: &str,
) -> Result<requests::WorldpayxmlFundingTransactionType, Report<IntegrationError>> {
    serde_json::from_value(serde_json::Value::String(raw.to_owned())).change_context(
        IntegrationError::InvalidConnectorConfig {
            config: "additional_connector_details.worldpayxml.funding_transaction_type",
            context: IntegrationErrorContext {
                additional_context: Some(format!("unknown funding transaction type {raw}")),
                suggested_action: Some(
                    "send a supported worldpayxml funding transaction type".to_string(),
                ),
                doc_url: None,
            },
        },
    )
}

fn parse_worldpayxml_payment_purpose(
    raw: &str,
) -> Result<requests::WorldpayxmlPaymentPurpose, Report<IntegrationError>> {
    serde_json::from_value(serde_json::Value::String(raw.to_owned())).change_context(
        IntegrationError::InvalidConnectorConfig {
            config: "additional_connector_details.worldpayxml.payment_purpose",
            context: IntegrationErrorContext {
                additional_context: Some(format!("unknown payment purpose {raw}")),
                suggested_action: Some("send a supported worldpayxml payment purpose".to_string()),
                doc_url: None,
            },
        },
    )
}

fn get_worldpayxml_account_reference(
    account: &RecipientAccount,
) -> Result<requests::WorldpayxmlAccountReference, Report<IntegrationError>> {
    let (account_type, value) = match account {
        RecipientAccount::BankAccount(bank_account) => match bank_account {
            RecipientBankAccount::Iban { iban } => (WORLDPAYXML_ACCOUNT_TYPE_IBAN, iban.clone()),
            RecipientBankAccount::RoutingNumber {
                account_number,
                routing_number,
            } => (
                WORLDPAYXML_ACCOUNT_TYPE_BANK_ROUTING,
                Secret::new(format!(
                    "{}+{}",
                    account_number.peek(),
                    routing_number.peek()
                )),
            ),
            RecipientBankAccount::Bic {
                account_number,
                bic,
            } => (
                WORLDPAYXML_ACCOUNT_TYPE_BANK_BIC,
                Secret::new(format!("{}+{}", account_number.peek(), bic.peek())),
            ),
            RecipientBankAccount::AccountNumber { account_number } => {
                (WORLDPAYXML_ACCOUNT_TYPE_CARD, account_number.clone())
            }
            RecipientBankAccount::TruncatedPan { .. } => {
                Err(error_stack::report!(IntegrationError::NotSupported {
                    message: "a truncated PAN as a recipient account identifier".to_string(),
                    connector: "Worldpayxml",
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "worldpayxml funds transfer needs a full recipient account identifier"
                                .to_string(),
                        ),
                        suggested_action: Some(
                            "send the recipient account as a full PAN, IBAN, or account number"
                                .to_string(),
                        ),
                        doc_url: None,
                    },
                }))?
            }
        },
        RecipientAccount::Card { card_number } => (
            WORLDPAYXML_ACCOUNT_TYPE_CARD,
            Secret::new(card_number.get_card_no()),
        ),
        RecipientAccount::Email { email } => (
            WORLDPAYXML_ACCOUNT_TYPE_EMAIL,
            Secret::new(email.peek().to_string()),
        ),
        RecipientAccount::Phone { phone_number } => {
            (WORLDPAYXML_ACCOUNT_TYPE_PHONE, phone_number.clone())
        }
        RecipientAccount::Wallet { wallet_id } => {
            (WORLDPAYXML_ACCOUNT_TYPE_WALLET, wallet_id.clone())
        }
        RecipientAccount::SocialNetwork { social_network_id } => {
            (WORLDPAYXML_ACCOUNT_TYPE_SOCIAL, social_network_id.clone())
        }
    };

    Ok(requests::WorldpayxmlAccountReference {
        account_type: account_type.to_string(),
        value,
    })
}

fn build_worldpayxml_recipient_party(
    recipient_details: Option<&RecipientDetails>,
    business_country: Option<common_enums::CountryAlpha2>,
) -> Result<requests::WorldpayxmlFundingParty, Report<IntegrationError>> {
    let recipient_details =
        recipient_details.ok_or_else(utils::missing_field_err("recipient_details"))?;

    let address = recipient_details
        .address
        .as_ref()
        .ok_or_else(utils::missing_field_err("recipient_details.address"))?;

    let account_reference = recipient_details
        .account
        .as_ref()
        .ok_or_else(utils::missing_field_err("recipient_details.account"))
        .and_then(get_worldpayxml_account_reference)?;

    Ok(requests::WorldpayxmlFundingParty {
        party_type: requests::WorldpayxmlFundingPartyType::Recipient,
        account_reference,
        full_name: requests::WorldpayxmlFullName {
            first: address.get_first_name()?.clone(),
            last: address.get_last_name()?.clone(),
        },
        funding_address: requests::WorldpayxmlFundingAddress {
            address1: address.get_line1()?.clone(),
            address2: address.get_optional_line2(),
            postal_code: address.get_zip()?.clone(),
            city: address.get_city()?.clone().expose(),
            state: address.get_state()?.clone(),
            country_code: *address.get_country()?,
        },
        funding_data: recipient_details
            .phone_number
            .clone()
            .map(|telephone_number| requests::WorldpayxmlFundingData {
                birth_date: None,
                telephone_number: Some(telephone_number),
            }),
        // Worldpay requires the recipient CPF/CNPJ for Brazilian merchants.
        tax_id: match business_country {
            Some(common_enums::CountryAlpha2::BR) => Some(
                recipient_details
                    .tax_id
                    .clone()
                    .ok_or_else(utils::missing_field_err("recipient_details.tax_id"))?,
            ),
            _ => recipient_details.tax_id.clone(),
        },
    })
}

fn build_worldpayxml_sender_party<
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
>(
    router_data: &RouterDataV2<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<T>,
        PaymentsResponseData,
    >,
    card_number: Secret<String>,
) -> Result<requests::WorldpayxmlFundingParty, Report<IntegrationError>> {
    let resource_common_data = &router_data.resource_common_data;
    Ok(requests::WorldpayxmlFundingParty {
        party_type: requests::WorldpayxmlFundingPartyType::Sender,
        account_reference: requests::WorldpayxmlAccountReference {
            account_type: WORLDPAYXML_ACCOUNT_TYPE_CARD.to_string(),
            value: card_number,
        },
        full_name: requests::WorldpayxmlFullName {
            first: resource_common_data.get_billing_first_name()?,
            last: resource_common_data.get_billing_last_name()?,
        },
        funding_address: requests::WorldpayxmlFundingAddress {
            address1: resource_common_data.get_billing_line1()?,
            address2: resource_common_data.get_optional_billing_line2(),
            postal_code: resource_common_data.get_billing_zip()?,
            city: resource_common_data.get_billing_city()?.expose(),
            state: resource_common_data.get_billing_state()?,
            country_code: resource_common_data.get_billing_country()?,
        },
        funding_data: Some(requests::WorldpayxmlFundingData {
            birth_date: router_data
                .request
                .customer
                .as_ref()
                .and_then(|customer| customer.date_of_birth.clone())
                .map(requests::WorldpayxmlBirthDate::from),
            telephone_number: resource_common_data.get_optional_billing_phone_number(),
        }),
        tax_id: None,
    })
}

fn build_worldpayxml_funding_transfer<
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
>(
    router_data: &RouterDataV2<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<T>,
        PaymentsResponseData,
    >,
) -> Result<Option<requests::WorldpayxmlFundingTransfer>, Report<IntegrationError>> {
    router_data
        .request
        .is_account_funding_transaction
        .unwrap_or(false)
        .then(|| {
            let card_number = match &router_data.request.payment_method_data {
                PaymentMethodData::Card(card) => {
                    Ok(Secret::new(card.card_number.peek().to_string()))
                }
                _ => Err(error_stack::report!(IntegrationError::NotSupported {
                    message: "account funded transactions for non-card payment methods".to_string(),
                    connector: "Worldpayxml",
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "worldpayxml derives the funds transfer sender from the card"
                                .to_string(),
                        ),
                        suggested_action: Some(
                            "authorize the account funding transaction with a card".to_string(),
                        ),
                        doc_url: None,
                    },
                })),
            }?;

            let worldpayxml_details = router_data
                .request
                .additional_connector_details
                .as_ref()
                .and_then(|details| details.worldpayxml.as_ref())
                .ok_or_else(utils::missing_field_err(
                    "additional_connector_details.worldpayxml",
                ))?;

            let transfer_type = worldpayxml_details
                .funding_transaction_type
                .as_deref()
                .map(parse_worldpayxml_funding_transaction_type)
                .transpose()?
                .ok_or_else(utils::missing_field_err(
                    "additional_connector_details.worldpayxml.funding_transaction_type",
                ))?
                .code();

            let payment_purpose = worldpayxml_details
                .payment_purpose
                .as_deref()
                .map(parse_worldpayxml_payment_purpose)
                .transpose()?
                .ok_or_else(utils::missing_field_err(
                    "additional_connector_details.worldpayxml.payment_purpose",
                ))?
                .code();

            Ok(requests::WorldpayxmlFundingTransfer {
                transfer_type: transfer_type.to_string(),
                category: WORLDPAYXML_FUNDING_CATEGORY.to_string(),
                payment_purpose: payment_purpose.to_string(),
                funding_party: vec![
                    build_worldpayxml_sender_party(router_data, card_number)?,
                    build_worldpayxml_recipient_party(
                        router_data.request.recipient_details.as_ref(),
                        router_data.request.business_country,
                    )?,
                ],
            })
        })
        .transpose()
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

        // A challenge return completes the pending order: only the order code, the session
        // reference and an empty completedAuthentication element are resubmitted.
        if parse_worldpayxml_challenge_return(router_data.request.redirect_response.as_ref())
            .is_some()
        {
            let order_code = router_data
                .resource_common_data
                .connector_request_reference_id
                .clone();
            return Ok(Self {
                version: API_VERSION.to_string(),
                merchant_code: auth.merchant_code,
                submit: requests::WorldpayxmlSubmit {
                    order: requests::WorldpayxmlOrder {
                        info_threed_secure: Some(requests::WorldpayxmlInfo3DSecure {
                            completed_authentication:
                                requests::WorldpayxmlCompletedAuthentication {},
                        }),
                        session: Some(requests::WorldpayxmlCompleteAuthSession {
                            id: Secret::new(order_code.clone()),
                        }),
                        additional_threeds_data: None,
                        order_code,
                        capture_delay: None,
                        description: None,
                        amount: None,
                        payment_details: None,
                        shopper: None,
                        billing_address: None,
                        create_token: None,
                        funding_transfer: None,
                    },
                },
            });
        }

        // A device-data-collection return submits the full order plus the collected
        // dfReferenceId so Worldpay can run 3DS authentication. Any redirect return that is
        // not a challenge completion is treated as the DDC return: when the DDC payload did
        // not parse (collection failed or timed out), the order still carries
        // additional3DSData so Worldpay cannot silently skip 3DS.
        let (additional_threeds_data, session) =
            match router_data.request.redirect_response.as_ref() {
                Some(_) => {
                    let browser_info = router_data.request.get_browser_info()?;
                    browser_info.get_accept_header()?;
                    browser_info.get_user_agent()?;
                    let shopper_ip_address = browser_info.get_ip_address()?;
                    let ddc_return = parse_worldpayxml_ddc_return(
                        router_data.request.redirect_response.as_ref(),
                    );
                    (
                        Some(requests::WorldpayxmlAdditionalThreeDSData {
                            df_reference_id: ddc_return.and_then(|ddc| ddc.session_id),
                            javascript_enabled: true,
                            device_channel: "Browser".to_string(),
                            challenge_preference:
                                requests::WorldpayxmlChallengePreference::ChallengeMandated,
                        }),
                        Some(requests::WorldpayxmlSession {
                            id: router_data
                                .resource_common_data
                                .connector_request_reference_id
                                .clone(),
                            shopper_ip_address,
                        }),
                    )
                }
                None => (None, None),
            };

        // Determine if manual capture
        let is_manual_capture = !router_data.request.is_auto_capture();

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
                    .map(Secret::new)
                    .or_else(|| {
                        router_data
                            .resource_common_data
                            .get_optional_billing_full_name()
                    })
                    .map(utils::normalize_cardholder_name);

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

        let stored_credentials =
            is_cit_mandate_payment.then(|| requests::WorldpayxmlStoredCredentials {
                usage: requests::WorldpayxmlUsageType::First,
                customer_initiated_reason: Some(requests::WorldpayxmlMandateType::from(
                    router_data.request.mit_category.clone(),
                )),
                merchant_initiated_reason: None,
                scheme_transaction_identifier: None,
            });

        let create_token = is_cit_mandate_payment.then(|| requests::WorldpayxmlCreateToken {
            token_scope: requests::WorldpayxmlTokenScope::Shopper,
            token_event_reference: router_data
                .resource_common_data
                .connector_request_reference_id
                .clone(),
        });

        let info_3d_secure =
            get_worldpayxml_info_3d_secure(router_data.request.authentication_data.as_ref())?;

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
                    info_threed_secure: None,
                    session: None,
                    additional_threeds_data,
                    order_code: router_data
                        .resource_common_data
                        .connector_request_reference_id
                        .clone(),
                    capture_delay: Some(if is_manual_capture {
                        CAPTURE_DELAY_MANUAL.to_string()
                    } else {
                        CAPTURE_DELAY_AUTOMATIC.to_string()
                    }),
                    description: Some(
                        router_data
                            .resource_common_data
                            .description
                            .clone()
                            .unwrap_or_else(|| DEFAULT_PAYMENT_DESCRIPTION.to_string()),
                    ),
                    amount: Some(requests::WorldpayxmlAmount {
                        value: converted_amount,
                        currency_code: router_data.request.currency,
                        exponent: get_worldpayxml_exponent(router_data.request.currency)?,
                    }),
                    payment_details: Some(requests::WorldpayxmlPaymentDetails {
                        action: Some(if is_manual_capture {
                            WorldpayxmlAction::Authorise
                        } else {
                            WorldpayxmlAction::Sale
                        }),
                        payment_method,
                        stored_credentials,
                        session,
                        info_3d_secure,
                    }),
                    shopper: Some(requests::WorldpayxmlShopper {
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
                                http_referer: browser_info.referer.clone(),
                                time_zone: browser_info.time_zone,
                                browser_language: browser_info.language.clone(),
                                browser_java_enabled: browser_info.java_enabled,
                                browser_java_script_enabled: browser_info.java_script_enabled,
                                browser_colour_depth: browser_info.color_depth.map(u32::from),
                                browser_screen_height: browser_info.screen_height,
                                browser_screen_width: browser_info.screen_width,
                            }),
                    }),
                    billing_address,
                    create_token,
                    funding_transfer: build_worldpayxml_funding_transfer(router_data)?,
                },
            },
        })
    }
}

// SetupMandate flow transformers
//
// A mandate setup is submitted as an ordinary order that additionally asks Worldpay to create a
// payment token, and flags the transaction to the scheme as the first of a stored-credential
// agreement. The order must be zero-amount: setting up an agreement should not move money.
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

        // Worldpay registers the agreement on a zero-amount verification order. A non-zero setup
        // would authorise funds that nothing subsequently captures.
        if router_data
            .request
            .minor_amount
            .is_some_and(|amount| amount.get_amount_as_i64() > 0)
        {
            return Err(IntegrationError::FlowNotSupported {
                flow: "SetupMandate with a non-zero amount".to_string(),
                connector: "worldpayxml".to_string(),
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Send a zero amount to register the mandate, then charge with a separate \
                         payment."
                            .to_string(),
                    ),
                    ..Default::default()
                },
            }
            .into());
        }

        let auth = WorldpayxmlAuthType::try_from(&router_data.connector_config)?;

        let is_manual_capture = !router_data.request.is_auto_capture();

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
                    .map(Secret::new)
                    .or_else(|| {
                        router_data
                            .resource_common_data
                            .get_optional_billing_full_name()
                    })
                    .map(utils::normalize_cardholder_name);

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

        let info_3d_secure =
            get_worldpayxml_info_3d_secure(router_data.request.authentication_data.as_ref())?;

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
                    info_threed_secure: None,
                    session: None,
                    additional_threeds_data: None,
                    order_code: router_data
                        .resource_common_data
                        .connector_request_reference_id
                        .clone(),
                    capture_delay: Some(if is_manual_capture {
                        CAPTURE_DELAY_MANUAL.to_string()
                    } else {
                        CAPTURE_DELAY_AUTOMATIC.to_string()
                    }),
                    description: Some(
                        router_data
                            .resource_common_data
                            .description
                            .clone()
                            .unwrap_or_else(|| DEFAULT_PAYMENT_DESCRIPTION.to_string()),
                    ),
                    amount: Some(requests::WorldpayxmlAmount {
                        value: converted_amount,
                        currency_code: router_data.request.currency,
                        exponent: get_worldpayxml_exponent(router_data.request.currency)?,
                    }),
                    payment_details: Some(requests::WorldpayxmlPaymentDetails {
                        action: Some(if is_manual_capture {
                            WorldpayxmlAction::Authorise
                        } else {
                            WorldpayxmlAction::Sale
                        }),
                        payment_method,
                        stored_credentials: Some(requests::WorldpayxmlStoredCredentials {
                            usage: requests::WorldpayxmlUsageType::First,
                            customer_initiated_reason: Some(
                                requests::WorldpayxmlMandateType::from(
                                    router_data.request.mit_category.clone(),
                                ),
                            ),
                            merchant_initiated_reason: None,
                            scheme_transaction_identifier: None,
                        }),
                        session: None,
                        info_3d_secure,
                    }),
                    shopper: Some(requests::WorldpayxmlShopper {
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
                                http_referer: browser_info.referer.clone(),
                                time_zone: browser_info.time_zone,
                                browser_language: browser_info.language.clone(),
                                browser_java_enabled: browser_info.java_enabled,
                                browser_java_script_enabled: browser_info.java_script_enabled,
                                browser_colour_depth: browser_info.color_depth.map(u32::from),
                                browser_screen_height: browser_info.screen_height,
                                browser_screen_width: browser_info.screen_width,
                            }),
                    }),
                    billing_address,
                    create_token: Some(requests::WorldpayxmlCreateToken {
                        token_scope: requests::WorldpayxmlTokenScope::Shopper,
                        token_event_reference: router_data
                            .resource_common_data
                            .connector_request_reference_id
                            .clone(),
                    }),
                    funding_transfer: None,
                },
            },
        })
    }
}

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
                    context: IntegrationErrorContext {
                        additional_context: Some(
                            "Worldpay charges a merchant-initiated payment against the payment \
                             token it issued on the customer-initiated transaction, submitted \
                             over TOKEN-SSL. A network transaction id or network token alone \
                             cannot identify the stored credential."
                                .to_string(),
                        ),
                        suggested_action: Some(
                            "Set up the mandate against this connector first so a connector \
                             mandate id is stored, then charge against that."
                                .to_string(),
                        ),
                        doc_url: None,
                    },
                }
                .into())
            }
        };

        let payment_token_id = connector_mandate.get_connector_mandate_id().ok_or(
            IntegrationError::MissingRequiredField {
                field_name: "connector_mandate_id",
                context: IntegrationErrorContext {
                    additional_context: Some(
                        "The stored mandate carries no Worldpay paymentTokenID, so there is \
                         nothing to submit over TOKEN-SSL for this merchant-initiated payment."
                            .to_string(),
                    ),
                    suggested_action: Some(
                        "Re-run the mandate setup so Worldpay issues a payment token.".to_string(),
                    ),
                    doc_url: None,
                },
            },
        )?;

        let is_manual_capture = router_data.request.capture_method == Some(CaptureMethod::Manual)
            || router_data.request.capture_method == Some(CaptureMethod::ManualMultiple);

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
                    info_threed_secure: None,
                    session: None,
                    additional_threeds_data: None,
                    order_code: router_data
                        .resource_common_data
                        .connector_request_reference_id
                        .clone(),
                    capture_delay: Some(if is_manual_capture {
                        CAPTURE_DELAY_MANUAL.to_string()
                    } else {
                        CAPTURE_DELAY_AUTOMATIC.to_string()
                    }),
                    description: Some(
                        router_data
                            .resource_common_data
                            .description
                            .clone()
                            .unwrap_or_else(|| DEFAULT_PAYMENT_DESCRIPTION.to_string()),
                    ),
                    amount: Some(requests::WorldpayxmlAmount {
                        value: converted_amount,
                        currency_code: router_data.request.currency,
                        exponent: get_worldpayxml_exponent(router_data.request.currency)?,
                    }),
                    payment_details: Some(requests::WorldpayxmlPaymentDetails {
                        action: None,
                        payment_method: requests::WorldpayxmlPaymentMethod::TokenSsl(
                            requests::WorldpayxmlTokenData {
                                token_scope: requests::WorldpayxmlTokenScope::Shopper,
                                payment_token_id: Secret::new(payment_token_id),
                            },
                        ),
                        stored_credentials: Some(requests::WorldpayxmlStoredCredentials {
                            usage: requests::WorldpayxmlUsageType::Used,
                            customer_initiated_reason: None,
                            merchant_initiated_reason: Some(
                                requests::WorldpayxmlMandateType::from(
                                    router_data.request.mit_category.clone(),
                                ),
                            ),
                            // Only sent when the customer-initiated transaction reported one;
                            // Worldpay accepts the merchant-initiated payment without it.
                            scheme_transaction_identifier: connector_mandate
                                .get_connector_mandate_request_reference_id()
                                .map(Secret::new),
                        }),
                        session: None,
                        info_3d_secure: None,
                    }),
                    shopper: Some(requests::WorldpayxmlShopper {
                        shopper_email_address: router_data.request.email.clone(),
                        authenticated_shopper_id,
                        browser: None,
                    }),
                    billing_address: get_worldpayxml_billing_address(
                        &router_data.resource_common_data,
                    ),
                    create_token: None,
                    funding_transfer: None,
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
                            exponent: get_worldpayxml_exponent(router_data.request.currency)?,
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
                            exponent: get_worldpayxml_exponent(router_data.request.currency)?,
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

/// Maps the `lastEvent` of a payment order onto an attempt status.
///
/// Only the events that belong to an authorisation/capture journey are mapped. A refund- or
/// payout-family event on a payment order means the response does not describe the order we asked
/// about, so it is reported as a protocol violation instead of being flattened into `Failure`,
/// which would be indistinguishable from a genuine decline.
fn map_worldpayxml_authorize_status(
    last_event: &WorldpayxmlLastEvent,
    is_auto_capture: bool,
    previous_status: Option<&AttemptStatus>,
    http_code: u16,
) -> Result<AttemptStatus, ConnectorError> {
    match last_event {
        WorldpayxmlLastEvent::Authorised => {
            if is_auto_capture {
                // The order was submitted for automatic capture, so an authorisation already
                // settles the attempt — there is no separate capture to wait for.
                Ok(AttemptStatus::Charged)
            } else {
                Ok(match previous_status {
                    Some(AttemptStatus::CaptureInitiated) => AttemptStatus::CaptureInitiated,
                    Some(AttemptStatus::VoidInitiated) => AttemptStatus::VoidInitiated,
                    _ => AttemptStatus::Authorized,
                })
            }
        }
        WorldpayxmlLastEvent::Refused => Ok(AttemptStatus::Failure),
        // Worldpay expires an authorisation that outlives its auth window (a manual-capture order
        // left uncaptured, say). The money can never be taken, so this is terminal — not a
        // protocol violation to error on.
        WorldpayxmlLastEvent::Expired => Ok(AttemptStatus::Failure),
        WorldpayxmlLastEvent::Cancelled => Ok(AttemptStatus::Voided),
        WorldpayxmlLastEvent::Captured
        | WorldpayxmlLastEvent::Settled
        | WorldpayxmlLastEvent::SettledByMerchant => Ok(AttemptStatus::Charged),
        WorldpayxmlLastEvent::SentForAuthorisation => Ok(AttemptStatus::Authorizing),
        WorldpayxmlLastEvent::Unknown => Ok(retain_previous_attempt_status(previous_status)),
        _ => Err(utils::unexpected_response_fail(
            http_code,
            "worldpayxml: lastEvent is not part of a payment authorisation lifecycle.",
        )),
    }
}

/// Maps `lastEvent` for a mandate setup, where an authorisation already completes the flow —
/// there is no capture to wait for on a (usually zero-amount) verification order.
///
/// Events outside that journey are rejected for the same reason as
/// [`map_worldpayxml_authorize_status`].
fn map_worldpayxml_setup_mandate_status(
    last_event: &WorldpayxmlLastEvent,
    previous_status: Option<&AttemptStatus>,
    http_code: u16,
) -> Result<AttemptStatus, ConnectorError> {
    match last_event {
        WorldpayxmlLastEvent::Refused => Ok(AttemptStatus::Failure),
        // See [`map_worldpayxml_authorize_status`]: an expired authorisation is terminal.
        WorldpayxmlLastEvent::Expired => Ok(AttemptStatus::Failure),
        WorldpayxmlLastEvent::Cancelled => Ok(AttemptStatus::Voided),
        WorldpayxmlLastEvent::Authorised
        | WorldpayxmlLastEvent::Captured
        | WorldpayxmlLastEvent::Settled
        | WorldpayxmlLastEvent::SettledByMerchant => Ok(AttemptStatus::Charged),
        WorldpayxmlLastEvent::SentForAuthorisation => Ok(AttemptStatus::Authorizing),
        WorldpayxmlLastEvent::Unknown => Ok(retain_previous_attempt_status(previous_status)),
        _ => Err(utils::unexpected_response_fail(
            http_code,
            "worldpayxml: lastEvent is not part of a mandate setup lifecycle.",
        )),
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
/// The two response elements a mandate reference is built from.
///
/// A bare local type rather than a tuple: the orphan rule only accepts a local type as the
/// conversion's parameter when it is not wrapped in a foreign type constructor, and a tuple is
/// foreign.
pub struct WorldpayxmlMandateSource<'a> {
    pub token: &'a responses::WorldpayxmlToken,
    pub payment: &'a responses::WorldpayxmlPayment,
}

impl From<WorldpayxmlMandateSource<'_>> for MandateReference {
    fn from(WorldpayxmlMandateSource { token, payment }: WorldpayxmlMandateSource<'_>) -> Self {
        Self {
            connector_mandate_id: Some(token.token_details.payment_token_id.peek().to_string()),
            payment_method_id: None,
            mandate_metadata: None,
            connector_mandate_request_reference_id: payment
                .scheme_response
                .as_ref()
                .map(|scheme_response| scheme_response.transaction_identifier.clone()),
        }
    }
}

fn get_worldpayxml_mandate_reference(
    order_status: &responses::WorldpayxmlOrderStatus,
    payment: &responses::WorldpayxmlPayment,
) -> Option<Box<MandateReference>> {
    order_status.token.as_ref().map(|token| {
        Box::new(MandateReference::from(WorldpayxmlMandateSource {
            token,
            payment,
        }))
    })
}

/// Maps the `lastEvent` of a refund order onto a refund status.
///
/// Only the events that belong to a refund journey are mapped. `CAPTURED`/`SETTLED` are included
/// because Worldpay keeps reporting the underlying capture until the refund itself moves, so they
/// mean "the refund has not progressed yet" rather than "the refund succeeded". Anything outside
/// that journey means the response does not describe the order we asked about, so it is reported as
/// a protocol violation instead of being flattened into `Pending`, which would leave the refund
/// polling forever.
fn map_worldpayxml_refund_status(
    last_event: &WorldpayxmlLastEvent,
    previous_status: RefundStatus,
    http_code: u16,
) -> Result<RefundStatus, ConnectorError> {
    match last_event {
        WorldpayxmlLastEvent::Refunded | WorldpayxmlLastEvent::RefundedByMerchant => {
            Ok(RefundStatus::Success)
        }
        WorldpayxmlLastEvent::SentForRefund
        | WorldpayxmlLastEvent::RefundRequested
        | WorldpayxmlLastEvent::SentForFastRefund => Ok(RefundStatus::Pending),
        WorldpayxmlLastEvent::RefundFailed => Ok(RefundStatus::Failure),
        // An expired order can no longer be refunded, so the refund will never complete.
        WorldpayxmlLastEvent::Expired => Ok(RefundStatus::Failure),
        WorldpayxmlLastEvent::Captured | WorldpayxmlLastEvent::Settled => Ok(RefundStatus::Pending),
        WorldpayxmlLastEvent::Unknown => {
            // An unrecognised event says nothing about the refund, so hold the status we already
            // had rather than reporting a terminal one.
            tracing::warn!(
                retained_status = ?previous_status,
                "worldpayxml: unknown lastEvent received; retaining previous refund status"
            );
            Ok(previous_status)
        }
        _ => Err(utils::unexpected_response_fail(
            http_code,
            "worldpayxml: lastEvent is not part of a refund lifecycle.",
        )),
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
            utils::response_deserialization_fail(item.http_code, "worldpayxml: response body did not match the expected format; confirm API version and connector documentation."),
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

        // A challengeRequired reply means 3DS authentication demands shopper interaction:
        // redirect the shopper to Cardinal StepUp with a signed JWT and stash the machine
        // cookie for the completion leg.
        if let Some(challenge_required) = order_status.challenge_required.as_ref() {
            let details = challenge_required
                .three_ds_challenge_details
                .as_ref()
                .ok_or_else(|| {
                    utils::unexpected_response_fail(
                        item.http_code,
                        "worldpayxml: challengeRequired is missing threeDSChallengeDetails.",
                    )
                })?;
            let acs_url = details.acs_url.clone().ok_or_else(|| {
                utils::unexpected_response_fail(
                    item.http_code,
                    "worldpayxml: challengeRequired is missing acsURL.",
                )
            })?;
            let payload = details.payload.clone().ok_or_else(|| {
                utils::unexpected_response_fail(
                    item.http_code,
                    "worldpayxml: challengeRequired is missing payload.",
                )
            })?;
            let transaction_id = details.transaction_id_3ds.clone().ok_or_else(|| {
                utils::unexpected_response_fail(
                    item.http_code,
                    "worldpayxml: challengeRequired is missing transactionId3DS.",
                )
            })?;
            let return_url = router_data
                .request
                .complete_authorize_url
                .clone()
                .ok_or_else(|| {
                    utils::response_handling_fail(
                        item.http_code,
                        "worldpayxml: complete_authorize_url is required for a 3ds challenge.",
                    )
                })?;
            let (iss, org_unit_id, jwt_mac_key) = match &router_data.connector_config {
                ConnectorSpecificConfig::Worldpayxml {
                    issuer_id: Some(issuer_id),
                    organizational_unit_id: Some(organizational_unit_id),
                    jwt_mac_key: Some(jwt_mac_key),
                    ..
                } => (
                    issuer_id.clone(),
                    organizational_unit_id.clone(),
                    jwt_mac_key.clone(),
                ),
                _ => {
                    return Err(utils::response_handling_fail(
                        item.http_code,
                        "worldpayxml: issuer_id, organizational_unit_id and jwt_mac_key must be configured in the connector metadata for 3ds.",
                    )
                    .into())
                }
            };
            let iat =
                u64::try_from(common_utils::date_time::now_unix_timestamp()).map_err(|_| {
                    utils::response_handling_fail(
                        item.http_code,
                        "worldpayxml: system time is before the unix epoch.",
                    )
                })?;
            let jwt = sign_worldpayxml_jwt(
                &requests::WorldpayxmlChallengeJwt {
                    jti: common_utils::fp_utils::generate_uuid_v4(),
                    iat,
                    iss,
                    org_unit_id,
                    return_url,
                    payload: requests::WorldpayxmlChallengeJwtPayload {
                        acs_url,
                        payload,
                        transaction_id,
                    },
                    objectify_payload: true,
                },
                &jwt_mac_key,
                item.http_code,
            )?;
            let redirection_data = RedirectForm::WorldpayxmlRedirectForm {
                jwt: Secret::new(jwt),
            };
            let cookie = router_data
                .resource_common_data
                .connector_response_headers
                .as_ref()
                .and_then(|headers| {
                    headers
                        .get_all("set-cookie")
                        .iter()
                        .filter_map(|value| value.to_str().ok())
                        .find(|cookie| cookie.trim_start().starts_with("machine="))
                        .map(|cookie| {
                            cookie
                                .split(';')
                                .next()
                                .unwrap_or(cookie)
                                .trim()
                                .to_string()
                        })
                });

            return Ok(Self {
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::AuthenticationPending,
                    ..router_data.resource_common_data.clone()
                },
                response: Ok(PaymentsResponseData::TransactionResponse {
                    resource_id: ResponseId::ConnectorTransactionId(
                        order_status.order_code.clone(),
                    ),
                    redirection_data: Some(Box::new(redirection_data)),
                    connector_metadata: cookie.map(|value| serde_json::json!({ "cookie": value })),
                    payment_account_reference: None,
                    mandate_reference: None,
                    network_txn_id: None,
                    network_txn_link_id: None,
                    connector_response_reference_id: Some(order_status.order_code.clone()),
                    incremental_authorization_allowed: None,
                    splits: None,
                    status_code: item.http_code,
                }),
                ..router_data.clone()
            });
        }

        // Extract payment details
        let payment = order_status.payment.as_ref().ok_or(
            utils::response_deserialization_fail(item.http_code, "worldpayxml: response body did not match the expected format; confirm API version and connector documentation."),
        )?;

        // Map status from lastEvent
        let status = map_worldpayxml_authorize_status(
            &payment.last_event,
            router_data.request.capture_method != Some(CaptureMethod::Manual)
                && router_data.request.capture_method != Some(CaptureMethod::ManualMultiple),
            Some(&router_data.resource_common_data.status),
            item.http_code,
        )?;

        // A refused authorization is the most common decline path, so surface the ISO 8583 return
        // code the same way the SetupMandate and RepeatPayment transformers do instead of handing
        // the merchant a bare Failure with no code or reason.
        if domain_types::utils::is_payment_failure(status) {
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
            payment_account_reference: payment.card_p_a_r.clone(),
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
            utils::response_deserialization_fail(item.http_code, "worldpayxml: response body did not match the expected format; confirm API version and connector documentation."),
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
            utils::response_deserialization_fail(item.http_code, "worldpayxml: response body did not match the expected format; confirm API version and connector documentation."),
        )?;

        let status = map_worldpayxml_setup_mandate_status(
            &payment.last_event,
            Some(&router_data.resource_common_data.status),
            item.http_code,
        )?;

        if domain_types::utils::is_payment_failure(status) {
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

        // A token-less AUTHORISED reply (tokenisation not enabled on the merchant profile, or a
        // token event conflict) still maps to success with an absent mandate reference. Failing
        // it here was considered and reverted: the reference connector implementation is lenient,
        // and behavioural parity with it takes precedence.
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
            payment_account_reference: payment.card_p_a_r.clone(),
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
            utils::response_deserialization_fail(item.http_code, "worldpayxml: response body did not match the expected format; confirm API version and connector documentation."),
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
            utils::response_deserialization_fail(item.http_code, "worldpayxml: response body did not match the expected format; confirm API version and connector documentation."),
        )?;

        let status = map_worldpayxml_authorize_status(
            &payment.last_event,
            router_data.request.capture_method != Some(CaptureMethod::Manual)
                && router_data.request.capture_method != Some(CaptureMethod::ManualMultiple),
            Some(&router_data.resource_common_data.status),
            item.http_code,
        )?;

        // A refused merchant-initiated payment is the case that most needs its decline detail
        // (retry and dunning logic key off it), so surface the ISO 8583 return code the same
        // way the SetupMandate transformer does instead of a bare Failure status.
        if domain_types::utils::is_payment_failure(status) {
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
            payment_account_reference: payment.card_p_a_r.clone(),
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
            utils::response_deserialization_fail(item.http_code, "worldpayxml: response body did not match the expected format; confirm API version and connector documentation."),
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
            payment_account_reference: None,
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
            utils::response_deserialization_fail(item.http_code, "worldpayxml: response body did not match the expected format; confirm API version and connector documentation."),
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
            payment_account_reference: None,
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
                    utils::response_deserialization_fail(
                        item.http_code,
                    "worldpayxml: response body did not match the expected format; confirm API version and connector documentation."),
                )?;

                // Special handling: If error exists but payment is None, return current status (don't fail)
                if let Some(error) = &order_status.error {
                    if order_status.payment.is_none() {
                        // An inquiry-level error with no payment element says nothing about the
                        // order itself, so hold the status we already had. Overwriting it would
                        // walk a terminal attempt (a Charged one, say) back to Pending.
                        let payments_response_data = PaymentsResponseData::TransactionResponse {
                            resource_id: ResponseId::ConnectorTransactionId(
                                order_status.order_code.clone(),
                            ),
                            redirection_data: None,
                            // A token can be present even when the inquiry itself errored, and it
                            // is the only place the mandate becomes observable, so surface it.
                            mandate_reference: order_status.token.as_ref().map(|token| {
                                Box::new(MandateReference {
                                    connector_mandate_id: Some(
                                        token.token_details.payment_token_id.peek().to_string(),
                                    ),
                                    payment_method_id: None,
                                    mandate_metadata: None,
                                    connector_mandate_request_reference_id: None,
                                })
                            }),
                            connector_metadata: None,
                            network_txn_id: None,
                            network_txn_link_id: None,
                            connector_response_reference_id: Some(order_status.order_code.clone()),
                            incremental_authorization_allowed: None,
                            status_code: item.http_code,
                            splits: None,
                            payment_account_reference: None,
                        };

                        return Ok(Self {
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
                    utils::response_deserialization_fail(
                        item.http_code,
                    "worldpayxml: response body did not match the expected format; confirm API version and connector documentation."),
                )?;

                // Map status from lastEvent - reuse the helper function
                let status = map_worldpayxml_authorize_status(
                    &payment.last_event,
                    router_data.request.capture_method != Some(CaptureMethod::Manual)
                        && router_data.request.capture_method
                            != Some(CaptureMethod::ManualMultiple),
                    Some(&router_data.resource_common_data.status),
                    item.http_code,
                )?;

                // A sync that observes a refused order carries the same decline detail an
                // Authorize reply does, so report it identically rather than as a bare Failure.
                if domain_types::utils::is_payment_failure(status) {
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
                    payment_account_reference: payment.card_p_a_r.clone(),
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

                // Map status from PaymentStatus
                let status = map_worldpayxml_authorize_status(
                    &webhook_response.payment_status,
                    router_data.request.capture_method != Some(CaptureMethod::Manual)
                        && router_data.request.capture_method
                            != Some(CaptureMethod::ManualMultiple),
                    Some(&router_data.resource_common_data.status),
                    item.http_code,
                )?;

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
                    payment_account_reference: None,
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
            utils::response_deserialization_fail(item.http_code, "worldpayxml: response body did not match the expected format; confirm API version and connector documentation."),
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
                        response: Err(utils::build_error_response(
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
                    utils::response_deserialization_fail(
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
                    utils::response_deserialization_fail(
                        item.http_code,
                    "worldpayxml: response body did not match the expected format; confirm API version and connector documentation."),
                )?;

                // Map status from lastEvent using refund status mapping
                let refund_status = map_worldpayxml_refund_status(
                    &payment.last_event,
                    router_data.request.refund_status,
                    item.http_code,
                )?;

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
                    item.http_code,
                )?;

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
            utils::response_deserialization_fail(item.http_code, "worldpayxml: response body did not match the expected format; confirm API version and connector documentation."),
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
