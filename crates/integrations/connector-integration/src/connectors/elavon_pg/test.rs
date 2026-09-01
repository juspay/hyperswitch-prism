#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
#[allow(clippy::panic)]
#[allow(clippy::indexing_slicing)]
mod tests {
    use common_enums::{AttemptStatus, Currency, RefundStatus};
    use common_utils::types::{AmountConvertor, MinorUnit, StringMajorUnitForConnector};
    use hyperswitch_masking::Secret;

    use crate::connectors::elavon_pg::transformers::{
        elavon_pg_payment_session_href, elavon_pg_resource_reference, summarize_failures,
        ElavonPgAmount, ElavonPgAuthorizeRequest, ElavonPgCaptureRequest, ElavonPgCaptureResponse,
        ElavonPgCard, ElavonPgCardSaleRequest, ElavonPgChildStatus, ElavonPgContact,
        ElavonPgCreateOrderRequest, ElavonPgCreateOrderResponse, ElavonPgErrorResponse,
        ElavonPgFailure, ElavonPgPaymentSessionSaleRequest, ElavonPgPreAuthenticateRequest,
        ElavonPgPreAuthenticateResponse, ElavonPgSaleStatus, ElavonPgShopperInteraction,
        ElavonPgThreeDSecure, ElavonPgThreeDsTransactionStatus, ElavonPgTransactionResponse,
        ElavonPgTransactionState, ElavonPgTransactionType,
    };
    use domain_types::payment_method_data::{DefaultPCIHolder, RawCardNumber};

    // ---------------------------------------------------------------------
    // Amount conversion — EPG carries amounts as decimal strings in MAJOR units
    // ---------------------------------------------------------------------

    #[test]
    fn amount_is_serialized_as_a_major_unit_string() {
        let converted = StringMajorUnitForConnector
            .convert(MinorUnit::new(1000), Currency::EUR)
            .unwrap();
        let amount = ElavonPgAmount {
            amount: converted,
            currency_code: Currency::EUR,
        };
        assert_eq!(
            serde_json::to_value(&amount).unwrap(),
            serde_json::json!({ "amount": "10.00", "currencyCode": "EUR" })
        );
    }

    #[test]
    fn zero_decimal_currency_amount_has_no_fraction() {
        let converted = StringMajorUnitForConnector
            .convert(MinorUnit::new(1500), Currency::JPY)
            .unwrap();
        let amount = ElavonPgAmount {
            amount: converted,
            currency_code: Currency::JPY,
        };
        assert_eq!(
            serde_json::to_value(&amount).unwrap(),
            serde_json::json!({ "amount": "1500", "currencyCode": "JPY" })
        );
    }

    // ---------------------------------------------------------------------
    // Serialization round-trips — the exact EPG wire shape
    // ---------------------------------------------------------------------

    fn sample_card() -> ElavonPgCard<DefaultPCIHolder> {
        ElavonPgCard {
            holder_name: Some(Secret::new("John Doe".to_string())),
            number: RawCardNumber(
                cards::CardNumber::try_from("4546341111111119".to_string()).unwrap(),
            ),
            expiration_month: 12,
            expiration_year: 2030,
            security_code: Secret::new("123".to_string()),
            bill_to: Some(ElavonPgContact {
                full_name: Some(Secret::new("John Doe".to_string())),
                street1: Some(Secret::new("221 Baker St".to_string())),
                street2: None,
                city: Some(Secret::new("London".to_string())),
                region: Some(Secret::new("England".to_string())),
                postal_code: Some(Secret::new("NW1 6XE".to_string())),
                country_code: Some(common_enums::CountryAlpha3::GBR),
                email: None,
            }),
        }
    }

