use crate::types::ResponseRouterData;
use base64::Engine;
use common_enums::{AttemptStatus, Currency, RefundStatus};
use common_utils::MinorUnit;
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, RSync, Refund, Void},
    connector_types::{
        PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, PaymentsSyncData, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, ResponseId,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    payment_method_data::{
        ApplePayPaymentData, ApplePayWalletData, PaymentMethodData, PaymentMethodDataTypes,
        RawCardNumber, WalletData,
    },
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use error_stack::ResultExt;
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::connectors::travelhub::TravelhubRouterData;

// Authentication Types

#[derive(Debug, Clone)]
pub struct TravelhubAuthType {
    pub username: Secret<String>,
    pub password: Secret<String>,
    pub merchant_id: Secret<String>,
}

impl TravelhubAuthType {
    pub fn generate_authorization_header(&self) -> String {
        let credentials = format!("{}:{}", self.username.peek(), self.password.peek());
        format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(credentials.as_bytes())
        )
    }

    pub fn get_merchant_id(&self) -> String {
        self.merchant_id.peek().to_string()
    }
}

impl TryFrom<&ConnectorSpecificConfig> for TravelhubAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Travelhub {
                username,
                password,
                merchant_id,
                ..
            } => Ok(Self {
                username: username.to_owned(),
                password: password.to_owned(),
                merchant_id: merchant_id.to_owned(),
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Ensure the connector account is configured with Travelhub credentials (merchant_id, username, password)"
                                .to_string(),
                        ),
                        doc_url: None,
                        additional_context: Some(
                            "ConnectorSpecificConfig variant mismatch: expected Travelhub credentials but received credentials for a different connector; the request may have been routed to the wrong connector"
                                .to_string(),
                        ),
                    }
                }
            )),
        }
    }
}

// Error Response Types

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TravelhubErrorResponse {
    pub timestamp: Option<i64>,
    pub status: Option<i32>,
    pub error: Option<String>,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exception: Option<String>,
    pub path: Option<String>,
}

// Travel / Airline Itinerary Types

