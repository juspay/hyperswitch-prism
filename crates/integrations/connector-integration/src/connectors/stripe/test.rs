#[cfg(test)]
mod tests {
    mod redirect_return_url {
        use crate::connectors::stripe::transformers::{
            validate_redirect_return_url, StripePaymentMethodType,
        };

        #[test]
        fn redirect_payment_method_without_return_url_is_refused() {
            assert!(
                validate_redirect_return_url(Some(&StripePaymentMethodType::Ideal), None).is_err()
            );
            assert!(validate_redirect_return_url(
                Some(&StripePaymentMethodType::Klarna),
                Some("  ")
            )
            .is_err());
        }

        #[test]
        fn redirect_payment_method_with_return_url_is_allowed() {
            assert!(validate_redirect_return_url(
                Some(&StripePaymentMethodType::Ideal),
                Some("https://merchant.example/return"),
            )
            .is_ok());
        }

        #[test]
        fn non_redirect_payment_method_without_return_url_is_allowed() {
            assert!(
                validate_redirect_return_url(Some(&StripePaymentMethodType::Card), None).is_ok()
            );
            assert!(
                validate_redirect_return_url(Some(&StripePaymentMethodType::Sepa), None).is_ok()
            );
            assert!(validate_redirect_return_url(None, None).is_ok());
        }
    }
}
