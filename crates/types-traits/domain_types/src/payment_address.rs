use common_enums::ProductType;
use common_utils::{ext_traits::ConfigExt, Email, MinorUnit};
use hyperswitch_masking::{PeekInterface, Secret, SerializableSecret};

use crate::utils::{
    convert_canada_state_to_code, convert_us_state_to_code, missing_field_err, Error,
};

#[derive(Clone, Default, Debug)]
pub struct PaymentAddress {
    shipping: Option<Address>,
    billing: Option<Address>,
    unified_payment_method_billing: Option<Address>,
    payment_method_billing: Option<Address>,
}

impl PaymentAddress {
    pub fn new(
        shipping: Option<Address>,
        billing: Option<Address>,
        payment_method_billing: Option<Address>,
        should_unify_address: Option<bool>,
    ) -> Self {
        // billing -> .billing, this is the billing details passed in the root of payments request
        // payment_method_billing -> .payment_method_data.billing

        let unified_payment_method_billing = if should_unify_address.unwrap_or(true) {
            // Merge the billing details field from both `payment.billing` and `payment.payment_method_data.billing`
            // The unified payment_method_billing will be used as billing address and passed to the connector module
            // This unification is required in order to provide backwards compatibility
            // so that if `payment.billing` is passed it should be sent to the connector module
            // Unify the billing details with `payment_method_data.billing`
            payment_method_billing
                .as_ref()
                .map(|payment_method_billing| {
                    payment_method_billing
                        .clone()
                        .unify_address(billing.as_ref())
                })
                .or(billing.clone())
        } else {
            payment_method_billing.clone()
        };

        Self {
            shipping,
            billing,
            unified_payment_method_billing,
            payment_method_billing,
        }
    }

    pub fn get_shipping(&self) -> Option<&Address> {
        self.shipping.as_ref()
    }

    pub fn get_payment_method_billing(&self) -> Option<&Address> {
        self.unified_payment_method_billing.as_ref()
    }

    /// Unify the billing details from `payment_method_data.[payment_method_data].billing details`.
    pub fn unify_with_payment_method_data_billing(
        self,
        payment_method_data_billing: Option<Address>,
    ) -> Self {
        // Unify the billing details with `payment_method_data.billing_details`
        let unified_payment_method_billing = payment_method_data_billing
            .map(|payment_method_data_billing| {
                payment_method_data_billing.unify_address(self.get_payment_method_billing())
            })
            .or(self.get_payment_method_billing().cloned());

        Self {
            shipping: self.shipping,
            billing: self.billing,
            unified_payment_method_billing,
            payment_method_billing: self.payment_method_billing,
        }
    }

    pub fn get_request_payment_method_billing(&self) -> Option<&Address> {
        self.payment_method_billing.as_ref()
    }

    pub fn get_payment_billing(&self) -> Option<&Address> {
        self.billing.as_ref()
    }
}

#[derive(Default, Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct Address {
    /// Provide the address details
    pub address: Option<AddressDetails>,

    pub phone: Option<PhoneDetails>,

    pub email: Option<Email>,
}

impl SerializableSecret for Address {}

impl Address {
    /// Unify the address, giving priority to `self` when details are present in both
    pub fn unify_address(self, other: Option<&Self>) -> Self {
        let other_address_details = other.and_then(|address| address.address.as_ref());
        Self {
            address: self
                .address
                .map(|address| address.unify_address_details(other_address_details))
                .or(other_address_details.cloned()),
            email: self.email.or(other.and_then(|other| other.email.clone())),
            phone: self.phone.or(other.and_then(|other| other.phone.clone())),
        }
    }
}

impl Address {
    pub fn get_email(&self) -> Result<Email, Error> {
        self.email.clone().ok_or_else(missing_field_err("email"))
    }

    pub fn get_phone_with_country_code(
        &self,
    ) -> Result<Secret<String>, error_stack::Report<crate::errors::ConnectorError>> {
        self.phone
            .clone()
            .map(|phone_details| phone_details.get_number_with_country_code())
            .transpose()?
            .ok_or_else(missing_field_err("phone"))
    }

    pub fn get_optional_country(&self) -> Option<common_enums::CountryAlpha2> {
        self.address
            .as_ref()
            .and_then(|billing_details| billing_details.country)
    }

    pub fn get_optional_full_name(&self) -> Option<Secret<String>> {
        self.address
            .as_ref()
            .and_then(|billing_address| billing_address.get_optional_full_name())
    }

    pub fn get_optional_first_name(&self) -> Option<Secret<String>> {
        self.address
            .as_ref()
            .and_then(|billing_address| billing_address.get_optional_first_name())
    }

