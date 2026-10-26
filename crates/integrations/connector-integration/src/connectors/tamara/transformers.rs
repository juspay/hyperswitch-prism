use std::{collections::BTreeMap, fmt::Debug};

use crate::connectors::tamara::TamaraRouterData;
use crate::types::ResponseRouterData;
use crate::utils;
use common_enums::EligibilityStatus;
use common_enums::{
    AttemptStatus, Currency, ProductType, DepositPaymentType, EducationType, FutureUsage, OnlineStatus,
    PremiumCustomerStatus, RefundStatus, RoomClass, TicketRefundability, TravelClass, TripType,
};
use common_utils::{
    types::{AmountConvertor, FloatMajorUnit, FloatMajorUnitForConnector, MinorUnit},
    Email,
};
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, PaymentMethodEligibility, RSync, Refund, Void},
    connector_types::{
        AirlineData, DomainData, EducationData, EventType, HotelData, PMEligibility,
        PaymentFlowData, PaymentMethodEligibilityData, PaymentMethodEligibilityResponse,
        PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData,
        PaymentsSyncData, RawConnectorStatus, RefundFlowData, RefundSyncData, RefundsData,
        RefundsResponseData, RentalData, ResponseId, RiskAssessmentData, SalesData,
        TicketDeliveryContact, TravelData,
    },
    errors,
    payment_address::AddressDetails,
    payment_method_data::{PayLaterData, PaymentMethodData, PaymentMethodDataTypes},
    router_data::ConnectorSpecificConfig,
    router_data_v2::RouterDataV2,
    router_response_types::RedirectForm,
};
use error_stack::ResultExt;
use hyperswitch_masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct TamaraAuthType {
    pub api_key: Secret<String>,
}

impl TryFrom<&ConnectorSpecificConfig> for TamaraAuthType {
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::Tamara { api_key, .. } => Ok(Self {
                api_key: api_key.to_owned(),
            }),
            _ => Err(error_stack::report!(
                errors::IntegrationError::FailedToObtainAuthType {
                    context: errors::IntegrationErrorContext {
                        additional_context: Some("Expected Tamara api_key".to_string()),
                        ..Default::default()
                    }
                }
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TamaraErrorResponse {
    pub message: String,
    pub errors: Option<Vec<TamaraErrorDetail>>,
    pub data: Option<serde_json::Value>,
    pub title: Option<String>,
    pub screen_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TamaraErrorDetail {
    pub error_code: String,
}

#[derive(Debug, Deserialize, Clone, Serialize, strum::Display)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum TamaraPaymentStatus {
    New,
    Approved,
    Declined,
    Authorised,
    FullyCaptured,
    PartiallyCaptured,
    Canceled,
    Updated,
    Expired,
}

impl From<TamaraPaymentStatus> for AttemptStatus {
    fn from(status: TamaraPaymentStatus) -> Self {
        match status {
            TamaraPaymentStatus::Authorised => Self::Authorized,
            TamaraPaymentStatus::FullyCaptured | TamaraPaymentStatus::PartiallyCaptured => {
                Self::Charged
            }
            TamaraPaymentStatus::Canceled | TamaraPaymentStatus::Updated => Self::Voided,
            TamaraPaymentStatus::Declined | TamaraPaymentStatus::Expired => Self::Failure,
            TamaraPaymentStatus::New | TamaraPaymentStatus::Approved => Self::Pending,
        }
    }
}

#[derive(Debug, Deserialize, Clone, Serialize, strum::Display)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum TamaraRefundStatus {
    New,
    Declined,
    Expired,
    Approved,
    Authorised,
    PartiallyCaptured,
    FullyCaptured,
    PartiallyRefunded,
    FullyRefunded,
    Canceled,
    Updated,
}

impl From<TamaraRefundStatus> for RefundStatus {
    fn from(status: TamaraRefundStatus) -> Self {
        match status {
            TamaraRefundStatus::FullyRefunded | TamaraRefundStatus::PartiallyRefunded => {
                Self::Success
            }
            TamaraRefundStatus::PartiallyCaptured
            | TamaraRefundStatus::FullyCaptured
            | TamaraRefundStatus::Approved
            | TamaraRefundStatus::Authorised => Self::Pending,
            TamaraRefundStatus::Declined
            | TamaraRefundStatus::Expired
            | TamaraRefundStatus::Canceled
            | TamaraRefundStatus::Updated => Self::Failure,
            TamaraRefundStatus::New => Self::Pending,
        }
    }
}
fn tamara_payment_raw_connector_status(status: &TamaraPaymentStatus) -> RawConnectorStatus {
    let connector_status = status.to_string();
    RawConnectorStatus {
        code: Some(connector_status),
        message: None,
        reason: None,
    }
}

fn tamara_refund_raw_connector_status(status: &TamaraRefundStatus) -> RawConnectorStatus {
    let connector_status = status.to_string();
    RawConnectorStatus {
        code: Some(connector_status),
        message: None,
        reason: None,
    }
}

// ===== AUTHORIZE (hits /checkout, creates order and returns checkout_url) =====

#[derive(Debug, Serialize)]
pub struct TamaraPaymentsRequest {
    pub total_amount: TamaraAmount,
    pub shipping_amount: TamaraAmount,
    pub tax_amount: TamaraAmount,
    pub order_reference_id: String,
    pub country_code: String,
    pub description: String,
    pub items: Vec<TamaraLineItem>,
    pub consumer: TamaraConsumer,
    pub merchant_url: TamaraMerchantUrl,
    pub shipping_address: TamaraAddress,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub risk_assessment: Option<TamaraRiskAssessment>,
}

// ===== RISK ASSESSMENT (the `risk_assessment` object on /checkout) =====
// Docs: https://docs.tamara.co/docs/additional-customer-details-for-risk-assessment.md
// General fields come from `RiskAssessmentData`; the vertical blocks (flights,
// hotels, trains, education, car sales, car rentals) live under `DomainData` at
// our side but nest inside `risk_assessment` in Tamara's payload.

/// Tamara dates are `dd-mm-yyyy`; domain dates arrive as `yyyy-MM-dd` or RFC 3339.
fn to_tamara_date(value: &str) -> String {
    let date = value.split(['T', ' ']).next().unwrap_or(value);
    let mut parts = date.split('-');
    match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some(year), Some(month), Some(day), None) if year.len() == 4 => {
            format!("{day}-{month}-{year}")
        }
        _ => value.to_string(),
    }
}

