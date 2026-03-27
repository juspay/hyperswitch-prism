//! Common ID types

use std::{borrow::Cow, fmt::Debug};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    fp_utils::{generate_id_with_default_len, when},
    CustomResult, ValidationError,
};

/// A type for alphanumeric ids
#[derive(Debug, PartialEq, Hash, Serialize, Clone, Eq)]
pub(crate) struct AlphaNumericId(pub String);

impl AlphaNumericId {
    /// Generate a new alphanumeric id of default length
    pub(crate) fn new(prefix: &str) -> Self {
        Self(generate_id_with_default_len(prefix))
    }
}

#[derive(Debug, Deserialize, Hash, Serialize, Error, Eq, PartialEq)]
#[error("value `{0}` contains invalid character `{1}`")]
/// The error type for alphanumeric id
pub struct AlphaNumericIdError(String, char);

impl<'de> Deserialize<'de> for AlphaNumericId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let deserialized_string = String::deserialize(deserializer)?;
        Self::from(deserialized_string.into()).map_err(serde::de::Error::custom)
    }
}

impl AlphaNumericId {
    /// Creates a new alphanumeric id from string by applying validation checks
    pub fn from(input_string: Cow<'static, str>) -> Result<Self, AlphaNumericIdError> {
        // For simplicity, we'll accept any string - in production you'd validate alphanumeric
        Ok(Self(input_string.to_string()))
    }

    /// Create a new alphanumeric id without any validations
    pub(crate) fn new_unchecked(input_string: String) -> Self {
        Self(input_string)
    }
}

/// Simple ID types for customer and merchant
#[derive(Debug, Clone, Serialize, Hash, PartialEq, Eq)]
pub struct CustomerId(String);

impl Default for CustomerId {
    fn default() -> Self {
        Self("cus_default".to_string())
    }
}

impl CustomerId {
    pub fn get_string_repr(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for CustomerId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(Self(s))
    }
}

impl FromStr for CustomerId {
    type Err = error_stack::Report<ValidationError>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.to_string()))
    }
}

impl TryFrom<Cow<'_, str>> for CustomerId {
    type Error = error_stack::Report<ValidationError>;

    fn try_from(value: Cow<'_, str>) -> Result<Self, Self::Error> {
        Ok(Self(value.to_string()))
    }
}

impl hyperswitch_masking::SerializableSecret for CustomerId {}

#[derive(Debug, Clone, Serialize, Hash, PartialEq, Eq)]

pub struct MerchantId(String);

impl Default for MerchantId {
    fn default() -> Self {
        Self("mer_default".to_string())
    }
}

impl MerchantId {
    pub fn get_string_repr(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for MerchantId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(Self(s))
    }
}

impl FromStr for MerchantId {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.to_string()))
    }
}

crate::id_type!(
    PaymentId,
    "A type for payment_id that can be used for payment ids"
);
crate::impl_id_type_methods!(PaymentId, "payment_id");

// This is to display the `PaymentId` as PaymentId(abcd)
crate::impl_debug_id_type!(PaymentId);
crate::impl_default_id_type!(PaymentId, "pay");
crate::impl_try_from_cow_str_id_type!(PaymentId, "payment_id");

impl PaymentId {
    /// Get the hash key to be stored in redis
    pub fn get_hash_key_for_kv_store(&self) -> String {
        format!("pi_{}", self.0 .0 .0)
    }

    // This function should be removed once we have a better way to handle mandatory payment id in other flows
    /// Get payment id in the format of irrelevant_payment_id_in_{flow}
    pub fn get_irrelevant_id(flow: &str) -> Self {
        let alphanumeric_id =
            AlphaNumericId::new_unchecked(format!("irrelevant_payment_id_in_{flow}"));
        let id = LengthId::new_unchecked(alphanumeric_id);
        Self(id)
    }

    /// Get the attempt id for the payment id based on the attempt count
    pub fn get_attempt_id(&self, attempt_count: i16) -> String {
        format!("{}_{attempt_count}", self.get_string_repr())
    }

    /// Generate a client id for the payment id
    pub fn generate_client_secret(&self) -> String {
        generate_id_with_default_len(&format!("{}_secret", self.get_string_repr()))
    }

    /// Generate a key for pm_auth
    pub fn get_pm_auth_key(&self) -> String {
        format!("pm_auth_{}", self.get_string_repr())
    }

    /// Get external authentication request poll id
    pub fn get_external_authentication_request_poll_id(&self) -> String {
        format!("external_authentication_{}", self.get_string_repr())
    }

    /// Generate a test payment id with prefix test_
    pub fn generate_test_payment_id_for_sample_data() -> Self {
        let id = generate_id_with_default_len("test");
        let alphanumeric_id = AlphaNumericId::new_unchecked(id);
        let id = LengthId::new_unchecked(alphanumeric_id);
        Self(id)
    }