    pub fn get_optional_last_name(&self) -> Option<Secret<String>> {
        self.address
            .as_ref()
            .and_then(|billing_address| billing_address.get_optional_last_name())
    }
}

// used by customers also, could be moved outside
/// Address details
#[derive(Clone, Default, Debug, Eq, serde::Deserialize, serde::Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AddressDetails {
    /// The city, district, suburb, town, or village of the address.
    pub city: Option<Secret<String>>,

    /// The two-letter ISO 3166-1 alpha-2 country code (e.g., US, GB).
    pub country: Option<common_enums::CountryAlpha2>,

    /// The first line of the street address or P.O. Box.
    pub line1: Option<Secret<String>>,

    /// The second line of the street address or P.O. Box (e.g., apartment, suite, unit, or building).
    pub line2: Option<Secret<String>>,

    /// The third line of the street address, if applicable.
    pub line3: Option<Secret<String>>,

    /// The zip/postal code for the address
    pub zip: Option<Secret<String>>,

    /// The address state
    pub state: Option<Secret<String>>,

    /// The first name for the address
    pub first_name: Option<Secret<String>>,

    /// The last name for the address
    pub last_name: Option<Secret<String>>,

    pub origin_zip: Option<Secret<String>>,
}

impl AddressDetails {
    pub fn get_optional_full_name(&self) -> Option<Secret<String>> {
        match (self.first_name.as_ref(), self.last_name.as_ref()) {
            (Some(first_name), Some(last_name)) => Some(Secret::new(format!(
                "{} {}",
                first_name.peek(),
                last_name.peek()
            ))),
            (Some(name), None) | (None, Some(name)) => Some(name.to_owned()),
            _ => None,
        }
    }

    pub fn get_optional_first_name(&self) -> Option<Secret<String>> {
        self.first_name.clone()
    }

    pub fn get_optional_last_name(&self) -> Option<Secret<String>> {
        self.last_name.clone()
    }

    pub fn unify_address_details(self, other: Option<&Self>) -> Self {
        if let Some(other) = other {
            let (first_name, last_name) = if self
                .first_name
                .as_ref()
                .is_some_and(|first_name| !first_name.is_empty_after_trim())
            {
                (self.first_name, self.last_name)
            } else {
                (other.first_name.clone(), other.last_name.clone())
            };

            Self {
                first_name,
                last_name,
                city: self.city.or(other.city.clone()),
                country: self.country.or(other.country),
                line1: self.line1.or(other.line1.clone()),
                line2: self.line2.or(other.line2.clone()),
                line3: self.line3.or(other.line3.clone()),
                zip: self.zip.or(other.zip.clone()),
                state: self.state.or(other.state.clone()),
                origin_zip: self.origin_zip.clone().or(other.origin_zip.clone()),
            }
        } else {
            self
        }
    }
}

impl AddressDetails {
    pub fn get_first_name(&self) -> Result<&Secret<String>, Error> {
        self.first_name
            .as_ref()
            .ok_or_else(missing_field_err("address.first_name"))
    }

    pub fn get_last_name(&self) -> Result<&Secret<String>, Error> {
        self.last_name
            .as_ref()
            .ok_or_else(missing_field_err("address.last_name"))
    }

    pub fn get_full_name(&self) -> Result<Secret<String>, Error> {
        let first_name = self.get_first_name()?.peek().to_owned();
        let last_name = self
            .get_last_name()
            .ok()
            .cloned()
            .unwrap_or(Secret::new("".to_string()));
        let last_name = last_name.peek();
        let full_name = format!("{first_name} {last_name}").trim().to_string();
        Ok(Secret::new(full_name))
    }

    pub fn get_line1(&self) -> Result<&Secret<String>, Error> {
        self.line1
            .as_ref()
            .ok_or_else(missing_field_err("address.line1"))
    }

    pub fn get_city(&self) -> Result<&Secret<String>, Error> {
        self.city
            .as_ref()
            .ok_or_else(missing_field_err("address.city"))
    }

    pub fn get_state(&self) -> Result<&Secret<String>, Error> {
        self.state
            .as_ref()
            .ok_or_else(missing_field_err("address.state"))
    }

    pub fn get_line2(&self) -> Result<&Secret<String>, Error> {
        self.line2
            .as_ref()
            .ok_or_else(missing_field_err("address.line2"))
    }

    pub fn get_zip(&self) -> Result<&Secret<String>, Error> {
        self.zip
            .as_ref()
            .ok_or_else(missing_field_err("address.zip"))
    }