    fn sample_request(
        three_d_secure: Option<ElavonPgThreeDSecure>,
    ) -> ElavonPgAuthorizeRequest<DefaultPCIHolder> {
        ElavonPgAuthorizeRequest::Card(Box::new(ElavonPgCardSaleRequest {
            transaction_type: ElavonPgTransactionType::Sale,
            total: ElavonPgAmount {
                amount: StringMajorUnitForConnector
                    .convert(MinorUnit::new(1000), Currency::EUR)
                    .unwrap(),
                currency_code: Currency::EUR,
            },
            card: sample_card(),
            shopper_interaction: ElavonPgShopperInteraction::Ecommerce,
            do_capture: true,
            three_d_secure,
            shopper_email_address: None,
            shopper_ip_address: None,
            custom_reference: Some("pay_1J2k3l4m5n6o".to_string()),
            description: None,
        }))
    }

    #[test]
    fn no_three_ds_sale_omits_three_d_secure_and_every_absent_optional() {
        let body = serde_json::to_value(sample_request(None)).unwrap();
        let object = body.as_object().unwrap();

        // No `null`s anywhere: every absent Option is skipped, not emitted.
        assert!(!object.contains_key("threeDSecure"));
        assert!(!object.contains_key("shopperEmailAddress"));
        assert!(!object.contains_key("shopperIpAddress"));
        assert!(!object.contains_key("description"));
        assert!(!object.values().any(serde_json::Value::is_null));
        assert!(!body["card"]["billTo"]
            .as_object()
            .unwrap()
            .values()
            .any(serde_json::Value::is_null));

        assert_eq!(body["type"], "sale");
        assert_eq!(body["shopperInteraction"], "ecommerce");
        assert_eq!(body["doCapture"], serde_json::json!(true));
        assert_eq!(body["total"]["amount"], "10.00");
        assert_eq!(body["total"]["currencyCode"], "EUR");
        assert_eq!(body["customReference"], "pay_1J2k3l4m5n6o");
        // Card number is bare digits; expiry is a pair of JSON integers.
        assert_eq!(body["card"]["number"], "4546341111111119");
        assert_eq!(body["card"]["expirationMonth"], serde_json::json!(12));
        assert_eq!(body["card"]["expirationYear"], serde_json::json!(2030));
        assert_eq!(body["card"]["securityCode"], "123");
        // Country is ISO 3166-1 alpha-3, never alpha-2.
        assert_eq!(body["card"]["billTo"]["countryCode"], "GBR");
    }

    #[test]
    fn three_ds_sale_carries_the_full_three_d_secure_object() {
        let body = serde_json::to_value(sample_request(Some(ElavonPgThreeDSecure {
            directory_server_transaction_id: "88093c16-4659-4b23-bc84-b5a790779107".to_string(),
            transaction_status: ElavonPgThreeDsTransactionStatus::Y,
            trans_status_reason: None,
            electronic_commerce_indicator: Some("05".to_string()),
            authentication_value: Some(Secret::new("DO+j0b3yB6NR9vJ+BO6O099GvzY=".to_string())),
            protocol_version: "2.1.0".to_string(),
        })))
        .unwrap();

        let three_ds = body["threeDSecure"].as_object().unwrap();
        assert_eq!(
            three_ds["directoryServerTransactionId"],
            "88093c16-4659-4b23-bc84-b5a790779107"
        );
        assert_eq!(three_ds["transactionStatus"], "Y");
        assert_eq!(three_ds["protocolVersion"], "2.1.0");
        assert_eq!(three_ds["electronicCommerceIndicator"], "05");
        assert_eq!(
            three_ds["authenticationValue"],
            "DO+j0b3yB6NR9vJ+BO6O099GvzY="
        );
        // `transStatusReason` was absent, so it must not be emitted as null.
        assert!(!three_ds.contains_key("transStatusReason"));
    }

    // ---------------------------------------------------------------------
    // Hosted payment page (gateway 3DS) — CreateOrder / PreAuthenticate / settle
    // ---------------------------------------------------------------------

    const ORDER_HREF: &str =
        "https://api.sandbox.elavonpayments.com/orders/6xxFwvM8BqmM6T6DcF3DyTB3";
    const PAYMENT_SESSION_HREF: &str =
        "https://api.sandbox.elavonpayments.com/payment-sessions/rd8y9xhx7qh9yj6r4vpxpqcv";