fn full_name(
    first_name: Option<&Secret<String>>,
    last_name: Option<&Secret<String>>,
) -> Option<String> {
    match (first_name, last_name) {
        (Some(first), Some(last)) => Some(format!("{} {}", first.peek(), last.peek())),
        (Some(name), None) | (None, Some(name)) => Some(name.peek().clone()),
        (None, None) => None,
    }
}

/// Tamara collects passenger / guest names as dynamic keys, e.g. `passenger1_name`.
fn tamara_name_map<I>(names: I, key_prefix: &str) -> Option<BTreeMap<String, String>>
where
    I: IntoIterator<Item = Option<String>>,
{
    let map: BTreeMap<String, String> = names
        .into_iter()
        .flatten()
        .enumerate()
        .map(|(index, name)| (format!("{key_prefix}{}_name", index + 1), name))
        .collect();
    (!map.is_empty()).then_some(map)
}

/// Tamara's `ticket_delivery_recipient` is "Email or Phone Number where tickets are sent".
fn tamara_ticket_delivery_recipient(contact: &TicketDeliveryContact) -> Option<String> {
    contact
        .email
        .as_ref()
        .map(|email| email.peek().clone())
        .or_else(|| {
            contact
                .phone_number
                .as_ref()
                .map(|phone_number| phone_number.peek().clone())
        })
}

fn is_refundable_booking(refundability: TicketRefundability) -> bool {
    matches!(refundability, TicketRefundability::Refundable)
}

/// Tamara takes a single-line location string ("Address of the ... location.
/// Airport code if applicable").
fn tamara_location(address: &AddressDetails) -> Option<String> {
    let parts = [
        address.line1.as_ref().map(|line| line.peek().clone()),
        address.line2.as_ref().map(|line| line.peek().clone()),
        address.line3.as_ref().map(|line| line.peek().clone()),
        address.city.as_ref().map(|city| city.peek().clone()),
        address.state.as_ref().map(|state| state.peek().clone()),
        address.zip.as_ref().map(|zip| zip.peek().clone()),
        address.country.map(|country| country.to_string()),
    ];
    let joined = parts
        .into_iter()
        .flatten()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(", ");
    (!joined.is_empty()).then_some(joined)
}

fn tamara_deposit_payment_type(deposit_payment_type: DepositPaymentType) -> String {
    match deposit_payment_type {
        DepositPaymentType::Cash => "Cash",
        DepositPaymentType::CreditCard => "Credit Card",
        DepositPaymentType::DebitCard => "Debit Card",
        DepositPaymentType::PrepaidCard => "Prepaid Card",
    }
    .to_string()
}

#[derive(Debug, Serialize)]
pub struct TamaraRiskAssessment {
    pub is_premium_customer: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_creation_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_order_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub risk_score: Option<u32>,
    // Required by Tamara; "will be null if first order".
    pub date_first_paid: Option<String>,
    pub date_last_paid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_count_last_24h: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flights_itinerary: Option<FlightsItinerary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hotels_itinerary: Option<HotelItinerary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trains_itinerary: Option<TrainsItinerary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub education: Option<Education>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub car_sales: Option<CarSales>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub car_rentals: Option<CarRentals>,
}