    pub fn get_country(&self) -> Result<&common_enums::CountryAlpha2, Error> {
        self.country
            .as_ref()
            .ok_or_else(missing_field_err("address.country"))
    }

    pub fn get_combined_address_line(&self) -> Result<Secret<String>, Error> {
        Ok(Secret::new(format!(
            "{},{}",
            self.get_line1()?.peek(),
            self.get_line2()?.peek()
        )))
    }

    pub fn get_optional_line2(&self) -> Option<Secret<String>> {
        self.line2.clone()
    }
    pub fn get_optional_country(&self) -> Option<common_enums::CountryAlpha2> {
        self.country
    }

    pub fn to_state_code(&self) -> Result<Secret<String>, Error> {
        let country = self.get_country()?;
        let state = self.get_state()?;
        match country {
            common_enums::CountryAlpha2::US => Ok(Secret::new(
                convert_us_state_to_code(&state.peek().to_string()).to_string(),
            )),
            common_enums::CountryAlpha2::CA => Ok(Secret::new(
                convert_canada_state_to_code(&state.peek().to_string()).to_string(),
            )),
            _ => Ok(state.clone()),
        }
    }

    pub fn to_state_code_as_optional(&self) -> Result<Option<Secret<String>>, Error> {
        self.state
            .as_ref()
            .map(|state| {
                if state.peek().len() == 2 {
                    Ok(state.to_owned())
                } else {
                    self.to_state_code()
                }
            })
            .transpose()
    }
}

#[derive(Debug, Clone, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct PhoneDetails {
    /// The contact number
    pub number: Option<Secret<String>>,
    /// The country code attached to the number
    pub country_code: Option<String>,
}

impl PhoneDetails {
    pub fn get_country_code(&self) -> Result<String, Error> {
        self.country_code
            .clone()
            .ok_or_else(missing_field_err("billing.phone.country_code"))
    }
    pub fn extract_country_code(&self) -> Result<String, Error> {
        self.get_country_code()
            .map(|cc| cc.trim_start_matches('+').to_string())
    }
    pub fn get_number(&self) -> Result<Secret<String>, Error> {
        self.number
            .clone()
            .ok_or_else(missing_field_err("billing.phone.number"))
    }
    pub fn get_number_with_country_code(&self) -> Result<Secret<String>, Error> {
        let number = self.get_number()?;
        let country_code = self.get_country_code()?;
        Ok(Secret::new(format!("{}{}", country_code, number.peek())))
    }
    pub fn get_number_with_hash_country_code(&self) -> Result<Secret<String>, Error> {
        let number = self.get_number()?;
        let country_code = self.get_country_code()?;
        let number_without_plus = country_code.trim_start_matches('+');
        Ok(Secret::new(format!(
            "{}#{}",
            number_without_plus,
            number.peek()
        )))
    }
}

#[derive(Debug, serde::Serialize, PartialEq, Eq, serde::Deserialize)]
pub struct RedirectionResponse {
    pub return_url_with_query_params: String,
}

#[derive(Debug, Default, PartialEq, serde::Deserialize, serde::Serialize, Clone)]
pub struct OrderDetailsWithAmount {
    /// Name of the product that is being purchased
    pub product_name: String,
    /// The quantity of the product to be purchased
    pub quantity: u16,
    /// the amount per quantity of product
    pub amount: MinorUnit,
    /// tax rate applicable to the product
    pub tax_rate: Option<f64>,
    /// total tax amount applicable to the product
    pub total_tax_amount: Option<MinorUnit>,
    // Does the order includes shipping
    pub requires_shipping: Option<bool>,
    /// The image URL of the product
    pub product_img_link: Option<String>,
    /// ID of the product that is being purchased
    pub product_id: Option<String>,
    /// Category of the product that is being purchased
    pub category: Option<String>,
    /// Sub category of the product that is being purchased
    pub sub_category: Option<String>,
    /// Brand of the product that is being purchased
    pub brand: Option<String>,
    /// Description for the item
    pub description: Option<String>,
    /// Unit of measure used for the item quantity.
    pub unit_of_measure: Option<String>,
    /// Type of the product that is being purchased
    pub product_type: Option<ProductType>,
    /// The tax code for the product
    pub product_tax_code: Option<String>,
    /// stock keeping unit of the product
    pub sku: Option<String>,
    /// universal product code of the product
    pub upc: Option<String>,
    /// commodity code of the product
    pub commodity_code: Option<String>,
    /// total amount of the product
    pub total_amount: Option<MinorUnit>,
    /// discount amount on the unit
    pub unit_discount_amount: Option<MinorUnit>,
}

impl SerializableSecret for OrderDetailsWithAmount {}
