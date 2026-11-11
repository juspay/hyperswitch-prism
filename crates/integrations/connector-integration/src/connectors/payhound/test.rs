#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
#[allow(clippy::panic)]
mod tests {
    use common_utils::{
        crypto::{self, SignMessage},
        types::{MinorUnit, Money},
    };
    use domain_types::{
        errors::IntegrationError,
        payment_method_data::{Card, CryptoData, DefaultPCIHolder, PaymentMethodData},
    };
    use hyperswitch_masking::Secret;

    use crate::connectors::payhound::{transformers::*, PayhoundAmountConvertor};

    /// Test vector 1 from `v1_authentication` — POST with a JSON body.
    const VECTOR_SECRET_POST: &str =
        "93yJJ8LBDe3zNSewHBdXlXIQDjCMDIn0EKNnXrd3kfzL72fvLz99uKnXFLYuCfkt";
    /// Test vector 2 from `v1_authentication` — GET with a query string.
    const VECTOR_SECRET_GET: &str =
        "M2NkN2EwZGI3NmZmOWRjYTQ4OTc5ZTI0YzM5YjQwOGMgIC0KM2NkN2EwZGI3NmZm";

    fn crypto_data(pay_currency: Option<&str>, network: Option<&str>) -> CryptoData {
        CryptoData {
            pay_currency: pay_currency.map(ToOwned::to_owned),
            network: network.map(ToOwned::to_owned),
        }
    }

    #[test]
    fn sign_post_vector() {
        let signature = payhound_request_signature(
            &Secret::new(VECTOR_SECRET_POST.to_owned()),
            "/api/v1/test",
            123,
            r#"{"attr1": 123, "attr2": "hello"}"#,
        )
        .expect("POST vector must sign");

        assert_eq!(
            signature,
            "f6cd63ea92d66baaa618cbf5ae0cc9c65bed284c93a91ebef079c747a930eb7e\
             497b5874658fb5557b42dfb7937c2fd273fef01205cae6a13ed068f0d0c4f735"
        );
    }

    #[test]
    fn sign_get_query_vector() {
        let signature = payhound_request_signature(
            &Secret::new(VECTOR_SECRET_GET.to_owned()),
            "/api/v1/info",
            4711,
            "first=this+is+a+field&second=was+it+clear+%28already%29%3F",
        )
        .expect("GET vector must sign");

        assert_eq!(
            signature,
            "24c2a83c15581c85de5b180716bd8e86467c089665d6ab51bd6e979815e9e740\
             a74a265d9b2aaee3db9146766583254d64280b1fbdf1e8cf91bf98ef09aff114"
        );
    }

