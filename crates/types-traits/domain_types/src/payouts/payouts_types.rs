use super::payout_method_data::{Bank, PayoutMethodData};
use crate::{
    connector_types::{
        ConnectorResponseHeaders, RawConnectorRequestResponse,
        ServerAuthenticationTokenResponseData,
    },
    errors::IntegrationError,
    payment_address::Address,
    types::Connectors,
    utils::{missing_field_err, Error},
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, PeekInterface, Secret};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct PayoutFlowData {
    pub merchant_id: common_utils::id_type::MerchantId,
    pub payout_id: String,
    pub connectors: Arc<Connectors>,
    pub connector_request_reference_id: String,
    pub raw_connector_response: Option<Secret<String>>,
    pub typed_connector_response: Option<String>,
    pub connector_response_headers: Option<http::HeaderMap>,
    pub raw_connector_request: Option<Secret<String>>,
    pub typed_connector_request: Option<String>,
    pub access_token: Option<ServerAuthenticationTokenResponseData>,
    pub test_mode: Option<bool>,
    pub description: Option<String>,
    pub merchant_request_id: Option<String>,
}

impl RawConnectorRequestResponse for PayoutFlowData {
    fn set_raw_connector_response(&mut self, response: Option<Secret<String>>) {
        self.raw_connector_response = response;
    }

    fn get_raw_connector_response(&self) -> Option<Secret<String>> {
        self.raw_connector_response.clone()
    }

    fn get_raw_connector_request(&self) -> Option<Secret<String>> {
        self.raw_connector_request.clone()
    }

    fn set_raw_connector_request(&mut self, request: Option<Secret<String>>) {
        self.raw_connector_request = request;
    }

    fn set_typed_connector_response(&mut self, response: Option<String>) {
        self.typed_connector_response = response;
    }

    fn get_typed_connector_response(&self) -> Option<String> {
        self.typed_connector_response.clone()
    }

    fn set_typed_connector_request(&mut self, request: Option<String>) {
        self.typed_connector_request = request;
    }

    fn get_typed_connector_request(&self) -> Option<String> {
        self.typed_connector_request.clone()
    }
}

impl ConnectorResponseHeaders for PayoutFlowData {
    fn set_connector_response_headers(&mut self, headers: Option<http::HeaderMap>) {
        self.connector_response_headers = headers;
    }

    fn get_connector_response_headers(&self) -> Option<&http::HeaderMap> {
        self.connector_response_headers.as_ref()
    }
}

impl PayoutFlowData {
    pub fn get_access_token(&self) -> Result<String, Error> {
        self.access_token
            .as_ref()
            .map(|token_data| token_data.access_token.clone().expose())
            .ok_or_else(missing_field_err("access_token"))
    }

    pub fn get_access_token_data(&self) -> Result<ServerAuthenticationTokenResponseData, Error> {
        self.access_token
            .clone()
            .ok_or_else(missing_field_err("access_token"))
    }

    pub fn set_access_token(
        mut self,
        access_token: Option<ServerAuthenticationTokenResponseData>,
    ) -> Self {
        self.access_token = access_token;
        self
    }
}

#[derive(Debug, Clone)]
pub struct PayoutCreateRequest {
    pub merchant_payout_id: Option<String>,
    pub connector_quote_id: Option<String>,
    pub connector_payout_id: Option<String>,
    pub amount: common_utils::types::MinorUnit,
    pub source_currency: common_enums::Currency,
    pub destination_currency: common_enums::Currency,
    pub priority: Option<common_enums::PayoutPriority>,
    pub connector_payout_method_id: Option<String>,
    pub webhook_url: Option<String>,
    pub payout_method_data: Option<PayoutMethodData>,
    pub source_bank_data: Option<Bank>,
    pub customer: Option<PayoutCustomer>,
}

#[derive(Debug, Clone)]
pub struct PayoutCreateResponse {
    pub merchant_payout_id: Option<String>,
    pub payout_status: common_enums::PayoutStatus,
    pub connector_payout_id: Option<String>,
    pub status_code: u16,
}