/// Connector-agnostic airline data (Euler `domainData.airlineData`) mapped onto
/// Travelhub's `travel` object. Only the authorize request carries it today —
/// `TravelhubCaptureRequest` has no `travel` field. `airlineCode` is mandatory
/// whenever `travel` is sent, so the whole block is omitted when no airline code
/// is available.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TravelhubTravel {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub booking_code: Option<String>,
    pub airline_code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issue_date: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub passenger: Vec<TravelhubTravelPassenger>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub flight: Vec<TravelhubTravelFlight>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_name: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TravelhubTravelPassenger {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<TravelhubTravelPassengerName>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ticket_number: Option<Secret<String>>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub passenger_type: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TravelhubTravelPassengerName {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_name: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub middle_name: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TravelhubTravelFlight {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub departure_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub departure_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arrival_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arrival_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub carrier_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flight_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flight_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub air_class: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fare: Option<TravelhubTravelFare>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub departure_country_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arrival_country_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arrival_date: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TravelhubTravelFare {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<MinorUnit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<Currency>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fare_class: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fare_basis: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stopover_allowed: Option<bool>,
}

/// TravelHub expects passenger type as its own canonical vocabulary (ADULT /
/// CHILD / INFANT). Euler airline data often carries IATA fare-style codes
/// (ADT, CHD, CNN, INF, SRC); map known aliases and drop anything unrecognized
/// instead of sending a value TravelHub would reject as a schema violation.
fn normalize_travelhub_passenger_type(passenger_type: &str) -> Option<String> {
    match passenger_type.trim().to_ascii_uppercase().as_str() {
        "ADT" | "ADULT" | "SRC" | "SENIOR" => Some("ADULT".to_string()),
        "CHD" | "CNN" | "CHILD" => Some("CHILD".to_string()),
        "INF" | "INFANT" => Some("INFANT".to_string()),
        _ => None,
    }
}

fn build_travel_data(
    domain_data: Option<&domain_types::connector_types::DomainData>,
) -> Option<TravelhubTravel> {
    let airline = domain_data?.airline_data.as_ref()?;
    // Travelhub marks airlineCode as required whenever travel is sent; prefer the
    // dedicated airline_code, fall back to the issuing carrier's code.
    let airline_code = airline
        .airline_code
        .clone()
        .or_else(|| airline.issuing_carrier_code.clone())?;
    Some(TravelhubTravel {
        booking_code: airline.pnr_code.clone(),
        airline_code,
        issue_date: airline.ticket_issue_date.clone(),
        passenger: airline
            .passengers
            .iter()
            .map(|p| TravelhubTravelPassenger {
                name: p.customer.as_ref().map(|c| TravelhubTravelPassengerName {
                    first_name: c.first_name.clone(),
                    last_name: c.last_name.clone(),
                    middle_name: p.middle_name.clone(),
                }),
                ticket_number: p.ticket_number.clone(),
                passenger_type: p
                    .passenger_type
                    .as_deref()
                    .and_then(normalize_travelhub_passenger_type),
            })
            .collect(),
        flight: airline
            .flight_segments
            .iter()
            .map(|s| TravelhubTravelFlight {
                departure_code: s.departure.as_ref().and_then(|d| d.airport_code.clone()),
                departure_name: s.departure.as_ref().and_then(|d| d.city_name.clone()),
                arrival_code: s.arrival.as_ref().and_then(|a| a.airport_code.clone()),
                arrival_name: s.arrival.as_ref().and_then(|a| a.city_name.clone()),
                carrier_code: s.marketing_carrier_code.clone(),
                flight_number: s.flight_number.clone(),
                flight_date: s
                    .departure
                    .as_ref()
                    .and_then(|d| d.date_time.clone())
                    .or_else(|| airline.flight_date.clone()),
                air_class: s.class_of_service.clone(),
                fare: s.fare_amount.as_ref().map(|m| TravelhubTravelFare {
                    amount: Some(m.amount),
                    currency: Some(m.currency),
                    fare_class: s.class_of_service.clone(),
                    fare_basis: s.fare_basis_code.clone(),
                    stopover_allowed: s.stopover_code.as_deref().map(|c| c == "O"),
                }),
                departure_country_code: s.departure.as_ref().and_then(|d| d.country_code.clone()),
                arrival_country_code: s.arrival.as_ref().and_then(|a| a.country_code.clone()),
                arrival_date: s.arrival.as_ref().and_then(|a| a.date_time.clone()),
            })
            .collect(),
        agent_code: airline.agency_code.clone(),
        agent_name: airline.agency_name.clone(),
    })
}

// Authorize Request Types

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TravelhubRequest3DS {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cavv: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cavv_algorithm: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eci: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ds_transaction_id: Option<String>,
    #[serde(
        rename = "threeDSecureVersion",
        skip_serializing_if = "Option::is_none"
    )]
    pub three_ds_secure_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acs_transaction_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TravelhubPaymentCard<T: PaymentMethodDataTypes> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_name: Option<Secret<String>>,
    pub card_number: RawCardNumber<T>,
    pub expiry_date: Secret<String>,
    pub cvc: Secret<String>,
    #[serde(rename = "request3DS", skip_serializing_if = "Option::is_none")]
    pub request3ds: Option<TravelhubRequest3DS>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authentication: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct TravelhubPaymentMethod {
    pub code: String,
}

const TRAVELHUB_APPLE_PAY_PAYMENT_METHOD_CODE: &str = "212";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TravelhubApplePay {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encrypted_payment_data: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decrypted_payment_data: Option<TravelhubApplePayDecryptedPaymentData>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TravelhubApplePayDecryptedPaymentData {
    pub dpan: Secret<String>,
    pub expiry_date: Secret<String>,
    pub cryptogram: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eci: Option<String>,
}

/// TravelHub `payment.billingAddress`. Live preprod evidence (direct curl against
/// Worldline preprod): authorizing without at least `billingAddress.country` is
/// rejected as INVALID with error code BILLING_ADDRESS_COUNTRY_CODE_IS_REQUIRED.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TravelhubBillingAddress {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub street: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub postal_code: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<common_enums::CountryAlpha2>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TravelhubPayment<T: PaymentMethodDataTypes> {
    pub payment_method: TravelhubPaymentMethod,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_card: Option<TravelhubPaymentCard<T>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub apple_pay: Option<TravelhubApplePay>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing_address: Option<TravelhubBillingAddress>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TravelhubPaymentsRequest<T: PaymentMethodDataTypes> {
    pub merchant_id: String,
    pub order_id: String,
    pub amount: MinorUnit,
    pub currency: Currency,
    pub capture: bool,
    pub payment: TravelhubPayment<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub travel: Option<TravelhubTravel>,
}

/// Resolve TravelHub's `paymentMethodCode` for a card from the supplied card network,
/// falling back to BIN detection when the network is not provided by the caller
/// (citigate/mirrors the same pattern).
fn get_card_payment_method_code<T: PaymentMethodDataTypes>(
    card: &domain_types::payment_method_data::Card<T>,
) -> Result<&str, error_stack::Report<IntegrationError>> {
    if let Some(network) = card.card_network.as_ref() {
        return match network {
            common_enums::CardNetwork::Visa => Ok("108"),
            common_enums::CardNetwork::Mastercard => Ok("102"),
            common_enums::CardNetwork::AmericanExpress => Ok("117"),
            common_enums::CardNetwork::Discover => Ok("159"),
            common_enums::CardNetwork::DinersClub => Ok("115"),
            common_enums::CardNetwork::JCB => Ok("123"),
            common_enums::CardNetwork::CartesBancaires => Ok("130"),
            common_enums::CardNetwork::UnionPay => Ok("197"),
            other => Err(invalid_card_network_error(format!(
                "Card network {other:?}"
            ))),
        };
    }

    // Network omitted by the caller (valid in UCS) — infer from the card BIN.
    match domain_types::utils::get_card_issuer(card.card_number.peek())? {
        domain_types::utils::CardIssuer::Visa => Ok("108"),
        domain_types::utils::CardIssuer::Master => Ok("102"),
        domain_types::utils::CardIssuer::AmericanExpress => Ok("117"),
        domain_types::utils::CardIssuer::Discover => Ok("159"),
        domain_types::utils::CardIssuer::DinersClub => Ok("115"),
        domain_types::utils::CardIssuer::JCB => Ok("123"),
        domain_types::utils::CardIssuer::CartesBancaires => Ok("130"),
        domain_types::utils::CardIssuer::UnionPay => Ok("197"),
        other => Err(invalid_card_network_error(format!("Card issuer {other:?}"))),
    }
}

fn invalid_card_network_error(detail: String) -> error_stack::Report<IntegrationError> {
    error_stack::report!(IntegrationError::NotSupported {
        message: "card network".to_string(),
        connector: "travelhub",
        context: IntegrationErrorContext {
            suggested_action: Some(
                "Use a card from a supported network: Visa, Mastercard, American Express, Discover, Diners Club, JCB, Cartes Bancaires, or UnionPay"
                    .to_string(),
            ),
            doc_url: None,
            additional_context: Some(
                format!("{detail} is not supported by travelhub card payments"),
            ),
        },
    })
}

fn build_travelhub_apple_pay(
    apple_pay_data: &ApplePayWalletData,
) -> Result<TravelhubApplePay, error_stack::Report<IntegrationError>> {
    match &apple_pay_data.payment_data {
        ApplePayPaymentData::Encrypted(encrypted_data) => Ok(TravelhubApplePay {
            encrypted_payment_data: Some(Secret::new(encrypted_data.clone())),
            decrypted_payment_data: None,
        }),
        ApplePayPaymentData::Decrypted(decrypted_data) => Ok(TravelhubApplePay {
            encrypted_payment_data: None,
            decrypted_payment_data: Some(TravelhubApplePayDecryptedPaymentData {
                dpan: Secret::new(
                    decrypted_data
                        .application_primary_account_number
                        .get_card_no(),
                ),
                expiry_date: decrypted_data.get_expiry_date_as_mmyy().change_context(
                    IntegrationError::InvalidDataFormat {
                        field_name: "apple_pay.decryptedPaymentData.expiryDate",
                        context: IntegrationErrorContext {
                            additional_context: Some(
                                "Apple Pay decrypted expiry data must be convertible to MMYY"
                                    .to_string(),
                            ),
                            ..Default::default()
                        },
                    },
                )?,
                cryptogram: decrypted_data
                    .payment_data
                    .online_payment_cryptogram
                    .clone(),
                // TEMP LOCAL TEST PATCH — DO NOT COMMIT. Default ECI when
                // Apple's token omits eciIndicator; preprod rejects without it.
                eci: Some(
                    decrypted_data
                        .payment_data
                        .eci_indicator
                        .clone()
                        .unwrap_or_else(|| "07".to_string()),
                ),
            }),
        }),
    }
}

fn build_travelhub_billing_address<T: PaymentMethodDataTypes>(
    item: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
) -> Option<TravelhubBillingAddress> {
    // TEMP LOCAL TEST PATCH — DO NOT COMMIT.
    // Preprod rejects authorize without billingAddress.country
    // (BILLING_ADDRESS_COUNTRY_CODE_IS_REQUIRED); default to US while the
    // upstream request does not send one.
    Some(TravelhubBillingAddress {
        number: None,
        street: item.resource_common_data.get_optional_billing_line1(),
        city: item.resource_common_data.get_optional_billing_city(),
        state: item.resource_common_data.get_optional_billing_state(),
        postal_code: item.resource_common_data.get_optional_billing_zip(),
        country: item
            .resource_common_data
            .get_optional_billing_country()
            .or(Some(common_enums::CountryAlpha2::US)),
    })
}

impl<T: PaymentMethodDataTypes>
    TryFrom<
        &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
    > for TravelhubPaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData<T>,
            PaymentsResponseData,
        >,
    ) -> Result<Self, Self::Error> {
        let auth = TravelhubAuthType::try_from(&item.connector_config)?;

        let payment_method_data = &item.request.payment_method_data;
        let is_auto_capture = !crate::utils::is_manual_capture(item.request.capture_method);
        let billing_address = build_travelhub_billing_address(item);

        let payment = match payment_method_data {
            PaymentMethodData::Card(card_data) => {
                // Travelhub's cardName is optional (mandatory: No in the API spec, max 51 chars),
                // so send the best available name instead of failing when none is provided.
                let cardholder_name = crate::utils::build_card_holder_name(
                    &card_data.card_holder_name,
                    item.resource_common_data.get_optional_billing_first_name(),
                    item.resource_common_data.get_optional_billing_last_name(),
                )
                .map(|name| crate::utils::truncate_secret_string(&name, 51));

                let expiry_date = card_data.get_expiry_date_as_mmyy()?;

                let is_already_authenticated = item.request.authentication_data.is_some();
                let authentication = if is_already_authenticated {
                    Some(false)
                } else {
                    match item.resource_common_data.auth_type {
                        common_enums::AuthenticationType::ThreeDs => Some(true),
                        common_enums::AuthenticationType::NoThreeDs => Some(false),
                    }
                };

                let request3ds = item.request.authentication_data.as_ref().map(|auth_data| {
                    let cavv_algorithm = auth_data.get_cavv_algorithm().map(ToString::to_string);
                    TravelhubRequest3DS {
                        cavv: auth_data.cavv.as_ref().map(|c| c.peek().to_string()),
                        cavv_algorithm,
                        eci: auth_data.eci.clone(),
                        xid: None,
                        ds_transaction_id: auth_data.ds_trans_id.clone(),
                        three_ds_secure_version: auth_data
                            .message_version
                            .as_ref()
                            .map(|v| v.to_string()),
                        acs_transaction_id: None,
                    }
                });

                TravelhubPayment {
                    payment_method: TravelhubPaymentMethod {
                        code: get_card_payment_method_code(card_data)?.to_string(),
                    },
                    payment_card: Some(TravelhubPaymentCard {
                        card_name: cardholder_name,
                        card_number: card_data.card_number.clone(),
                        expiry_date,
                        cvc: card_data.card_cvc.clone(),
                        request3ds,
                        authentication,
                    }),
                    apple_pay: None,
                    billing_address,
                }
            }
            PaymentMethodData::Wallet(WalletData::ApplePay(apple_pay_data)) => TravelhubPayment {
                payment_method: TravelhubPaymentMethod {
                    code: TRAVELHUB_APPLE_PAY_PAYMENT_METHOD_CODE.to_string(),
                },
                payment_card: None,
                apple_pay: Some(build_travelhub_apple_pay(apple_pay_data)?),
                billing_address,
            },
            _ => {
                return Err(IntegrationError::NotSupported {
                    message: "Selected payment method".to_string(),
                    connector: "travelhub",
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Use card or Apple Pay as the payment method".to_string(),
                        ),
                        doc_url: None,
                        additional_context: Some(
                            "Travelhub currently supports card and Apple Pay payments".to_string(),
                        ),
                    },
                }
                .into());
            }
        };

        Ok(Self {
            merchant_id: auth.get_merchant_id(),
            order_id: item
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            amount: item.request.minor_amount,
            currency: item.request.currency,
            capture: is_auto_capture,
            travel: build_travel_data(item.request.domain_data.as_ref()),
            payment,
        Ok(Self {
            merchant_id: auth.get_merchant_id(),
            order_id: item
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            amount: item.request.amount.amount,
            currency: item.request.currency,
            capture: is_auto_capture,
            travel: build_travel_data(item.request.domain_data.as_ref()),
            payment: TravelhubPayment {
                payment_method: TravelhubPaymentMethod {
                    code: payment_method_code,
                },
                payment_card: TravelhubPaymentCard {
                    card_name: cardholder_name,
                    card_number: card_data.card_number.clone(),
                    expiry_date,
                    cvc: card_data.card_cvc.clone(),
                    request3ds,
                    authentication,
                },
                billing_address: item
                    .resource_common_data
                    .get_billing_address()
                    .ok()
                    .map(|_| TravelhubBillingAddress {
                        number: None,
                        street: item.resource_common_data.get_optional_billing_line1(),
                        city: item.resource_common_data.get_optional_billing_city(),
                        state: item.resource_common_data.get_optional_billing_state(),
                        postal_code: item.resource_common_data.get_optional_billing_zip(),
                        country: item.resource_common_data.get_optional_billing_country(),
                    }),
            },
        })
    }
}

// Response Types

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TravelhubResponse3DS {
    #[serde(rename = "acsURL", default, skip_serializing_if = "Option::is_none")]
    pub acs_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pa_req: Option<Secret<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub md: Option<Secret<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cavv: Option<Secret<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eci: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub three_ds_secure_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub directory_server_transaction_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub three_d_server_transaction_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TravelhubRedirect {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TravelhubResult {
    Approved,
    Captured,
    Settled,
    Declined,
    Error,
    Invalid,
    Redirected,
    Cancelled,
    Refunded,
    Pending,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TravelhubPaymentsResponse {
    #[serde(rename = "merchantId", default)]
    pub merchant_id: Option<String>,
    #[serde(rename = "orderId", default)]
    pub order_id: Option<String>,
    #[serde(rename = "transactionId", default)]
    pub transaction_id: Option<String>,
    #[serde(default)]
    pub amount: Option<MinorUnit>,
    #[serde(default)]
    pub currency: Option<Currency>,
    #[serde(default)]
    pub result: Option<TravelhubResult>,
    #[serde(
        rename = "authorizationCode",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub authorization_code: Option<String>,
    #[serde(
        rename = "paymentMethodCode",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub payment_method_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redirect: Option<TravelhubRedirect>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(
        rename = "response3DS",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub response3ds: Option<TravelhubResponse3DS>,
}

pub type TravelhubAuthorizeResponse = TravelhubPaymentsResponse;
pub type TravelhubCaptureResponse = TravelhubPaymentsResponse;
pub type TravelhubPSyncResponse = TravelhubPaymentsResponse;
pub type TravelhubVoidResponse = TravelhubPaymentsResponse;
pub type TravelhubRefundResponse = TravelhubPaymentsResponse;
pub type TravelhubRSyncResponse = TravelhubPaymentsResponse;

fn map_travelhub_status(result: &TravelhubResult) -> AttemptStatus {
    match result {
        TravelhubResult::Approved => AttemptStatus::Authorized,
        TravelhubResult::Captured | TravelhubResult::Settled => AttemptStatus::Charged,
        TravelhubResult::Declined | TravelhubResult::Error | TravelhubResult::Invalid => {
            AttemptStatus::Failure
        }
        TravelhubResult::Redirected => AttemptStatus::AuthenticationPending,
        TravelhubResult::Cancelled => AttemptStatus::Voided,
        TravelhubResult::Pending | TravelhubResult::Unknown => AttemptStatus::Pending,
        TravelhubResult::Refunded => AttemptStatus::Charged,
    }
}

fn map_travelhub_refund_status(result: &TravelhubResult) -> RefundStatus {
    match result {
        TravelhubResult::Approved | TravelhubResult::Refunded | TravelhubResult::Settled => {
            RefundStatus::Success
        }
        TravelhubResult::Declined | TravelhubResult::Error | TravelhubResult::Invalid => {
            RefundStatus::Failure
        }
        TravelhubResult::Pending | TravelhubResult::Unknown => RefundStatus::Pending,
        _ => RefundStatus::Pending,
    }
}

/// Wire-format code for a failure `result`. TravelHub reports business failures on HTTP 200
/// through the `result` field alone — the payments envelope has no error code/message fields
/// to deserialize — so the result literal itself is carried as the error code rather than
/// inventing phantom response fields. (Real HTTP-error failures already surface with full
/// detail via the Spring Boot envelope parsed in `build_error_response`.)
fn travelhub_result_code(result: &TravelhubResult) -> &'static str {
    match result {
        TravelhubResult::Declined => "DECLINED",
        TravelhubResult::Error => "ERROR",
        TravelhubResult::Invalid => "INVALID",
        _ => "UNKNOWN",
    }
}

/// True when TravelHub reports an HTTP-200 business failure. Drives the citigate-style
/// early-return that converts such a response into `Err(ErrorResponse)`.
fn travelhub_is_failure(result: &TravelhubResult) -> bool {
    matches!(
        result,
        TravelhubResult::Declined | TravelhubResult::Error | TravelhubResult::Invalid
    )
}

/// Builds the `Err(ErrorResponse)` for an HTTP-200 business failure (DECLINED / ERROR /
/// INVALID), citigate-style: failures are surfaced as `Err` rather than as an
/// Ok-with-Failure response, so the merchant receives an actual error payload instead of an
/// unexplained failure status with empty code/message/reason.
fn travelhub_error_response(
    result: &TravelhubResult,
    http_code: u16,
    transaction_id: Option<String>,
    attempt_status: Option<FlowStatus>,
) -> ErrorResponse {
    let code = travelhub_result_code(result);
    ErrorResponse {
        status_code: http_code,
        code: code.to_string(),
        message: format!("TravelHub reported the transaction as {code}"),
        reason: None,
        attempt_status,
        connector_transaction_id: transaction_id,
        network_decline_code: None,
        network_advice_code: None,
        network_error_message: None,
        typed_connector_response: None,
        raw_connector_response: None,
        raw_connector_request: None,
        typed_connector_request: None,
    }
}

// Authorize Response Transformation

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<TravelhubPaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TravelhubPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let result = item
            .response
            .result
            .as_ref()
            .unwrap_or(&TravelhubResult::Pending);

        if travelhub_is_failure(result) {
            return Ok(Self {
                response: Err(travelhub_error_response(
                    result,
                    item.http_code,
                    item.response.transaction_id.clone(),
                    Some(FlowStatus::Payment(map_travelhub_status(result))),
                )),
                resource_common_data: PaymentFlowData {
                    status: map_travelhub_status(result),
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            });
        }

        let redirection_data = item.response.response3ds.as_ref().and_then(|r3ds| {
            r3ds.acs_url.as_ref().map(|acs_url| {
                let mut form_fields = std::collections::HashMap::new();
                if let Some(pa_req) = &r3ds.pa_req {
                    form_fields.insert("PaReq".to_string(), pa_req.peek().to_owned());
                }
                if let Some(md) = &r3ds.md {
                    form_fields.insert("MD".to_string(), md.peek().to_owned());
                }
                Box::new(RedirectForm::Form {
                    endpoint: acs_url.clone(),
                    method: common_utils::Method::Post,
                    form_fields,
                })
            })
        });

        let status = if redirection_data.is_some() {
            AttemptStatus::AuthenticationPending
        } else if result == &TravelhubResult::Approved {
            let is_auto_capture =
                !crate::utils::is_manual_capture(item.router_data.request.capture_method);
            if is_auto_capture {
                AttemptStatus::Charged
            } else {
                AttemptStatus::Authorized
            }
        } else {
            map_travelhub_status(result)
        };

        let resource_id = item
            .response
            .transaction_id
            .clone()
            .map(ResponseId::ConnectorTransactionId)
            .unwrap_or(ResponseId::NoResponseId);

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id,
                redirection_data,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: item.response.order_id.clone(),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
                payment_account_reference: None,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// Capture Request

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TravelhubCaptureRequest {
    pub merchant_id: String,
    pub order_id: String,
    pub amount: MinorUnit,
    pub currency: Currency,
}

impl TryFrom<&RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>>
    for TravelhubCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
    ) -> Result<Self, Self::Error> {
        let auth = TravelhubAuthType::try_from(&item.connector_config)?;

        Ok(Self {
            merchant_id: auth.get_merchant_id(),
            order_id: item
                .resource_common_data
                .connector_request_reference_id
                .clone(),
            amount: item.request.amount_to_capture.amount,
            currency: item.request.currency,
        })
    }
}

// Capture Response

impl TryFrom<ResponseRouterData<TravelhubCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TravelhubCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let result = item
            .response
            .result
            .as_ref()
            .unwrap_or(&TravelhubResult::Pending);

        if travelhub_is_failure(result) {
            // CaptureFailed, not Failure: the capture leg failed, but the authorization is
            // still live at TravelHub — the merchant may retry the capture. citigate models
            // the same at citigate/transformers.rs:695.
            return Ok(Self {
                response: Err(travelhub_error_response(
                    result,
                    item.http_code,
                    item.response.transaction_id.clone(),
                    Some(FlowStatus::Payment(AttemptStatus::CaptureFailed)),
                )),
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::CaptureFailed,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            });
        }

        let status = match result {
            // A capture TravelHub approved means the funds are captured on its side; mapping
            // it to Authorized (the shared mapper's meaning: merely held) would invite a
            // second capture call.
            TravelhubResult::Approved => AttemptStatus::Charged,
            other => map_travelhub_status(other),
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: item
                    .response
                    .transaction_id
                    .clone()
                    .map(ResponseId::ConnectorTransactionId)
                    .unwrap_or(ResponseId::NoResponseId),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: item.response.order_id.clone(),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
                payment_account_reference: None,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// Void Request

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TravelhubVoidRequest {
    pub merchant_id: String,
    pub order_id: String,
}

impl TryFrom<&RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>>
    for TravelhubVoidRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
    ) -> Result<Self, Self::Error> {
        let auth = TravelhubAuthType::try_from(&item.connector_config)?;

        Ok(Self {
            merchant_id: auth.get_merchant_id(),
            order_id: item
                .resource_common_data
                .connector_request_reference_id
                .clone(),
        })
    }
}

// Void Response

impl TryFrom<ResponseRouterData<TravelhubVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TravelhubVoidResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let result = item
            .response
            .result
            .as_ref()
            .unwrap_or(&TravelhubResult::Pending);

        if travelhub_is_failure(result) {
            // VoidFailed, not Failure: the cancel was declined, but the payment remains
            // authorized and capturable. citigate models the same at
            // citigate/transformers.rs:706.
            return Ok(Self {
                response: Err(travelhub_error_response(
                    result,
                    item.http_code,
                    item.response.transaction_id.clone(),
                    Some(FlowStatus::Payment(AttemptStatus::VoidFailed)),
                )),
                resource_common_data: PaymentFlowData {
                    status: AttemptStatus::VoidFailed,
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            });
        }

        let status = match result {
            // An approved /cancel is a successful void; the shared mapper would leave it
            // Authorized, looking capturable.
            TravelhubResult::Approved => AttemptStatus::Voided,
            other => map_travelhub_status(other),
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: item
                    .response
                    .transaction_id
                    .clone()
                    .map(ResponseId::ConnectorTransactionId)
                    .unwrap_or(ResponseId::NoResponseId),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: item.response.order_id.clone(),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
                payment_account_reference: None,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// PSync Request

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TravelhubPSyncRequest {
    pub merchant_id: String,
    pub order_id: String,
}

impl TryFrom<&RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>>
    for TravelhubPSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
    ) -> Result<Self, Self::Error> {
        let auth = TravelhubAuthType::try_from(&item.connector_config)?;

        Ok(Self {
            merchant_id: auth.get_merchant_id(),
            order_id: item
                .resource_common_data
                .connector_request_reference_id
                .clone(),
        })
    }
}

// PSync Response

impl TryFrom<ResponseRouterData<TravelhubPSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TravelhubPSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let result = item
            .response
            .result
            .as_ref()
            .unwrap_or(&TravelhubResult::Pending);

        if travelhub_is_failure(result) {
            return Ok(Self {
                response: Err(travelhub_error_response(
                    result,
                    item.http_code,
                    item.response.transaction_id.clone(),
                    Some(FlowStatus::Payment(map_travelhub_status(result))),
                )),
                resource_common_data: PaymentFlowData {
                    status: map_travelhub_status(result),
                    ..item.router_data.resource_common_data
                },
                ..item.router_data
            });
        }

        let status = if result == &TravelhubResult::Approved {
            let is_auto_capture =
                !crate::utils::is_manual_capture(item.router_data.request.capture_method);
            if is_auto_capture {
                AttemptStatus::Charged
            } else {
                AttemptStatus::Authorized
            }
        } else {
            map_travelhub_status(result)
        };

        Ok(Self {
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: item
                    .response
                    .transaction_id
                    .clone()
                    .map(ResponseId::ConnectorTransactionId)
                    .unwrap_or(ResponseId::NoResponseId),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: item.response.order_id.clone(),
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
                payment_account_reference: None,
            }),
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            ..item.router_data
        })
    }
}

