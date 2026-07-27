//! Tests for the Juspay account-updater (card sync) provider layer.
//!
//! Covers the §4 consistency rules, every documented response code, the error
//! envelope, and card-sync config resolution. The JWE tests live beside the
//! crypto module itself, since they exercise its private header validation.

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
#[allow(clippy::panic)]
#[allow(clippy::indexing_slicing)]
mod card_sync_tests {
    use super::super::{crypto, transformers::*};
    use common_enums::CardNetwork;
    use domain_types::{
        connector_types::{
            CardRefreshOutcome, RefreshPaymentMethodData, RefreshPaymentMethodResponseData,
            RefreshPaymentMethodResult,
        },
        errors,
        payment_method_data::{
            CardWithNoCvc, DefaultPCIHolder, PaymentMethodData, UpiCollectData, UpiData,
        },
        router_data::ConnectorSpecificConfig,
    };
    use hyperswitch_masking::{PeekInterface, Secret};
    use std::str::FromStr;

    /// A throwaway key pair per test run. Key material is never a fixture.
    fn auth() -> JuspayCardSyncAuthType {
        let rsa = openssl::rsa::Rsa::generate(2048).expect("rsa");
        let pkey = openssl::pkey::PKey::from_rsa(rsa).expect("pkey");

        JuspayCardSyncAuthType {
            api_key: Secret::new("test_api_key".to_string()),
            juspay_encryption_public_key: Secret::new(
                String::from_utf8(pkey.public_key_to_pem().expect("pem")).expect("utf8"),
            ),
            response_decryption_private_key: Secret::new(
                String::from_utf8(pkey.private_key_to_pem_pkcs8().expect("pem")).expect("utf8"),
            ),
            card_sync_key_id: Secret::new("key_0123456789abcdef0123456789abcd".to_string()),
        }
    }

    /// Encrypt a payload under the same key the auth decrypts with, so
    /// responses can be built the way Juspay would build them.
    fn encrypted(auth: &JuspayCardSyncAuthType, payload: &str) -> Secret<String> {
        crypto::encrypt_card_data(
            &Secret::new(payload.to_string()),
            &auth.juspay_encryption_public_key,
        )
        .expect("encrypt")
    }

    fn response(
        code: JuspayCardSyncResponseCode,
        payload: Option<Secret<String>>,
    ) -> JuspayCardSyncResponse {
        JuspayCardSyncResponse {
            status: Some(JuspayCardSyncStatus::Success),
            response_code: Some(code),
            response_message: Some("test".to_string()),
            payload,
        }
    }

    fn card(number: &str) -> CardWithNoCvc {
        CardWithNoCvc {
            card_number: cards::CardNumber::from_str(number).expect("valid test card"),
            card_exp_month: Secret::new("08".to_string()),
            card_exp_year: Secret::new("2027".to_string()),
            card_network: Some(CardNetwork::Visa),
            ..Default::default()
        }
    }

    fn outcome_of(parsed: &RefreshPaymentMethodResponseData) -> CardRefreshOutcome {
        match parsed.result.as_ref().expect("result") {
            RefreshPaymentMethodResult::Card(result) => result.outcome,
        }
    }

    fn card_of(parsed: &RefreshPaymentMethodResponseData) -> CardWithNoCvc {
        match parsed.result.as_ref().expect("result") {
            RefreshPaymentMethodResult::Card(result) => result.card.clone(),
        }
    }

    /// Every outcome returns a full card, so "the network changed nothing" is
    /// asserted as "the result is the submitted card", not as an absent field.
    fn assert_unchanged(parsed: &RefreshPaymentMethodResponseData, submitted: &CardWithNoCvc, ctx: &str) {
        let returned = card_of(parsed);
        assert_eq!(
            returned.card_number, submitted.card_number,
            "{ctx} must echo the submitted card number"
        );
        assert_eq!(
            returned.card_exp_month.peek(),
            submitted.card_exp_month.peek(),
            "{ctx} must echo the submitted expiry month"
        );
        assert_eq!(
            returned.card_exp_year.peek(),
            submitted.card_exp_year.peek(),
            "{ctx} must echo the submitted expiry year"
        );
    }

    const VISA: &str = "4111111111111111";
    const MASTERCARD: &str = "5555555555554444";

    // ---------- request construction ----------

    fn decrypt_card_data(
        card_data: &Secret<String>,
        auth: &JuspayCardSyncAuthType,
    ) -> serde_json::Value {
        let plaintext =
            crypto::decrypt_payload(card_data.peek(), &auth.response_decryption_private_key)
                .expect("decrypt");
        let inner: String = serde_json::from_str(plaintext.peek()).expect("outer json string");
        serde_json::from_str(&inner).expect("inner json object")
    }

