//! Types that can be used in other crates

use std::{fmt::Display, str::FromStr};

use error_stack::ResultExt;
use hyperswitch_masking::Deserialize;
use semver::Version;
use serde::Serialize;
use time::PrimitiveDateTime;
use utoipa::ToSchema;

use crate::errors::ParsingError;

pub use crate::money::{
    AmountConvertor, ConnectorMinorUnit, ConnectorMinorUnitForConnector, FloatMajorUnit,
    FloatMajorUnitForConnector, MinorUnit, MinorUnitForConnector, Money, StringMajorUnit,
    StringMajorUnitForConnector, StringMajorUnitForCore, StringMinorUnit,
    StringMinorUnitForConnector, StringTwoDecimalUnit, StringTwoDecimalUnitForConnector,
};

/// A type representing a range of time for filtering, including a mandatory start time and an optional end time.
#[derive(
    Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash, ToSchema,
)]
pub struct TimeRange {
    /// The start time to filter payments list or to get list of filters. To get list of filters start time is needed to be passed
    #[serde(with = "crate::custom_serde::iso8601")]
    #[serde(alias = "startTime")]
    pub start_time: PrimitiveDateTime,
    /// The end time to filter payments list or to get list of filters. If not passed the default time is now
    #[serde(default, with = "crate::custom_serde::iso8601::option")]
    #[serde(alias = "endTime")]
    pub end_time: Option<PrimitiveDateTime>,
}

/// This struct lets us represent a semantic version type
#[derive(Debug, Clone, PartialEq, Eq, Ord, PartialOrd, Serialize, serde::Deserialize)]
pub struct SemanticVersion(#[serde(with = "Version")] Version);

impl SemanticVersion {
    /// returns major version number
    pub fn get_major(&self) -> u64 {
        self.0.major
    }

    /// returns minor version number
    pub fn get_minor(&self) -> u64 {
        self.0.minor
    }
    /// Constructs new SemanticVersion instance
    pub fn new(major: u64, minor: u64, patch: u64) -> Self {
        Self(Version::new(major, minor, patch))
    }
}

impl Display for SemanticVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for SemanticVersion {
    type Err = error_stack::Report<ParsingError>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Version::from_str(s).change_context(
            ParsingError::StructParseFailure("SemanticVersion"),
        )?))
    }
}

/// Primary execution or shadow mirror, derived from the `x-shadow-mode` metadata flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    Primary,
    Shadow,
}

impl ExecutionMode {
    /// Map the boolean shadow flag (from the `x-shadow-mode` metadata) to an execution mode.
    pub fn from_shadow_flag(shadow_mode: bool) -> Self {
        if shadow_mode {
            Self::Shadow
        } else {
            Self::Primary
        }
    }

    /// Stable string form for log/span fields; matches the serde representation used in events.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Shadow => "shadow",
        }
    }
}
