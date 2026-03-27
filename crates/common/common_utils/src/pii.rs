//! Personal Identifiable Information protection.

use std::{convert::AsRef, fmt, ops, str::FromStr};

use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Secret, Strategy, WithType};
use serde::Deserialize;

use crate::{
    consts::REDACTED,
    errors::{self, ValidationError},
};

/// Type alias for serde_json value which has Secret Information
pub type SecretSerdeValue = Secret<serde_json::Value>;

/// Strategy for masking Email
#[derive(Debug, Copy, Clone, Deserialize)]
pub enum EmailStrategy {}

impl<T> Strategy<T> for EmailStrategy
where
    T: AsRef<str> + fmt::Debug,
{
    fn fmt(val: &T, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let val_str: &str = val.as_ref();
        match val_str.split_once('@') {
            Some((a, b)) => write!(f, "{}@{}", "*".repeat(a.len()), b),
            None => WithType::fmt(val, f),
        }
    }
}

/// Email address
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq, Default)]
#[serde(try_from = "String")]
pub struct Email(Secret<String, EmailStrategy>);

impl ExposeInterface<Secret<String, EmailStrategy>> for Email {
    fn expose(self) -> Secret<String, EmailStrategy> {
        self.0
    }
}

impl TryFrom<String> for Email {
    type Error = error_stack::Report<errors::ParsingError>;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::from_str(&value).change_context(errors::ParsingError::EmailParsingError)
    }
}

impl ops::Deref for Email {
    type Target = Secret<String, EmailStrategy>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ops::DerefMut for Email {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl FromStr for Email {
    type Err = error_stack::Report<ValidationError>;
    fn from_str(email: &str) -> Result<Self, Self::Err> {
        if email.eq(REDACTED) {
            return Ok(Self(Secret::new(email.to_string())));
        }
        // Basic email validation - in production you'd use a more robust validator
        if email.contains('@') && email.len() > 3 {
            let secret = Secret::<String, EmailStrategy>::new(email.to_string());
            Ok(Self(secret))
        } else {
            Err(ValidationError::InvalidValue {
                message: "Invalid email address format".into(),
            }
            .into())
        }
    }
}

/// IP address strategy
#[derive(Debug)]
pub enum IpAddress {}

impl<T> Strategy<T> for IpAddress
where
    T: AsRef<str>,
{
    fn fmt(val: &T, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let val_str: &str = val.as_ref();
        let segments: Vec<&str> = val_str.split('.').collect();

        if segments.len() != 4 {
            return WithType::fmt(val, f);
        }

        for seg in segments.iter() {
            if seg.is_empty() || seg.len() > 3 {
                return WithType::fmt(val, f);
            }
        }

        if let Some(segments) = segments.first() {
            write!(f, "{segments}.**.**.**")
        } else {
            WithType::fmt(val, f)
        }
    }
}

/// Strategy for masking UPI VPA's
#[derive(Debug)]
pub enum UpiVpaMaskingStrategy {}

impl<T> Strategy<T> for UpiVpaMaskingStrategy
where
    T: AsRef<str> + fmt::Debug,
{
    fn fmt(val: &T, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let vpa_str: &str = val.as_ref();
        if let Some((user_identifier, bank_or_psp)) = vpa_str.split_once('@') {
            let masked_user_identifier = "*".repeat(user_identifier.len());
            write!(f, "{masked_user_identifier}@{bank_or_psp}")
        } else {
            WithType::fmt(val, f)
        }
    }
}

#[derive(Debug)]
pub enum EncryptionStrategy {}

impl<T> Strategy<T> for EncryptionStrategy
where
    T: AsRef<[u8]>,
{
    fn fmt(value: &T, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            fmt,
            "*** Encrypted data of length {} bytes ***",
            value.as_ref().len()
        )
    }
}
