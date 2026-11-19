#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use domain_types::connector_types::GpayTransactionInfo;

    use crate::connectors::trustpay::transformers::GooglePayTransactionInfo;

    fn transaction_info(total_price: &str) -> GooglePayTransactionInfo {
        serde_json::from_value(serde_json::json!({
            "countryCode": "US",
            "currencyCode": "USD",
            "totalPriceStatus": "FINAL",
            "totalPrice": total_price,
        }))
        .unwrap()
    }

    #[test]
    fn google_pay_total_price_converts_to_minor_units() {
        let info = GpayTransactionInfo::try_from(transaction_info("10.00")).unwrap();
        assert_eq!(info.total_price.get_amount_as_i64(), 1000);
    }

    #[test]
    fn unconvertible_google_pay_total_price_is_an_error_not_zero() {
        assert!(GpayTransactionInfo::try_from(transaction_info("not-a-number")).is_err());
    }
}
