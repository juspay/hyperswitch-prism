//! Types that can be used in other crates

use std::{
    fmt::Display,
    iter::Sum,
    ops::{Add, Mul, Sub},
    str::FromStr,
};

use common_enums::enums;
use error_stack::ResultExt;
use hyperswitch_masking::Deserialize;
use rust_decimal::{
    prelude::{FromPrimitive, ToPrimitive},
    Decimal,
};
use semver::Version;
use serde::Serialize;
use time::PrimitiveDateTime;
use utoipa::ToSchema;

use crate::errors::ParsingError;

/// Amount convertor trait for connector
pub trait AmountConvertor: Send {
    /// Output type for the connector
    type Output;
    /// helps in conversion of connector required amount type
    fn convert(
        &self,
        amount: MinorUnit,
        currency: enums::Currency,
    ) -> Result<Self::Output, error_stack::Report<ParsingError>>;

    /// helps in converting back connector required amount type to core minor unit
    fn convert_back(
        &self,
        amount: Self::Output,
        currency: enums::Currency,
    ) -> Result<MinorUnit, error_stack::Report<ParsingError>>;
}

/// Connector required amount type
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct StringMinorUnitForConnector;

impl AmountConvertor for StringMinorUnitForConnector {
    type Output = StringMinorUnit;
    fn convert(
        &self,
        amount: MinorUnit,
        _currency: enums::Currency,
    ) -> Result<Self::Output, error_stack::Report<ParsingError>> {
        amount.to_minor_unit_as_string()
    }

    fn convert_back(
        &self,
        amount: Self::Output,
        _currency: enums::Currency,
    ) -> Result<MinorUnit, error_stack::Report<ParsingError>> {
        amount.to_minor_unit_as_i64()
    }
}

/// Core required conversion type
#[derive(Default, Debug, serde::Deserialize, serde::Serialize, Clone, Copy, PartialEq)]
pub struct StringMajorUnitForCore;
impl AmountConvertor for StringMajorUnitForCore {
    type Output = StringMajorUnit;
    fn convert(
        &self,
        amount: MinorUnit,
        currency: enums::Currency,
    ) -> Result<Self::Output, error_stack::Report<ParsingError>> {
        amount.to_major_unit_as_string(currency)
    }

    fn convert_back(
        &self,
        amount: StringMajorUnit,
        currency: enums::Currency,
    ) -> Result<MinorUnit, error_stack::Report<ParsingError>> {
        amount.to_minor_unit_as_i64(currency)
    }
}

/// Connector required amount type
#[derive(Default, Debug, serde::Deserialize, serde::Serialize, Clone, Copy, PartialEq)]
pub struct StringMajorUnitForConnector;

impl AmountConvertor for StringMajorUnitForConnector {
    type Output = StringMajorUnit;
    fn convert(
        &self,
        amount: MinorUnit,
        currency: enums::Currency,
    ) -> Result<Self::Output, error_stack::Report<ParsingError>> {
        amount.to_major_unit_as_string(currency)
    }

    fn convert_back(
        &self,
        amount: StringMajorUnit,
        currency: enums::Currency,
    ) -> Result<MinorUnit, error_stack::Report<ParsingError>> {
        amount.to_minor_unit_as_i64(currency)
    }
}

/// Connector required amount type
#[derive(Default, Debug, serde::Deserialize, serde::Serialize, Clone, Copy, PartialEq)]
pub struct FloatMajorUnitForConnector;

impl AmountConvertor for FloatMajorUnitForConnector {
    type Output = FloatMajorUnit;
    fn convert(
        &self,
        amount: MinorUnit,
        currency: enums::Currency,
    ) -> Result<Self::Output, error_stack::Report<ParsingError>> {
        amount.to_major_unit_as_f64(currency)
    }
    fn convert_back(
        &self,
        amount: FloatMajorUnit,
        currency: enums::Currency,
    ) -> Result<MinorUnit, error_stack::Report<ParsingError>> {
        amount.to_minor_unit_as_i64(currency)
    }
}

/// Connector required amount type
#[derive(Default, Debug, serde::Deserialize, serde::Serialize, Clone, Copy, PartialEq)]
pub struct MinorUnitForConnector;

impl AmountConvertor for MinorUnitForConnector {
    type Output = MinorUnit;
    fn convert(
        &self,
        amount: MinorUnit,
        _currency: enums::Currency,
    ) -> Result<Self::Output, error_stack::Report<ParsingError>> {
        Ok(amount)
    }
    fn convert_back(
        &self,
        amount: MinorUnit,
        _currency: enums::Currency,
    ) -> Result<MinorUnit, error_stack::Report<ParsingError>> {
        Ok(amount)
    }
}