// Refund Request

/// TravelHub binds a refund (and its status lookup) to the original payment by `orderId`
/// alone — the same reference that the Authorize call sent as `orderId`, carried by the
/// refund body as its only pointer to the original transaction.
///
/// `RefundFlowData::connector_request_reference_id` cannot serve here: the framework derives
/// it from `merchant_refund_id`, so it identifies *this refund*, not the payment.
///
/// The payment reference is instead taken from the refund request's `connector_order_id` —
/// the field whose documented purpose is exactly this ("connector-side identifier for the
/// original payment that this refund targets"). Both `RefundsData` (Refund) and
/// `RefundSyncData` (RSync) carry it, so this helper is shared by both. When it is absent,
/// the request is refused locally rather than sending TravelHub a reference that can never
/// match an order.
fn resolve_original_order_id(
    connector_order_id: Option<&str>,
) -> Result<String, error_stack::Report<IntegrationError>> {
    connector_order_id
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            error_stack::report!(IntegrationError::MissingRequiredField {
                field_name: "connector_order_id",
                context: IntegrationErrorContext {
                    suggested_action: Some(
                        "Pass the original Authorize call's order reference (the \
                         merchant_transaction_id that was sent to TravelHub as `orderId`) as \
                         `connector_order_id` on the refund (or refund-get) request."
                            .to_string(),
                    ),
                    doc_url: None,
                    additional_context: Some(
                        "TravelHub resolves refunds by `orderId` alone, the only \
                         original-payment identifier the refund body carries. \
                         `RefundFlowData::connector_request_reference_id` is derived from \
                         `merchant_refund_id` and identifies this refund call, so it cannot \
                         locate the original payment."
                            .to_string(),
                    ),
                },
            })
        })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TravelhubRefundRequest {
    pub merchant_id: String,
    pub order_id: String,
    pub amount: MinorUnit,
    pub currency: Currency,
}