impl TamaraRiskAssessment {
    /// General signals from [`RiskAssessmentData`]; vertical blocks from [`DomainData`].
    /// `purchase_type` is Tamara's "Subscription" / "One-time" vocabulary and is
    /// inferred from the payment's setup signals by the caller.
    fn from_domain(
        risk_assessment_data: &RiskAssessmentData,
        domain_data: Option<&DomainData>,
        purchase_type: &str,
    ) -> Self {
        Self {
            is_premium_customer: matches!(
                risk_assessment_data.is_premium_customer,
                Some(PremiumCustomerStatus::PremiumCustomer)
            ),
            account_creation_date: risk_assessment_data
                .customer_account_creation_date
                .as_deref()
                .map(to_tamara_date),
            total_order_count: risk_assessment_data.number_of_orders,
            risk_score: risk_assessment_data.risk_score,
            date_first_paid: risk_assessment_data
                .date_first_paid
                .as_deref()
                .map(to_tamara_date),
            date_last_paid: risk_assessment_data
                .date_last_paid
                .as_deref()
                .map(to_tamara_date),
            order_count_last_24h: risk_assessment_data.number_of_orders_in_past_24h,
            flights_itinerary: domain_data
                .and_then(|domain| domain.airline_data.as_ref())
                .map(FlightsItinerary::from),
            hotels_itinerary: domain_data
                .and_then(|domain| domain.hotel_data.as_ref())
                .map(HotelItinerary::from),
            trains_itinerary: domain_data
                .and_then(|domain| domain.travel_data.as_ref())
                .map(TrainsItinerary::from),
            education: domain_data
                .and_then(|domain| domain.education_data.as_ref())
                .map(|education| Education::from_education_data(education, purchase_type)),
            car_sales: domain_data
                .and_then(|domain| domain.sales_data.as_ref())
                .map(CarSales::from),
            car_rentals: domain_data
                .and_then(|domain| domain.rental_data.as_ref())
                .map(CarRentals::from),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct FlightsItinerary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destination: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub travel_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trip_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub carrier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number_of_passengers: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name_of_passengers: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ticket_delivery_recipient: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub travel_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_refundable_booking: Option<bool>,
}

impl From<&AirlineData> for FlightsItinerary {
    fn from(airline_data: &AirlineData) -> Self {
        let first_segment = airline_data.flight_segments.first();
        let last_segment = airline_data.flight_segments.last();
        let location_code = |location: &domain_types::connector_types::AirlineLocation| {
            location
                .airport_code
                .clone()
                .or_else(|| location.city_code.clone())
        };
        Self {
            origin: first_segment
                .and_then(|segment| segment.departure.as_ref())
                .and_then(location_code),
            destination: last_segment
                .and_then(|segment| segment.arrival.as_ref())
                .and_then(location_code),
            travel_type: first_segment.and_then(|segment| segment.flight_type.clone()),
            trip_type: airline_data.trip_type.map(|trip_type| {
                match trip_type {
                    TripType::OneWay => "OneWay",
                    TripType::Return => "Return",
                    TripType::MultipleDestination => "MultipleDestination",
                }
                .to_string()
            }),
            class: first_segment.and_then(|segment| segment.class_of_service.clone()),
            carrier: first_segment
                .and_then(|segment| segment.marketing_carrier_name.clone())
                .or_else(|| airline_data.issuing_carrier_code.clone())
                .or_else(|| airline_data.airline_code.clone()),
            number_of_passengers: airline_data.number_of_passengers.or_else(|| {
                u32::try_from(airline_data.passengers.len())
                    .ok()
                    .filter(|count| *count > 0)
            }),
            name_of_passengers: tamara_name_map(
                airline_data.passengers.iter().map(|passenger| {
                    passenger.customer.as_ref().and_then(|customer| {
                        full_name(customer.first_name.as_ref(), customer.last_name.as_ref())
                    })
                }),
                "passenger",
            ),
            ticket_delivery_recipient: airline_data.flight_segments.iter().find_map(|segment| {
                segment
                    .ticket_delivery_recipient
                    .as_ref()
                    .and_then(tamara_ticket_delivery_recipient)
            }),
            travel_date: airline_data
                .flight_date
                .as_deref()
                .map(to_tamara_date)
                .or_else(|| {
                    first_segment
                        .and_then(|segment| segment.departure.as_ref())
                        .and_then(|departure| departure.date_time.as_deref())
                        .map(to_tamara_date)
                }),
            is_refundable_booking: airline_data.refundability.map(is_refundable_booking),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct TrainsItinerary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destination: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub carrier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number_of_passengers: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name_of_passengers: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub travel_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_refundable_booking: Option<bool>,
}

impl From<&TravelData> for TrainsItinerary {
    fn from(travel_data: &TravelData) -> Self {
        let first_segment = travel_data.travel_segments.first();
        Self {
            origin: first_segment.and_then(|segment| segment.origin.clone()),
            destination: first_segment.and_then(|segment| segment.destination.clone()),
            class: first_segment
                .and_then(|segment| segment.travel_class)
                .map(|travel_class| {
                    match travel_class {
                        TravelClass::Economy => "Economy",
                        TravelClass::PremiumEconomy => "Premium",
                        TravelClass::Business => "Business",
                        TravelClass::First => "First",
                    }
                    .to_string()
                }),
            carrier: travel_data.carrier_name.clone(),
            number_of_passengers: travel_data.number_of_passengers,
            name_of_passengers: tamara_name_map(
                travel_data
                    .travel_segments
                    .iter()
                    .flat_map(|segment| segment.passengers.iter())
                    .map(|traveller| {
                        full_name(traveller.first_name.as_ref(), traveller.last_name.as_ref())
                    }),
                "passenger",
            ),
            travel_date: travel_data.travel_date.as_deref().map(to_tamara_date),
            is_refundable_booking: travel_data.refundability.map(is_refundable_booking),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct HotelItinerary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checkin_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checkout_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number_of_guests: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name_of_guests: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ticket_delivery_recipient: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number_of_rooms: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hotel_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hotel_city: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hotel_country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hotel_room_class: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_refundable_booking: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<HotelItineraryHost>,
}

/// Host details; Tamara requires these for rentals ("Required in case of rentals").
#[derive(Debug, Serialize)]
pub struct HotelItineraryHost {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_registration_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_number_of_reservations: Option<u32>,
}

impl From<&HotelData> for HotelItinerary {
    fn from(hotel_data: &HotelData) -> Self {
        // Tamara's `hotels_itinerary` describes a single property; take the first stay.
        let segment = hotel_data.hotel_segments.first();
        let host = segment
            .and_then(|segment| segment.host.as_ref())
            .and_then(|host| {
                let host_registration_date = host.registered_at.as_deref().map(to_tamara_date);
                let host_number_of_reservations = host.number_of_reservations;
                (host_registration_date.is_some() || host_number_of_reservations.is_some())
                    .then_some(HotelItineraryHost {
                        host_registration_date,
                        host_number_of_reservations,
                    })
            });
        Self {
            checkin_date: segment
                .and_then(|segment| segment.checkin.as_deref())
                .map(to_tamara_date),
            checkout_date: segment
                .and_then(|segment| segment.checkout.as_deref())
                .map(to_tamara_date),
            number_of_guests: hotel_data
                .number_of_guests
                .or_else(|| segment.and_then(|segment| segment.number_of_guests)),
            name_of_guests: tamara_name_map(
                segment
                    .map(|segment| segment.guests.iter())
                    .into_iter()
                    .flatten()
                    .map(|guest| full_name(guest.first_name.as_ref(), guest.last_name.as_ref())),
                "guest",
            ),
            ticket_delivery_recipient: hotel_data
                .ticket_delivery_recipient
                .as_ref()
                .and_then(tamara_ticket_delivery_recipient),
            number_of_rooms: segment.and_then(|segment| segment.number_of_rooms),
            hotel_name: segment.and_then(|segment| segment.hotel_name.clone()),
            hotel_city: segment.and_then(|segment| segment.hotel_city.clone()),
            hotel_country: segment.and_then(|segment| segment.hotel_country.clone()),
            hotel_room_class: segment
                .and_then(|segment| segment.room_class)
                .map(|room_class| {
                    match room_class {
                        RoomClass::Standard => "Standard",
                        RoomClass::Suite => "Suite",
                        RoomClass::Deluxe => "Deluxe",
                    }
                    .to_string()
                }),
            is_refundable_booking: hotel_data.refundability.map(is_refundable_booking),
            host,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Education {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub education_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_location: Option<String>,
    pub purchase_type: String,
}

impl Education {
    fn from_education_data(education_data: &EducationData, purchase_type: &str) -> Self {
        Self {
            education_type: education_data.education_type.map(|education_type| {
                match education_type {
                    EducationType::Course => "Course",
                    EducationType::Workshop => "Workshop",
                    EducationType::Tutoring => "Tutoring",
                }
                .to_string()
            }),
            start_date: education_data.start_date.as_deref().map(to_tamara_date),
            end_date: education_data.end_date.as_deref().map(to_tamara_date),
            event_location: education_data.is_online.map(|is_online| {
                match is_online {
                    OnlineStatus::Online => "Online",
                    OnlineStatus::Offline => "Offline",
                }
                .to_string()
            }),
            purchase_type: purchase_type.to_string(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct CarSales {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub year: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub make: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trim: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub odometer: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_previous_owners: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_spec: Option<String>,
}

impl From<&SalesData> for CarSales {
    fn from(sales_data: &SalesData) -> Self {
        Self {
            year: sales_data.year,
            make: sales_data.make.clone(),
            model: sales_data.model.clone(),
            trim: sales_data.trim.clone(),
            odometer: sales_data.odometer_km,
            num_previous_owners: sales_data.number_of_previous_owners,
            market_spec: sales_data.market_spec.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct CarRentals {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rental_company: Option<String>,
    // `pickup_date` / `dropoff_date` are not sent: `RentalData` carries the
    // pickup / dropoff locations but no rental dates.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pickup_location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dropoff_location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub car_class: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number_of_drivers: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name_of_drivers: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub insurance_purchased: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub add_ons: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deposit_required: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deposit_payment_type: Option<String>,
}

impl From<&RentalData> for CarRentals {
    fn from(rental_data: &RentalData) -> Self {
        let deposit_payment_type = rental_data
            .deposit_payment_type
            .map(tamara_deposit_payment_type);
        let add_ons = rental_data
            .extra_charges
            .iter()
            .filter_map(|charge| charge.charge_type.clone())
            .collect::<Vec<_>>()
            .join(", ");
        Self {
            rental_company: rental_data.rental_company.clone(),
            pickup_location: rental_data.pickup.as_ref().and_then(tamara_location),
            dropoff_location: rental_data.dropoff.as_ref().and_then(tamara_location),
            car_class: rental_data
                .vehicle
                .as_ref()
                .and_then(|vehicle| vehicle.car_class.clone()),
            number_of_drivers: rental_data.number_of_drivers.or_else(|| {
                u32::try_from(rental_data.drivers.len())
                    .ok()
                    .filter(|count| *count > 0)
            }),
            name_of_drivers: tamara_name_map(
                rental_data.drivers.iter().map(|driver| {
                    driver.driver.as_ref().and_then(|customer| {
                        full_name(customer.first_name.as_ref(), customer.last_name.as_ref())
                    })
                }),
                "driver",
            ),
            insurance_purchased: Some(!rental_data.insurances.is_empty()),
            add_ons: (!add_ons.is_empty()).then_some(add_ons),
            // No dedicated deposit flag on `RentalData`; a deposit funding type
            // implies the rental takes a deposit.
            deposit_required: deposit_payment_type.as_ref().map(|_| true),
            deposit_payment_type,
        }
    }
}
#[derive(Debug, Serialize)]
pub struct TamaraLineItem {
    pub name: String,
    pub quantity: u16,
    pub reference_id: String,
    pub sku: String,
    pub total_amount: TamaraAmount,
    #[serde(rename = "type")]
    pub item_type: ProductType,
    pub item_url: Option<String>,
    pub image_url: Option<String>,
    pub unit_price: TamaraAmount,
    pub discount_amount: Option<TamaraAmount>,
    pub tax_amount: Option<TamaraAmount>,
}

#[derive(Debug, Serialize)]
pub struct TamaraConsumer {
    pub first_name: Secret<String>,
    pub last_name: Secret<String>,
    pub phone_number: Secret<String>,
    pub email: Email,
}

#[derive(Debug, Serialize)]
pub struct TamaraMerchantUrl {
    pub success: String,
    pub failure: String,
    pub cancel: String,
}

#[derive(Debug, Serialize)]
pub struct TamaraAddress {
    pub first_name: Secret<String>,
    pub last_name: Secret<String>,
    pub line1: Secret<String>,
    pub city: Secret<String>,
    pub country_code: Secret<String>,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TamaraRouterData<
            RouterDataV2<
                Authorize,
                PaymentFlowData,
                PaymentsAuthorizeData<T>,
                PaymentsResponseData,
            >,
            T,
        >,
    > for TamaraPaymentsRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: TamaraRouterData<
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
        //Tamaraconnector only supports PayLater payment method.
        match &router_data.request.payment_method_data {
            PaymentMethodData::PayLater(PayLaterData::TamaraRedirect {}) => {}
            _ => {
                return Err(errors::IntegrationError::NotImplemented(
                    utils::get_unimplemented_payment_method_error_message("Tamara"),
                    errors::IntegrationErrorContext::default(),
                )
                .into())
            }
        }
        let amount = router_data.request.amount.amount;
        let currency = router_data.request.currency;
        let order_ref = router_data
            .resource_common_data
            .connector_request_reference_id
            .clone();
        let return_url = router_data
            .request
            .router_return_url
            .clone()
            .unwrap_or_default();
        let converter = FloatMajorUnitForConnector;
        let amount = converter.convert(amount, currency).change_context(
            errors::IntegrationError::RequestEncodingFailed {
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Failed to convert authorize amount from minor to major units".to_string(),
                    ),
                    ..Default::default()
                },
            },
        )?;
        let shipping_amount = converter
            .convert(
                router_data
                    .request
                    .shipping_cost
                    .as_ref()
                    .map(|money| money.amount)
                    .unwrap_or(MinorUnit(0)),
                currency,
            )
            .change_context(errors::IntegrationError::RequestEncodingFailed {
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Failed to convert shipping amount from minor to major units".to_string(),
                    ),
                    ..Default::default()
                },
            })?;
        let tax_amount = converter
            .convert(
                router_data
                    .request
                    .order_tax_amount
                    .as_ref()
                    .map(|money| money.amount)
                    .unwrap_or(MinorUnit(0)),
                currency,
            )
            .change_context(errors::IntegrationError::RequestEncodingFailed {
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Failed to convert tax amount from minor to major units".to_string(),
                    ),
                    ..Default::default()
                },
            })?;
        let country_code = router_data
            .resource_common_data
            .get_shipping_or_billing_country()?
            .to_string();
        let description = router_data
            .resource_common_data
            .description
            .clone()
            .unwrap_or_default();
        let email = router_data.request.get_email()?;
        let first_name = router_data
            .resource_common_data
            .get_shipping_or_billing_first_name()?;
        let last_name = router_data
            .resource_common_data
            .get_shipping_or_billing_last_name()?;
        let phone_number = router_data
            .resource_common_data
            .get_shipping_or_billing_phone_number_plain()?;
        let shipping_line1 = router_data
            .resource_common_data
            .get_optional_shipping_or_billing_line1_string();
        let shipping_city = router_data
            .resource_common_data
            .get_optional_shipping_or_billing_city_string();
        let shipping_country = router_data
            .resource_common_data
            .get_optional_shipping_or_billing_country_string();

        let to_tamara_amount = |value: MinorUnit, context: &'static str| {
            converter
                .convert(value, currency)
                .map(|amount| TamaraAmount { amount, currency })
                .change_context(errors::IntegrationError::RequestEncodingFailed {
                    context: errors::IntegrationErrorContext {
                        additional_context: Some(context.to_string()),
                        ..Default::default()
                    },
                })
        };
        let optional_amount = |value: Option<MinorUnit>, context: &'static str| {
            value.map(|v| to_tamara_amount(v, context)).transpose()
        };

        let items: Vec<TamaraLineItem> = router_data
            .resource_common_data
            .order_details
            .clone()
            .filter(|od| !od.is_empty())
            .ok_or(error_stack::report!(
                errors::IntegrationError::MissingRequiredField {
                    field_name: "order_details",
                    context: errors::IntegrationErrorContext::default(),
                }
            ))?
            .iter()
            .map(|data| {
                Ok(TamaraLineItem {
                    name: data.get_product_name(),
                    quantity: data.get_quantity(),
                    reference_id: data.get_product_id()?,
                    sku: data.get_sku()?,
                    total_amount: to_tamara_amount(
                        data.get_total_amount(),
                        "Failed to convert line item total amount from minor to major units",
                    )?,
                    item_type: data.get_product_type()?,
                    item_url: data.get_optional_product_link(),
                    image_url: data.get_optional_product_img_link(),
                    unit_price: to_tamara_amount(
                        data.get_unit_amount(),
                        "Failed to convert line item unit price from minor to major units",
                    )?,
                    discount_amount: optional_amount(
                        data.get_optional_unit_discount_amount(),
                        "Failed to convert line item discount amount from minor to major units",
                    )?,
                    tax_amount: optional_amount(
                        data.get_optional_total_tax_amount(),
                        "Failed to convert line item tax amount from minor to major units",
                    )?,
                })
            })
            .collect::<Result<Vec<_>, error_stack::Report<errors::IntegrationError>>>()?;

        // Tamara's education block wants "Subscription" / "One-time"; our
        // `EducationData.purchase_type` is a purchase-channel enum and cannot
        // express that axis, so infer it from the payment's setup signals.
        let purchase_type = if router_data.request.setup_mandate_details.is_some()
            || matches!(
                router_data.request.setup_future_usage,
                Some(FutureUsage::OffSession)
            ) {
            "Subscription"
        } else {
            "One-time"
        };

        Ok(Self {
            total_amount: TamaraAmount { amount, currency },
            shipping_amount: TamaraAmount {
                amount: shipping_amount,
                currency,
            },
            tax_amount: TamaraAmount {
                amount: tax_amount,
                currency,
            },
            order_reference_id: order_ref.clone(),
            country_code,
            description: description.clone(),
            items,
            consumer: TamaraConsumer {
                first_name: first_name.clone(),
                last_name: last_name.clone(),
                phone_number,
                email,
            },
            merchant_url: TamaraMerchantUrl {
                success: return_url.clone(),
                failure: return_url.clone(),
                cancel: return_url.clone(),
            },
            shipping_address: TamaraAddress {
                first_name,
                last_name,
                line1: shipping_line1.into(),
                city: shipping_city.into(),
                country_code: shipping_country.into(),
            },
            // Only sent when the merchant supplied risk-assessment signals.
            risk_assessment: router_data.request.risk_assessment_data.as_ref().map(
                |risk_assessment_data| {
                    TamaraRiskAssessment::from_domain(
                        risk_assessment_data,
                        router_data.request.domain_data.as_ref(),
                        purchase_type,
                    )
                },
            ),
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TamaraPaymentsResponse {
    pub order_id: String,
    pub checkout_id: Option<String>,
    pub checkout_url: Option<String>,
    pub status: TamaraPaymentStatus,
}

impl<T: PaymentMethodDataTypes> TryFrom<ResponseRouterData<TamaraPaymentsResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TamaraPaymentsResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = AttemptStatus::from(item.response.status.clone());
        let raw_connector_status = tamara_payment_raw_connector_status(&item.response.status);
        let connector_order_id = item.response.order_id.clone();

        let redirection_data = item
            .response
            .checkout_url
            .as_ref()
            .map(|url| Box::new(RedirectForm::Uri { uri: url.clone() }));

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                connector_order_id: Some(connector_order_id),
                raw_connector_status: Some(raw_connector_status),
                ..item.router_data.resource_common_data.clone()
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.order_id.clone()),
                redirection_data,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: item.response.checkout_id.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
            }),
            ..item.router_data.clone()
        })
    }
}

// ===== PSYNC (calls GET /orders/{order_id}) =====

#[derive(Debug, Deserialize, Serialize)]
pub struct TamaraPSyncResponse {
    pub order_id: String,
    pub status: TamaraPaymentStatus,
    pub order_reference_id: Option<String>,
}

impl TryFrom<ResponseRouterData<TamaraPSyncResponse, Self>>
    for RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(item: ResponseRouterData<TamaraPSyncResponse, Self>) -> Result<Self, Self::Error> {
        let raw_connector_status = tamara_payment_raw_connector_status(&item.response.status);
        let status = AttemptStatus::from(item.response.status);

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                raw_connector_status: Some(raw_connector_status),
                ..item.router_data.resource_common_data.clone()
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.order_id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: item.response.order_reference_id.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
            }),
            ..item.router_data.clone()
        })
    }
}