    #[test]
    fn seals_the_plaintext_as_a_json_string() {
        let auth = auth();
        let request = build_card_sync_request(&card(VISA), &auth).expect("build");

        let plaintext = crypto::decrypt_payload(
            request.card_data.peek(),
            &auth.response_decryption_private_key,
        )
        .expect("decrypt");

        let value: serde_json::Value = serde_json::from_str(plaintext.peek()).expect("json");
        let inner = value
            .as_str()
            .expect("the sealed plaintext must be a JSON string, not an object");

        let object: serde_json::Value = serde_json::from_str(inner).expect("inner json");
        assert_eq!(object["accountNumber"], VISA);
    }

    #[test]
    fn builds_a_request_whose_card_data_decrypts_to_the_submitted_card() {
        let auth = auth();
        let request = build_card_sync_request(&card(VISA), &auth).expect("build");

        assert_eq!(request.network, JuspayCardNetwork::Visa);
        assert_eq!(request.key_id.peek(), auth.card_sync_key_id.peek());

        // Round-trip through the same key pair to confirm the plaintext shape.
        let parsed = decrypt_card_data(&request.card_data, &auth);

        assert_eq!(parsed["accountNumber"], VISA);
        assert_eq!(parsed["expiryMonth"], "08");
        assert_eq!(parsed["expiryYear"], "2027");
        // Only the three fields Juspay requires — no CVC, no customer data.
        assert_eq!(parsed.as_object().expect("object").len(), 3);
    }