#[derive(Debug, Clone)]
pub struct PayoutAddress {
    pub shipping_address: Option<Address>,
    pub billing_address: Option<Address>,
}

#[derive(Debug, Clone)]
pub struct PayoutTransferRequest {
    pub merchant_payout_id: Option<String>,
    pub connector_quote_id: Option<String>,
    pub connector_payout_id: Option<String>,
    pub amount: common_utils::types::MinorUnit,
    pub source_currency: common_enums::Currency,
    pub destination_currency: common_enums::Currency,
    pub priority: Option<common_enums::PayoutPriority>,
    pub connector_payout_method_id: Option<String>,
    pub webhook_url: Option<String>,
    pub payout_method_data: Option<PayoutMethodData>,
    pub address: Option<PayoutAddress>,
    pub source_bank_data: Option<Bank>,
    pub customer: Option<PayoutCustomer>,
    pub connector_eligibility_reference_id: Option<String>,
    pub payout_connector_metadata: Option<common_utils::pii::SecretSerdeValue>,
}

impl PayoutTransferRequest {
    pub fn get_billing(&self) -> Result<&Address, Error> {
        self.address
            .as_ref()
            .and_then(|a| a.billing_address.as_ref())
            .ok_or_else(missing_field_err("address.billing_address"))
    }

    pub fn get_billing_address(&self) -> Result<&crate::payment_address::AddressDetails, Error> {
        self.get_billing()?
            .address
            .as_ref()
            .ok_or_else(missing_field_err("address.billing_address.address"))
    }

    pub fn get_billing_first_name(&self) -> Result<Secret<String>, Error> {
        self.get_billing_address()?
            .first_name
            .clone()
            .ok_or_else(missing_field_err(
                "address.billing_address.address.first_name",
            ))
    }

    pub fn get_billing_last_name(&self) -> Result<Secret<String>, Error> {
        self.get_billing_address()?
            .last_name
            .clone()
            .ok_or_else(missing_field_err(
                "address.billing_address.address.last_name",
            ))
    }

    pub fn get_customer_id(
        &self,
    ) -> Result<common_utils::id_type::CustomerId, error_stack::Report<IntegrationError>> {
        self.customer
            .as_ref()
            .and_then(|c| c.merchant_customer_id.clone())
            .ok_or_else(|| {
                error_stack::report!(IntegrationError::MissingRequiredField {
                    field_name: "customer.merchant_customer_id",
                    context: crate::errors::IntegrationErrorContext {
                        additional_context: Some(
                            "Customer merchant_customer_id is required for Loonio payouts"
                                .to_string()
                        ),
                        suggested_action: Some(
                            "Provide a valid merchant_customer_id in the customer object"
                                .to_string()
                        ),
                        doc_url: None,
                    },
                })
            })
            .and_then(|id| {
                common_utils::id_type::CustomerId::try_from(std::borrow::Cow::from(id))
                    .change_context(IntegrationError::InvalidDataFormat {
                        field_name: "customer.merchant_customer_id",
                        context: crate::errors::IntegrationErrorContext {
                            additional_context: Some(
                                "Failed to parse merchant_customer_id as a valid CustomerId"
                                    .to_string(),
                            ),
                            suggested_action: Some(
                                "Ensure the merchant_customer_id is a valid non-empty string"
                                    .to_string(),
                            ),
                            doc_url: None,
                        },
                    })
            })
    }

    pub fn get_optional_customer_id(
        &self,
    ) -> Result<Option<common_utils::id_type::CustomerId>, error_stack::Report<IntegrationError>>
    {
        match self
            .customer
            .as_ref()
            .and_then(|c| c.merchant_customer_id.clone())
        {
            Some(id) => {
                let customer_id =
                    common_utils::id_type::CustomerId::try_from(std::borrow::Cow::from(id))
                        .change_context(IntegrationError::InvalidDataFormat {
                            field_name: "customer.merchant_customer_id",
                            context: crate::errors::IntegrationErrorContext {
                                additional_context: Some(
                                    "Failed to parse merchant_customer_id as a valid CustomerId"
                                        .to_string(),
                                ),
                                suggested_action: Some(
                                    "Ensure the merchant_customer_id is a valid non-empty string"
                                        .to_string(),
                                ),
                                doc_url: None,
                            },
                        })?;
                Ok(Some(customer_id))
            }
            None => Ok(None),
        }
    }