/// This Unit struct represents MinorUnit in which core amount works
#[derive(
    Default,
    Debug,
    serde::Deserialize,
    serde::Serialize,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    ToSchema,
    PartialOrd,
)]

pub struct MinorUnit(pub i64);

impl MinorUnit {
    /// gets amount as i64 value will be removed in future
    pub fn get_amount_as_i64(self) -> i64 {
        self.0
    }

    /// forms a new minor default unit i.e zero
    pub fn zero() -> Self {
        Self(0)
    }

    /// forms a new minor unit from amount
    pub fn new(value: i64) -> Self {
        Self(value)
    }

    /// checks if the amount is greater than the given value
    pub fn is_greater_than(&self, value: i64) -> bool {
        self.get_amount_as_i64() > value
    }

    /// Convert the amount to its major denomination based on Currency and return String
    /// This method now validates currency support and will error for unsupported currencies.
    /// Paypal Connector accepts Zero and Two decimal currency but not three decimal and it should be updated as required for 3 decimal currencies.
    /// Paypal Ref - https://developer.paypal.com/docs/reports/reference/paypal-supported-currencies/
    fn to_major_unit_as_string(
        self,
        currency: enums::Currency,
    ) -> Result<StringMajorUnit, error_stack::Report<ParsingError>> {
        let amount_f64 = self.to_major_unit_as_f64(currency)?;
        let decimal_places = currency
            .number_of_digits_after_decimal_point()
            .change_context(ParsingError::StructParseFailure(
                "currency decimal configuration",
            ))?;

        let amount_string = if decimal_places == 0 {
            amount_f64.0.to_string()
        } else if decimal_places == 3 {
            format!("{:.3}", amount_f64.0)
        } else if decimal_places == 4 {
            format!("{:.4}", amount_f64.0)
        } else {
            format!("{:.2}", amount_f64.0) // 2 decimal places
        };
        Ok(StringMajorUnit::new(amount_string))
    }

    /// Convert the amount to its major denomination based on Currency and return f64
    /// This method now validates currency support and will error for unsupported currencies.
    fn to_major_unit_as_f64(
        self,
        currency: enums::Currency,
    ) -> Result<FloatMajorUnit, error_stack::Report<ParsingError>> {
        let amount_decimal =
            Decimal::from_i64(self.0).ok_or(ParsingError::I64ToDecimalConversionFailure)?;

        let decimal_places = currency
            .number_of_digits_after_decimal_point()
            .change_context(ParsingError::StructParseFailure(
                "currency decimal configuration",
            ))?;

        let amount = if decimal_places == 0 {
            amount_decimal
        } else if decimal_places == 3 {
            amount_decimal / Decimal::from(1000)
        } else if decimal_places == 4 {
            amount_decimal / Decimal::from(10000)
        } else {
            amount_decimal / Decimal::from(100) // 2 decimal places
        };

        let amount_f64 = amount
            .to_f64()
            .ok_or(ParsingError::FloatToDecimalConversionFailure)?;
        Ok(FloatMajorUnit::new(amount_f64))
    }

    ///Convert minor unit to string minor unit
    fn to_minor_unit_as_string(self) -> Result<StringMinorUnit, error_stack::Report<ParsingError>> {
        Ok(StringMinorUnit::new(self.0.to_string()))
    }
}

impl Display for MinorUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Add for MinorUnit {
    type Output = Self;
    fn add(self, a2: Self) -> Self {
        Self(self.0 + a2.0)
    }
}

impl Sub for MinorUnit {
    type Output = Self;
    fn sub(self, a2: Self) -> Self {
        Self(self.0 - a2.0)
    }
}

impl Mul<u16> for MinorUnit {
    type Output = Self;

    fn mul(self, a2: u16) -> Self::Output {
        Self(self.0 * i64::from(a2))
    }
}

impl Sum for MinorUnit {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self(0), |a, b| a + b)
    }
}

/// Connector specific types to send
#[derive(
    Default,
    Debug,
    serde::Deserialize,
    serde::Serialize,
    Clone,
    PartialEq,
    Eq,
    Hash,
    ToSchema,
    PartialOrd,
)]

pub struct StringMinorUnit(String);

impl StringMinorUnit {
    /// forms a new minor unit in string from amount
    fn new(value: String) -> Self {
        Self(value)
    }