impl TryFrom<&RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>>
    for TravelhubRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
    ) -> Result<Self, Self::Error> {
        let auth = TravelhubAuthType::try_from(&item.connector_config)?;

        Ok(Self {
            merchant_id: auth.get_merchant_id(),
            order_id: resolve_original_order_id(item.request.connector_order_id.as_deref())?,
            amount: item.request.refund_amount.amount,
            currency: item.request.currency,
        })
    }
}

// Refund Response

impl TryFrom<ResponseRouterData<TravelhubRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TravelhubRefundResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let result = item
            .response
            .result
            .as_ref()
            .unwrap_or(&TravelhubResult::Pending);

        if travelhub_is_failure(result) {
            return Ok(Self {
                response: Err(travelhub_error_response(
                    result,
                    item.http_code,
                    item.response.transaction_id.clone(),
                    Some(FlowStatus::Refund(map_travelhub_refund_status(result))),
                )),
                ..item.router_data
            });
        }

        let refund_status = map_travelhub_refund_status(result);

        let connector_refund_id = item.response.transaction_id.clone().ok_or_else(|| {
            ConnectorError::response_handling_failed_with_context(
                item.http_code,
                Some("transaction_id missing in travelhub refund response".to_string()),
            )
        })?;

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id,
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            ..item.router_data
        })
    }
}