// ===== RSYNC =====

#[derive(Debug, Deserialize, Serialize)]
pub struct TamaraRSyncResponse {
    pub order_id: String,
    pub status: TamaraRefundStatus,
    pub order_reference_id: Option<String>,
}

impl TryFrom<ResponseRouterData<TamaraRSyncResponse, Self>>
    for RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(item: ResponseRouterData<TamaraRSyncResponse, Self>) -> Result<Self, Self::Error> {
        let raw_connector_status = tamara_refund_raw_connector_status(&item.response.status);
        let refund_status = RefundStatus::from(item.response.status);

        Ok(Self {
            resource_common_data: RefundFlowData {
                raw_connector_status: Some(raw_connector_status),
                ..item.router_data.resource_common_data.clone()
            },
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.order_id.clone(),
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            ..item.router_data.clone()
        })
    }
}

// ===== CAPTURE =====

#[derive(Debug, Serialize)]
pub struct TamaraCaptureRequest {
    pub order_id: String,
    pub total_amount: TamaraAmount,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TamaraRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    > for TamaraCaptureRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: TamaraRouterData<
            RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let order_id = item
            .router_data
            .request
            .get_connector_transaction_id()
            .change_context(errors::IntegrationError::MissingConnectorTransactionID {
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Missing connector transaction ID (order_id)".to_string(),
                    ),
                    ..Default::default()
                },
            })?;
        let converter = FloatMajorUnitForConnector;
        let amount = converter
            .convert(
                item.router_data.request.amount_to_capture.amount,
                item.router_data.request.currency,
            )
            .change_context(errors::IntegrationError::RequestEncodingFailed {
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Failed to convert capture amount from minor to major units".to_string(),
                    ),
                    ..Default::default()
                },
            })?;
        Ok(Self {
            order_id,
            total_amount: TamaraAmount {
                amount,
                currency: item.router_data.request.currency,
            },
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TamaraAmount {
    pub amount: FloatMajorUnit,
    pub currency: Currency,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TamaraCaptureResponse {
    pub order_id: String,
    pub capture_id: Option<String>,
    pub status: TamaraPaymentStatus,
}

impl TryFrom<ResponseRouterData<TamaraCaptureResponse, Self>>
    for RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TamaraCaptureResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let raw_connector_status = tamara_payment_raw_connector_status(&item.response.status);
        let status = AttemptStatus::from(item.response.status);

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                raw_connector_status: Some(raw_connector_status),
                ..item.router_data.resource_common_data.clone()
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.order_id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: item.response.capture_id.clone(),
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
            }),
            ..item.router_data.clone()
        })
    }
}