    #[test]
    fn create_order_body_carries_nothing_but_the_total() {
        let body = serde_json::to_value(ElavonPgCreateOrderRequest {
            total: ElavonPgAmount {
                amount: StringMajorUnitForConnector
                    .convert(MinorUnit::new(1000), Currency::EUR)
                    .unwrap(),
                currency_code: Currency::EUR,
            },
        })
        .unwrap();

        assert_eq!(
            body,
            serde_json::json!({ "total": { "amount": "10.00", "currencyCode": "EUR" } })
        );
    }

    #[test]
    fn payment_session_body_types_the_switches_as_booleans_and_omits_hpp_type() {
        let body = serde_json::to_value(ElavonPgPreAuthenticateRequest {
            order: ORDER_HREF.to_string(),
            return_url: "https://merchant.example.com/redirect/complete".to_string(),
            cancel_url: "https://merchant.example.com/return".to_string(),
            do_three_d_secure: true,
            do_create_transaction: false,
        })
        .unwrap();

        assert_eq!(
            body,
            serde_json::json!({
                "order": ORDER_HREF,
                "returnUrl": "https://merchant.example.com/redirect/complete",
                "cancelUrl": "https://merchant.example.com/return",
                "doThreeDSecure": true,
                "doCreateTransaction": false,
            })
        );
        // The published doc example writes doCreateTransaction as the string "true";
        // the OpenAPI schema types it boolean, and the schema is what EPG validates.
        assert!(body["doCreateTransaction"].is_boolean());
        assert!(body["doThreeDSecure"].is_boolean());
        // hppType defaults to fullPageRedirect, so it is not sent.
        assert!(!body.as_object().unwrap().contains_key("hppType"));
    }

    #[test]
    fn payment_session_settle_body_is_only_the_session_url() {
        let body = serde_json::to_value(
            ElavonPgAuthorizeRequest::<DefaultPCIHolder>::PaymentSession(
                ElavonPgPaymentSessionSaleRequest {
                    payment_session: PAYMENT_SESSION_HREF.to_string(),
                },
            ),
        )
        .unwrap();

        assert_eq!(
            body,
            serde_json::json!({ "paymentSession": PAYMENT_SESSION_HREF })
        );
        // EPG documents that nothing else belongs on this call.
        let object = body.as_object().unwrap();
        assert!(!object.contains_key("card"));
        assert!(!object.contains_key("threeDSecure"));
        assert!(!object.contains_key("hostedCard"));
        assert!(!object.contains_key("type"));
    }

    #[test]
    fn the_card_sale_body_is_untouched_by_the_untagged_authorize_enum() {
        // The `Card` variant must serialize exactly like the old struct did: the two
        // already-certified card paths are wire-compatible only if this holds.
        let card_body = serde_json::to_value(sample_request(None)).unwrap();
        assert_eq!(card_body["type"], "sale");
        assert_eq!(card_body["card"]["number"], "4546341111111119");
        assert!(!card_body
            .as_object()
            .unwrap()
            .contains_key("paymentSession"));
    }

    #[test]
    fn only_a_payment_session_url_is_read_back_as_a_hosted_session() {
        use domain_types::router_request_types::AuthenticationData;

        fn authentication_data(threeds_server_transaction_id: Option<&str>) -> AuthenticationData {
            AuthenticationData {
                threeds_server_transaction_id: threeds_server_transaction_id.map(str::to_owned),
                trans_status: None,
                eci: None,
                cavv: None,
                ucaf_collection_indicator: None,
                message_version: None,
                ds_trans_id: None,
                acs_transaction_id: None,
                transaction_id: None,
                network_params: None,
                exemption_indicator: None,
                created_at: None,
                challenge_code: None,
                challenge_cancel: None,
                challenge_code_reason: None,
                message_extension: None,
                authentication_type: None,
            }
        }

        // The hosted-session href round-trips.
        assert_eq!(
            elavon_pg_payment_session_href(Some(&authentication_data(Some(PAYMENT_SESSION_HREF)))),
            Some(PAYMENT_SESSION_HREF.to_string())
        );

        // A real external-3DS server transaction id must NOT be mistaken for one —
        // that is what keeps the pass-through 3DS Authorize body unchanged.
        assert_eq!(
            elavon_pg_payment_session_href(Some(&authentication_data(Some(
                "88093c16-4659-4b23-bc84-b5a790779107"
            )))),
            None
        );
        // Neither may some other EPG resource URL.
        assert_eq!(
            elavon_pg_payment_session_href(Some(&authentication_data(Some(ORDER_HREF)))),
            None
        );
        assert_eq!(
            elavon_pg_payment_session_href(Some(&authentication_data(None))),
            None
        );
        assert_eq!(elavon_pg_payment_session_href(None), None);
    }