// RSync Request

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TravelhubRSyncRequest {
    pub merchant_id: String,
    pub order_id: String,
}

impl TryFrom<&RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>>
    for TravelhubRSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: &RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
    ) -> Result<Self, Self::Error> {
        let auth = TravelhubAuthType::try_from(&item.connector_config)?;

        Ok(Self {
            merchant_id: auth.get_merchant_id(),
            order_id: resolve_original_order_id(item.request.connector_order_id.as_deref())?,
        })
    }
}

// RSync Response

impl TryFrom<ResponseRouterData<TravelhubRSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TravelhubRSyncResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let result = item
            .response
            .result
            .as_ref()
            .unwrap_or(&TravelhubResult::Pending);

        if travelhub_is_failure(result) {
            return Ok(Self {
                response: Err(travelhub_error_response(
                    result,
                    item.http_code,
                    item.response.transaction_id.clone(),
                    Some(FlowStatus::Refund(map_travelhub_refund_status(result))),
                )),
                ..item.router_data
            });
        }

        let refund_status = map_travelhub_refund_status(result);

        let connector_refund_id = item.response.transaction_id.clone().ok_or_else(|| {
            ConnectorError::response_handling_failed_with_context(
                item.http_code,
                Some("transaction_id missing in travelhub refund sync response".to_string()),
            )
        })?;

        Ok(Self {
            response: Ok(RefundsResponseData {
                connector_refund_id,
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            ..item.router_data
        })
    }
}

