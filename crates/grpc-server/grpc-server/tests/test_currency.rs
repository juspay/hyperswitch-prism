#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::print_stdout)]
mod tests {
    use common_enums::{Currency, CurrencyError};

    #[test]
    fn test_zero_decimal_currencies() {
        // Test currencies that should have 0 decimal places
        assert_eq!(
            Currency::JPY
                .number_of_digits_after_decimal_point()
                .unwrap(),
            0
        );
        assert_eq!(
            Currency::KRW
                .number_of_digits_after_decimal_point()
                .unwrap(),
            0
        );
        assert_eq!(
            Currency::VND
                .number_of_digits_after_decimal_point()
                .unwrap(),
            0
        );
        assert_eq!(
            Currency::BIF
                .number_of_digits_after_decimal_point()
                .unwrap(),
            0
        );
        assert_eq!(
            Currency::CLP
                .number_of_digits_after_decimal_point()
                .unwrap(),
            0
        );
    }

    #[test]
    fn test_two_decimal_currencies() {
        // Test currencies that should have 2 decimal places
        assert_eq!(
            Currency::USD
                .number_of_digits_after_decimal_point()
                .unwrap(),
            2
        );
        assert_eq!(
            Currency::EUR
                .number_of_digits_after_decimal_point()
                .unwrap(),
            2
        );
        assert_eq!(
            Currency::GBP
                .number_of_digits_after_decimal_point()
                .unwrap(),
            2
        );
        assert_eq!(
            Currency::CAD
                .number_of_digits_after_decimal_point()
                .unwrap(),
            2
        );
        assert_eq!(
            Currency::AUD
                .number_of_digits_after_decimal_point()
                .unwrap(),
            2
        );
    }

    #[test]
    fn test_three_decimal_currencies() {
        // Test currencies that should have 3 decimal places
        assert_eq!(
            Currency::BHD
                .number_of_digits_after_decimal_point()
                .unwrap(),
            3
        );
        assert_eq!(
            Currency::JOD
                .number_of_digits_after_decimal_point()
                .unwrap(),
            3
        );
        assert_eq!(
            Currency::KWD
                .number_of_digits_after_decimal_point()
                .unwrap(),
            3
        );
        assert_eq!(
            Currency::OMR
                .number_of_digits_after_decimal_point()
                .unwrap(),
            3
        );
        assert_eq!(
            Currency::TND
                .number_of_digits_after_decimal_point()
                .unwrap(),
            3
        );
    }

    #[test]
    fn test_four_decimal_currencies() {
        // Test currencies that should have 4 decimal places
        assert_eq!(
            Currency::CLF
                .number_of_digits_after_decimal_point()
                .unwrap(),
            4
        );
    }

    #[test]
    fn test_currency_classification_completeness() {
        // Test that all currencies in the enum are properly classified
        let mut tested_currencies = 0;
        let mut successful_classifications = 0;

        // We'll iterate through some key currencies to verify they're all classified
        let test_currencies = vec![
            Currency::USD,
            Currency::EUR,
            Currency::GBP,
            Currency::JPY,
            Currency::KRW,
            Currency::BHD,
            Currency::JOD,
            Currency::CLF,
            Currency::CNY,
            Currency::INR,
            Currency::CAD,
            Currency::AUD,
            Currency::CHF,
            Currency::SEK,
            Currency::NOK,
            Currency::DKK,
            Currency::PLN,
            Currency::CZK,
            Currency::HUF,
            Currency::RUB,
        ];

        let mut failed_currencies = Vec::new();

        for currency in test_currencies {
            tested_currencies += 1;
            match currency.number_of_digits_after_decimal_point() {
                Ok(_) => successful_classifications += 1,
                Err(_) => {
                    failed_currencies.push(currency);
                    println!("❌ Currency {currency:?} not properly classified");
                }
            }
        }

        // Fail the test if any currencies failed
        assert!(
            failed_currencies.is_empty(),
            "The following currencies are not properly classified: {failed_currencies:?}"
        );

        println!("✅ Tested {tested_currencies} currencies, {successful_classifications} successful classifications");
        assert_eq!(
            tested_currencies, successful_classifications,
            "All tested currencies should be properly classified"
        );
    }

    #[test]
    fn test_currency_error_message() {
        // Since all current currencies should be classified, we can't easily test
        // the error case without adding a fake currency. Instead, let's verify
        // the error type exists and can be created
        let error = CurrencyError::UnsupportedCurrency {
            currency: "TEST".to_string(),
        };
        let error_string = format!("{error}");
        assert!(error_string.contains("Unsupported currency: TEST"));
        assert!(error_string.contains("Please add this currency to the supported currency list"));
    }

    #[test]
    fn test_comprehensive_currency_coverage() {
        // Test a representative sample from each classification
        let currencies_to_test = vec![
            // Zero decimal currencies
            (Currency::BIF, 0),
            (Currency::CLP, 0),
            (Currency::DJF, 0),
            (Currency::GNF, 0),
            (Currency::JPY, 0),
            (Currency::KMF, 0),
            (Currency::KRW, 0),
            (Currency::MGA, 0),
            (Currency::PYG, 0),
            (Currency::RWF, 0),
            (Currency::UGX, 0),
            (Currency::VND, 0),
            (Currency::VUV, 0),
            (Currency::XAF, 0),
            (Currency::XOF, 0),
            (Currency::XPF, 0),
            // Three decimal currencies
            (Currency::BHD, 3),
            (Currency::JOD, 3),
            (Currency::KWD, 3),
            (Currency::OMR, 3),
            (Currency::TND, 3),
            // Four decimal currencies
            (Currency::CLF, 4),
            // Two decimal currencies (sample)
            (Currency::USD, 2),
            (Currency::EUR, 2),
            (Currency::GBP, 2),
            (Currency::AED, 2),
            (Currency::AFN, 2),
            (Currency::ALL, 2),
            (Currency::AMD, 2),
            (Currency::ANG, 2),
            (Currency::AOA, 2),
        ];

        for (currency, expected_decimals) in currencies_to_test {
            match currency.number_of_digits_after_decimal_point() {
                Ok(decimals) => {
                    assert_eq!(decimals, expected_decimals,
                              "Currency {currency:?} should have {expected_decimals} decimals, got {decimals}");
                }
                Err(e) => {
                    panic!("Currency {currency:?} should be classified but got error: {e}");
                }
            }
        }
    }
}
