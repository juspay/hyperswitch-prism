use common_enums::Currency;
use common_utils::{date_time, pii::IpAddress, SecretSerdeValue};
use error_stack::ResultExt;
use hyperswitch_masking::Secret;
use time::PrimitiveDateTime;

use crate::utils::{missing_field_err, Error};

#[derive(Default, Eq, PartialEq, Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct CustomerAcceptance {
    /// Type of acceptance provided by the
    pub acceptance_type: AcceptanceType,
    /// Specifying when the customer acceptance was provided
    #[serde(with = "common_utils::custom_serde::iso8601::option")]
    pub accepted_at: Option<PrimitiveDateTime>,
    /// Information required for online mandate generation
    pub online: Option<OnlineMandate>,
}

#[derive(Default, Eq, PartialEq, Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct OnlineMandate {
    /// Ip address of the customer machine from which the mandate was created
    #[serde(skip_deserializing)]
    pub ip_address: Option<Secret<String, IpAddress>>,
    /// The user-agent of the customer's browser
    pub user_agent: String,
}

#[derive(Default, Eq, PartialEq, Debug, Clone, serde::Serialize)]
pub struct MandateData {
    /// A way to update the mandate's payment method details
    pub update_mandate_id: Option<String>,
    /// A consent from the customer to store the payment method
    pub customer_acceptance: Option<CustomerAcceptance>,
    /// A way to select the type of mandate used
    pub mandate_type: Option<MandateDataType>,
}

#[derive(Default, Debug, PartialEq, Eq, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AcceptanceType {
    Online,
    #[default]
    Offline,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MandateDataType {
    SingleUse(MandateAmountData),
    MultiUse(Option<MandateAmountData>),
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct MandateAmountData {
    pub amount: common_utils::types::MinorUnit,
    pub currency: Currency,
    pub start_date: Option<PrimitiveDateTime>,
    pub end_date: Option<PrimitiveDateTime>,
    pub metadata: Option<SecretSerdeValue>,
    pub amount_type: Option<String>, // Amount type for variable mandates (exact, max, variable)
    pub frequency: Option<String>,   // Frequency for recurring mandates (daily, weekly, monthly)
}

impl MandateAmountData {
    pub fn get_end_date(&self, format: date_time::DateFormat) -> Result<String, Error> {
        let date = self.end_date.ok_or_else(missing_field_err(
            "mandate_data.mandate_type.{multi_use|single_use}.end_date",
        ))?;
        date_time::format_date(date, format).change_context(
            crate::errors::IntegrationError::InvalidDataFormat {
                field_name: "mandate_amount_data.end_date",
                context: crate::errors::IntegrationErrorContext {
                    additional_context: Some(
                        "Failed to format end date with specified format".to_owned(),
                    ),
                    ..Default::default()
                },
            },
        )
    }
    pub fn get_metadata(&self) -> Result<SecretSerdeValue, Error> {
        self.metadata.clone().ok_or_else(missing_field_err(
            "mandate_data.mandate_type.{multi_use|single_use}.metadata",
        ))
    }
}
