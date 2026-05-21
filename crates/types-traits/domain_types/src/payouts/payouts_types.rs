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

#[derive(Debug, Clone)]
pub struct PayoutFlowData {
    pub merchant_id: common_utils::id_type::MerchantId,
    pub payout_id: String,
    pub connectors: Connectors,
    pub connector_request_reference_id: String,
    pub raw_connector_response: Option<Secret<String>>,
    pub connector_response_headers: Option<http::HeaderMap>,
    pub raw_connector_request: Option<Secret<String>>,
    pub access_token: Option<ServerAuthenticationTokenResponseData>,
    pub test_mode: Option<bool>,
    pub description: Option<String>,
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
}

#[derive(Debug, Clone)]
pub struct PayoutCreateRecipientResponse {
    pub merchant_payout_id: Option<String>,
    pub payout_status: common_enums::PayoutStatus,
    pub connector_payout_id: Option<String>,
    pub status_code: u16,
}

#[derive(Debug, Clone)]
pub struct PayoutEnrollDisburseAccountRequest {
    pub merchant_payout_id: Option<String>,
    pub amount: common_utils::types::MinorUnit,
    pub source_currency: common_enums::Currency,
    pub payout_method_data: Option<PayoutMethodData>,
}

#[derive(Debug, Clone)]
pub struct PayoutEnrollDisburseAccountResponse {
    pub merchant_payout_id: Option<String>,
    pub payout_status: common_enums::PayoutStatus,
    pub connector_payout_id: Option<String>,
    pub status_code: u16,
}