    #[test]
    fn a_resource_reference_prefers_the_href_and_degrades_to_the_id() {
        assert_eq!(
            elavon_pg_resource_reference(
                Some(ORDER_HREF.to_string()),
                "6xxFwvM8BqmM6T6DcF3DyTB3",
                "Order"
            ),
            ORDER_HREF
        );
        // EPG parses a reference as either an href or a bare id, so an absent self
        // link is still usable.
        assert_eq!(
            elavon_pg_resource_reference(None, "6xxFwvM8BqmM6T6DcF3DyTB3", "Order"),
            "6xxFwvM8BqmM6T6DcF3DyTB3"
        );
    }

    #[test]
    fn payment_session_response_deserializes_with_and_without_the_shopper_url() {
        let with_url: ElavonPgPreAuthenticateResponse = serde_json::from_str(
            r#"{
                "id": "rd8y9xhx7qh9yj6r4vpxpqcv",
                "href": "https://api.sandbox.elavonpayments.com/payment-sessions/rd8y9xhx7qh9yj6r4vpxpqcv",
                "hppType": "fullPageRedirect",
                "doThreeDSecure": true,
                "expiresAt": "2026-09-02T13:11:23.123Z",
                "url": "https://hpp.sandbox.elavonpayments.com/rd8y9xhx7qh9yj6r4vpxpqcv"
            }"#,
        )
        .unwrap();
        assert_eq!(
            with_url.url.as_deref(),
            Some("https://hpp.sandbox.elavonpayments.com/rd8y9xhx7qh9yj6r4vpxpqcv")
        );