// Macro Wrapper Type Implementations

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TravelhubRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for TravelhubPaymentsRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: TravelhubRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&wrapper.router_data)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TravelhubRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for TravelhubCaptureRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: TravelhubRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&wrapper.router_data)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TravelhubRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for TravelhubVoidRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: TravelhubRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&wrapper.router_data)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TravelhubRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    > for TravelhubRefundRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: TravelhubRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&wrapper.router_data)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TravelhubRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    > for TravelhubPSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: TravelhubRouterData<
            RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&wrapper.router_data)
    }
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TravelhubRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    > for TravelhubRSyncRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        wrapper: TravelhubRouterData<
            RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Self::try_from(&wrapper.router_data)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
#[allow(clippy::indexing_slicing)]
mod tests {
    use std::borrow::Cow;
    use std::str::FromStr;

    use cards::CardNumber;
    use common_enums::{AttemptStatus, AuthenticationType, Currency, PaymentMethod};
    use common_utils::{id_type::MerchantId, types::MinorUnit};
    use domain_types::{
        connector_flow::Authorize,
        connector_types::{PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData},
        payment_address::{Address, AddressDetails, PaymentAddress},
        payment_method_data::{
            ApplePayCryptogramData, ApplePayDecryptedData, ApplePayPaymentData, ApplePayWalletData,
            ApplepayPaymentMethod, DefaultPCIHolder, PaymentMethodData, WalletData,
        },
        router_data::{ConnectorSpecificConfig, ErrorResponse},
        router_data_v2::RouterDataV2,
        types::{ConnectorParams, Connectors},
    };
    use hyperswitch_masking::Secret;
    use serde_json::json;