    /// converts to minor unit i64 from minor unit string value
    fn to_minor_unit_as_i64(&self) -> Result<MinorUnit, error_stack::Report<ParsingError>> {
        let amount_string = &self.0;
        let amount_decimal = Decimal::from_str(amount_string).map_err(|e| {
            ParsingError::StringToDecimalConversionFailure {
                error: e.to_string(),
            }
        })?;
        let amount_i64 = amount_decimal
            .to_i64()
            .ok_or(ParsingError::DecimalToI64ConversionFailure)?;
        Ok(MinorUnit::new(amount_i64))
    }
}

impl Display for StringMinorUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Connector specific types to send
#[derive(Default, Debug, serde::Deserialize, serde::Serialize, Clone, Copy, PartialEq)]
pub struct FloatMajorUnit(pub f64);

impl FloatMajorUnit {
    /// forms a new major unit from amount
    fn new(value: f64) -> Self {
        Self(value)
    }

    /// forms a new major unit with zero amount
    pub fn zero() -> Self {
        Self(0.0)
    }

    /// converts to minor unit as i64 from FloatMajorUnit
    fn to_minor_unit_as_i64(
        self,
        currency: enums::Currency,
    ) -> Result<MinorUnit, error_stack::Report<ParsingError>> {
        let amount_decimal =
            Decimal::from_f64(self.0).ok_or(ParsingError::FloatToDecimalConversionFailure)?;

        let amount = if currency.is_zero_decimal_currency() {
            amount_decimal
        } else if currency.is_three_decimal_currency() {
            amount_decimal * Decimal::from(1000)
        } else {
            amount_decimal * Decimal::from(100)
        };

        let amount_i64 = amount
            .to_i64()
            .ok_or(ParsingError::DecimalToI64ConversionFailure)?;
        Ok(MinorUnit::new(amount_i64))
    }
}

/// Connector specific types to send
#[derive(Default, Debug, serde::Deserialize, serde::Serialize, Clone, PartialEq, Eq)]
pub struct StringMajorUnit(String);

impl StringMajorUnit {
    /// forms a new major unit from amount
    fn new(value: String) -> Self {
        Self(value)
    }

    /// Converts to minor unit as i64 from StringMajorUnit
    fn to_minor_unit_as_i64(
        &self,
        currency: enums::Currency,
    ) -> Result<MinorUnit, error_stack::Report<ParsingError>> {
        let amount_decimal = Decimal::from_str(&self.0).map_err(|e| {
            ParsingError::StringToDecimalConversionFailure {
                error: e.to_string(),
            }
        })?;

        let amount = if currency.is_zero_decimal_currency() {
            amount_decimal
        } else if currency.is_three_decimal_currency() {
            amount_decimal * Decimal::from(1000)
        } else {
            amount_decimal * Decimal::from(100)
        };
        let amount_i64 = amount
            .to_i64()
            .ok_or(ParsingError::DecimalToI64ConversionFailure)?;
        Ok(MinorUnit::new(amount_i64))
    }
    /// forms a new StringMajorUnit default unit i.e zero
    pub fn zero() -> Self {
        Self("0".to_string())
    }
    /// Get string amount from struct to be removed in future
    pub fn get_amount_as_string(&self) -> String {
        self.0.clone()
    }
}

/// The number of implied decimals [`StringImpliedDecimal`] always carries.
const IMPLIED_DECIMAL_EXPONENT: u32 = 2;

/// `10^exponent` for a currency's own ISO 4217 exponent (0, 2, 3 or 4). Errors for a
/// currency with no decimal configuration rather than assuming two.
fn currency_scale(currency: enums::Currency) -> Result<i128, error_stack::Report<ParsingError>> {
    let exponent = currency
        .number_of_digits_after_decimal_point()
        .change_context(ParsingError::StructParseFailure(
            "currency decimal configuration",
        ))?;
    Ok(10_i128.pow(u32::from(exponent)))
}

/// Connector specific types to send
///
/// The major amount rendered with exactly two decimal places and the decimal point
/// dropped, i.e. `minor * 100 / 10^currency_exponent`. The two decimals are implied for
/// **every** currency, including zero-exponent ones: USD 1234 and JPY 1234 serialize as
/// `"1234"` and `"123400"` respectively.
///
/// Use this for gateways that fix the scale of the amount field instead of taking the
/// currency's own exponent. Field constraints that belong to a particular gateway rather
/// than to the encoding — a maximum length, or a refusal to send a negative — are not
/// applied here; call [`Self::validate_max_len`] and [`Self::validate_unsigned`] at the
/// point of use so each connector's own rules stay visible in its transformer.
#[derive(Default, Debug, serde::Deserialize, serde::Serialize, Clone, PartialEq, Eq)]
pub struct StringImpliedDecimal(String);