// ===== VOID =====

#[derive(Debug, Serialize)]
pub struct TamaraVoidRequest {
    pub total_amount: TamaraAmount,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TamaraRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    > for TamaraVoidRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: TamaraRouterData<
            RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let amount = router_data
            .request
            .amount
            .as_ref()
            .ok_or(error_stack::report!(
                errors::IntegrationError::MissingRequiredField {
                    field_name: "amount",
                    context: errors::IntegrationErrorContext {
                        additional_context: Some("Missing void amount".to_string()),
                        ..Default::default()
                    }
                }
            ))?;
        let currency = router_data.request.currency.ok_or(error_stack::report!(
            errors::IntegrationError::MissingRequiredField {
                field_name: "currency",
                context: errors::IntegrationErrorContext {
                    additional_context: Some("Missing void currency".to_string()),
                    ..Default::default()
                }
            }
        ))?;
        let converter = FloatMajorUnitForConnector;
        let amount = converter.convert(amount.amount, currency).change_context(
            errors::IntegrationError::RequestEncodingFailed {
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Failed to convert void amount from minor to major units".to_string(),
                    ),
                    ..Default::default()
                },
            },
        )?;
        Ok(Self {
            total_amount: TamaraAmount { amount, currency },
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TamaraVoidResponse {
    pub order_id: String,
    pub status: TamaraPaymentStatus,
}

impl TryFrom<ResponseRouterData<TamaraVoidResponse, Self>>
    for RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(item: ResponseRouterData<TamaraVoidResponse, Self>) -> Result<Self, Self::Error> {
        let raw_connector_status = tamara_payment_raw_connector_status(&item.response.status);
        let status = AttemptStatus::from(item.response.status);

        Ok(Self {
            resource_common_data: PaymentFlowData {
                status,
                raw_connector_status: Some(raw_connector_status),
                ..item.router_data.resource_common_data.clone()
            },
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.order_id.clone()),
                redirection_data: None,
                mandate_reference: None,
                connector_metadata: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                status_code: item.http_code,
                splits: None,
                payment_account_reference: None,
            }),
            ..item.router_data.clone()
        })
    }
}