    #[test]
    fn serializes_the_request_with_juspay_field_names() {
        let auth = auth();
        let request = build_card_sync_request(&card(VISA), &auth).expect("build");
        let body: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&request).expect("serialize"))
                .expect("json");

        assert_eq!(body["network"], "VISA");
        assert!(body["cardData"].is_string());
        assert!(body["keyId"].is_string());
        // We ship keyId, never the inline publicKey form.
        assert!(body.get("publicKey").is_none());
    }

    #[test]
    fn maps_mastercard() {
        let auth = auth();
        let mut details = card(MASTERCARD);
        details.card_network = Some(CardNetwork::Mastercard);

        let request = build_card_sync_request(&details, &auth).expect("build");
        assert_eq!(request.network, JuspayCardNetwork::Mastercard);
    }

    #[test]
    fn rejects_an_unsupported_network_naming_it_and_the_supported_set() {
        let auth = auth();
        let mut details = card(VISA);
        details.card_network = Some(CardNetwork::AmericanExpress);

        let err = build_card_sync_request(&details, &auth).expect_err("must reject");
        let message = format!("{:?}", err.current_context());

        assert!(message.contains("AmericanExpress"), "{message}");
        assert!(message.contains("Visa") && message.contains("Mastercard"), "{message}");
    }

    #[test]
    fn expands_a_two_digit_expiry_year() {
        let auth = auth();
        let mut details = card(VISA);
        details.card_exp_year = Secret::new("29".to_string());

        let request = build_card_sync_request(&details, &auth).expect("build");
        let parsed = decrypt_card_data(&request.card_data, &auth);

        assert_eq!(parsed["expiryYear"], "2029");
    }

    #[test]
    fn pads_a_single_digit_expiry_month() {
        let auth = auth();
        let mut details = card(VISA);
        details.card_exp_month = Secret::new("8".to_string());

        let request = build_card_sync_request(&details, &auth).expect("build");
        let parsed = decrypt_card_data(&request.card_data, &auth);

        assert_eq!(parsed["expiryMonth"], "08");
    }

    #[test]
    fn rejects_an_out_of_range_expiry_month() {
        let auth = auth();
        for month in ["0", "13", "99", "ab", ""] {
            let mut details = card(VISA);
            details.card_exp_month = Secret::new(month.to_string());
            assert!(
                build_card_sync_request(&details, &auth).is_err(),
                "month {month:?} must be rejected"
            );
        }
    }

    // ---------- capability boundary: card-only ----------
    //
    // The shared refresh request carries any instrument (see the domain_types
    // refresh_flow tests). Juspay narrows to card_with_no_cvc here; these pin
    // that boundary living in the connector, not in shared types.

    fn refresh_data(
        payment_method_data: PaymentMethodData<DefaultPCIHolder>,
    ) -> RefreshPaymentMethodData<DefaultPCIHolder> {
        RefreshPaymentMethodData {
            payment_method_data,
        }
    }

    #[test]
    fn card_with_no_cvc_is_the_refreshable_instrument() {
        let request = refresh_data(PaymentMethodData::CardWithNoCvc(card(VISA)));
        let extracted = refreshable_card(&request).expect("a card must be accepted");
        assert_eq!(extracted.card_number, card(VISA).card_number);
    }

    #[test]
    fn a_non_card_instrument_is_rejected_by_the_connector() {
        let request = refresh_data(PaymentMethodData::Upi(UpiData::UpiCollect(UpiCollectData {
            vpa_id: None,
            upi_source: None,
        })));

        let err = refreshable_card(&request).expect_err("Juspay refreshes cards only");
        let message = format!("{:?}", err.current_context());
        assert!(
            message.contains("card_with_no_cvc"),
            "the rejection must name what Juspay does support: {message}"
        );
    }

    // ---------- documented outcomes ----------

    #[test]
    fn parses_expiry_updated_with_the_echoed_card_number() {
        let auth = auth();
        // The observed shape: the submitted card comes back unchanged, and only
        // the expiry has moved.
        let payload = encrypted(
            &auth,
            r#"{"updatedAccountNumber":"4111111111111111","isAccountUpdated":true,"updatedExpiryDate":"0829"}"#,
        );

        let parsed = parse_card_sync_response(
            &response(JuspayCardSyncResponseCode::ExpiryUpdated, Some(payload)),
            &auth,
            200,
            &card(VISA),
        )
        .expect("parse");

        assert_eq!(outcome_of(&parsed), CardRefreshOutcome::ExpiryUpdated);

        let refreshed = card_of(&parsed);
        // Forwarded verbatim, not suppressed on equality with the request.
        assert_eq!(refreshed.card_number.get_card_no(), VISA);
        assert_eq!(refreshed.card_exp_month.peek(), "08");
        assert_eq!(refreshed.card_exp_year.peek(), "2029");
    }

    #[test]
    fn parses_expiry_updated_without_a_card_number() {
        let auth = auth();
        let payload = encrypted(
            &auth,
            r#"{"updatedAccountNumber":null,"isAccountUpdated":true,"updatedExpiryDate":"0829"}"#,
        );

        let parsed = parse_card_sync_response(
            &response(JuspayCardSyncResponseCode::ExpiryUpdated, Some(payload)),
            &auth,
            200,
            &card(VISA),
        )
        .expect("parse");

        let refreshed = card_of(&parsed);
        // The card is complete, so an unchanged number carries over from the request.
        assert_eq!(refreshed.card_number.get_card_no(), VISA);
        assert_eq!(refreshed.card_exp_year.peek(), "2029");
    }

    #[test]
    fn parses_account_updated_with_a_replacement_card() {
        let auth = auth();
        let payload = encrypted(
            &auth,
            r#"{"updatedAccountNumber":"5555555555554444","isAccountUpdated":true,"updatedExpiryDate":"1230"}"#,
        );

        let parsed = parse_card_sync_response(
            &response(JuspayCardSyncResponseCode::AccountUpdated, Some(payload)),
            &auth,
            200,
            &card(VISA),
        )
        .expect("parse");

        assert_eq!(outcome_of(&parsed), CardRefreshOutcome::AccountUpdated);
        let refreshed = card_of(&parsed);
        assert_eq!(refreshed.card_number.get_card_no(), MASTERCARD);
        assert_eq!(refreshed.card_exp_month.peek(), "12");
        assert_eq!(refreshed.card_exp_year.peek(), "2030");
    }

    #[test]
    fn parses_account_updated_without_an_expiry_change() {
        let auth = auth();
        let payload = encrypted(
            &auth,
            r#"{"updatedAccountNumber":"5555555555554444","isAccountUpdated":true,"updatedExpiryDate":null}"#,
        );

        let parsed = parse_card_sync_response(
            &response(JuspayCardSyncResponseCode::AccountUpdated, Some(payload)),
            &auth,
            200,
            &card(VISA),
        )
        .expect("parse");

        let refreshed = card_of(&parsed);
        assert_eq!(refreshed.card_number.get_card_no(), MASTERCARD);
        // Unchanged expiry carries over from the request.
        assert_eq!(refreshed.card_exp_month.peek(), "08");
        assert_eq!(refreshed.card_exp_year.peek(), "2027");
    }

    #[test]
    fn accepts_account_updated_carrying_is_account_updated_false() {
        let auth = auth();
        // `responseCode` is the sole authority. `isAccountUpdated` is a second
        // encoding of the same fact and is deliberately not a validation gate.
        let payload = encrypted(
            &auth,
            r#"{"updatedAccountNumber":"5555555555554444","isAccountUpdated":false,"updatedExpiryDate":null}"#,
        );

        let parsed = parse_card_sync_response(
            &response(JuspayCardSyncResponseCode::AccountUpdated, Some(payload)),
            &auth,
            200,
            &card(VISA),
        )
        .expect("the flag must not gate the outcome");

        assert_eq!(outcome_of(&parsed), CardRefreshOutcome::AccountUpdated);
    }

    #[test]
    fn parses_the_terminal_outcomes_with_explicit_nulls() {
        let auth = auth();
        let cases = [
            (JuspayCardSyncResponseCode::NoChange, CardRefreshOutcome::NoChange, "NO_CHANGE"),
            (JuspayCardSyncResponseCode::CardClosed, CardRefreshOutcome::Closed, "CARD_CLOSED"),
            (
                JuspayCardSyncResponseCode::CardNotFound,
                CardRefreshOutcome::NotFound,
                "CARD_NOT_FOUND",
            ),
            (
                JuspayCardSyncResponseCode::ContactIssuer,
                CardRefreshOutcome::ContactIssuer,
                "CONTACT_ISSUER",
            ),
        ];

        for (code, expected, raw) in cases {
            let payload = encrypted(
                &auth,
                r#"{"updatedAccountNumber":null,"isAccountUpdated":false,"updatedExpiryDate":null}"#,
            );

            let parsed = parse_card_sync_response(&response(code, Some(payload)), &auth, 200, &card(VISA))
                .expect("parse");

            assert_eq!(outcome_of(&parsed), expected);
            assert_unchanged(&parsed, &card(VISA), raw);
        }
    }

    #[test]
    fn parses_the_terminal_outcomes_with_the_payload_absent() {
        let auth = auth();
        // Both shapes are observed; both are accepted.
        for code in [
            JuspayCardSyncResponseCode::NoChange,
            JuspayCardSyncResponseCode::CardClosed,
            JuspayCardSyncResponseCode::CardNotFound,
            JuspayCardSyncResponseCode::ContactIssuer,
        ] {
            let parsed =
                parse_card_sync_response(&response(code, None), &auth, 200, &card(VISA)).expect("parse");
            assert_unchanged(&parsed, &card(VISA), "an absent payload");
        }
    }

    // ---------- unknown codes ----------

    #[test]
    fn an_unknown_code_is_a_successful_inquiry_not_an_error() {
        let auth = auth();
        let parsed = parse_card_sync_response(
            &response(
                JuspayCardSyncResponseCode::Unknown("SOME_NEW_CODE".to_string()),
                None,
            ),
            &auth,
            200,
            &card(VISA),
        )
        .expect("an unmapped code must not fail the call");

        assert_eq!(outcome_of(&parsed), CardRefreshOutcome::Unrecognized);
        assert_unchanged(&parsed, &card(VISA), "an unmapped code");
    }

    #[test]
    fn an_unknown_code_ignores_its_payload_and_echoes_the_submitted_card() {
        let auth = auth();
        // A card number distinct from the submitted one, so echoing and
        // applying the payload are distinguishable.
        let payload = encrypted(
            &auth,
            r#"{"updatedAccountNumber":"5555555555554444","isAccountUpdated":true,"updatedExpiryDate":"0829"}"#,
        );

        let parsed = parse_card_sync_response(
            &response(
                JuspayCardSyncResponseCode::Unknown("SOME_NEW_CODE".to_string()),
                Some(payload),
            ),
            &auth,
            200,
            &card(VISA),
        )
        .expect("must not fail");

        assert_unchanged(
            &parsed,
            &card(VISA),
            "we cannot say what a payload means under a code we do not know, so",
        );
    }

    #[test]
    fn deserializes_an_undocumented_code_into_the_unknown_variant() {
        let parsed: JuspayCardSyncResponse = serde_json::from_str(
            r#"{"status":"SUCCESS","responseCode":"SOMETHING_NEW","responseMessage":"x","payload":null}"#,
        )
        .expect("deserialize");

        assert_eq!(
            parsed.response_code,
            Some(JuspayCardSyncResponseCode::Unknown("SOMETHING_NEW".to_string()))
        );
    }

    #[test]
    fn deserializes_the_documented_codes() {
        for (raw, expected) in [
            ("ACCOUNT_UPDATED", JuspayCardSyncResponseCode::AccountUpdated),
            ("EXPIRY_UPDATED", JuspayCardSyncResponseCode::ExpiryUpdated),
            ("NO_CHANGE", JuspayCardSyncResponseCode::NoChange),
            ("CARD_CLOSED", JuspayCardSyncResponseCode::CardClosed),
            ("CARD_NOT_FOUND", JuspayCardSyncResponseCode::CardNotFound),
            ("CONTACT_ISSUER", JuspayCardSyncResponseCode::ContactIssuer),
        ] {
            let body = format!(
                r#"{{"status":"SUCCESS","responseCode":"{raw}","responseMessage":"x","payload":null}}"#
            );
            let parsed: JuspayCardSyncResponse =
                serde_json::from_str(&body).expect("deserialize");
            assert_eq!(parsed.response_code, Some(expected), "{raw}");
        }
    }

    // ---------- consistency violations ----------

    #[test]
    fn rejects_an_update_outcome_without_a_payload() {
        let auth = auth();
        for code in [
            JuspayCardSyncResponseCode::AccountUpdated,
            JuspayCardSyncResponseCode::ExpiryUpdated,
        ] {
            let err = parse_card_sync_response(&response(code, None), &auth, 200, &card(VISA))
                .expect_err("must reject");
            assert_eq!(err.current_context(), &invalid_gateway_response("payload"));
        }
    }

    #[test]
    fn rejects_account_updated_without_a_card_number() {
        let auth = auth();
        let payload = encrypted(
            &auth,
            r#"{"updatedAccountNumber":null,"isAccountUpdated":true,"updatedExpiryDate":"0829"}"#,
        );

        let err = parse_card_sync_response(
            &response(JuspayCardSyncResponseCode::AccountUpdated, Some(payload)),
            &auth,
            200,
            &card(VISA),
        )
        .expect_err("must reject");

        assert_eq!(
            err.current_context(),
            &invalid_gateway_response("updated_account_number")
        );
    }

    #[test]
    fn rejects_expiry_updated_without_an_expiry() {
        let auth = auth();
        let payload = encrypted(
            &auth,
            r#"{"updatedAccountNumber":"4111111111111111","isAccountUpdated":true,"updatedExpiryDate":null}"#,
        );

        let err = parse_card_sync_response(
            &response(JuspayCardSyncResponseCode::ExpiryUpdated, Some(payload)),
            &auth,
            200,
            &card(VISA),
        )
        .expect_err("must reject");

        assert_eq!(
            err.current_context(),
            &invalid_gateway_response("updated_expiry_date")
        );
    }

    #[test]
    fn rejects_a_terminal_outcome_carrying_a_card_number() {
        let auth = auth();
        let payload = encrypted(
            &auth,
            r#"{"updatedAccountNumber":"4111111111111111","isAccountUpdated":false,"updatedExpiryDate":null}"#,
        );

        let err = parse_card_sync_response(
            &response(JuspayCardSyncResponseCode::NoChange, Some(payload)),
            &auth,
            200,
            &card(VISA),
        )
        .expect_err("must reject rather than partially apply");

        assert_eq!(
            err.current_context(),
            &invalid_gateway_response("updated_account_number")
        );
    }

    #[test]
    fn rejects_a_terminal_outcome_carrying_an_expiry() {
        let auth = auth();
        let payload = encrypted(
            &auth,
            r#"{"updatedAccountNumber":null,"isAccountUpdated":false,"updatedExpiryDate":"0829"}"#,
        );

        let err = parse_card_sync_response(
            &response(JuspayCardSyncResponseCode::CardClosed, Some(payload)),
            &auth,
            200,
            &card(VISA),
        )
        .expect_err("must reject");

        assert_eq!(
            err.current_context(),
            &invalid_gateway_response("updated_expiry_date")
        );
    }

    #[test]
    fn rejects_a_returned_card_number_that_fails_luhn() {
        let auth = auth();
        let payload = encrypted(
            &auth,
            r#"{"updatedAccountNumber":"4111111111111112","isAccountUpdated":true,"updatedExpiryDate":"0829"}"#,
        );

        let err = parse_card_sync_response(
            &response(JuspayCardSyncResponseCode::AccountUpdated, Some(payload)),
            &auth,
            200,
            &card(VISA),
        )
        .expect_err("must reject");

        assert_eq!(
            err.current_context(),
            &invalid_gateway_response("updated_account_number")
        );
    }

    #[test]
    fn rejects_a_malformed_updated_expiry_date() {
        let auth = auth();
        for expiry in ["082", "08299", "1329", "0029", "abcd"] {
            let payload = encrypted(
                &auth,
                &format!(
                    r#"{{"updatedAccountNumber":null,"isAccountUpdated":true,"updatedExpiryDate":"{expiry}"}}"#
                ),
            );

            assert!(
                parse_card_sync_response(
                    &response(JuspayCardSyncResponseCode::ExpiryUpdated, Some(payload)),
                    &auth,
                    200,
                    &card(VISA),
                )
                .is_err(),
                "expiry {expiry:?} must be rejected"
            );
        }
    }

    #[test]
    fn rejects_a_payload_that_is_not_valid_json() {
        let auth = auth();
        let payload = encrypted(&auth, "not json at all");

        let err = parse_card_sync_response(
            &response(JuspayCardSyncResponseCode::ExpiryUpdated, Some(payload)),
            &auth,
            200,
            &card(VISA),
        )
        .expect_err("must reject");

        assert_eq!(err.current_context(), &invalid_gateway_response("payload"));
    }

    #[test]
    fn rejects_a_payload_encrypted_under_the_wrong_key() {
        let other = auth();
        let auth = auth();
        let payload = encrypted(
            &other,
            r#"{"updatedAccountNumber":null,"isAccountUpdated":true,"updatedExpiryDate":"0829"}"#,
        );

        assert!(parse_card_sync_response(
            &response(JuspayCardSyncResponseCode::ExpiryUpdated, Some(payload)),
            &auth,
            200,
            &card(VISA),
        )
        .is_err());
    }

    #[test]
    fn rejects_a_malformed_payload_jwe() {
        let auth = auth();
        let payload = Secret::new("some-random-decryption".to_string());

        assert!(parse_card_sync_response(
            &response(JuspayCardSyncResponseCode::ExpiryUpdated, Some(payload)),
            &auth,
            200,
            &card(VISA),
        )
        .is_err());
    }

    // ---------- config ----------

    #[test]
    fn resolves_card_sync_config_when_every_field_is_present() {
        let config = ConnectorSpecificConfig::Juspay {
            api_key: Secret::new("k".to_string()),
            merchant_id: Secret::new("m".to_string()),
            juspay_encryption_public_key: Some(Secret::new("pub".to_string())),
            response_decryption_private_key: Some(Secret::new("priv".to_string())),
            card_sync_key_id: Some(Secret::new("key_x".to_string())),
            base_url: None,
        };

        assert!(JuspayCardSyncAuthType::try_from(&config).is_ok());
    }

    #[test]
    fn names_each_missing_card_sync_config_field_individually() {
        // The compiler cannot enforce that Refresh gets its three extra fields,
        // because they stay optional so payment flows are unaffected. This is
        // the test that does.
        let field_names = [
            "juspay_encryption_public_key",
            "response_decryption_private_key",
            "card_sync_key_id",
        ];

        for missing in field_names {
            let config = ConnectorSpecificConfig::Juspay {
                api_key: Secret::new("k".to_string()),
                merchant_id: Secret::new("m".to_string()),
                juspay_encryption_public_key: (missing != "juspay_encryption_public_key")
                    .then(|| Secret::new("pub".to_string())),
                response_decryption_private_key: (missing != "response_decryption_private_key")
                    .then(|| Secret::new("priv".to_string())),
                card_sync_key_id: (missing != "card_sync_key_id")
                    .then(|| Secret::new("key_x".to_string())),
                base_url: None,
            };

            let err = JuspayCardSyncAuthType::try_from(&config)
                .expect_err("{missing} must be reported");

            assert_eq!(
                err.current_context(),
                &errors::IntegrationError::MissingRequiredField {
                    field_name: missing,
                    context: Default::default(),
                },
                "the error must name {missing} specifically"
            );
        }
    }

    // ---------- error envelope ----------

    #[test]
    fn parses_the_card_sync_error_envelope() {
        // A real sandbox error body. Note the absent `status`/`error_code`
        // (success and error shapes are disjoint), and the `error`, `category`,
        // `href`, `request_id` fields we do not declare — serde drops them, which
        // is why the struct omits them. What matters is what the error builder
        // reads: error_info.code, error_message, and the user messages.
        let body = r#"{
            "error": true,
            "error_message": "LTR_ENTRY_NOT_FOUND",
            "user_message": "Ltr Entry Not Found",
            "error_info": {
                "code": "INVALID_INPUT",
                "category": "USER_ERROR",
                "href": "NA",
                "request_id": "53818c1c-2c33-44b9-994e-96f51e92e39b",
                "user_message": "Invalid request params. Please verify your input.",
                "developer_message": "Ltr Entry Not Found"
            }
        }"#;

        let parsed: JuspayErrorResponse = serde_json::from_str(body).expect("deserialize");

        assert!(parsed.status.is_none());
        assert!(parsed.error_code.is_none());
        assert_eq!(parsed.error_message.as_deref(), Some("LTR_ENTRY_NOT_FOUND"));
        assert_eq!(parsed.user_message.as_deref(), Some("Ltr Entry Not Found"));

        let info = parsed.error_info.expect("error_info");
        assert_eq!(info.code.as_deref(), Some("INVALID_INPUT"));
        assert_eq!(
            info.user_message.as_deref(),
            Some("Invalid request params. Please verify your input.")
        );
        assert_eq!(info.developer_message.as_deref(), Some("Ltr Entry Not Found"));
    }

    #[test]
    fn parses_the_unregistered_key_error_envelope() {
        // The likely first-run symptom: a stale or unregistered keyId surfacing
        // as a gateway error.
        let body = r#"{
            "error": true,
            "error_message": "Merchant public key not found",
            "user_message": "Cannot encrypt response: no active CLIENT_ENCRYPTION key for this merchant",
            "error_info": { "code": "UNKNOWN_ERROR", "category": "UNKNOWN", "request_id": "req_456" }
        }"#;

        let parsed: JuspayErrorResponse = serde_json::from_str(body).expect("deserialize");

        assert_eq!(
            parsed.error_message.as_deref(),
            Some("Merchant public key not found")
        );
        assert_eq!(
            parsed.error_info.expect("info").code.as_deref(),
            Some("UNKNOWN_ERROR")
        );
    }

    #[test]
    fn still_parses_the_orchestrator_error_envelope() {
        // The existing payment flows' shape must keep working: the card-sync
        // fields were added as optional precisely so this stays true.
        let body = r#"{"status":"ERROR","error_code":"invalid_request","error_message":"bad"}"#;
        let parsed: JuspayErrorResponse = serde_json::from_str(body).expect("deserialize");

        assert_eq!(parsed.status.as_deref(), Some("ERROR"));
        assert_eq!(parsed.error_code.as_deref(), Some("invalid_request"));
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
#[allow(clippy::panic)]
#[allow(clippy::indexing_slicing)]
mod card_sync_failure_tests {
    use super::super::transformers::*;

    fn failure(code: &str, message: &str) -> JuspayCardSyncResponse {
        JuspayCardSyncResponse {
            status: Some(JuspayCardSyncStatus::Failure),
            response_code: Some(JuspayCardSyncResponseCode::Unknown(code.to_string())),
            response_message: Some(message.to_string()),
            payload: None,
        }
    }

    #[test]
    fn deserializes_the_failure_envelope() {
        // A FAILURE shares the success envelope's shape but always has a null
        // payload — it is not the HTTP 4xx/5xx `error: true` envelope.
        let parsed: JuspayCardSyncResponse = serde_json::from_str(
            r#"{"status":"FAILURE","responseCode":"UNKNOWN",
                "responseMessage":"PAN length must be between 13 and 19","payload":null}"#,
        )
        .expect("deserialize");

        assert_eq!(parsed.status, Some(JuspayCardSyncStatus::Failure));
        assert!(parsed.payload.is_none());
    }

    #[test]
    fn maps_each_documented_failure_code_to_a_connector_error() {
        let cases = [
            ("INVALID_CARD_DATA", "Card details are invalid or malformed"),
            ("NOT_ENROLLED", "Merchant or acquirer not enrolled"),
            ("NETWORK_ERROR", "Temporary network issue"),
            ("UNKNOWN", "PAN length must be between 13 and 19"),
        ];

        for (code, message) in cases {
            let error = build_card_sync_failure(&failure(code, message), 200);

            // The raw provider code is forwarded rather than mapped — the
            // caller owns retry policy, not us.
            assert_eq!(error.code, code);
            assert_eq!(error.message, message);
            assert_eq!(error.reason.as_deref(), Some(message));
        }
    }

    #[test]
    fn a_failure_without_a_response_code_still_produces_an_error() {
        let response = JuspayCardSyncResponse {
            status: Some(JuspayCardSyncStatus::Failure),
            response_code: None,
            response_message: None,
            payload: None,
        };

        let error = build_card_sync_failure(&response, 200);
        assert_eq!(error.code, "UNKNOWN");
        assert!(!error.message.is_empty());
    }

    #[test]
    fn deserializes_a_success_envelope_without_a_response_code() {
        // `responseCode` is optional per the provider spec, so its absence must
        // not fail deserialization — it is rejected later, as a bad response.
        let parsed: JuspayCardSyncResponse =
            serde_json::from_str(r#"{"status":"SUCCESS","payload":null}"#)
                .expect("deserialize");

        assert!(parsed.response_code.is_none());
    }

    #[test]
    fn an_unrecognised_status_is_neither_success_nor_failure() {
        let parsed: JuspayCardSyncResponse =
            serde_json::from_str(r#"{"status":"PENDING","responseCode":"NO_CHANGE"}"#)
                .expect("deserialize");

        assert_eq!(
            parsed.status,
            Some(JuspayCardSyncStatus::Unknown("PENDING".to_string()))
        );
    }

    #[test]
    fn the_visa_reject_record_stays_a_success_outcome() {
        // The same validation fault surfaces as a FAILURE on Mastercard but as
        // a SUCCESS carrying a reject record on Visa. The Visa form must remain
        // a normal outcome, not an error.
        let parsed: JuspayCardSyncResponse = serde_json::from_str(
            r#"{"status":"SUCCESS","responseCode":"CARD_NOT_FOUND",
                "responseMessage":"Non-participating BIN","payload":"<jwe>"}"#,
        )
        .expect("deserialize");

        assert_eq!(parsed.status, Some(JuspayCardSyncStatus::Success));
        assert_eq!(
            parsed.response_code,
            Some(JuspayCardSyncResponseCode::CardNotFound)
        );
    }

    #[test]
    fn the_mastercard_form_of_the_same_fault_is_a_failure() {
        let parsed: JuspayCardSyncResponse = serde_json::from_str(
            r#"{"status":"FAILURE","responseCode":"INVALID_CARD_DATA",
                "responseMessage":"PAN length must be between 13 and 19","payload":null}"#,
        )
        .expect("deserialize");

        assert_eq!(parsed.status, Some(JuspayCardSyncStatus::Failure));
        let error = build_card_sync_failure(&parsed, 200);
        assert_eq!(error.code, "INVALID_CARD_DATA");
    }

    #[test]
    fn the_spec_account_updated_sample_yields_a_replacement_card() {
        // From the provider spec: a genuinely different PAN, not an echo of the
        // submitted card.
        let parsed: serde_json::Value = serde_json::from_str(
            r#"{"isAccountUpdated":true,"updatedAccountNumber":"4111111111119999",
                "updatedExpiryDate":"1228"}"#,
        )
        .expect("json");

        assert_ne!(parsed["updatedAccountNumber"], "4111111111111111");
        assert_eq!(parsed["updatedExpiryDate"], "1228");
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
#[allow(clippy::panic)]
#[allow(clippy::indexing_slicing)]
mod card_sync_length_tests {
    use super::super::transformers::*;
    use common_enums::CardNetwork;
    use domain_types::payment_method_data::CardWithNoCvc;
    use hyperswitch_masking::Secret;
    use std::str::FromStr;

    fn auth() -> JuspayCardSyncAuthType {
        let rsa = openssl::rsa::Rsa::generate(2048).expect("rsa");
        let pkey = openssl::pkey::PKey::from_rsa(rsa).expect("pkey");
        JuspayCardSyncAuthType {
            api_key: Secret::new("k".to_string()),
            juspay_encryption_public_key: Secret::new(
                String::from_utf8(pkey.public_key_to_pem().expect("pem")).expect("utf8"),
            ),
            response_decryption_private_key: Secret::new(
                String::from_utf8(pkey.private_key_to_pem_pkcs8().expect("pem")).expect("utf8"),
            ),
            card_sync_key_id: Secret::new("key_x".to_string()),
        }
    }

    fn card(number: &str) -> CardWithNoCvc {
        CardWithNoCvc {
            card_number: cards::CardNumber::from_str(number).expect("valid card"),
            card_exp_month: Secret::new("08".to_string()),
            card_exp_year: Secret::new("2027".to_string()),
            card_network: Some(CardNetwork::Visa),
            ..Default::default()
        }
    }

    #[test]
    fn rejects_a_pan_shorter_than_juspay_accepts() {
        // Luhn-valid at 12 digits: passes our own validation (floor of 8) but
        // Juspay requires 13-19 and answers with a generic error.
        let short = "123456789031";
        assert_eq!(short.len(), 12);

        let err = build_card_sync_request(&card(short), &auth()).expect_err("must reject");
        assert!(format!("{err:?}").contains("13"), "error should name the limit");
    }

    #[test]
    fn accepts_a_pan_at_the_lower_bound() {
        let thirteen = "4222222222222";
        assert_eq!(thirteen.len(), 13);
        assert!(build_card_sync_request(&card(thirteen), &auth()).is_ok());
    }
}
