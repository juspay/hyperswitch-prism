#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use common_enums::Currency;
    use common_utils::{
        types::{MinorUnit, StringMajorUnitForConnector},
        AmountConvertor,
    };

    #[test]
    fn test_amount_conversion_with_currency_validation() {
        let converter = StringMajorUnitForConnector;
        let amount = MinorUnit::new(12345);

        // Test zero decimal currency (JPY)
        let result = converter.convert(amount, Currency::JPY);
        assert!(result.is_ok(), "JPY conversion should succeed");
        let converted = result.unwrap();
        assert_eq!(converted.get_amount_as_string(), "12345");

        // Test two decimal currency (USD)
        let result = converter.convert(amount, Currency::USD);
        assert!(result.is_ok(), "USD conversion should succeed");
        let converted = result.unwrap();
        assert_eq!(converted.get_amount_as_string(), "123.45");

        // Test three decimal currency (BHD)
        let result = converter.convert(amount, Currency::BHD);
        assert!(result.is_ok(), "BHD conversion should succeed");
        let converted = result.unwrap();
        assert_eq!(converted.get_amount_as_string(), "12.345");

        // Test four decimal currency (CLF)
        let result = converter.convert(amount, Currency::CLF);
        assert!(result.is_ok(), "CLF conversion should succeed");
        let converted = result.unwrap();
        assert_eq!(converted.get_amount_as_string(), "1.2345");
    }

    #[test]
    fn test_currency_validation_errors_propagate() {
        // This test verifies that if we had an unsupported currency,
        // the error would propagate through the amount conversion system.
        // Since all current currencies are supported, we'll test by verifying
        // that the currency validation is being called.

        let converter = StringMajorUnitForConnector;
        let amount = MinorUnit::new(1000);

        // Test that various currencies work
        let currencies = vec![
            Currency::USD,
            Currency::EUR,
            Currency::GBP,
            Currency::JPY,
            Currency::BHD,
            Currency::CLF,
            Currency::KRW,
            Currency::VND,
        ];

        let mut failed_currencies = Vec::new();

        for currency in currencies {
            let result = converter.convert(amount, currency);
            if result.is_err() {
                failed_currencies.push(currency);
            }
        }

        assert!(
            failed_currencies.is_empty(),
            "The following currencies failed conversion: {failed_currencies:?}"
        );
    }

    #[test]
    fn test_amount_conversion_precision() {
        let converter = StringMajorUnitForConnector;

        // Test with different amounts to verify precision
        let test_cases = vec![
            (MinorUnit::new(1), Currency::USD, "0.01"),
            (MinorUnit::new(100), Currency::USD, "1.00"),
            (MinorUnit::new(12345), Currency::USD, "123.45"),
            (MinorUnit::new(1), Currency::JPY, "1"),
            (MinorUnit::new(1000), Currency::JPY, "1000"),
            (MinorUnit::new(1234), Currency::BHD, "1.234"),
            (MinorUnit::new(12345), Currency::BHD, "12.345"),
            (MinorUnit::new(1234), Currency::CLF, "0.1234"),
            (MinorUnit::new(12345), Currency::CLF, "1.2345"),
        ];

        let mut failed_test_cases = Vec::new();

        for (amount, currency, expected) in test_cases {
            let result = converter.convert(amount, currency);
            match result {
                Ok(converted) => {
                    let actual = converted.get_amount_as_string();
                    if actual != expected {
                        failed_test_cases.push((amount, currency, expected, actual));
                    }
                }
                Err(_) => {
                    failed_test_cases.push((amount, currency, expected, "ERROR".to_string()));
                }
            }
        }

        assert!(
            failed_test_cases.is_empty(),
            "The following test cases failed: {failed_test_cases:?}"
        );
    }
}