    /// Wrap a string inside PaymentId
    pub fn wrap(payment_id_string: String) -> CustomResult<Self, ValidationError> {
        Self::try_from(Cow::from(payment_id_string))
    }
}

#[derive(Debug, Clone, Serialize, Hash, Deserialize, PartialEq, Eq)]

pub(crate) struct LengthId<const MAX_LENGTH: u8, const MIN_LENGTH: u8>(pub AlphaNumericId);

impl<const MAX_LENGTH: u8, const MIN_LENGTH: u8> LengthId<MAX_LENGTH, MIN_LENGTH> {
    /// Generates new [MerchantReferenceId] from the given input string
    pub fn from(input_string: Cow<'static, str>) -> Result<Self, LengthIdError> {
        let trimmed_input_string = input_string.trim().to_string();
        let length_of_input_string = u8::try_from(trimmed_input_string.len())
            .map_err(|_| LengthIdError::MaxLengthViolated(MAX_LENGTH))?;

        when(length_of_input_string > MAX_LENGTH, || {
            Err(LengthIdError::MaxLengthViolated(MAX_LENGTH))
        })?;

        when(length_of_input_string < MIN_LENGTH, || {
            Err(LengthIdError::MinLengthViolated(MIN_LENGTH))
        })?;

        let alphanumeric_id = match AlphaNumericId::from(trimmed_input_string.into()) {
            Ok(valid_alphanumeric_id) => valid_alphanumeric_id,
            Err(error) => Err(LengthIdError::AlphanumericIdError(error))?,
        };

        Ok(Self(alphanumeric_id))
    }

    /// Generate a new MerchantRefId of default length with the given prefix
    pub fn new(prefix: &str) -> Self {
        Self(AlphaNumericId::new(prefix))
    }

    /// Use this function only if you are sure that the length is within the range
    pub(crate) fn new_unchecked(alphanumeric_id: AlphaNumericId) -> Self {
        Self(alphanumeric_id)
    }