    pub fn get_optional_billing_phone(&self) -> Option<Secret<String>> {
        self.address
            .as_ref()
            .and_then(|a| a.billing_address.as_ref())
            .and_then(|b| b.phone.as_ref())
            .and_then(|p| p.number.clone())
    }

    pub fn get_optional_billing_line1(&self) -> Option<Secret<String>> {
        self.address
            .as_ref()
            .and_then(|a| a.billing_address.as_ref())
            .and_then(|b| b.address.as_ref())
            .and_then(|addr| addr.line1.clone())
    }

    pub fn get_optional_billing_city(&self) -> Option<String> {
        self.address
            .as_ref()
            .and_then(|a| a.billing_address.as_ref())
            .and_then(|b| b.address.as_ref())
            .and_then(|addr| addr.city.as_ref())
            .map(|c| c.peek().clone())
    }

    pub fn get_optional_billing_state(&self) -> Option<Secret<String>> {
        self.address
            .as_ref()
            .and_then(|a| a.billing_address.as_ref())
            .and_then(|b| b.address.as_ref())
            .and_then(|addr| addr.state.clone())
    }

    pub fn get_optional_billing_zip(&self) -> Option<Secret<String>> {
        self.address
            .as_ref()
            .and_then(|a| a.billing_address.as_ref())
            .and_then(|b| b.address.as_ref())
            .and_then(|addr| addr.zip.clone())
    }

    pub fn get_optional_billing_country(&self) -> Option<common_enums::CountryAlpha2> {
        self.address
            .as_ref()
            .and_then(|a| a.billing_address.as_ref())
            .and_then(|b| b.address.as_ref())
            .and_then(|addr| addr.country)
    }
}

#[derive(Debug, Clone)]
pub struct PayoutCustomer {
    pub name: Option<String>,
    pub email: Option<common_utils::pii::Email>,
    pub merchant_customer_id: Option<String>,
    pub connector_customer_id: Option<String>,
    pub phone_number: Option<Secret<String>>,
    pub phone_country_code: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PayoutTransferResponse {
    pub merchant_payout_id: Option<String>,
    pub payout_status: common_enums::PayoutStatus,
    pub connector_payout_id: Option<String>,
    pub status_code: u16,
}

#[derive(Debug, Clone)]
pub struct PayoutGetRequest {
    pub merchant_payout_id: Option<String>,
    pub connector_payout_id: Option<String>,
    pub customer: Option<PayoutCustomer>,
    /// Source (debtor) bank data — required by connectors (e.g. Deutsche Bank)
    /// that need the debtor account to perform a status enquiry.
    pub source_bank_data: Option<Bank>,
    pub payout_method_type: Option<common_enums::PaymentMethodType>,
}

#[derive(Debug, Clone)]
pub struct PayoutGetResponse {
    pub merchant_payout_id: Option<String>,
    pub payout_status: common_enums::PayoutStatus,
    pub connector_payout_id: Option<String>,
    pub status_code: u16,
}

#[derive(Debug, Clone)]
pub struct PayoutStageRequest {
    pub merchant_quote_id: Option<String>,
    pub amount: common_utils::types::MinorUnit,
    pub source_currency: common_enums::Currency,
    pub destination_currency: common_enums::Currency,
}

#[derive(Debug, Clone)]
pub struct PayoutStageResponse {
    pub merchant_payout_id: Option<String>,
    pub payout_status: common_enums::PayoutStatus,
    pub connector_payout_id: Option<String>,
    pub status_code: u16,
}

#[derive(Debug, Clone)]
pub struct PayoutVoidRequest {
    pub merchant_payout_id: Option<String>,
    pub connector_payout_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PayoutVoidResponse {
    pub merchant_payout_id: Option<String>,
    pub payout_status: common_enums::PayoutStatus,
    pub connector_payout_id: Option<String>,
    pub status_code: u16,
}

#[derive(Debug, Clone)]
pub struct PayoutCreateLinkRequest {
    pub merchant_payout_id: Option<String>,
    pub connector_quote_id: Option<String>,
    pub connector_payout_id: Option<String>,
    pub amount: common_utils::types::MinorUnit,
    pub source_currency: common_enums::Currency,
    pub destination_currency: common_enums::Currency,
    pub priority: Option<common_enums::PayoutPriority>,
    pub connector_payout_method_id: Option<String>,
    pub webhook_url: Option<String>,
    pub payout_method_data: Option<PayoutMethodData>,
}

#[derive(Debug, Clone)]
pub struct PayoutCreateLinkResponse {
    pub merchant_payout_id: Option<String>,
    pub payout_status: common_enums::PayoutStatus,
    pub connector_payout_id: Option<String>,
    pub status_code: u16,
}

#[derive(Debug, Clone)]
pub struct PayoutCreateRecipientRequest {
    pub merchant_payout_id: Option<String>,
    pub amount: common_utils::types::MinorUnit,
    pub source_currency: common_enums::Currency,
    pub payout_method_data: Option<PayoutMethodData>,
    pub recipient_type: common_enums::PayoutRecipientType,