    use super::TravelhubPaymentsRequest;

    fn apple_pay_wallet(payment_data: ApplePayPaymentData) -> PaymentMethodData<DefaultPCIHolder> {
        PaymentMethodData::Wallet(WalletData::ApplePay(Box::new(ApplePayWalletData {
            payment_data,
            payment_method: ApplepayPaymentMethod {
                display_name: "Apple Pay".to_string(),
                network: "Visa".to_string(),
                pm_type: "debit".to_string(),
            },
            transaction_identifier: "apple-pay-txn-1".to_string(),
        })))
    }

    fn authorize_router_data(
        payment_method_data: PaymentMethodData<DefaultPCIHolder>,
    ) -> RouterDataV2<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<DefaultPCIHolder>,
        PaymentsResponseData,
    > {
        RouterDataV2 {
            flow: std::marker::PhantomData,
            resource_common_data: PaymentFlowData {
                raw_connector_status: None,
                merchant_id: MerchantId::default(),
                customer_id: None,
                connector_customer: None,
                payment_id: "pay_travelhub_applepay".to_string(),
                attempt_id: "attempt_travelhub_applepay".to_string(),
                status: AttemptStatus::Pending,
                payment_method: PaymentMethod::Wallet,
                payment_method_type: Some(common_enums::PaymentMethodType::ApplePay),
                description: None,
                return_url: None,
                order_details: None,
                address: PaymentAddress::new(
                    None,
                    Some(Address {
                        address: Some(AddressDetails {
                            first_name: Some(Secret::new("Ada".to_string())),
                            last_name: Some(Secret::new("Lovelace".to_string())),
                            line1: Some(Secret::new("1 Apple Park Way".to_string())),
                            city: Some(Secret::new("Cupertino".to_string())),
                            zip: Some(Secret::new("95014".to_string())),
                            country: Some(common_enums::CountryAlpha2::US),
                            ..Default::default()
                        }),
                        phone: None,
                        email: None,
                    }),
                    None,
                    None,
                ),
                auth_type: AuthenticationType::NoThreeDs,
                connector_feature_data: None,
                amount_captured: None,
                minor_amount_captured: None,
                minor_amount_capturable: None,
                amount: None,
                minor_amount_authorized: None,
                access_token: None,
                session_token: None,
                reference_id: None,
                connector_order_id: None,
                preprocessing_id: None,
                connector_api_version: None,
                connector_request_reference_id: "travelhub-apple-pay-ref".to_string(),
                test_mode: None,
                connector_http_status_code: None,
                external_latency: None,
                raw_connector_response: None,
                typed_connector_response: None,
                connectors: Connectors {
                    travelhub: ConnectorParams {
                        base_url: "https://preprod.travel.worldline-solutions.com/travelhub/"
                            .to_string(),
                        dispute_base_url: None,
                        ..Default::default()
                    },
                    ..Default::default()
                }
                .into(),
                vault_headers: None,
                connector_response_headers: None,
                raw_connector_request: None,
                typed_connector_request: None,
                connector_response: None,
                recurring_mandate_payment_data: None,
                l2_l3_data: None,
                merchant_request_id: None,
                sender_payment_instrument_id: None,
                connector_returned_payment_method_details: None,
                settlement_status: None,
            },
            connector_config: ConnectorSpecificConfig::Travelhub {
                username: Secret::new("travelhub-user".to_string()),
                password: Secret::new("travelhub-password".to_string()),
                merchant_id: Secret::new("travelhub-merchant".to_string()),
                base_url: None,
            },
            request: PaymentsAuthorizeData {
                split_settlement: None,
                customer_document_details: None,
                customer_date_of_birth: None,
                payment_channel: None,
                authentication_data: None,
                connector_testing_data: None,
                currency_conversion_data: None,
                access_token: None,
                payment_method_data,
                amount: MinorUnit::new(1000),
                order_tax_amount: None,
                surcharge_amount: None,
                email: None,
                customer_name: None,
                currency: Currency::USD,
                confirm: true,
                capture_method: None,
                integrity_object: None,
                router_return_url: None,
                webhook_url: None,
                complete_authorize_url: None,
                mandate_id: None,
                setup_future_usage: None,
                off_session: None,
                browser_info: None,
                order_category: None,
                session_token: None,
                enrolled_for_3ds: Some(false),
                related_transaction_id: None,
                payment_experience: None,
                payment_method_type: Some(common_enums::PaymentMethodType::ApplePay),
                customer_id: Some(
                    common_utils::id_type::CustomerId::try_from(Cow::from(
                        "cust_travelhub_applepay".to_string(),
                    ))
                    .expect("customer id"),
                ),
                request_incremental_authorization: None,
                metadata: None,
                minor_amount: MinorUnit::new(1000),
                merchant_order_id: None,
                shipping_cost: None,
                merchant_account_id: None,
                merchant_config_currency: None,
                all_keys_required: None,
                customer_acceptance: None,
                split_payments: None,
                request_extended_authorization: None,
                setup_mandate_details: None,
                enable_overcapture: None,
                connector_feature_data: None,
                billing_descriptor: None,
                enable_partial_authorization: None,
                locale: None,
                continue_redirection_url: None,
                redirect_response: None,
                threeds_method_comp_ind: None,
                tokenization: None,
                is_account_funding_transaction: None,
                recipient_details: None,
                business_country: None,
                additional_connector_details: None,
                customer: None,
                mit_category: None,
                domain_data: None,
                partner_merchant_identifier_details: None,
            },
            response: Err(ErrorResponse::default()),
        }
    }

