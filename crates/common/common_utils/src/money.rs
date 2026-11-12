//! Shared money encoding and conversion for connector wire formats.
use crate::{
    errors::ParsingError,
    types::{currency_scale, AmountConvertor, MinorUnit, Money},
};
use common_enums as enums;
use std::str::FromStr;
const TWO_DECIMAL_EXPONENT: u32 = 2;

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
mod tests {
    use super::*;

    #[test]
    fn exact_decimal_conversion_and_signature_bytes() -> Result<(), Box<dyn std::error::Error>> {
        let converter = StringMajorUnitWithTwoDecimalsForConnector;
        for (currency, minor, wire) in [
            (enums::Currency::MYR, 5240, "52.40"),
            (enums::Currency::MYR, -1, "-0.01"),
            (enums::Currency::MYR, 0, "0.00"),
            (enums::Currency::JPY, 1234, "1234.00"),
            (enums::Currency::KWD, 12340, "12.34"),
            (enums::Currency::MYR, i64::MAX, "92233720368547758.07"),
            (enums::Currency::MYR, i64::MIN, "-92233720368547758.08"),
        ] {
            let money = Money {
                amount: MinorUnit::new(minor),
                currency,
            };
            let amount = converter.convert(&money)?;
            assert_eq!(serde_json::to_value(&amount)?, wire);
            let mut signed = Vec::new();
            crate::crypto::update_signature(&[&amount], |bytes| signed.extend_from_slice(bytes));
            assert_eq!(signed, wire.as_bytes());
            assert_eq!(converter.convert_back(amount, currency)?, money);
        }
        assert!(converter
            .convert(&Money {
                amount: MinorUnit::new(12341),
                currency: enums::Currency::KWD
            })
            .is_err());
        Ok(())
    }

    #[test]
    fn parser_rejects_malformed_and_overflowing_amounts() {
        for amount in [
            "",
            "1",
            "1.0",
            "1.000",
            "+1.00",
            ".00",
            "1.a0",
            "1.00.00",
            "170141183460469231731687303715884105728.00",
        ] {
            assert!(
                amount.parse::<StringMajorUnitWithTwoDecimals>().is_err(),
                "{amount}"
            );
        }
    }
}