impl StringImpliedDecimal {
    /// forms a new implied-decimal unit from amount
    fn new(value: String) -> Self {
        Self(value)
    }

    /// Get the wire string.
    pub fn get_amount_as_string(&self) -> &str {
        &self.0
    }

    /// Reject a negative amount, for gateways whose amount field is unsigned.
    pub fn validate_unsigned(
        self,
        field_name: &'static str,
    ) -> Result<Self, error_stack::Report<ParsingError>> {
        if self.0.starts_with('-') {
            return Err(
                error_stack::report!(ParsingError::StructParseFailure(field_name))
                    .attach_printable(format!("{field_name}: `{}` is negative", self.0)),
            );
        }
        Ok(self)
    }

    /// Reject an amount longer than the gateway's field width.
    pub fn validate_max_len(
        self,
        max_len: usize,
        field_name: &'static str,
    ) -> Result<Self, error_stack::Report<ParsingError>> {
        if self.0.len() > max_len {
            return Err(
                error_stack::report!(ParsingError::StructParseFailure(field_name))
                    .attach_printable(format!(
                        "{field_name}: `{}` exceeds the {max_len}-character maximum",
                        self.0
                    )),
            );
        }
        Ok(self)
    }

    /// Converts to minor unit as i64 from StringImpliedDecimal
    fn to_minor_unit_as_i64(
        &self,
        currency: enums::Currency,
    ) -> Result<MinorUnit, error_stack::Report<ParsingError>> {
        let wire = self.0.parse::<i128>().map_err(|_| {
            error_stack::report!(ParsingError::StructParseFailure("implied decimal amount"))
                .attach_printable(format!("`{}` is not an integer", self.0))
        })?;

        let scaled = wire * currency_scale(currency)?;
        let divisor = 10_i128.pow(IMPLIED_DECIMAL_EXPONENT);
        if scaled % divisor != 0 {
            return Err(error_stack::report!(ParsingError::StructParseFailure(
                "implied decimal amount"
            ))
            .attach_printable(format!(
                "`{wire}` is not a whole number of {currency:?} minor units"
            )));
        }

        let minor = i64::try_from(scaled / divisor)
            .map_err(|_| ParsingError::DecimalToI64ConversionFailure)?;
        Ok(MinorUnit::new(minor))
    }
}

/// Connector required amount type
#[derive(Default, Debug, serde::Deserialize, serde::Serialize, Clone, Copy, PartialEq)]
pub struct StringImpliedDecimalForConnector;

impl AmountConvertor for StringImpliedDecimalForConnector {
    type Output = StringImpliedDecimal;

    /// `wire = minor * 100 / 10^currency_exponent`, kept in `i128` so there is neither a
    /// float hop nor an intermediate decimal string to re-parse.
    fn convert(
        &self,
        amount: MinorUnit,
        currency: enums::Currency,
    ) -> Result<Self::Output, error_stack::Report<ParsingError>> {
        let minor = i128::from(amount.get_amount_as_i64());
        let scale = currency_scale(currency)?;
        let scaled = minor * 10_i128.pow(IMPLIED_DECIMAL_EXPONENT);
        // A currency with more than two decimals cannot always be expressed with two
        // implied ones, so refuse rather than silently dropping the sub-unit digits.
        // Unlike a length or sign limit this is intrinsic to the encoding, so no caller
        // can meaningfully opt out of it.
        if scaled % scale != 0 {
            return Err(error_stack::report!(ParsingError::StructParseFailure(
                "implied decimal amount"
            ))
            .attach_printable(format!(
                "{currency:?} minor amount {minor} has non-zero digits below the two \
                 decimals this format can represent"
            )));
        }
        Ok(StringImpliedDecimal::new((scaled / scale).to_string()))
    }

    fn convert_back(
        &self,
        amount: Self::Output,
        currency: enums::Currency,
    ) -> Result<MinorUnit, error_stack::Report<ParsingError>> {
        amount.to_minor_unit_as_i64(currency)
    }
}

#[derive(
    Default, Debug, serde::Deserialize, serde::Serialize, Clone, PartialEq, Eq, Hash, ToSchema,
)]

pub struct Money {
    pub amount: MinorUnit,
    pub currency: enums::Currency,
}

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