// ===== REFUND =====

#[derive(Debug, Serialize)]
pub struct TamaraRefundRequest {
    pub total_amount: TamaraAmount,
    pub comment: String,
    pub merchant_refund_id: String,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TamaraRouterData<RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>, T>,
    > for TamaraRefundRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: TamaraRouterData<
            RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let comment = router_data
            .request
            .reason
            .clone()
            .ok_or(error_stack::report!(
                errors::IntegrationError::MissingRequiredField {
                    field_name: "reason",
                    context: errors::IntegrationErrorContext {
                        additional_context: Some("Missing refund reason/comment".to_string()),
                        ..Default::default()
                    }
                }
            ))?;
        let converter = FloatMajorUnitForConnector;
        let amount = converter
            .convert(
                item.router_data.request.refund_amount.amount,
                item.router_data.request.currency,
            )
            .change_context(errors::IntegrationError::RequestEncodingFailed {
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Failed to convert refund amount from minor to major units".to_string(),
                    ),
                    ..Default::default()
                },
            })?;
        Ok(Self {
            total_amount: TamaraAmount {
                amount,
                currency: item.router_data.request.currency,
            },
            merchant_refund_id: item.router_data.request.refund_id.clone(),
            comment,
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TamaraRefundResponse {
    pub order_id: String,
    pub refund_id: String,
    pub status: TamaraRefundStatus,
}

impl TryFrom<ResponseRouterData<TamaraRefundResponse, Self>>
    for RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(item: ResponseRouterData<TamaraRefundResponse, Self>) -> Result<Self, Self::Error> {
        let raw_connector_status = tamara_refund_raw_connector_status(&item.response.status);
        let refund_status = RefundStatus::from(item.response.status);

        Ok(Self {
            resource_common_data: RefundFlowData {
                raw_connector_status: Some(raw_connector_status),
                ..item.router_data.resource_common_data.clone()
            },
            response: Ok(RefundsResponseData {
                connector_refund_id: item.response.refund_id,
                refund_status,
                status_code: item.http_code,
                acquirer_reference_number: None,
            }),
            ..item.router_data.clone()
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TamaraWebhookEvent {
    OrderApproved,
    OrderAuthorised,
    OrderCanceled,
    OrderCaptured,
    OrderRefunded,
    OrderUpdated,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TamaraWebhookData {
    pub capture_id: Option<String>,
    pub refund_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TamaraWebhookEventType {
    pub order_id: String,
    pub order_reference_id: Option<String>,
    pub order_number: Option<String>,
    pub event_type: TamaraWebhookEvent,
    pub data: Option<TamaraWebhookData>,
}

impl From<TamaraWebhookEvent> for interfaces::webhooks::IncomingWebhookEvent {
    fn from(event: TamaraWebhookEvent) -> Self {
        match event {
            TamaraWebhookEvent::OrderApproved | TamaraWebhookEvent::OrderAuthorised => {
                Self::PaymentIntentAuthorizationSuccess
            }
            TamaraWebhookEvent::OrderCanceled => Self::PaymentIntentCancelled,
            TamaraWebhookEvent::OrderCaptured => Self::PaymentIntentCaptureSuccess,
            TamaraWebhookEvent::OrderRefunded => Self::RefundSuccess,
            TamaraWebhookEvent::OrderUpdated => Self::PaymentIntentProcessing,
            TamaraWebhookEvent::Unknown => Self::EventNotSupported,
        }
    }
}

impl From<TamaraWebhookEventType> for interfaces::webhooks::IncomingWebhookEvent {
    fn from(event: TamaraWebhookEventType) -> Self {
        Self::from(event.event_type)
    }
}

impl From<TamaraWebhookEvent> for AttemptStatus {
    fn from(event: TamaraWebhookEvent) -> Self {
        match event {
            TamaraWebhookEvent::OrderApproved | TamaraWebhookEvent::OrderAuthorised => {
                Self::Authorized
            }
            TamaraWebhookEvent::OrderCaptured | TamaraWebhookEvent::OrderRefunded => Self::Charged,
            TamaraWebhookEvent::OrderCanceled => Self::Voided,
            TamaraWebhookEvent::OrderUpdated | TamaraWebhookEvent::Unknown => Self::Pending,
        }
    }
}

impl From<TamaraWebhookEvent> for RefundStatus {
    fn from(event: TamaraWebhookEvent) -> Self {
        match event {
            TamaraWebhookEvent::OrderRefunded => Self::Success,
            _ => Self::Pending,
        }
    }
}

impl From<TamaraWebhookEvent> for EventType {
    fn from(event: TamaraWebhookEvent) -> Self {
        match event {
            TamaraWebhookEvent::OrderApproved | TamaraWebhookEvent::OrderAuthorised => {
                Self::PaymentIntentAuthorizationSuccess
            }
            TamaraWebhookEvent::OrderCanceled => Self::PaymentIntentCancelled,
            TamaraWebhookEvent::OrderCaptured => Self::PaymentIntentCaptureSuccess,
            TamaraWebhookEvent::OrderRefunded => Self::RefundSuccess,
            TamaraWebhookEvent::OrderUpdated => Self::PaymentIntentProcessing,
            TamaraWebhookEvent::Unknown => Self::IncomingWebhookEventUnspecified,
        }
    }
}

// ===== ELIGIBILITY =====

#[derive(Debug, Serialize)]
pub struct TamaraEligibilityRequest {
    pub order: TamaraEligibilityOrder,
    pub customer: TamaraEligibilityCustomer,
}

#[derive(Debug, Serialize)]
pub struct TamaraEligibilityOrder {
    pub amount: FloatMajorUnit,
    pub currency: Currency,
}

#[derive(Debug, Serialize)]
pub struct TamaraEligibilityCustomer {
    pub phone_number: Secret<String>,
    pub email: Email,
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        TamaraRouterData<
            RouterDataV2<
                PaymentMethodEligibility,
                PaymentFlowData,
                PaymentMethodEligibilityData,
                PaymentMethodEligibilityResponse,
            >,
            T,
        >,
    > for TamaraEligibilityRequest
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: TamaraRouterData<
            RouterDataV2<
                PaymentMethodEligibility,
                PaymentFlowData,
                PaymentMethodEligibilityData,
                PaymentMethodEligibilityResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let data = &item.router_data.request;
        let customer = data.customer.as_ref().ok_or(error_stack::report!(
            errors::IntegrationError::MissingRequiredField {
                field_name: "customer",
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Customer details are required for Tamara eligibility check".to_string()
                    ),
                    ..Default::default()
                },
            }
        ))?;
        let email = customer.get_email()?;
        let phone_number = customer.get_phone_number()?;
        let converter = FloatMajorUnitForConnector;
        let amount = converter
            .convert(data.amount.amount, data.amount.currency)
            .change_context(errors::IntegrationError::RequestEncodingFailed {
                context: errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Failed to convert eligibility amount from minor to major units"
                            .to_string(),
                    ),
                    ..Default::default()
                },
            })?;
        Ok(Self {
            order: TamaraEligibilityOrder {
                amount,
                currency: data.amount.currency,
            },
            customer: TamaraEligibilityCustomer {
                phone_number,
                email,
            },
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TamaraEligibilityResponse {
    #[serde(alias = "is_eligible")]
    pub eligible: Option<bool>,
    #[serde(alias = "description")]
    pub message: Option<String>,
}

impl TryFrom<ResponseRouterData<TamaraEligibilityResponse, Self>>
    for RouterDataV2<
        PaymentMethodEligibility,
        PaymentFlowData,
        PaymentMethodEligibilityData,
        PaymentMethodEligibilityResponse,
    >
{
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(
        item: ResponseRouterData<TamaraEligibilityResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let eligibility = if item.response.eligible.unwrap_or(false) {
            EligibilityStatus::Eligible
        } else {
            EligibilityStatus::Ineligible
        };
        // PM-agnostic verdict fanned across every requested payment method.
        let results = item
            .router_data
            .request
            .payment_method_types
            .iter()
            .map(|payment_method_type| PMEligibility {
                payment_method_type: *payment_method_type,
                eligibility,
                error_info: None,
                payment_method_details: None,
            })
            .collect();
        Ok(Self {
            response: Ok(PaymentMethodEligibilityResponse {
                results,
                status_code: u32::from(item.http_code),
            }),
            ..item.router_data.clone()
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TamaraWebhookResourceObject {
    pub order_id: String,
    pub order_reference_id: Option<String>,
    pub order_number: Option<String>,
    pub event_type: TamaraWebhookEvent,
}