    pub address: Option<PayoutAddress>,

    pub customer: Option<PayoutCustomer>,

    pub vendor_account_details: Option<PayoutVendorAccountDetails>,
}

/// Day, month and year parts of a date of birth.
pub type DateOfBirthParts = (Secret<String>, Secret<String>, Secret<String>);

#[derive(Debug, Clone, Default)]
pub struct PayoutVendorAccountDetails {
    pub vendor_details: Option<PayoutVendorDetails>,
    pub individual_details: Option<PayoutIndividualDetails>,
}
#[derive(Debug, Clone, Default)]
pub struct PayoutVendorDetails {
    pub account_type: Option<String>,
    pub business_type: Option<String>,
    pub merchant_category_code: Option<String>,
    pub business_url: Option<Secret<String>>,
    pub business_name: Option<Secret<String>>,
    pub statement_descriptor: Option<Secret<String>>,
    pub owners_provided: Option<bool>,
    pub card_payments_enabled: Option<bool>,
    pub transfers_enabled: Option<bool>,
}

#[derive(Debug, Clone, Default)]
pub struct PayoutIndividualDetails {
    pub first_name: Option<Secret<String>>,
    pub last_name: Option<Secret<String>>,
    pub phone: Option<Secret<String>>,
    pub ssn_last_4: Option<Secret<String>>,
    pub id_number: Option<Secret<String>>,
    pub date_of_birth: Option<Secret<String>>,
    pub tos_acceptance_date: Option<i64>,
    pub tos_acceptance_ip: Option<Secret<String>>,
    pub external_account_account_holder_type: Option<String>,
}

pub type IdNumberOrSsnLast4 = (Option<Secret<String>>, Option<Secret<String>>);

impl PayoutCreateRecipientRequest {
    /// Navigate to the billing `AddressDetails`; per-field accessors live on
    /// [`crate::payment_address::AddressDetails`] and are reused from there.
    pub fn get_optional_billing_address(&self) -> Option<&crate::payment_address::AddressDetails> {
        self.address
            .as_ref()
            .and_then(|a| a.billing_address.as_ref())
            .and_then(|b| b.address.as_ref())
    }

    fn vendor_details(&self) -> Option<&PayoutVendorDetails> {
        self.vendor_account_details
            .as_ref()
            .and_then(|v| v.vendor_details.as_ref())
    }

    fn individual_details(&self) -> Option<&PayoutIndividualDetails> {
        self.vendor_account_details
            .as_ref()
            .and_then(|v| v.individual_details.as_ref())
    }

