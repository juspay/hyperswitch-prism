//! Money types and amount converters.

use std::{fmt::Display, str::FromStr};

use common_enums::enums;
use error_stack::ResultExt;
use rust_decimal::{
    prelude::{FromPrimitive, ToPrimitive},
    Decimal,
};
use utoipa::ToSchema;

use crate::errors::ParsingError;

/// Amount convertor trait for connector
pub trait AmountConvertor: Send {
    /// Output type for the connector
    type Output;
    /// Converts domain money into the connector's amount representation.
    fn convert(&self, money: &Money) -> Result<Self::Output, error_stack::Report<ParsingError>>;

    /// Converts a connector amount into domain money with the supplied currency.
    fn convert_back(
        &self,
        amount: Self::Output,
        currency: enums::Currency,
    ) -> Result<Money, error_stack::Report<ParsingError>>;
}

/// Connector required amount type
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct StringMinorUnitForConnector;

impl AmountConvertor for StringMinorUnitForConnector {
    type Output = StringMinorUnit;
    fn convert(&self, money: &Money) -> Result<Self::Output, error_stack::Report<ParsingError>> {
        money.amount.to_minor_unit_as_string()
    }

    fn convert_back(
        &self,
        amount: Self::Output,
        currency: enums::Currency,
    ) -> Result<Money, error_stack::Report<ParsingError>> {
        Ok(Money::from_minor_unit(
            amount.to_minor_unit_as_i64()?,
            currency,
        ))
    }
}

/// Core required conversion type
#[derive(Default, Debug, serde::Deserialize, serde::Serialize, Clone, Copy, PartialEq)]
pub struct StringMajorUnitForCore;
impl AmountConvertor for StringMajorUnitForCore {
    type Output = StringMajorUnit;
    fn convert(&self, money: &Money) -> Result<Self::Output, error_stack::Report<ParsingError>> {
        money.amount.to_major_unit_as_string(money.currency)
    }

    fn convert_back(
        &self,
        amount: StringMajorUnit,
        currency: enums::Currency,
    ) -> Result<Money, error_stack::Report<ParsingError>> {
        Ok(Money::from_minor_unit(
            amount.to_minor_unit_as_i64(currency)?,
            currency,
        ))
    }
}

/// Connector required amount type
#[derive(Default, Debug, serde::Deserialize, serde::Serialize, Clone, Copy, PartialEq)]
pub struct StringMajorUnitForConnector;

impl AmountConvertor for StringMajorUnitForConnector {
    type Output = StringMajorUnit;
    fn convert(&self, money: &Money) -> Result<Self::Output, error_stack::Report<ParsingError>> {
        money.amount.to_major_unit_as_string(money.currency)
    }

    fn convert_back(
        &self,
        amount: StringMajorUnit,
        currency: enums::Currency,
    ) -> Result<Money, error_stack::Report<ParsingError>> {
        Ok(Money::from_minor_unit(
            amount.to_minor_unit_as_i64(currency)?,
            currency,
        ))
    }
}

/// Legacy fixed-hundredths text with a truncated whole-unit part and absolute remainder.
/// Currency exponents are deliberately ignored. Values from -99 through -1 lose their sign.
/// This encoding is one-way because those negative values cannot be recovered from the text.
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct TruncatedTwoDecimalMajorUnitForConnector;

impl TruncatedTwoDecimalMajorUnitForConnector {
    pub fn convert(&self, money: &Money) -> StringMajorUnit {
        let amount = money.amount.0;
        StringMajorUnit::new(format!("{}.{:02}", amount / 100, (amount % 100).abs()))
    }
}

/// Connector required amount type
#[derive(Default, Debug, serde::Deserialize, serde::Serialize, Clone, Copy, PartialEq)]
pub struct FloatMajorUnitForConnector;