    #[test]
    fn authorize_request_maps_encrypted_apple_pay_to_travelhub_payload() {
        let router_data = authorize_router_data(apple_pay_wallet(ApplePayPaymentData::Encrypted(
            "encrypted-apple-pay-token".to_string(),
        )));

        let request =
            TravelhubPaymentsRequest::try_from(&router_data).expect("travelhub apple pay request");
        let payload = serde_json::to_value(request).expect("serialized request");

        assert_eq!(payload["payment"]["paymentMethod"]["code"], json!("212"));
        assert_eq!(
            payload["payment"]["applePay"]["encryptedPaymentData"],
            json!("encrypted-apple-pay-token")
        );
        assert!(
            payload["payment"].get("paymentCard").is_none(),
            "Apple Pay requests must not serialize card details: {payload}"
        );
    }

    #[test]
    fn authorize_request_maps_decrypted_apple_pay_to_travelhub_payload() {
        let router_data = authorize_router_data(apple_pay_wallet(ApplePayPaymentData::Decrypted(
            ApplePayDecryptedData {
                application_primary_account_number: CardNumber::from_str("4761739001010010")
                    .expect("valid card number"),
                application_expiration_month: Secret::new("12".to_string()),
                application_expiration_year: Secret::new("2027".to_string()),
                payment_data: ApplePayCryptogramData {
                    online_payment_cryptogram: Secret::new(
                        "YwAAAB4EEI0Vn70BcjyTgFhgAgA=".to_string(),
                    ),
                    eci_indicator: Some("05".to_string()),
                },
                device_manufacturer_identifier: None,
                merchant_token_identifier: None,
            },
        )));

        let request =
            TravelhubPaymentsRequest::try_from(&router_data).expect("travelhub apple pay request");
        let payload = serde_json::to_value(request).expect("serialized request");

        assert_eq!(payload["payment"]["paymentMethod"]["code"], json!("212"));
        assert_eq!(
            payload["payment"]["applePay"]["decryptedPaymentData"]["dpan"],
            json!("4761739001010010")
        );
        assert_eq!(
            payload["payment"]["applePay"]["decryptedPaymentData"]["expiryDate"],
            json!("1227")
        );
        assert_eq!(
            payload["payment"]["applePay"]["decryptedPaymentData"]["cryptogram"],
            json!("YwAAAB4EEI0Vn70BcjyTgFhgAgA=")
        );
        assert_eq!(
            payload["payment"]["applePay"]["decryptedPaymentData"]["eci"],
            json!("05")
        );
        assert!(
            payload["payment"].get("paymentCard").is_none(),
            "Apple Pay requests must not serialize card details: {payload}"
        );
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
#[allow(clippy::indexing_slicing)]
mod tests {
    use domain_types::connector_types::{AirlineData, AirlinePassenger, DomainData};
    use serde_json::json;

    use super::build_travel_data;

    #[test]
    fn build_travel_data_normalizes_iata_passenger_type_for_travelhub() {
        let domain_data = DomainData {
            airline_data: Some(AirlineData {
                airline_code: Some("6E".to_string()),
                passengers: vec![AirlinePassenger {
                    passenger_type: Some("ADT".to_string()),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };

        let travel = build_travel_data(Some(&domain_data)).expect("travel data");
        let payload = serde_json::to_value(travel).expect("serialized travel data");

        assert_eq!(payload["passenger"][0]["type"], json!("ADULT"));
    }
}
