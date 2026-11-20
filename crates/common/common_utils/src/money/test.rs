use super::*;

#[test]
#[allow(clippy::expect_used)]
fn exact_decimal_conversion_and_signature_bytes() {
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
        let amount = converter.convert(&money).expect("exact amount conversion");
        assert_eq!(
            serde_json::to_value(&amount).expect("amount serialization"),
            wire
        );
        let mut signed = Vec::new();
        crate::crypto::update_signature(&[&amount], |bytes| signed.extend_from_slice(bytes));
        assert_eq!(signed, wire.as_bytes());
        assert_eq!(
            converter
                .convert_back(amount, currency)
                .expect("amount round trip"),
            money
        );
    }
    assert!(converter
        .convert(&Money {
            amount: MinorUnit::new(12341),
            currency: enums::Currency::KWD
        })
        .is_err());
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