impl AmountConvertor for FloatMajorUnitForConnector {
    type Output = FloatMajorUnit;
    fn convert(&self, money: &Money) -> Result<Self::Output, error_stack::Report<ParsingError>> {
        money.amount.to_major_unit_as_f64(money.currency)
    }
    fn convert_back(
        &self,
        amount: FloatMajorUnit,
        currency: enums::Currency,
    ) -> Result<Money, error_stack::Report<ParsingError>> {
        Ok(Money::from_minor_unit(
            amount.to_minor_unit_as_i64(currency)?,
            currency,
        ))
    }
}

/// Converts domain minor units to and from connector-side integer minor units.
///
/// Both representations use the currency's minor denomination, so this converter
/// preserves the integer value without applying a currency scale.
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct ConnectorMinorUnitForConnector;

/// Convert money to an integer string with a minimum, zero-filled field width.
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct PaddedStringMinorUnitForConnector;

impl PaddedStringMinorUnitForConnector {
    pub fn convert(&self, money: &Money, minimum_width: u8) -> StringMinorUnit {
        let minimum_width = usize::from(minimum_width);
        StringMinorUnit::new(format!("{:0>minimum_width$}", money.amount.0))
    }
}

/// Failure to encode a checked, strictly positive scaled minor-unit amount.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PositiveScaledMinorUnitError {
    #[error("minor-unit scaling overflow")]
    Overflow,
    #[error("scaled minor-unit amount is not positive")]
    NonPositive,
}

/// Checked scaling and positive-value validation before integer-string encoding.
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct PositiveScaledStringMinorUnitForConnector;

impl PositiveScaledStringMinorUnitForConnector {
    pub fn convert(
        &self,
        money: &Money,
        multiplier: u16,
    ) -> Result<StringMinorUnit, PositiveScaledMinorUnitError> {
        let scaled = money
            .amount
            .0
            .checked_mul(i64::from(multiplier))
            .ok_or(PositiveScaledMinorUnitError::Overflow)?;
        if scaled <= 0 {
            return Err(PositiveScaledMinorUnitError::NonPositive);
        }
        Ok(StringMinorUnit::new(scaled.to_string()))
    }
}

impl ConnectorMinorUnitForConnector {
    /// Preserves the integer minor-unit value without currency scaling.
    pub fn convert(&self, money: &Money) -> ConnectorMinorUnit {
        ConnectorMinorUnit(money.amount.get_amount_as_i64())
    }

    /// Decode an optional quoted minor-unit response amount, ignoring invalid integer text.
    /// Non-string JSON values remain deserialization errors.
    pub fn deserialize_optional_string<'de, D>(
        deserializer: D,
    ) -> Result<Option<ConnectorMinorUnit>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = <Option<String> as serde::Deserialize>::deserialize(deserializer)?;
        Ok(value.and_then(|value| value.parse::<i64>().ok().map(ConnectorMinorUnit)))
    }

    /// Encode an optional connector amount as quoted, canonical minor-unit text.
    pub fn serialize_optional_string<S>(
        amount: &Option<ConnectorMinorUnit>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serde::Serialize::serialize(&amount.as_ref().map(ToString::to_string), serializer)
    }
}

impl AmountConvertor for ConnectorMinorUnitForConnector {
    type Output = ConnectorMinorUnit;

    fn convert(&self, money: &Money) -> Result<Self::Output, error_stack::Report<ParsingError>> {
        Ok(Self::convert(self, money))
    }