    pub fn get_phone(&self) -> Option<Secret<String>> {
        self.individual_details().and_then(|i| i.phone.clone())
    }

    pub fn get_first_name(&self) -> Option<Secret<String>> {
        self.individual_details().and_then(|i| i.first_name.clone())
    }

    pub fn get_last_name(&self) -> Option<Secret<String>> {
        self.individual_details().and_then(|i| i.last_name.clone())
    }

    /// Split `date_of_birth` (ISO 8601, `yyyy-MM-dd`) into day, month and year.
    pub fn get_date_of_birth_parts(&self) -> Result<Option<DateOfBirthParts>, Error> {
        let Some(date_of_birth) = self
            .individual_details()
            .and_then(|i| i.date_of_birth.clone())
        else {
            return Ok(None);
        };
        let mut parts = date_of_birth.peek().split('-');
        match (parts.next(), parts.next(), parts.next()) {
            (Some(year), Some(month), Some(day)) => Ok(Some((
                Secret::new(day.to_string()),
                Secret::new(month.to_string()),
                Secret::new(year.to_string()),
            ))),
            _ => Err(error_stack::report!(IntegrationError::InvalidDataFormat {
                field_name: "date_of_birth",
                context: crate::errors::IntegrationErrorContext {
                    additional_context: Some(
                        "date_of_birth must be an ISO 8601 date, yyyy-MM-dd".to_string(),
                    ),
                    suggested_action: Some("Send the date of birth as yyyy-MM-dd".to_string()),
                    doc_url: None,
                },
            })),
        }
    }

    pub fn get_date_of_birth(&self) -> Result<Secret<String>, Error> {
        self.individual_details()
            .and_then(|i| i.date_of_birth.clone())
            .ok_or_else(missing_field_err("date_of_birth"))
    }

    pub fn get_account_type(&self) -> Option<String> {
        self.vendor_details().and_then(|v| v.account_type.clone())
    }

    pub fn get_business_type(&self) -> Option<String> {
        self.vendor_details().and_then(|v| v.business_type.clone())
    }

    pub fn get_business_url(&self) -> Option<Secret<String>> {
        self.vendor_details().and_then(|v| v.business_url.clone())
    }

    pub fn get_business_name(&self) -> Option<Secret<String>> {
        self.vendor_details().and_then(|v| v.business_name.clone())
    }

    pub fn get_statement_descriptor(&self) -> Option<Secret<String>> {
        self.vendor_details()
            .and_then(|v| v.statement_descriptor.clone())
    }

    pub fn get_owners_provided(&self) -> Option<bool> {
        self.vendor_details().and_then(|v| v.owners_provided)
    }

    pub fn get_card_payments_enabled(&self) -> Option<bool> {
        self.vendor_details().and_then(|v| v.card_payments_enabled)
    }

    pub fn get_transfers_enabled(&self) -> Option<bool> {
        self.vendor_details().and_then(|v| v.transfers_enabled)
    }

    pub fn get_tos_acceptance_ip(&self) -> Option<Secret<String>> {
        self.individual_details()
            .and_then(|i| i.tos_acceptance_ip.clone())
    }

    pub fn get_tos_acceptance_date(&self) -> Option<i64> {
        self.individual_details()
            .and_then(|i| i.tos_acceptance_date)
    }

    pub fn get_merchant_category_code_i32(&self) -> Result<Option<i32>, Error> {
        let Some(raw) = self
            .vendor_details()
            .and_then(|v| v.merchant_category_code.as_deref())
        else {
            return Ok(None);
        };
        raw.parse::<i32>().map(Some).map_err(|_| {
            error_stack::report!(IntegrationError::InvalidDataFormat {
                field_name: "merchant_category_code",
                context: crate::errors::IntegrationErrorContext {
                    additional_context: Some(
                        "merchant_category_code must be a 4-digit numeric MCC".to_string(),
                    ),
                    suggested_action: Some(
                        "Send the merchant category code as digits only, for example 5734"
                            .to_string(),
                    ),
                    doc_url: None,
                },
            })
        })
    }