    #[test]
    fn sign_get_empty_body_digest() {
        // UCS's PSync issues a GET with no query string, so the empty-`request_data` branch of the
        // signer is the one that actually runs in production.
        assert_eq!(
            payhound_sha256_hex(b"").expect("empty digest"),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn callback_signature_vector() {
        let message = payhound_callback_message("ABCDEFGH", br#"{"attr1": 123, "attr2": "hello"}"#)
            .expect("callback message");

        assert_eq!(
            String::from_utf8(message.clone()).expect("utf8"),
            "ABCDEFGH947753ba472927154c534cf2e4e11de27ed7a9560dc033e77d6cc24ee950ea56"
        );

        let signature = crypto::HmacSha512
            .sign_message(VECTOR_SECRET_POST.as_bytes(), &message)
            .map(hex::encode)
            .expect("callback signature");

        assert_eq!(
            signature,
            "545466159ca3fc8584ab1115b83614f45c2bc12bc596720579668cde289965eb\
             6fe4c3f33ae3fefbd942691aa1cf7b3c1f081740d0ae7d0936ceb7608b670146"
        );
    }

    #[test]
    fn nonce_is_strictly_increasing() {
        let mut previous = next_nonce().expect("first nonce");
        for _ in 0..10_000 {
            let current = next_nonce().expect("nonce");
            assert!(
                current > previous,
                "payhound nonce must strictly increase: {current} followed {previous}"
            );
            previous = current;
        }
    }

    /// Payhound rejects a replayed or out-of-order `X-MB-Nonce`, so the counter must never hand the
    /// same value to two callers even when several flows sign requests at the same instant — which
    /// is the normal case for a server under load, where many threads read an identical
    /// `now_micros`. Distinctness across threads is the property that matters: if it holds, the
    /// stronger per-caller ordering follows from the CAS loop in `next_nonce`.
    #[test]
    fn nonce_is_distinct_across_threads() {
        const THREADS: usize = 8;
        const PER_THREAD: usize = 2_000;

        let handles: Vec<_> = (0..THREADS)
            .map(|_| {
                std::thread::spawn(|| {
                    (0..PER_THREAD)
                        .map(|_| next_nonce().expect("nonce"))
                        .collect::<Vec<u64>>()
                })
            })
            .collect();

        let mut observed = std::collections::HashSet::new();
        for handle in handles {
            for nonce in handle.join().expect("nonce thread must not panic") {
                assert!(
                    observed.insert(nonce),
                    "payhound nonce {nonce} was handed out to more than one concurrent caller"
                );
            }
        }

        assert_eq!(
            observed.len(),
            THREADS * PER_THREAD,
            "every concurrent payhound nonce must be unique"
        );
    }

    fn sample_request(
        currency: common_enums::Currency,
        minor_amount: i64,
    ) -> PayhoundInvoiceRequest {
        PayhoundInvoiceRequest {
            currency: PayhoundSettlementCurrency::try_from(currency).expect("supported currency"),
            price: PayhoundAmountConvertor::convert(&Money {
                amount: MinorUnit::new(minor_amount),
                currency,
            })
            .expect("amount conversion"),
            invoice_currency: PayhoundInvoiceCurrency::Btc,
            name: None,
            description: None,
            reference: None,
            callback_url: None,
            success_url: None,
            cancel_url: None,
            share_to_keep_in_btc: None,
        }
    }

    #[test]
    fn amount_is_major_unit_string() {
        let body = serde_json::to_string(&sample_request(common_enums::Currency::EUR, 1000))
            .expect("serialize");
        assert!(
            body.contains(r#""price":"10.00""#),
            "expected a major-unit decimal string, got {body}"
        );
        assert!(
            !body.contains(r#""price":1000"#),
            "minor units must never be sent to Payhound, got {body}"
        );
    }

    #[test]
    fn optional_fields_are_omitted_not_null() {
        let body = serde_json::to_string(&sample_request(common_enums::Currency::EUR, 26645))
            .expect("serialize");
        assert!(
            !body.contains("null"),
            "absent optional fields must be omitted, not serialized as null: {body}"
        );
        assert_eq!(
            body,
            r#"{"currency":"EUR","price":"266.45","invoice_currency":"BTC"}"#
        );
    }

    #[test]
    fn crypto_currency_resolution() {
        assert_eq!(
            resolve_invoice_currency(&crypto_data(Some("USDT_TRC20"), None)).expect("symbol"),
            PayhoundInvoiceCurrency::UsdtTrc20
        );
        assert_eq!(
            resolve_invoice_currency(&crypto_data(Some("usdt_trc20"), None))
                .expect("case-insensitive symbol"),
            PayhoundInvoiceCurrency::UsdtTrc20
        );
        assert_eq!(
            resolve_invoice_currency(&crypto_data(Some("USDT"), Some("TRC20")))
                .expect("composed symbol"),
            PayhoundInvoiceCurrency::UsdtTrc20
        );
        assert_eq!(
            resolve_invoice_currency(&crypto_data(Some("USDT_TRC20"), Some("TRC20")))
                .expect("agreeing network"),
            PayhoundInvoiceCurrency::UsdtTrc20
        );
        assert!(
            resolve_invoice_currency(&crypto_data(Some("USDT_TRC20"), Some("ERC20"))).is_err(),
            "a conflicting network must be rejected, never silently ignored"
        );
        assert_eq!(
            resolve_invoice_currency(&crypto_data(Some("BTC"), None)).expect("btc"),
            PayhoundInvoiceCurrency::Btc
        );

        let unsupported = resolve_invoice_currency(&crypto_data(Some("DOGE"), None))
            .expect_err("DOGE is not a Payhound asset");
        match unsupported.current_context() {
            IntegrationError::NotSupported { context, .. } => assert!(
                context
                    .additional_context
                    .as_deref()
                    .is_some_and(|detail| detail.contains("USDT_TRC20")),
                "the error context must enumerate the supported symbols, got {context:?}"
            ),
            other => panic!("expected NotSupported, got {other:?}"),
        }

        let missing = resolve_invoice_currency(&crypto_data(None, None))
            .expect_err("pay_currency is required");
        match missing.current_context() {
            IntegrationError::MissingRequiredField { field_name, .. } => assert_eq!(
                *field_name, "crypto_data.pay_currency",
                "the missing-field error must name the caller-facing request path"
            ),
            other => panic!("expected MissingRequiredField, got {other:?}"),
        }
    }

    #[test]
    fn status_mapping() {
        use common_enums::AttemptStatus;
        let cases = [
            (
                PayhoundInvoiceStatus::Pending,
                AttemptStatus::AuthenticationPending,
            ),
            (PayhoundInvoiceStatus::Completed, AttemptStatus::Charged),
            (PayhoundInvoiceStatus::Overpaid, AttemptStatus::Charged),
            (PayhoundInvoiceStatus::Underpaid, AttemptStatus::Pending),
            (PayhoundInvoiceStatus::Aborted, AttemptStatus::Failure),
            (PayhoundInvoiceStatus::Timeout, AttemptStatus::Failure),
            (PayhoundInvoiceStatus::Unknown, AttemptStatus::Pending),
        ];
        for (payhound_status, expected) in cases {
            assert_eq!(
                AttemptStatus::from(payhound_status),
                expected,
                "unexpected mapping for {payhound_status:?}"
            );
        }
    }

    #[test]
    fn unknown_status_deserializes_to_unknown() {
        let parsed: PayhoundInvoiceStatus =
            serde_json::from_str(r#""brand_new""#).expect("unknown status must not fail to parse");
        assert_eq!(parsed, PayhoundInvoiceStatus::Unknown);
    }

    #[test]
    fn invoice_url_relative_is_resolved() {
        let resolved = build_invoice_redirect("/invoices/abc").expect("relative url");
        assert_eq!(resolved.as_str(), "https://pay.payhound.com/invoices/abc");
    }

    #[test]
    fn invoice_url_absolute_passes_through() {
        let resolved =
            build_invoice_redirect("https://pay.payhound.com/invoices/abc").expect("absolute url");
        assert_eq!(resolved.as_str(), "https://pay.payhound.com/invoices/abc");
    }

    #[test]
    fn error_envelope_maps_status_as_code() {
        let parsed: PayhoundErrorResponse =
            serde_json::from_str(r#"{"message":"Invalid signature"}"#).expect("error envelope");
        assert_eq!(parsed.message, "Invalid signature");
        assert_eq!(payhound_error_code(403), "403");
    }

    /// The real sandbox `201` body, verbatim.
    const LIVE_201_BODY: &str = r#"{
        "id": "9d8c0ade8d1f78048b4905821e44691b",
        "status": "pending",
        "address": "mrpNRa5PydTcTQBV2i5BpWaqN1DgofkoZB",
        "alt_addresses": [{"address": "tb1qexample", "type": "base58", "default": true}],
        "merchant_currency": "EUR", "merchant_amount": "20.00",
        "invoice_currency": "BTC", "invoice_amount": "0.00100000",
        "paid_currency": "BTC", "paid_amount": "0.0",
        "pending_currency": "BTC", "pending_amount": "0.0",
        "share_to_keep_in_btc": 0,
        "name": "Test Invoice", "description": "Lorem Ipsum Test Invoice",
        "reference": "TSTINCUSTOMREF",
        "sender_addresses": [],
        "invoice_url": "/invoices/9d8c0ade8d1f78048b4905821e44691b",
        "callback_url": null, "success_url": null, "cancel_url": null, "notify_email": null,
        "create_time": 1560168720.0, "valid_until_time": 1787591345.0,
        "max_risk_rating_of_related_txes": null
    }"#;

    #[test]
    fn float_timestamps_deserialize() {
        let parsed: PayhoundInvoiceResponse =
            serde_json::from_str(LIVE_201_BODY).expect("float timestamps must parse");
        assert_eq!(parsed.create_time, Some(1_560_168_720.0));
        assert_eq!(parsed.valid_until_time, Some(1_787_591_345.0));
    }

    #[test]
    fn live_201_body_round_trips() {
        let parsed: PayhoundInvoiceResponse =
            serde_json::from_str(LIVE_201_BODY).expect("live 201 body must parse");
        assert_eq!(parsed.id, "9d8c0ade8d1f78048b4905821e44691b");
        assert_eq!(parsed.status, PayhoundInvoiceStatus::Pending);
        assert_eq!(
            parsed
                .alt_addresses
                .as_ref()
                .map(|addresses| addresses.len()),
            Some(1)
        );
        assert!(parsed.max_risk_rating_of_related_txes.is_none());
        assert_eq!(
            parsed.invoice_url.as_deref(),
            Some("/invoices/9d8c0ade8d1f78048b4905821e44691b")
        );
    }

    #[test]
    fn capture_method_manual_is_rejected() {
        for capture_method in [
            common_enums::CaptureMethod::Manual,
            common_enums::CaptureMethod::ManualMultiple,
            common_enums::CaptureMethod::Scheduled,
        ] {
            let error = ensure_supported_capture_method(Some(capture_method))
                .expect_err("Payhound cannot settle or release a manual-capture authorization");
            assert!(
                matches!(
                    error.current_context(),
                    IntegrationError::CaptureMethodNotSupported { .. }
                ),
                "expected CaptureMethodNotSupported for {capture_method:?}, got {error:?}"
            );
        }
        for capture_method in [
            Some(common_enums::CaptureMethod::Automatic),
            Some(common_enums::CaptureMethod::SequentialAutomatic),
            None,
        ] {
            assert!(ensure_supported_capture_method(capture_method).is_ok());
        }
    }

    #[test]
    fn non_crypto_payment_method_is_rejected() {
        let error = invoice_currency_from_payment_method(&PaymentMethodData::Card(Card::<
            DefaultPCIHolder,
        >::default(
        )))
        .expect_err("Payhound accepts only PaymentMethodData::Crypto");
        match error.current_context() {
            IntegrationError::NotSupported {
                connector, context, ..
            } => {
                assert_eq!(*connector, PAYHOUND_CONNECTOR_DISPLAY);
                assert!(
                    context
                        .additional_context
                        .as_deref()
                        .is_some_and(|detail| detail.contains("hosted crypto invoice gateway")),
                    "the rejection must carry Payhound-specific context, got {context:?}"
                );
            }
            other => panic!("expected NotSupported, got {other:?}"),
        }
    }

    #[test]
    fn unsupported_settlement_currency_is_rejected() {
        let error = PayhoundSettlementCurrency::try_from(common_enums::Currency::AFN)
            .expect_err("AFN is not a Payhound settlement currency");
        match error.current_context() {
            IntegrationError::CurrencyNotSupported {
                message, context, ..
            } => {
                assert_eq!(message, "AFN");
                assert!(
                    context
                        .additional_context
                        .as_deref()
                        .is_some_and(|detail| detail.contains("EUR")),
                    "the error context must enumerate the supported settlement currencies, got \
                     {context:?}"
                );
            }
            other => panic!("expected CurrencyNotSupported, got {other:?}"),
        }
    }
}