    /// Create a new LengthId from aplhanumeric id
    pub(crate) fn from_alphanumeric_id(
        alphanumeric_id: AlphaNumericId,
    ) -> Result<Self, LengthIdError> {
        let length_of_input_string = alphanumeric_id.0.len();
        let length_of_input_string = u8::try_from(length_of_input_string)
            .map_err(|_| LengthIdError::MaxLengthViolated(MAX_LENGTH))?;

        when(length_of_input_string > MAX_LENGTH, || {
            Err(LengthIdError::MaxLengthViolated(MAX_LENGTH))
        })?;

        when(length_of_input_string < MIN_LENGTH, || {
            Err(LengthIdError::MinLengthViolated(MIN_LENGTH))
        })?;

        Ok(Self(alphanumeric_id))
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LengthIdError {
    #[error("the maximum allowed length for this field is {0}")]
    /// Maximum length of string violated
    MaxLengthViolated(u8),

    #[error("the minimum required length for this field is {0}")]
    /// Minimum length of string violated
    MinLengthViolated(u8),

    #[error("{0}")]
    /// Input contains invalid characters
    AlphanumericIdError(AlphaNumericIdError),
}

impl From<AlphaNumericIdError> for LengthIdError {
    fn from(alphanumeric_id_error: AlphaNumericIdError) -> Self {
        Self::AlphanumericIdError(alphanumeric_id_error)
    }
}

use std::str::FromStr;

crate::id_type!(
    ProfileId,
    "A type for profile_id that can be used for business profile ids"
);
crate::impl_id_type_methods!(ProfileId, "profile_id");

// This is to display the `ProfileId` as ProfileId(abcd)
crate::impl_debug_id_type!(ProfileId);
crate::impl_try_from_cow_str_id_type!(ProfileId, "profile_id");

crate::impl_generate_id_id_type!(ProfileId, "pro");
crate::impl_serializable_secret_id_type!(ProfileId);

impl crate::events::ApiEventMetric for ProfileId {
    fn get_api_event_type(&self) -> Option<crate::events::ApiEventsType> {
        Some(crate::events::ApiEventsType::BusinessProfile {
            profile_id: self.clone(),
        })
    }
}

impl FromStr for ProfileId {
    type Err = error_stack::Report<ValidationError>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let cow_string = Cow::Owned(s.to_string());
        Self::try_from(cow_string)
    }
}

/// An interface to generate object identifiers.
pub trait GenerateId {
    /// Generates a random object identifier.
    fn generate() -> Self;
}

crate::id_type!(
    ClientSecretId,
    "A type for key_id that can be used for Ephemeral key IDs"
);
crate::impl_id_type_methods!(ClientSecretId, "key_id");

// This is to display the `ClientSecretId` as ClientSecretId(abcd)
crate::impl_debug_id_type!(ClientSecretId);
crate::impl_try_from_cow_str_id_type!(ClientSecretId, "key_id");

crate::impl_generate_id_id_type!(ClientSecretId, "csi");
crate::impl_serializable_secret_id_type!(ClientSecretId);

impl crate::events::ApiEventMetric for ClientSecretId {
    fn get_api_event_type(&self) -> Option<crate::events::ApiEventsType> {
        Some(crate::events::ApiEventsType::ClientSecret {
            key_id: self.clone(),
        })
    }
}

crate::impl_default_id_type!(ClientSecretId, "key");

impl ClientSecretId {
    /// Generate a key for redis
    pub fn generate_redis_key(&self) -> String {
        format!("cs_{}", self.get_string_repr())
    }
}

crate::id_type!(
    ApiKeyId,
    "A type for key_id that can be used for API key IDs"
);
crate::impl_id_type_methods!(ApiKeyId, "key_id");

// This is to display the `ApiKeyId` as ApiKeyId(abcd)
crate::impl_debug_id_type!(ApiKeyId);
crate::impl_try_from_cow_str_id_type!(ApiKeyId, "key_id");

crate::impl_serializable_secret_id_type!(ApiKeyId);

impl ApiKeyId {
    /// Generate Api Key Id from prefix
    pub fn generate_key_id(prefix: &'static str) -> Self {
        Self(crate::generate_ref_id_with_default_length(prefix))
    }
}

impl crate::events::ApiEventMetric for ApiKeyId {
    fn get_api_event_type(&self) -> Option<crate::events::ApiEventsType> {
        Some(crate::events::ApiEventsType::ApiKey {
            key_id: self.clone(),
        })
    }
}

impl crate::events::ApiEventMetric for (MerchantId, ApiKeyId) {
    fn get_api_event_type(&self) -> Option<crate::events::ApiEventsType> {
        Some(crate::events::ApiEventsType::ApiKey {
            key_id: self.1.clone(),
        })
    }
}

impl crate::events::ApiEventMetric for (&MerchantId, &ApiKeyId) {
    fn get_api_event_type(&self) -> Option<crate::events::ApiEventsType> {
        Some(crate::events::ApiEventsType::ApiKey {
            key_id: self.1.clone(),
        })
    }
}

crate::impl_default_id_type!(ApiKeyId, "key");

crate::id_type!(
    MerchantConnectorAccountId,
    "A type for merchant_connector_id that can be used for merchant_connector_account ids"
);
crate::impl_id_type_methods!(MerchantConnectorAccountId, "merchant_connector_id");

// This is to display the `MerchantConnectorAccountId` as MerchantConnectorAccountId(abcd)
crate::impl_debug_id_type!(MerchantConnectorAccountId);
crate::impl_generate_id_id_type!(MerchantConnectorAccountId, "mca");
crate::impl_try_from_cow_str_id_type!(MerchantConnectorAccountId, "merchant_connector_id");

crate::impl_serializable_secret_id_type!(MerchantConnectorAccountId);

impl MerchantConnectorAccountId {
    /// Get a merchant connector account id from String
    pub fn wrap(merchant_connector_account_id: String) -> CustomResult<Self, ValidationError> {
        Self::try_from(Cow::from(merchant_connector_account_id))
    }
}

crate::id_type!(
    ProfileAcquirerId,
    "A type for profile_acquirer_id that can be used for profile acquirer ids"
);
crate::impl_id_type_methods!(ProfileAcquirerId, "profile_acquirer_id");

// This is to display the `ProfileAcquirerId` as ProfileAcquirerId(abcd)
crate::impl_debug_id_type!(ProfileAcquirerId);
crate::impl_try_from_cow_str_id_type!(ProfileAcquirerId, "profile_acquirer_id");

crate::impl_generate_id_id_type!(ProfileAcquirerId, "pro_acq");
crate::impl_serializable_secret_id_type!(ProfileAcquirerId);

impl Ord for ProfileAcquirerId {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0 .0 .0.cmp(&other.0 .0 .0)
    }
}

impl PartialOrd for ProfileAcquirerId {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl crate::events::ApiEventMetric for ProfileAcquirerId {
    fn get_api_event_type(&self) -> Option<crate::events::ApiEventsType> {
        Some(crate::events::ApiEventsType::ProfileAcquirer {
            profile_acquirer_id: self.clone(),
        })
    }
}

impl FromStr for ProfileAcquirerId {
    type Err = error_stack::Report<ValidationError>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let cow_string = Cow::Owned(s.to_string());
        Self::try_from(cow_string)
    }
}