    pub fn get_id_number_or_ssn_last_4(&self) -> IdNumberOrSsnLast4 {
        let individual = self.individual_details();
        match individual.and_then(|i| i.id_number.clone()) {
            Some(id) => (Some(id), None),
            None => (None, individual.and_then(|i| i.ssn_last_4.clone())),
        }
    }

    pub fn get_email_from_customer_or_billing(&self) -> Option<common_utils::pii::Email> {
        self.customer
            .as_ref()
            .and_then(|c| c.email.clone())
            .or_else(|| {
                self.address
                    .as_ref()
                    .and_then(|a| a.billing_address.as_ref())
                    .and_then(|b| b.email.clone())
            })
    }

    pub fn is_company(&self) -> bool {
        matches!(
            self.recipient_type,
            common_enums::PayoutRecipientType::Company
                | common_enums::PayoutRecipientType::NonProfit
                | common_enums::PayoutRecipientType::PublicSector
                | common_enums::PayoutRecipientType::Business
        )
    }
}

impl PayoutEnrollDisburseAccountRequest {
    pub fn get_payout_method_data(&self) -> Result<&PayoutMethodData, Error> {
        self.payout_method_data
            .as_ref()
            .ok_or_else(missing_field_err("payout_method_data"))
    }

    pub fn get_customer_name(&self) -> Option<Secret<String>> {
        self.customer
            .as_ref()
            .and_then(|c| c.name.clone())
            .map(Secret::new)
    }

    pub fn get_external_account_account_holder_type(&self) -> Result<String, Error> {
        self.vendor_account_details
            .as_ref()
            .and_then(|v| v.individual_details.as_ref())
            .and_then(|i| i.external_account_account_holder_type.clone())
            .ok_or_else(missing_field_err("external_account_account_holder_type"))
    }
}

#[derive(Debug, Clone)]
pub struct PayoutCreateRecipientResponse {
    pub merchant_payout_id: Option<String>,
    pub payout_status: common_enums::PayoutStatus,
    pub connector_payout_id: Option<String>,
    pub status_code: u16,
    pub payout_connector_metadata: Option<common_utils::pii::SecretSerdeValue>,
}

#[derive(Debug, Clone)]
pub struct PayoutEnrollDisburseAccountRequest {
    pub merchant_payout_id: Option<String>,
    pub connector_payout_id: Option<String>,
    pub amount: common_utils::types::MinorUnit,
    pub source_currency: common_enums::Currency,
    /// Currency in which the payout will be received. Optional because callers
    /// may only send the amount currency.
    pub destination_currency: Option<common_enums::Currency>,
    pub payout_method_data: Option<PayoutMethodData>,

    pub customer: Option<PayoutCustomer>,

    pub vendor_account_details: Option<PayoutVendorAccountDetails>,
}

#[derive(Debug, Clone)]
pub struct PayoutEnrollDisburseAccountResponse {
    pub merchant_payout_id: Option<String>,
    pub payout_status: common_enums::PayoutStatus,
    pub connector_payout_id: Option<String>,
    pub status_code: u16,
}

#[derive(Debug, Clone)]
pub struct PayoutEligibilityRequest {
    pub merchant_payout_id: Option<String>,
    pub amount: common_utils::types::Money,
    pub destination_currency: common_enums::Currency,
    pub payout_method_data: Option<PayoutMethodData>,
    pub source_bank_data: Option<Bank>,
    pub customer: Option<PayoutCustomer>,
    pub address: Option<PayoutAddress>,
}

#[derive(Debug, Clone)]
pub struct PayoutEligibilityResponse {
    pub merchant_payout_id: Option<String>,
    pub payout_status: common_enums::PayoutStatus,
    pub connector_payout_id: Option<String>,
    pub payout_eligible: Option<bool>,
    pub status_code: u16,
    pub connector_metadata: Option<common_utils::pii::SecretSerdeValue>,
    pub connector_eligibility_reference_id: Option<String>,
}