        // EPG's own published 201 example omits `url`; the response transformer turns
        // that into an actionable error rather than a guessed redirect target.
        let without_url: ElavonPgPreAuthenticateResponse = serde_json::from_str(
            r#"{ "id": "rd8y9xhx7qh9yj6r4vpxpqcv", "hppType": "fullPageRedirect" }"#,
        )
        .unwrap();
        assert!(without_url.url.is_none());
        assert!(without_url.href.is_none());
    }

    #[test]
    fn order_response_deserializes_leniently() {
        let order: ElavonPgCreateOrderResponse = serde_json::from_str(
            r#"{
                "id": "6xxFwvM8BqmM6T6DcF3DyTB3",
                "href": "https://api.sandbox.elavonpayments.com/orders/6xxFwvM8BqmM6T6DcF3DyTB3",
                "createdAt": "2026-09-02T13:01:23.123Z",
                "total": { "amount": "10.00", "currencyCode": "EUR" }
            }"#,
        )
        .unwrap();
        assert_eq!(order.id, "6xxFwvM8BqmM6T6DcF3DyTB3");
        assert_eq!(order.href.as_deref(), Some(ORDER_HREF));
    }

    #[test]
    fn capture_body_states_the_amount_explicitly() {
        assert_eq!(
            serde_json::to_value(ElavonPgCaptureRequest {
                transaction: "bdf87mqhtj4rjpvhy49cvdpp".to_string(),
                total: ElavonPgAmount {
                    amount: StringMajorUnitForConnector
                        .convert(MinorUnit::new(500), Currency::EUR)
                        .unwrap(),
                    currency_code: Currency::EUR,
                },
                is_final: true,
            })
            .unwrap(),
            serde_json::json!({
                "transaction": "bdf87mqhtj4rjpvhy49cvdpp",
                "total": { "amount": "5.00", "currencyCode": "EUR" },
                "isFinal": true
            })
        );
    }

    #[test]
    fn transaction_response_deserializes_leniently() {
        // Verbatim shape from the EPG docs, trimmed, plus fields that are not in the
        // published OpenAPI document — deserialization must tolerate both.
        let raw = serde_json::json!({
            "href": "https://api.sandbox.elavonpayments.com/transactions/h67d6hjb8866bpc3kddrgtpjmjvq",
            "id": "h67d6hjb8866bpc3kddrgtpjmjvq",
            "type": "sale",
            "total": { "amount": "10.00", "currencyCode": "EUR" },
            "processorReference": "HT47QCT5196",
            "isHeldForReview": false,
            "doCapture": true,
            "isAuthorized": true,
            "authorizationCode": "511653",
            "verificationResults": { "threeDSecureV1": "unprovided" },
            "modifiedBy": "someone",
            "state": "authorized",
            "failures": []
        });
        let parsed: ElavonPgTransactionResponse = serde_json::from_value(raw).unwrap();
        assert_eq!(parsed.id, "h67d6hjb8866bpc3kddrgtpjmjvq");
        assert_eq!(parsed.state, ElavonPgTransactionState::Authorized);
        assert_eq!(parsed.is_authorized, Some(true));
        assert_eq!(parsed.do_capture, Some(true));
        assert_eq!(parsed.transaction_type, Some(ElavonPgTransactionType::Sale));
    }

    #[test]
    fn unrecognised_enum_values_do_not_break_deserialization() {
        let raw = serde_json::json!({
            "id": "abc",
            "type": "somethingEpgAddedLater",
            "state": "aStateEpgAddedLater"
        });
        let parsed: ElavonPgTransactionResponse = serde_json::from_value(raw).unwrap();
        assert_eq!(parsed.state, ElavonPgTransactionState::Unrecognized);
        assert_eq!(
            parsed.transaction_type,
            Some(ElavonPgTransactionType::Unrecognized)
        );
    }

    #[test]
    fn partial_capture_response_deserializes_into_the_shared_capture_struct() {
        let raw = serde_json::json!({
            "id": "kd8djwg49k4pdkcyyhbpb9tf",
            "transaction": "https://api.sandbox.elavonpayments.com/transactions/bdf87mqhtj4rjpvhy49cvdpp",
            "total": { "amount": "5.00", "currencyCode": "EUR" },
            "isFinal": true,
            "state": "captured"
        });
        let parsed: ElavonPgCaptureResponse = serde_json::from_value(raw).unwrap();
        assert_eq!(parsed.state, ElavonPgTransactionState::Captured);
        // A PartialCapture resource simply has no isAuthorized field.
        assert_eq!(parsed.is_authorized, None);
    }

    // ---------------------------------------------------------------------
    // Status mapping
    // ---------------------------------------------------------------------

    fn sale_status(
        state: ElavonPgTransactionState,
        is_authorized: Option<bool>,
        is_held_for_review: bool,
        is_auto_capture: bool,
    ) -> AttemptStatus {
        AttemptStatus::from(ElavonPgSaleStatus {
            state: &state,
            is_authorized,
            is_held_for_review,
            is_auto_capture,
        })
    }

    #[test]
    fn authorized_is_charged_on_auto_capture_and_authorized_on_manual_capture() {
        // The single most likely bug in this connector: EPG reports `authorized`
        // for both, and only `doCapture` separates them.
        assert_eq!(
            sale_status(
                ElavonPgTransactionState::Authorized,
                Some(true),
                false,
                true
            ),
            AttemptStatus::Charged
        );
        assert_eq!(
            sale_status(
                ElavonPgTransactionState::Authorized,
                Some(true),
                false,
                false
            ),
            AttemptStatus::Authorized
        );
    }

    #[test]
    fn held_for_review_is_pending_however_it_is_reported() {
        assert_eq!(
            sale_status(ElavonPgTransactionState::Authorized, Some(true), true, true),
            AttemptStatus::Pending
        );
        assert_eq!(
            sale_status(ElavonPgTransactionState::HeldForReview, None, false, true),
            AttemptStatus::Pending
        );
    }

    #[test]
    fn terminal_sale_states_map_to_their_outcome() {
        assert_eq!(
            sale_status(
                ElavonPgTransactionState::Authorized,
                Some(false),
                false,
                true
            ),
            AttemptStatus::Failure
        );
        for state in [
            ElavonPgTransactionState::Captured,
            ElavonPgTransactionState::Settled,
            ElavonPgTransactionState::SettlementDelayed,
        ] {
            assert_eq!(
                sale_status(state, Some(true), false, false),
                AttemptStatus::Charged
            );
        }
        for state in [
            ElavonPgTransactionState::Declined,
            ElavonPgTransactionState::Rejected,
            ElavonPgTransactionState::Expired,
        ] {
            assert_eq!(
                sale_status(state, Some(false), false, true),
                AttemptStatus::Failure
            );
        }
        assert_eq!(
            sale_status(ElavonPgTransactionState::Voided, Some(true), false, true),
            AttemptStatus::Voided
        );
    }

    #[test]
    fn indeterminate_sale_states_are_pending_not_a_verdict() {
        for state in [
            ElavonPgTransactionState::Unknown,
            ElavonPgTransactionState::Unrecognized,
            ElavonPgTransactionState::AuthorizationPending,
        ] {
            assert_eq!(
                sale_status(state, Some(true), false, true),
                AttemptStatus::Pending
            );
        }
    }

    #[test]
    fn a_successful_void_reads_authorized_and_maps_to_voided() {
        // A void is a *new* transaction whose own success state is `authorized`.
        // Running it through the sale table would yield `Charged`.
        assert_eq!(
            AttemptStatus::from(ElavonPgChildStatus {
                state: &ElavonPgTransactionState::Authorized,
                is_authorized: Some(true),
            }),
            AttemptStatus::Voided
        );
        assert_eq!(
            AttemptStatus::from(ElavonPgChildStatus {
                state: &ElavonPgTransactionState::Authorized,
                is_authorized: Some(false),
            }),
            AttemptStatus::VoidFailed
        );
        assert_eq!(
            AttemptStatus::from(ElavonPgChildStatus {
                state: &ElavonPgTransactionState::Declined,
                is_authorized: None,
            }),
            AttemptStatus::VoidFailed
        );
    }

    #[test]
    fn refund_status_has_a_terminal_failure_path() {
        assert_eq!(
            RefundStatus::from(ElavonPgChildStatus {
                state: &ElavonPgTransactionState::Authorized,
                is_authorized: Some(true),
            }),
            RefundStatus::Success
        );
        assert_eq!(
            RefundStatus::from(ElavonPgChildStatus {
                state: &ElavonPgTransactionState::Settled,
                is_authorized: None,
            }),
            RefundStatus::Success
        );
        // Nothing may hang in Pending forever.
        for state in [
            ElavonPgTransactionState::Declined,
            ElavonPgTransactionState::Rejected,
            ElavonPgTransactionState::Expired,
            ElavonPgTransactionState::Voided,
        ] {
            assert_eq!(
                RefundStatus::from(ElavonPgChildStatus {
                    state: &state,
                    is_authorized: None,
                }),
                RefundStatus::Failure
            );
        }
        assert_eq!(
            RefundStatus::from(ElavonPgChildStatus {
                state: &ElavonPgTransactionState::Authorized,
                is_authorized: Some(false),
            }),
            RefundStatus::Failure
        );
    }

    // ---------------------------------------------------------------------
    // 3DS transaction status
    // ---------------------------------------------------------------------

    #[test]
    fn only_finished_three_ds_outcomes_are_accepted() {
        use common_enums::TransactionStatus;
        assert_eq!(
            ElavonPgThreeDsTransactionStatus::try_from(TransactionStatus::Success).unwrap(),
            ElavonPgThreeDsTransactionStatus::Y
        );
        assert_eq!(
            ElavonPgThreeDsTransactionStatus::try_from(TransactionStatus::Failure).unwrap(),
            ElavonPgThreeDsTransactionStatus::N
        );
        assert_eq!(
            ElavonPgThreeDsTransactionStatus::try_from(TransactionStatus::VerificationNotPerformed)
                .unwrap(),
            ElavonPgThreeDsTransactionStatus::U
        );
        assert_eq!(
            ElavonPgThreeDsTransactionStatus::try_from(TransactionStatus::NotVerified).unwrap(),
            ElavonPgThreeDsTransactionStatus::A
        );
        // A challenge that has not been answered must never be presented to EPG as
        // a completed authentication.
        for status in [
            TransactionStatus::ChallengeRequired,
            TransactionStatus::ChallengeRequiredDecoupledAuthentication,
            TransactionStatus::Rejected,
            TransactionStatus::InformationOnly,
        ] {
            assert!(ElavonPgThreeDsTransactionStatus::try_from(status).is_err());
        }
    }

    // ---------------------------------------------------------------------
    // Error mapping
    // ---------------------------------------------------------------------

    #[test]
    fn every_failure_is_folded_into_reason_not_just_the_first() {
        let failures = vec![
            ElavonPgFailure {
                code: Some("badRequest".to_string()),
                description: Some(
                    "The request is invalid; correct all issues before resending".to_string(),
                ),
                field: None,
            },
            ElavonPgFailure {
                code: Some("fieldValidationFailure".to_string()),
                description: Some("must not be null".to_string()),
                field: Some("total.currencyCode".to_string()),
            },
        ];
        let (code, message, reason) = summarize_failures(&failures);
        assert_eq!(code, "badRequest");
        assert_eq!(
            message,
            "The request is invalid; correct all issues before resending"
        );
        assert_eq!(
            reason.unwrap(),
            "badRequest: The request is invalid; correct all issues before resending; \
             fieldValidationFailure (total.currencyCode): must not be null"
        );
    }

    #[test]
    fn an_empty_failure_list_falls_back_to_the_shared_defaults() {
        let (code, message, reason) = summarize_failures(&[]);
        assert_eq!(code, common_utils::consts::NO_ERROR_CODE);
        assert_eq!(message, common_utils::consts::NO_ERROR_MESSAGE);
        assert!(reason.is_none());
    }

    #[test]
    fn a_body_less_error_response_still_parses() {
        let parsed: ElavonPgErrorResponse =
            serde_json::from_str(r#"{"status":401,"failures":[{"code":"unauthorized","description":"A valid API key is required","field":null}]}"#)
                .unwrap();
        assert_eq!(parsed.status, Some(401));
        let (code, message, _) = summarize_failures(&parsed.failures);
        assert_eq!(code, "unauthorized");
        assert_eq!(message, "A valid API key is required");

        // 5xx responses may carry no body at all.
        let empty = ElavonPgErrorResponse::default();
        assert_eq!(
            summarize_failures(&empty.failures).0,
            common_utils::consts::NO_ERROR_CODE
        );
    }

    #[test]
    fn an_in_band_decline_on_a_201_is_read_from_the_transaction_body() {
        // Verbatim `saleexpiredcard` example: HTTP 201, but the payment failed.
        let raw = serde_json::json!({
            "id": "xdthhybp77468cp68b8v62rk",
            "type": "sale",
            "doCapture": true,
            "isAuthorized": false,
            "authorizationCode": null,
            "state": "declined",
            "failures": [
                { "code": "declinedByProcessor", "description": "Transaction was declined by the payment processor", "field": null },
                { "code": "cardExpired", "description": "Transaction was declined because the card has expired", "field": null }
            ]
        });
        let parsed: ElavonPgTransactionResponse = serde_json::from_value(raw).unwrap();
        assert_eq!(
            AttemptStatus::from(ElavonPgSaleStatus {
                state: &parsed.state,
                is_authorized: parsed.is_authorized,
                is_held_for_review: parsed.is_held_for_review.unwrap_or(false),
                is_auto_capture: parsed.do_capture.unwrap_or(false),
            }),
            AttemptStatus::Failure
        );
        let (code, _, reason) = summarize_failures(&parsed.failures);
        assert_eq!(code, "declinedByProcessor");
        assert!(reason.unwrap().contains("cardExpired"));
    }
}