    fn convert_back(
        &self,
        amount: Self::Output,
        currency: enums::Currency,
    ) -> Result<Money, error_stack::Report<ParsingError>> {
        Ok(Money::from_minor_unit(
            MinorUnit::new(amount.get_amount_as_i64()),
            currency,
        ))
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

pub struct MinorUnit(i64);

/// An opaque percentage with two decimal places: 2.50 represents 2.5%.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Percentage(Decimal);

impl Percentage {
    /// Derive a percentage in the domain, truncating to whole basis points.
    pub fn from_minor_unit_ratio(numerator: MinorUnit, denominator: MinorUnit) -> Option<Self> {
        if denominator.0 <= 0 {
            return None;
        }

        // Preserve checked i64 scaling before division, including overflow omission.
        let basis_points = numerator.0.checked_mul(10_000)? / denominator.0;
        Some(Self(Decimal::new(basis_points, 2)))
    }
}

/// Encode a domain percentage as a nonnegative, bounded basis-point string.
pub struct BasisPointsPercentageForConnector;

impl BasisPointsPercentageForConnector {
    pub fn convert(&self, percentage: &Percentage, maximum_basis_points: u16) -> Option<String> {
        let basis_points = percentage.0.checked_mul(Decimal::from(100))?.to_i64()?;
        (0..=i64::from(maximum_basis_points))
            .contains(&basis_points)
            .then(|| basis_points.to_string())
    }
}

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

        let amount_string = match decimal_places {
            0 => amount_f64.0.to_string(),
            3 => format!("{:.3}", amount_f64.0),
            4 => format!("{:.4}", amount_f64.0),
            _ => format!("{:.2}", amount_f64.0),
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

/// An integer minor-unit amount carried by a connector request or response.
///
/// This is distinct from domain [`MinorUnit`]. Use
/// [`ConnectorMinorUnitForConnector`] to cross the conversion boundary. Its wire
/// representation remains a signed 64-bit integer.
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
pub struct ConnectorMinorUnit(i64);

impl ConnectorMinorUnit {
    /// Returns the integer minor-unit value for a connector payload.
    pub fn get_amount_as_i64(self) -> i64 {
        self.0
    }

    /// Creates a zero connector amount.
    pub fn zero() -> Self {
        Self(0)
    }

    /// Checks a connector amount against an integer minor-unit threshold.
    pub fn is_greater_than(&self, value: i64) -> bool {
        self.0 > value
    }
}

impl Display for ConnectorMinorUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
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
    /// Checks whether the converted amount is the canonical minor-unit zero.
    pub fn is_zero(&self) -> bool {
        self.0 == "0"
    }

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

    /// Checks whether the converted amount is positive.
    pub fn is_positive(&self) -> bool {
        self.0 > 0.0
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

impl crate::crypto::SignatureValue for StringMajorUnit {
    fn update_signature(&self, update: &mut dyn FnMut(&[u8])) {
        update(self.0.as_bytes());
    }
}

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
}

impl Display for StringMajorUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// The number of implied decimals [`StringTwoDecimalUnit`] always carries.
const TWO_DECIMAL_EXPONENT: u32 = 2;

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

/// An amount fixed at **exactly two** implied decimal places, whatever the currency's own
/// ISO 4217 exponent is.
///
/// The major amount rendered with two decimal places and the decimal point dropped, i.e.
/// `minor * 100 / 10^currency_exponent`. The scale is two for **every** currency,
/// including zero-exponent ones: USD 1234 and JPY 1234 serialize as `"1234"` and
/// `"123400"` respectively.
///
/// Use this for gateways that fix the scale of the amount field instead of taking the
/// currency's own exponent. Field constraints that belong to a particular gateway rather
/// than to the encoding — a maximum length, or a refusal to send a negative — are not
/// applied here; call [`Self::validate_max_len`] and [`Self::validate_unsigned`] at the
/// point of use so each connector's own rules stay visible in its transformer.
#[derive(Default, Debug, serde::Deserialize, serde::Serialize, Clone, PartialEq, Eq)]
pub struct StringTwoDecimalUnit(String);

impl StringTwoDecimalUnit {
    /// forms a new implied-decimal unit from amount
    fn new(value: String) -> Self {
        Self(value)
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

    /// Converts to minor unit as i64 from StringTwoDecimalUnit
    fn to_minor_unit_as_i64(
        &self,
        currency: enums::Currency,
    ) -> Result<MinorUnit, error_stack::Report<ParsingError>> {
        let wire = self.0.parse::<i128>().map_err(|_| {
            error_stack::report!(ParsingError::StructParseFailure("two-decimal amount"))
                .attach_printable(format!("`{}` is not an integer", self.0))
        })?;

        let scaled = wire * currency_scale(currency)?;
        let divisor = 10_i128.pow(TWO_DECIMAL_EXPONENT);
        if scaled % divisor != 0 {
            return Err(error_stack::report!(ParsingError::StructParseFailure(
                "two-decimal amount"
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

impl Display for StringTwoDecimalUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Connector required amount type
#[derive(Default, Debug, serde::Deserialize, serde::Serialize, Clone, Copy, PartialEq)]
pub struct StringTwoDecimalUnitForConnector;

impl AmountConvertor for StringTwoDecimalUnitForConnector {
    type Output = StringTwoDecimalUnit;

    /// `wire = minor * 100 / 10^currency_exponent`, kept in `i128` so there is neither a
    /// float hop nor an intermediate decimal string to re-parse.
    fn convert(&self, money: &Money) -> Result<Self::Output, error_stack::Report<ParsingError>> {
        let minor = i128::from(money.amount.get_amount_as_i64());
        let currency = money.currency;
        let scale = currency_scale(currency)?;
        let scaled = minor * 10_i128.pow(TWO_DECIMAL_EXPONENT);
        // A currency with more than two decimals cannot always be expressed with two
        // implied ones, so refuse rather than silently dropping the sub-unit digits.
        // Unlike a length or sign limit this is intrinsic to the encoding, so no caller
        // can meaningfully opt out of it.
        if scaled % scale != 0 {
            return Err(error_stack::report!(ParsingError::StructParseFailure(
                "two-decimal amount"
            ))
            .attach_printable(format!(
                "{currency:?} minor amount {minor} has non-zero digits below the two \
                 decimals this format can represent"
            )));
        }
        Ok(StringTwoDecimalUnit::new((scaled / scale).to_string()))
    }

    fn convert_back(
        &self,
        amount: Self::Output,
        currency: enums::Currency,
    ) -> Result<Money, error_stack::Report<ParsingError>> {
        Ok(Money::from_minor_unit(
            amount.to_minor_unit_as_i64(currency)?,
            currency,
        ))
    }
}

/// Domain amount and currency. Other crates can read these fields through methods,
/// but cannot build or mutate this value with a struct literal.
///
/// ```compile_fail
/// use ucs_common_utils::types::Money;
/// let _ = Money { amount: Default::default(), currency: Default::default() };
/// ```
#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, PartialEq, Eq, Hash, ToSchema)]
pub struct Money {
    amount: MinorUnit,
    currency: enums::Currency,
}

impl Money {
    /// Raw construction for domain mapping and legacy ingress only.
    /// Connector code must receive Money or use a response converter instead.
    pub fn from_minor_unit(amount: MinorUnit, currency: enums::Currency) -> Self {
        Self { amount, currency }
    }

    /// Creates a zero amount in the supplied currency.
    pub fn zero(currency: enums::Currency) -> Self {
        Self::from_minor_unit(MinorUnit::zero(), currency)
    }

    /// Creates one minor unit, not one major currency unit.
    pub fn one_minor_unit(currency: enums::Currency) -> Self {
        Self::from_minor_unit(MinorUnit::new(1), currency)
    }

    pub fn amount(&self) -> MinorUnit {
        self.amount
    }

    pub fn currency(&self) -> enums::Currency {
        self.currency
    }
}

/// A major-unit amount with exactly two decimal places and a literal decimal point,
/// whatever the currency's own ISO 4217 exponent is (for example, `"52.40"`).
///
/// Use this when the exact formatted string has semantic meaning, such as when it is
/// included in a connector signature. Unlike [`crate::types::StringTwoDecimalUnit`], this does not
/// drop the decimal point.
#[derive(Default, Debug, serde::Deserialize, serde::Serialize, Clone, PartialEq, Eq)]
pub struct StringMajorUnitWithTwoDecimals(String);

fn parse_major_unit_with_two_decimals(
    amount: &str,
) -> Result<i128, error_stack::Report<ParsingError>> {
    let malformed = || {
        error_stack::report!(ParsingError::StructParseFailure(
            "major-unit amount with two decimals"
        ))
        .attach_printable(format!("`{amount}` is not in major-with-2-decimals form"))
    };
    let (sign, digits) = match amount.strip_prefix('-') {
        Some(rest) => (-1_i128, rest),
        None => (1_i128, amount),
    };
    let (whole, fraction) = digits.split_once('.').ok_or_else(malformed)?;
    let is_digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if !is_digits(whole) || !is_digits(fraction) || fraction.len() != 2 {
        return Err(malformed());
    }
    let whole: i128 = whole.parse().map_err(|_| malformed())?;
    let fraction: i128 = fraction.parse().map_err(|_| malformed())?;
    whole
        .checked_mul(100)
        .and_then(|minor| minor.checked_add(fraction))
        .and_then(|minor| minor.checked_mul(sign))
        .ok_or_else(malformed)
}

impl StringMajorUnitWithTwoDecimals {
    fn new(value: String) -> Self {
        Self(value)
    }

    /// Reject a negative amount for gateways whose amount field is unsigned.
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

    fn to_minor_unit_as_i64(
        &self,
        currency: enums::Currency,
    ) -> Result<MinorUnit, error_stack::Report<ParsingError>> {
        let wire = parse_major_unit_with_two_decimals(&self.0)?;
        let scaled = wire
            .checked_mul(currency_scale(currency)?)
            .ok_or(ParsingError::DecimalToI64ConversionFailure)?;
        if scaled % 100 != 0 {
            return Err(error_stack::report!(ParsingError::StructParseFailure(
                "major-unit amount with two decimals"
            ))
            .attach_printable(format!(
                "`{}` is not a whole number of {currency:?} minor units",
                self.0
            )));
        }
        let minor =
            i64::try_from(scaled / 100).map_err(|_| ParsingError::DecimalToI64ConversionFailure)?;
        Ok(MinorUnit::new(minor))
    }
}

impl FromStr for StringMajorUnitWithTwoDecimals {
    type Err = error_stack::Report<ParsingError>;

    fn from_str(amount: &str) -> Result<Self, Self::Err> {
        parse_major_unit_with_two_decimals(amount)?;
        Ok(Self::new(amount.to_string()))
    }
}

/// Converts domain money to and from a major-unit string with exactly two decimals.
#[derive(Default, Debug, serde::Deserialize, serde::Serialize, Clone, Copy, PartialEq)]
pub struct StringMajorUnitWithTwoDecimalsForConnector;

impl AmountConvertor for StringMajorUnitWithTwoDecimalsForConnector {
    type Output = StringMajorUnitWithTwoDecimals;

    fn convert(&self, money: &Money) -> Result<Self::Output, error_stack::Report<ParsingError>> {
        let minor = i128::from(money.amount.get_amount_as_i64());
        let currency = money.currency;
        let scale = currency_scale(currency)?;
        let scaled = minor
            .checked_mul(10_i128.pow(TWO_DECIMAL_EXPONENT))
            .ok_or(ParsingError::DecimalToI64ConversionFailure)?;
        if scaled % scale != 0 {
            return Err(error_stack::report!(ParsingError::StructParseFailure(
                "major-unit amount with two decimals"
            ))
            .attach_printable(format!(
                "{currency:?} minor amount {minor} has non-zero digits below the two decimals \
                 this format can represent"
            )));
        }
        let wire = scaled / scale;
        let (sign, absolute) = if wire < 0 {
            ("-", wire.abs())
        } else {
            ("", wire)
        };
        Ok(StringMajorUnitWithTwoDecimals::new(format!(
            "{sign}{}.{:02}",
            absolute / 100,
            absolute % 100
        )))
    }

    fn convert_back(
        &self,
        amount: Self::Output,
        currency: enums::Currency,
    ) -> Result<Money, error_stack::Report<ParsingError>> {
        Ok(Money {
            amount: amount.to_minor_unit_as_i64(currency)?,
            currency,
        })
    }
}

impl crate::crypto::SignatureValue for StringMajorUnitWithTwoDecimals {
    fn update_signature(&self, update: &mut dyn FnMut(&[u8])) {
        update(self.0.as_bytes());
    }
}

#[cfg(test)]
mod test;
