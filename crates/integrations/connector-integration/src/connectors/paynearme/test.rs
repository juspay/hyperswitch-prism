//! PayNearMe connector tests.
//!
//! * Signing: both worked vectors published on the Authentication page (spec
//!   §3.1), plus the SetupMandate, RepeatPayment and CreateOrder bodies.
//! * Stored credentials: token matching, the `connector_mandate_id` codec, and
//!   the request guards and response mapping of every flow that stores or charges
//!   a card, driven through the connector's `get_request_body` /
//!   `handle_response_v2` exactly as the gRPC handlers call them.
//! * Wallet refusals.
//! * CreateOrder: the proto `customer` / `setup_future_usage` conversion and the
//!   PayNearMe order it produces.

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
#[allow(clippy::panic)]
#[allow(clippy::panic_in_result_fn)]
#[allow(clippy::unwrap_in_result)]
#[allow(clippy::indexing_slicing)]
mod tests {
    use std::{borrow::Cow, marker::PhantomData, str::FromStr};

    use cards::CardNumber;
    use common_enums::{
        AttemptStatus, AuthenticationType, CaptureMethod, Currency, FutureUsage, PaymentMethod,
    };
    use common_utils::{
        id_type::{CustomerId, MerchantId},
        metadata::MaskedMetadata,
        request::RequestContent,
        types::{MinorUnit, StringMajorUnit},
    };
    use domain_types::{
        connector_flow::{Authorize, CreateOrder, RepeatPayment, SetupMandate},
        connector_types::{
            ConnectorMandateReferenceId, MandateReferenceId, NetworkMandateIdRef,
            NetworkTokenWithNTIRef, PaymentCreateOrderData, PaymentCreateOrderResponse,
            PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData, RepeatPaymentData,
            ResponseId, SetupMandateRequestData,
        },
        errors::{ConnectorError, IntegrationError},
        mandates::CustomerAcceptance,
        payment_address::{Address, AddressDetails, PaymentAddress, PhoneDetails},
        payment_method_data::{
            ApplePayPaymentData, ApplePayWalletData, Card, DefaultPCIHolder, GooglePayWalletData,
            GpayTokenizationData, PaymentMethodData, RawCardNumber,
        },
        router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
        router_data_v2::RouterDataV2,
        router_request_types::AuthenticationData,
        router_response_types::Response,
        types::Connectors,
        utils::ForeignTryFrom,
    };
    use grpc_api_types::payments as grpc;
    use hyperswitch_masking::{PeekInterface, Secret};
    use interfaces::connector_integration_v2::ConnectorIntegrationV2;
    use serde::Deserialize;
    use serde_json::{json, Value};

    use crate::connectors::paynearme::{transformers::*, Paynearme};

    // =========================================================================
    // SIGNING
    // =========================================================================

    /// Worked example #1 (spec §3.1, the `/create_order` sample): the full
    /// envelope, including the empty `signature` field that must not sign
    /// itself.
    #[test]
    fn signs_the_create_order_worked_example() {
        let body = json!({
            "site_identifier": "S2155373459",
            "timestamp": "1636142061",
            "version": "3.0",
            "signature": "",
            "order_amount": "500",
            "order_currency": "USD",
            "site_customer_identifier": "11223344",
            "order_type": "any",
            "order_is_standing": "true",
        });

        let string_to_sign = paynearme_string_to_sign(&body).expect("string_to_sign");
        assert_eq!(
            string_to_sign,
            // Split at the currency/`order_is_standing` boundary: concatenated
            // end-to-end, those two run together into a token the spell checker
            // flags as a misspelling. `concat!` keeps the runtime value identical.
            concat!(
                "order_amount500order_currencyUSD",
                "order_is_standingtrueorder_typeany",
                "site_customer_identifier11223344site_identifierS2155373459",
                "timestamp1636142061version3.0",
            )
        );

        let signature =
            paynearme_signature(&Secret::new("ab7b539ea1317cca67c63c552".to_string()), &body)
                .expect("signature");
        assert_eq!(
            signature.peek(),
            "65c694f4b632187aa02fc4144cdd374373307f0a200448c9e53f2e47d84b3b82"
        );
    }

    /// Worked example #2 (spec §3.1), the vector shipped in every reference
    /// implementation: `format` is dropped from the input even though it is sent
    /// on the wire, and the remaining keys are concatenated in alphabetical —
    /// not insertion — order.
    #[test]
    fn signs_the_documented_unit_test_vector() {
        let body = json!({
            "version": "3.0",
            "site_identifier": "MySiteID",
            "timestamp": "1582149833",
            "hello": "world",
            "foo": "bar",
            "format": "json",
        });

        let string_to_sign = paynearme_string_to_sign(&body).expect("string_to_sign");
        assert_eq!(
            string_to_sign,
            "foobarhelloworldsite_identifierMySiteIDtimestamp1582149833version3.0"
        );

        let signature =
            paynearme_signature(&Secret::new("abc123".to_string()), &body).expect("signature");
        assert_eq!(
            signature.peek(),
            "7986b4e59c15cd22fd496113c916f9739f619778812bf9ab8943af80749aadcc"
        );
    }

    /// `version`, `site_identifier` and `timestamp` are mandatory inputs to the
    /// signature (§3.1 step 2) — a body missing one must be refused rather than
    /// signed into a request PayNearMe will reject with a bare 401.
    #[test]
    fn refuses_to_sign_without_the_required_envelope_fields() {
        let body = json!({
            "site_identifier": "MySiteID",
            "version": "3.0",
        });
        assert!(paynearme_string_to_sign(&body).is_err());
    }

    /// A field that is skipped by `skip_serializing_if = "Option::is_none"` is
    /// absent from the wire, so it must be absent from the signature too — the
    /// full-refund shape, where `refund_amount` / `refund_currency` are omitted,
    /// depends on this.
    #[test]
    fn omitted_fields_do_not_enter_the_signature() {
        let with_null = json!({
            "site_identifier": "MySiteID",
            "timestamp": "1582149833",
            "version": "3.0",
            "refund_amount": Value::Null,
        });
        let without = json!({
            "site_identifier": "MySiteID",
            "timestamp": "1582149833",
            "version": "3.0",
        });
        assert_eq!(
            paynearme_string_to_sign(&with_null).expect("string_to_sign"),
            paynearme_string_to_sign(&without).expect("string_to_sign")
        );
    }

    /// The SetupMandate body, built from the spec's "tokenize only" example values
    /// (expiry in the normative `MM/YYYY`). The flattened card block must sign as
    /// plain top-level fields, and nothing a charge would add (`send_payment`,
    /// `payment_amount`, `payment_currency`, `site_channel`) may reach the
    /// preimage. The expected digest was computed outside this codebase (Python
    /// `hmac` + `hashlib`) with the secret from worked example #1.
    #[test]
    fn signs_the_tokenize_only_setup_mandate_body() {
        let request = PaynearmeSetupMandateRequest {
            site_identifier: Secret::new("S2411573363".to_string()),
            timestamp: "1702333839".to_string(),
            version: "3.0".to_string(),
            signature: Secret::new(String::new()),
            pnm_order_identifier: "85011138740".to_string(),
            card: PaynearmeCardPaymentMethod {
                payment_method_type: "card".to_string(),
                payment_method_card_number_pii: Secret::new("9999916516806651".to_string()),
                payment_method_card_expiry_pii: Secret::new("01/2027".to_string()),
                payment_method_cvv_pii: Secret::new("416".to_string()),
                payment_method_billing_name: Secret::new("John Smith".to_string()),
                payment_method_billing_address: Secret::new("123 Fake Street".to_string()),
                payment_method_billing_zipcode: Secret::new("75013".to_string()),
                payment_method_billing_phone: Secret::new("469-555-5878".to_string()),
            },
        };

        let body = serde_json::to_value(&request).expect("body");
        assert_eq!(
            paynearme_string_to_sign(&body).expect("string_to_sign"),
            concat!(
                "payment_method_billing_address",
                "123 Fake Street",
                "payment_method_billing_name",
                "John Smith",
                "payment_method_billing_phone",
                "469-555-5878",
                "payment_method_billing_zipcode",
                "75013",
                "payment_method_card_expiry_pii",
                "01/2027",
                "payment_method_card_number_pii",
                "9999916516806651",
                "payment_method_cvv_pii",
                "416",
                "payment_method_type",
                "card",
                "pnm_order_identifier",
                "85011138740",
                "site_identifier",
                "S2411573363",
                "timestamp",
                "1702333839",
                "version",
                "3.0",
            )
        );

        let signature = paynearme_signature(
            &Secret::new("ab7b539ea1317cca67c63c552".to_string()),
            &request,
        )
        .expect("signature");
        assert_eq!(
            signature.peek(),
            "fc2a7784d9a2c329a106e844db94f465e0768fa1e32dd56798a4da437ab4dbcc"
        );
    }

    /// The RepeatPayment body, built from the `/make_payment` schema example values
    /// plus `recurring` and `site_payment_identifier`. Every field is a string and
    /// signs as-is; `cvv_pii` is absent from both the wire and the preimage. The
    /// expected digest was computed outside this codebase (Python `hmac` +
    /// `hashlib`) with the secret from worked example #1.
    #[test]
    fn signs_the_make_payment_body() {
        let request = PaynearmeRepeatPaymentRequest {
            site_identifier: Secret::new("S2411573363".to_string()),
            timestamp: "1702333839".to_string(),
            version: "3.0".to_string(),
            signature: Secret::new(String::new()),
            pnm_order_identifier: "84338052224".to_string(),
            payment_method_identifier: Secret::new("3b7ac9d4c86e6".to_string()),
            payment_amount: major_amount("100"),
            payment_currency: Currency::USD,
            site_channel: SITE_CHANNEL_CONSUMER.to_string(),
            recurring: RECURRING_TRUE.to_string(),
            site_payment_identifier: "0123456789".to_string(),
        };

        let body = serde_json::to_value(&request).expect("body");
        assert_eq!(
            paynearme_string_to_sign(&body).expect("string_to_sign"),
            concat!(
                "payment_amount",
                "100",
                "payment_currency",
                "USD",
                "payment_method_identifier",
                "3b7ac9d4c86e6",
                "pnm_order_identifier",
                "84338052224",
                "recurring",
                "true",
                "site_channel",
                "consumer",
                "site_identifier",
                "S2411573363",
                "site_payment_identifier",
                "0123456789",
                "timestamp",
                "1702333839",
                "version",
                "3.0",
            )
        );

        let signature = paynearme_signature(
            &Secret::new("ab7b539ea1317cca67c63c552".to_string()),
            &request,
        )
        .expect("signature");
        assert_eq!(
            signature.peek(),
            "c7651120cdd7bb236b1432aad551bb86bba03bae6efd43f4df938e822c289896"
        );
    }

    // =========================================================================
    // STORED-CARD TOKEN MATCHING AND THE MANDATE REFERENCE CODEC
    // =========================================================================

    /// The spec's verbatim "Create a Card Payment Method" `order`, trimmed to the
    /// fields SetupMandate reads: a `debit` card account next to an unrelated
    /// `ach` account.
    fn documented_card_order() -> PaynearmeStoredCredentialOrder {
        serde_json::from_value(json!({
            "order_is_standing": true,
            "customer": { "site_customer_identifier": "470070000" },
            "electronic_payments": {
                "payment_methods": [
                    { "type": "apple_pay_debit", "fee_amount": "4.99", "accounts": [] },
                    {
                        "type": "debit",
                        "accounts": [{
                            "payment_method_identifier": "a95ac4a03ef38",
                            "status": "active",
                            "account_type": "Debit",
                            "number": "7641",
                            "expiration_date": "06/2024",
                            "card_brand": "PULSE"
                        }]
                    },
                    {
                        "type": "ach",
                        "accounts": [{
                            "payment_method_identifier": "534be6d8afcf4",
                            "status": "active",
                            "number": "7016"
                        }]
                    }
                ]
            }
        }))
        .expect("documented order")
    }

    #[test]
    fn finds_the_stored_card_by_last_four_and_expiry() {
        let order = documented_card_order();
        assert_eq!(
            order
                .identify_card_token("7641", "06/2024", None)
                .expect("token")
                .peek(),
            "a95ac4a03ef38"
        );
        // Right last four, wrong expiry; and an ACH account's last four.
        assert!(order.identify_card_token("7641", "07/2024", None).is_err());
        assert!(order.identify_card_token("7016", "06/2024", None).is_err());
    }

    /// Two different active tokens for what looks like the same card must not be
    /// guessed between; an inactive duplicate does not count.
    #[test]
    fn refuses_to_guess_between_matching_cards() {
        let account = |token: &str, status: &str| {
            json!({
                "payment_method_identifier": token,
                "status": status,
                "number": "7641",
                "expiration_date": "06/2024"
            })
        };
        let order = |accounts: Value| -> PaynearmeStoredCredentialOrder {
            serde_json::from_value(json!({
                "electronic_payments": {
                    "payment_methods": [{ "type": "credit", "accounts": accounts }]
                }
            }))
            .expect("order")
        };

        let ambiguous = order(json!([
            account("a95ac4a03ef38", "active"),
            account("3b7ac9d4c86e6", "active")
        ]));
        assert!(ambiguous
            .identify_card_token("7641", "06/2024", None)
            .is_err());

        let one_retired = order(json!([
            account("a95ac4a03ef38", "active"),
            account("3b7ac9d4c86e6", "inactive")
        ]));
        assert_eq!(
            one_retired
                .identify_card_token("7641", "06/2024", None)
                .expect("token")
                .peek(),
            "a95ac4a03ef38"
        );
    }

    /// Two active cards with the same last four and expiry: the token the charged
    /// payment was made with picks between them, but only if it is one of them.
    #[test]
    fn the_charged_token_identifies_the_stored_card() {
        let order: PaynearmeStoredCredentialOrder = serde_json::from_value(json!({
            "electronic_payments": {
                "payment_methods": [{ "type": "credit", "accounts": [
                    { "payment_method_identifier": "a95ac4a03ef38", "status": "active",
                      "number": "7641", "expiration_date": "06/2024" },
                    { "payment_method_identifier": "3b7ac9d4c86e6", "status": "active",
                      "number": "7641", "expiration_date": "06/2024" }
                ]}]
            }
        }))
        .expect("order");

        let charged = Secret::new("3b7ac9d4c86e6".to_string());
        assert_eq!(
            order
                .identify_card_token("7641", "06/2024", Some(&charged))
                .expect("token")
                .peek(),
            "3b7ac9d4c86e6"
        );
        // Without the charged token the two cannot be told apart.
        assert!(order.identify_card_token("7641", "06/2024", None).is_err());
        // A charged token that is not a matching card account is not trusted.
        let unrelated = Secret::new("c5f2addb3aa90".to_string());
        assert!(order
            .identify_card_token("7641", "06/2024", Some(&unrelated))
            .is_err());
        // Right token, but the card sent had another expiry.
        assert!(order
            .identify_card_token("7641", "07/2024", Some(&charged))
            .is_err());
    }

    /// The documented "Create a Payment Method and Make a Payment" shape, trimmed
    /// and with a card account: the envelope Authorize always reads and the
    /// stored-credential view a customer-initiated mandate payment reads both come
    /// from the one body, and the charged token is read masked.
    #[test]
    fn reads_the_stored_card_from_a_store_and_charge_response() {
        let response: PaynearmeAuthorizeResponse = serde_json::from_value(json!({
            "status": "ok",
            "order": {
                "pnm_order_identifier": "89807002232",
                "order_is_standing": "true",
                "customer": { "site_customer_identifier": "820360000" },
                "electronic_payments": { "payment_methods": [
                    { "type": "credit", "accounts": [
                        { "payment_method_identifier": "3edd144056183", "status": "active",
                          "number": "6651", "expiration_date": "01/2027" }
                    ]}
                ]},
                "payments": [{
                    "payment_status": "approved",
                    "payment_method_identifier": "3edd144056183",
                    "pnm_payment_identifier": "477556596186"
                }]
            }
        }))
        .expect("response");

        let payment = response
            .envelope
            .orders
            .as_ref()
            .and_then(PaynearmeOrder::last_payment)
            .expect("payment");
        let order = response
            .stored_credential_order
            .as_ref()
            .expect("stored credential order parses")
            .as_ref()
            .expect("stored credential order present");
        assert_eq!(order.order_is_standing, Some(true));
        assert_eq!(
            order
                .identify_card_token(
                    "6651",
                    "01/2027",
                    payment.payment_method_identifier.as_ref()
                )
                .expect("token")
                .peek(),
            "3edd144056183"
        );
    }

    /// A stored-account list PayNearMe changes shape on must not take a one-off
    /// Authorize down: the envelope still parses and only the stored-credential
    /// view reports the problem, without quoting the offending value.
    #[test]
    fn a_malformed_account_list_only_fails_the_stored_credential_view() {
        let response: PaynearmeAuthorizeResponse = serde_json::from_value(json!({
            "status": "ok",
            "order": {
                "pnm_order_identifier": "89807002232",
                "electronic_payments": { "payment_methods": "not-a-list" },
                "payments": [{ "payment_status": "approved", "pnm_payment_identifier": "477556596186" }]
            }
        }))
        .expect("response");
        assert!(response.envelope.is_ok());
        let reason = response
            .stored_credential_order
            .as_ref()
            .expect_err("stored credential view fails");
        assert!(!reason.contains("not-a-list"), "the reason quotes the body");
    }

    /// The token `/make_payment` charges is masked wherever the payment is logged
    /// or serialised masked (`typed_connector_response` is), and is still readable
    /// for the stored-card match.
    #[test]
    fn the_payment_method_token_is_masked() {
        let payment: PaynearmePayment = serde_json::from_value(json!({
            "payment_status": "approved",
            "pnm_payment_identifier": PAYMENT,
            "payment_method_identifier": TOKEN
        }))
        .expect("payment");
        assert_eq!(
            payment
                .payment_method_identifier
                .as_ref()
                .map(|token| token.peek().as_str()),
            Some(TOKEN)
        );
        let masked = hyperswitch_masking::masked_serialize(&payment)
            .expect("masked")
            .to_string();
        assert!(
            !masked.contains(TOKEN),
            "token in the masked view: {masked}"
        );
        assert!(!format!("{payment:?}").contains(TOKEN));
    }

    /// The reference fits Hyperswitch's `VARCHAR(128)` `connector_mandate_id`, and
    /// one that would not is refused rather than truncated.
    #[test]
    fn mandate_reference_fits_the_connector_mandate_id_column() {
        let reference = PaynearmeMandateReference {
            payment_method_identifier: Secret::new("a95ac4a03ef38".to_string()),
            pnm_order_identifier: "86383382942".to_string(),
        };
        assert_eq!(
            reference.to_connector_mandate_id().expect("encoded"),
            r#"{"pmi":"a95ac4a03ef38","oid":"86383382942"}"#
        );

        let oversized = PaynearmeMandateReference {
            payment_method_identifier: Secret::new("f".repeat(CONNECTOR_MANDATE_ID_MAX_LEN)),
            pnm_order_identifier: "86383382942".to_string(),
        };
        assert!(oversized.to_connector_mandate_id().is_err());
    }

    /// The exact reference SetupMandate issued decodes, in any key order, and
    /// round-trips through the encoder.
    #[test]
    fn decodes_the_connector_mandate_id() {
        let issued = r#"{"pmi":"a95ac4a03ef38","oid":"86383382942"}"#;
        let reference =
            PaynearmeMandateReference::from_connector_mandate_id(issued).expect("reference");
        assert_eq!(reference.payment_method_identifier.peek(), "a95ac4a03ef38");
        assert_eq!(reference.pnm_order_identifier, "86383382942");
        assert_eq!(
            reference.to_connector_mandate_id().expect("encoded"),
            issued
        );

        let reordered = PaynearmeMandateReference::from_connector_mandate_id(
            r#"{ "oid": "86383382942", "pmi": "a95ac4a03ef38" }"#,
        )
        .expect("reordered reference");
        assert_eq!(reordered.pnm_order_identifier, "86383382942");
    }

    /// Anything that could not have come from this connector is refused rather
    /// than sent to `/make_payment`: a bare token, non-JSON, missing or empty
    /// identifiers, the wrong JSON shape, and an over-length value.
    #[test]
    fn refuses_unusable_connector_mandate_ids() {
        for raw in [
            "a95ac4a03ef38",
            "",
            "{",
            r#""a95ac4a03ef38""#,
            r#"["a95ac4a03ef38","86383382942"]"#,
            r#"{"pmi":"a95ac4a03ef38"}"#,
            r#"{"oid":"86383382942"}"#,
            r#"{"pmi":"","oid":"86383382942"}"#,
            r#"{"pmi":"a95ac4a03ef38","oid":""}"#,
            r#"{"pmi":13,"oid":"86383382942"}"#,
        ] {
            assert!(
                PaynearmeMandateReference::from_connector_mandate_id(raw).is_err(),
                "accepted {raw:?}"
            );
        }

        let oversized = format!(
            r#"{{"pmi":"{}","oid":"86383382942"}}"#,
            "f".repeat(CONNECTOR_MANDATE_ID_MAX_LEN)
        );
        let reason = PaynearmeMandateReference::from_connector_mandate_id(&oversized)
            .expect_err("oversized reference refused");
        assert!(!reason.contains("ffff"), "the reason echoes the token");
    }

    /// Every `MandateReferenceId` variant is decided explicitly: the PayNearMe
    /// `connector_mandate_id` decodes; a malformed one is the caller's data error
    /// (`InvalidDataFormat`, gRPC `InvalidArgument`), an absent one a missing
    /// field; both network-transaction-id variants are `NotSupported`.
    #[test]
    fn mandate_reference_variants() {
        let reference =
            PaynearmeMandateReference::try_from(&connector_mandate(MANDATE)).expect("reference");
        assert_eq!(reference.payment_method_identifier.peek(), TOKEN);
        assert_eq!(reference.pnm_order_identifier, ORDER);

        let report = PaynearmeMandateReference::try_from(&connector_mandate("not-a-reference"))
            .expect_err("malformed reference refused");
        assert!(matches!(
            report.current_context(),
            IntegrationError::InvalidDataFormat { field_name: "connector_mandate_id", context }
                if context.additional_context.is_some()
        ));
        assert!(!format!("{report:?}").contains("not-a-reference"));

        let absent = MandateReferenceId::ConnectorMandateId(ConnectorMandateReferenceId::new(
            None, None, None, None, None,
        ));
        assert!(matches!(
            PaynearmeMandateReference::try_from(&absent)
                .expect_err("absent reference refused")
                .current_context(),
            IntegrationError::MissingRequiredField {
                field_name: "connector_mandate_id",
                ..
            }
        ));

        let network_transaction_id = MandateReferenceId::NetworkMandateId(NetworkMandateIdRef {
            network_transaction_id: "016153570198200".to_string(),
            transaction_link_id: None,
        });
        let network_token = MandateReferenceId::NetworkTokenWithNTI(NetworkTokenWithNTIRef {
            network_transaction_id: "016153570198200".to_string(),
            transaction_link_id: None,
            token_exp_month: None,
            token_exp_year: None,
        });
        for nti in [network_transaction_id, network_token] {
            assert!(matches!(
                PaynearmeMandateReference::try_from(&nti)
                    .expect_err("NTI refused")
                    .current_context(),
                IntegrationError::NotSupported {
                    connector: PAYNEARME,
                    ..
                }
            ));
        }
    }

    // =========================================================================
    // FIXTURES FOR THE FLOW TESTS
    // =========================================================================

    const CUSTOMER: &str = "cus_pnm_0001";
    const ORDER: &str = "53171609947";
    const TOKEN: &str = "62f83581aa10d";
    const PAYMENT: &str = "883566157672";
    const MANDATE: &str = r#"{"pmi":"62f83581aa10d","oid":"53171609947"}"#;

    fn major_amount(amount: &str) -> StringMajorUnit {
        StringMajorUnit::deserialize(Value::String(amount.to_string())).expect("amount")
    }

    fn auth(api_secret_key: &str, site_identifier: &str) -> PaynearmeAuthType {
        PaynearmeAuthType {
            api_secret_key: Secret::new(api_secret_key.to_string()),
            site_identifier: Secret::new(site_identifier.to_string()),
        }
    }

    fn connector() -> &'static Paynearme<DefaultPCIHolder> {
        Paynearme::new()
    }

    fn billing() -> Address {
        Address {
            address: Some(AddressDetails {
                city: None,
                country: None,
                line1: Some(Secret::new("123 Fake Street".to_string())),
                line2: None,
                line3: None,
                zip: Some(Secret::new("75013".to_string())),
                state: None,
                first_name: Some(Secret::new("John".to_string())),
                last_name: Some(Secret::new("Smith".to_string())),
                origin_zip: None,
            }),
            phone: Some(PhoneDetails {
                number: Some(Secret::new("4695555878".to_string())),
                country_code: Some("+1".to_string()),
            }),
            email: None,
        }
    }

    fn flow_data(customer_id: Option<&str>, connector_order_id: Option<&str>) -> PaymentFlowData {
        PaymentFlowData {
            merchant_id: MerchantId::default(),
            customer_id: customer_id
                .map(|id| CustomerId::try_from(Cow::from(id)).expect("customer id")),
            connector_customer: None,
            payment_id: "pay_pnm_0001".to_string(),
            attempt_id: "pay_pnm_0001_1".to_string(),
            status: AttemptStatus::Pending,
            payment_method: PaymentMethod::Card,
            payment_method_type: None,
            description: None,
            return_url: None,
            address: PaymentAddress::new(None, Some(billing()), Some(billing()), Some(false)),
            auth_type: AuthenticationType::NoThreeDs,
            connector_feature_data: None,
            amount_captured: None,
            minor_amount_captured: None,
            minor_amount_capturable: None,
            amount: None,
            access_token: None,
            session_token: None,
            reference_id: None,
            connector_order_id: connector_order_id.map(str::to_string),
            preprocessing_id: None,
            connector_api_version: None,
            connector_request_reference_id: "pay_pnm_0001_1".to_string(),
            test_mode: Some(true),
            connector_http_status_code: None,
            connector_response_headers: None,
            external_latency: None,
            connectors: Connectors::default().into(),
            raw_connector_response: None,
            typed_connector_response: None,
            raw_connector_request: None,
            typed_connector_request: None,
            vault_headers: None,
            connector_response: None,
            recurring_mandate_payment_data: None,
            order_details: None,
            minor_amount_authorized: None,
            l2_l3_data: None,
            merchant_request_id: None,
            sender_payment_instrument_id: None,
            connector_returned_payment_method_details: None,
            settlement_status: None,
            raw_connector_status: None,
        }
    }

    /// The PayNearMe documented test card the mock also uses: last four 7884.
    fn card() -> PaymentMethodData<DefaultPCIHolder> {
        PaymentMethodData::Card(Card {
            card_number: RawCardNumber(CardNumber::from_str("9999915317077884").expect("card")),
            card_exp_month: Secret::new("05".to_string()),
            card_exp_year: Secret::new("2029".to_string()),
            card_cvc: Secret::new("123".to_string()),
            card_holder_name: Some(Secret::new("John Smith".to_string())),
            ..Default::default()
        })
    }

    fn authentication_data() -> AuthenticationData {
        AuthenticationData {
            trans_status: None,
            eci: Some("05".to_string()),
            cavv: Some(Secret::new("cavv-value".to_string())),
            ucaf_collection_indicator: None,
            threeds_server_transaction_id: None,
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

    fn authorize_data(
        setup_future_usage: Option<FutureUsage>,
        customer_acceptance: bool,
    ) -> PaymentsAuthorizeData<DefaultPCIHolder> {
        PaymentsAuthorizeData {
            payment_method_data: card(),
            amount: MinorUnit::new(1000),
            order_tax_amount: None,
            surcharge_amount: None,
            email: None,
            customer_document_details: None,
            customer_date_of_birth: None,
            customer_name: None,
            currency: Currency::USD,
            confirm: true,
            billing_descriptor: None,
            capture_method: Some(CaptureMethod::Automatic),
            router_return_url: None,
            webhook_url: None,
            complete_authorize_url: None,
            mandate_id: None,
            setup_future_usage,
            off_session: None,
            browser_info: None,
            order_category: None,
            session_token: None,
            access_token: None,
            customer_acceptance: customer_acceptance.then(CustomerAcceptance::default),
            enrolled_for_3ds: Some(true),
            related_transaction_id: None,
            payment_experience: None,
            payment_method_type: None,
            customer_id: None,
            request_incremental_authorization: None,
            metadata: None,
            authentication_data: None,
            split_payments: None,
            split_settlement: None,
            minor_amount: MinorUnit::new(1000),
            merchant_order_id: None,
            shipping_cost: None,
            merchant_account_id: None,
            integrity_object: None,
            merchant_config_currency: None,
            all_keys_required: None,
            request_extended_authorization: None,
            enable_overcapture: None,
            setup_mandate_details: None,
            connector_feature_data: None,
            connector_testing_data: None,
            payment_channel: None,
            enable_partial_authorization: None,
            locale: None,
            redirect_response: None,
            threeds_method_comp_ind: None,
            continue_redirection_url: None,
            tokenization: None,
            mit_category: None,
            domain_data: None,
            partner_merchant_identifier_details: None,
            currency_conversion_data: None,
            is_account_funding_transaction: None,
            recipient_details: None,
            additional_connector_details: None,
            customer: None,
            business_country: None,
        }
    }

    fn setup_mandate_data(minor_amount: i64) -> SetupMandateRequestData<DefaultPCIHolder> {
        SetupMandateRequestData {
            currency: Currency::USD,
            payment_method_data: card(),
            amount: Some(minor_amount),
            confirm: true,
            billing_descriptor: None,
            customer_acceptance: Some(CustomerAcceptance::default()),
            mandate_id: None,
            setup_future_usage: Some(FutureUsage::OffSession),
            off_session: None,
            setup_mandate_details: None,
            router_return_url: None,
            webhook_url: None,
            browser_info: None,
            email: None,
            customer_document_details: None,
            customer_name: None,
            return_url: None,
            payment_method_type: None,
            request_incremental_authorization: false,
            metadata: None,
            complete_authorize_url: None,
            capture_method: None,
            merchant_order_id: None,
            minor_amount: Some(MinorUnit::new(minor_amount)),
            shipping_cost: None,
            customer_id: None,
            integrity_object: None,
            payment_channel: None,
            enable_partial_authorization: None,
            locale: None,
            connector_testing_data: None,
            mit_category: None,
            split_payments: None,
            authentication_data: None,
            partner_merchant_identifier_details: None,
            is_account_funding_transaction: None,
            recipient_details: None,
            additional_connector_details: None,
            customer: None,
        }
    }

    fn connector_mandate(connector_mandate_id: &str) -> MandateReferenceId {
        MandateReferenceId::ConnectorMandateId(ConnectorMandateReferenceId::new(
            Some(connector_mandate_id.to_string()),
            None,
            None,
            None,
            None,
        ))
    }

    fn repeat_payment_data(
        mandate_reference: MandateReferenceId,
    ) -> RepeatPaymentData<DefaultPCIHolder> {
        RepeatPaymentData {
            mandate_reference,
            amount: 2500,
            minor_amount: MinorUnit::new(2500),
            currency: Currency::USD,
            merchant_order_id: None,
            metadata: None,
            webhook_url: None,
            integrity_object: None,
            capture_method: Some(CaptureMethod::Automatic),
            browser_info: None,
            email: None,
            customer_document_details: None,
            payment_method_type: None,
            connector_feature_data: None,
            off_session: Some(true),
            router_return_url: None,
            complete_authorize_url: None,
            split_payments: None,
            split_settlement: None,
            recurring_mandate_payment_data: None,
            shipping_cost: None,
            payment_channel: None,
            mit_category: None,
            enable_partial_authorization: None,
            billing_descriptor: None,
            payment_method_data: PaymentMethodData::MandatePayment,
            authentication_data: None,
            locale: None,
            connector_testing_data: None,
            merchant_account_id: None,
            merchant_configured_currency: None,
            additional_payment_data: None,
            partner_merchant_identifier_details: None,
            is_account_funding_transaction: None,
            recipient_details: None,
            additional_connector_details: None,
            customer: None,
        }
    }

    fn router_data<F, Req, Res>(
        common: PaymentFlowData,
        request: Req,
    ) -> RouterDataV2<F, PaymentFlowData, Req, Res> {
        RouterDataV2 {
            flow: PhantomData,
            resource_common_data: common,
            connector_config: ConnectorSpecificConfig::Paynearme {
                api_key: Secret::new("dummy_api_secret_key".to_string()),
                key1: Secret::new("S0000000001".to_string()),
                base_url: None,
            },
            request,
            response: Err(ErrorResponse::default()),
        }
    }

    type AuthorizeData = RouterDataV2<
        Authorize,
        PaymentFlowData,
        PaymentsAuthorizeData<DefaultPCIHolder>,
        PaymentsResponseData,
    >;
    type SetupMandateData = RouterDataV2<
        SetupMandate,
        PaymentFlowData,
        SetupMandateRequestData<DefaultPCIHolder>,
        PaymentsResponseData,
    >;
    type RepeatPaymentRouterData = RouterDataV2<
        RepeatPayment,
        PaymentFlowData,
        RepeatPaymentData<DefaultPCIHolder>,
        PaymentsResponseData,
    >;
    type CreateOrderData = RouterDataV2<
        CreateOrder,
        PaymentFlowData,
        PaymentCreateOrderData,
        PaymentCreateOrderResponse,
    >;

    /// The JSON body the connector sends for this router data.
    fn request_json<F, Req, Res>(
        router_data: &RouterDataV2<F, PaymentFlowData, Req, Res>,
    ) -> Result<Value, error_stack::Report<IntegrationError>>
    where
        Paynearme<DefaultPCIHolder>: ConnectorIntegrationV2<F, PaymentFlowData, Req, Res>,
    {
        let content = connector()
            .get_request_body(router_data)?
            .expect("request body")
            .content;
        match content {
            RequestContent::Json(payload) => Ok(serde_json::to_value(&payload).expect("json")),
            _ => panic!("expected a JSON body"),
        }
    }

    /// Runs a 201 PayNearMe response through the connector's response handler.
    fn handle<F, Req, Res>(
        router_data: &RouterDataV2<F, PaymentFlowData, Req, Res>,
        body: Value,
    ) -> Result<RouterDataV2<F, PaymentFlowData, Req, Res>, error_stack::Report<ConnectorError>>
    where
        F: Clone,
        Req: Clone,
        Res: Clone,
        Paynearme<DefaultPCIHolder>: ConnectorIntegrationV2<F, PaymentFlowData, Req, Res>,
    {
        connector().handle_response_v2(
            router_data,
            None,
            Response {
                headers: None,
                response: body.to_string().into(),
                status_code: 201,
            },
        )
    }

    /// A `/create_payment_method` store-and-charge response in the documented shape.
    fn store_and_charge_response(
        order_is_standing: &str,
        site_customer_identifier: &str,
        card_accounts: Value,
        payments: Value,
    ) -> Value {
        json!({
            "status": "ok",
            "order": {
                "pnm_order_identifier": ORDER,
                "order_is_standing": order_is_standing,
                "customer": { "site_customer_identifier": site_customer_identifier },
                "electronic_payments": { "payment_methods": [
                    { "type": "debit", "accounts": [] },
                    { "type": "credit", "accounts": card_accounts }
                ]},
                "payments": payments
            }
        })
    }

    /// The account `/create_payment_method` created for [`card`].
    fn stored_card_account() -> Value {
        json!([{
            "payment_method_identifier": TOKEN,
            "status": "active",
            "number": "7884",
            "expiration_date": "05/2029"
        }])
    }

    fn approved_payment(payment_method_identifier: Option<&str>) -> Value {
        let mut payment = json!({
            "payment_status": "approved",
            "pnm_payment_identifier": PAYMENT,
            "payment_amount": "10.00",
            "payment_type": "credit"
        });
        if let Some(token) = payment_method_identifier {
            payment["payment_method_identifier"] = json!(token);
        }
        json!([payment])
    }

    fn transaction_response(
        result: &Result<PaymentsResponseData, ErrorResponse>,
    ) -> (&ResponseId, Option<String>, &Option<Value>, &Option<String>) {
        match result {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id,
                mandate_reference,
                connector_metadata,
                connector_response_reference_id,
                ..
            }) => (
                resource_id,
                mandate_reference
                    .as_ref()
                    .and_then(|reference| reference.connector_mandate_id.clone()),
                connector_metadata,
                connector_response_reference_id,
            ),
            other => panic!("expected a TransactionResponse, got {other:?}"),
        }
    }

    // =========================================================================
    // AUTHORIZE: CUSTOMER-INITIATED MANDATE PAYMENTS
    // =========================================================================

    /// The card is stored exactly when Hyperswitch persists the mandate:
    /// `OffSession` with `customer_acceptance` (or `setup_mandate_details`). Only
    /// then is `customer.id` required, before any card data is sent; an absent or
    /// empty one is the same `MissingRequiredField`. `OffSession` alone is a plain
    /// one-off payment.
    #[test]
    fn a_customer_initiated_mandate_payment_requires_customer_id() {
        for customer_id in [None, Some("")] {
            let router_data: AuthorizeData = router_data(
                flow_data(customer_id, Some(ORDER)),
                authorize_data(Some(FutureUsage::OffSession), true),
            );
            let report = request_json(&router_data).expect_err("CIT without customer.id");
            assert!(matches!(
                report.current_context(),
                IntegrationError::MissingRequiredField {
                    field_name: "customer.id",
                    ..
                }
            ));
        }

        for (setup_future_usage, customer_acceptance) in [
            (Some(FutureUsage::OffSession), false),
            (Some(FutureUsage::OnSession), true),
            (None, false),
        ] {
            let router_data: AuthorizeData = router_data(
                flow_data(None, Some(ORDER)),
                authorize_data(setup_future_usage, customer_acceptance),
            );
            let body = request_json(&router_data).expect("one-off body");
            assert_eq!(body["send_payment"], "true");
            assert_eq!(body["pnm_order_identifier"], ORDER);
        }
    }

    /// Charged and stored: the payment is reported under its own id with the
    /// stored card as the mandate reference, and the token is not copied into
    /// `connector_metadata`.
    #[test]
    fn a_charged_and_stored_cit_returns_the_mandate() {
        let router_data: AuthorizeData = router_data(
            flow_data(Some(CUSTOMER), Some(ORDER)),
            authorize_data(Some(FutureUsage::OffSession), true),
        );
        let result = handle(
            &router_data,
            store_and_charge_response(
                "true",
                CUSTOMER,
                stored_card_account(),
                approved_payment(Some(TOKEN)),
            ),
        )
        .expect("handled");

        assert_eq!(result.resource_common_data.status, AttemptStatus::Charged);
        let (resource_id, mandate, metadata, reference) = transaction_response(&result.response);
        assert!(matches!(resource_id, ResponseId::ConnectorTransactionId(id) if id == PAYMENT));
        assert_eq!(mandate.as_deref(), Some(MANDATE));
        assert_eq!(metadata, &Some(json!({ "pnm_order_identifier": ORDER })));
        assert_eq!(reference.as_deref(), Some(ORDER));
    }

    /// Charged but the stored card cannot be identified (the order is not standing,
    /// belongs to another customer, or no matching account is listed): the charge
    /// is reported as it is, `Charged` under its own payment id, with no mandate.
    /// Never `Failure`: Hyperswitch can neither void nor refund a failed payment.
    #[test]
    fn a_charged_cit_whose_card_cannot_be_stored_stays_charged_without_a_mandate() {
        for (order_is_standing, owner, accounts, payments) in [
            (
                "false",
                CUSTOMER,
                stored_card_account(),
                approved_payment(Some(TOKEN)),
            ),
            (
                "true",
                "cus_someone_else",
                stored_card_account(),
                approved_payment(Some(TOKEN)),
            ),
            ("true", CUSTOMER, json!([]), approved_payment(None)),
        ] {
            let router_data: AuthorizeData = router_data(
                flow_data(Some(CUSTOMER), Some(ORDER)),
                authorize_data(Some(FutureUsage::OffSession), true),
            );
            let result = handle(
                &router_data,
                store_and_charge_response(order_is_standing, owner, accounts, payments),
            )
            .expect("handled");

            assert_eq!(result.resource_common_data.status, AttemptStatus::Charged);
            let (resource_id, mandate, _, reference) = transaction_response(&result.response);
            assert!(matches!(resource_id, ResponseId::ConnectorTransactionId(id) if id == PAYMENT));
            assert_eq!(mandate, None);
            assert_eq!(reference.as_deref(), Some(ORDER));
        }
    }

    /// Nothing charged and nothing stored: the attempt fails, with the reason and
    /// no transaction id.
    #[test]
    fn a_cit_that_charged_nothing_and_stored_nothing_fails() {
        let router_data: AuthorizeData = router_data(
            flow_data(Some(CUSTOMER), Some(ORDER)),
            authorize_data(Some(FutureUsage::OffSession), true),
        );
        let result = handle(
            &router_data,
            store_and_charge_response("false", CUSTOMER, json!([]), json!([])),
        )
        .expect("handled");

        assert_eq!(result.resource_common_data.status, AttemptStatus::Failure);
        let error = result.response.expect_err("failed attempt");
        assert_eq!(
            error.attempt_status,
            Some(FlowStatus::Payment(AttemptStatus::Failure))
        );
        assert_eq!(error.connector_transaction_id, None);
        assert!(
            error.message.contains("nothing was charged"),
            "{}",
            error.message
        );
        assert!(!error.message.contains("void or refund"));
    }

    /// `OffSession` without `customer_acceptance`: a one-off. Even a response that
    /// lists a matching card on a standing order yields no mandate, because
    /// Hyperswitch would not persist one.
    #[test]
    fn an_off_session_payment_without_acceptance_is_a_one_off() {
        let router_data: AuthorizeData = router_data(
            flow_data(None, Some(ORDER)),
            authorize_data(Some(FutureUsage::OffSession), false),
        );
        let result = handle(
            &router_data,
            store_and_charge_response(
                "true",
                CUSTOMER,
                stored_card_account(),
                approved_payment(Some(TOKEN)),
            ),
        )
        .expect("handled");

        assert_eq!(result.resource_common_data.status, AttemptStatus::Charged);
        let (resource_id, mandate, _, _) = transaction_response(&result.response);
        assert!(matches!(resource_id, ResponseId::ConnectorTransactionId(id) if id == PAYMENT));
        assert_eq!(mandate, None);
    }

    // =========================================================================
    // SETUP MANDATE
    // =========================================================================

    /// Everything SetupMandate refuses is refused before any card data is sent.
    #[test]
    fn setup_mandate_request_guards() {
        let refused = |common: PaymentFlowData, request| {
            let router_data: SetupMandateData = router_data(common, request);
            request_json(&router_data).expect_err("refused")
        };

        let non_zero = refused(
            flow_data(Some(CUSTOMER), Some(ORDER)),
            setup_mandate_data(100),
        );
        assert!(matches!(
            non_zero.current_context(),
            IntegrationError::NotSupported { message, .. } if message.contains("non-zero amount")
        ));

        let mut three_ds = flow_data(Some(CUSTOMER), Some(ORDER));
        three_ds.auth_type = AuthenticationType::ThreeDs;
        assert!(matches!(
            refused(three_ds, setup_mandate_data(0)).current_context(),
            IntegrationError::NotSupported { message, .. } if message.contains("Three DS")
        ));

        let mut with_authentication = setup_mandate_data(0);
        with_authentication.authentication_data = Some(authentication_data());
        assert!(matches!(
            refused(flow_data(Some(CUSTOMER), Some(ORDER)), with_authentication).current_context(),
            IntegrationError::NotSupported { .. }
        ));

        for customer_id in [None, Some("")] {
            assert!(matches!(
                refused(flow_data(customer_id, Some(ORDER)), setup_mandate_data(0))
                    .current_context(),
                IntegrationError::MissingRequiredField {
                    field_name: "customer.id",
                    ..
                }
            ));
        }

        assert!(matches!(
            refused(flow_data(Some(CUSTOMER), None), setup_mandate_data(0)).current_context(),
            IntegrationError::MissingRequiredField {
                field_name: "order_id",
                context
            } if context.suggested_action.is_some()
        ));

        let router_data: SetupMandateData = router_data(
            flow_data(Some(CUSTOMER), Some(ORDER)),
            setup_mandate_data(0),
        );
        let body = request_json(&router_data).expect("tokenise-only body");
        assert_eq!(body["pnm_order_identifier"], ORDER);
        assert!(body.get("send_payment").is_none());
        assert!(body.get("payment_amount").is_none());
    }

    fn setup_mandate_response(order_is_standing: bool, owner: &str) -> Value {
        json!({
            "response_code": "0",
            "status": "ok",
            "order": {
                "order_is_standing": order_is_standing,
                "customer": { "site_customer_identifier": owner },
                "electronic_payments": { "payment_methods": [
                    { "type": "credit", "accounts": stored_card_account() }
                ]}
            }
        })
    }

    /// A stored card becomes a mandate only on a standing order owned by
    /// `customer.id`; otherwise the tokenise-only setup (nothing charged) fails.
    #[test]
    fn setup_mandate_stores_only_on_a_standing_order_of_this_customer() {
        let router_data: SetupMandateData = router_data(
            flow_data(Some(CUSTOMER), Some(ORDER)),
            setup_mandate_data(0),
        );
        let stored = handle(&router_data, setup_mandate_response(true, CUSTOMER)).expect("handled");
        assert_eq!(stored.resource_common_data.status, AttemptStatus::Charged);
        let (resource_id, mandate, metadata, reference) = transaction_response(&stored.response);
        assert!(matches!(resource_id, ResponseId::NoResponseId));
        assert_eq!(mandate.as_deref(), Some(MANDATE));
        assert_eq!(metadata, &None);
        assert_eq!(reference.as_deref(), Some(ORDER));

        for (order_is_standing, owner, expected) in [
            (false, CUSTOMER, "is not a standing order"),
            (true, "cus_someone_else", "does not belong to customer.id"),
        ] {
            let failed = handle(
                &router_data,
                setup_mandate_response(order_is_standing, owner),
            )
            .expect("handled");
            assert_eq!(failed.resource_common_data.status, AttemptStatus::Failure);
            let error = failed.response.expect_err("refused");
            assert_eq!(
                error.attempt_status,
                Some(FlowStatus::Payment(AttemptStatus::Failure))
            );
            assert!(error.message.contains(expected), "{}", error.message);
        }
    }

    // =========================================================================
    // REPEAT PAYMENT
    // =========================================================================

    /// `/make_payment` responses: the payment's own status and id; `authorized`
    /// is `Pending`; a declared error or a `rejected` payment is a terminal
    /// `Failure` carrying PayNearMe's code. No mandate is echoed and the token is
    /// not copied into `connector_metadata`.
    #[test]
    fn repeat_payment_maps_the_make_payment_response() {
        let router_data: RepeatPaymentRouterData = router_data(
            flow_data(Some(CUSTOMER), None),
            repeat_payment_data(connector_mandate(MANDATE)),
        );
        let payment = |status: &str| {
            json!({ "status": "ok", "payment": {
                "payment_status": status,
                "pnm_payment_identifier": PAYMENT,
                "payment_method_identifier": TOKEN,
                "payment_amount": "25.00"
            }})
        };

        for (status, expected) in [
            ("approved", AttemptStatus::Charged),
            ("authorized", AttemptStatus::Pending),
        ] {
            let result = handle(&router_data, payment(status)).expect("handled");
            assert_eq!(result.resource_common_data.status, expected);
            let (resource_id, mandate, metadata, reference) =
                transaction_response(&result.response);
            assert!(matches!(resource_id, ResponseId::ConnectorTransactionId(id) if id == PAYMENT));
            assert_eq!(mandate, None);
            assert_eq!(metadata, &Some(json!({ "pnm_order_identifier": ORDER })));
            assert_eq!(reference.as_deref(), Some(ORDER));
        }

        let declined = handle(
            &router_data,
            json!({
                "status": "error",
                "response_code": "1051",
                "errors": ["There are insufficient funds on this card."]
            }),
        )
        .expect("handled");
        assert_eq!(declined.resource_common_data.status, AttemptStatus::Failure);
        let error = declined.response.expect_err("declined");
        assert_eq!(error.code, "1051");
        assert_eq!(
            error.attempt_status,
            Some(FlowStatus::Payment(AttemptStatus::Failure))
        );

        let rejected = handle(&router_data, payment("rejected")).expect("handled");
        assert_eq!(rejected.resource_common_data.status, AttemptStatus::Failure);
        let error = rejected.response.expect_err("rejected");
        assert_eq!(error.connector_transaction_id.as_deref(), Some(PAYMENT));
        assert!(error.message.contains("rejected"), "{}", error.message);
    }

    /// An ok `/make_payment` response without `payment.pnm_payment_identifier`
    /// does not match the documented response. It is neither reported `Pending`
    /// under the shared standing order id (which PSync cannot look up) nor
    /// declared failed (the money may have moved): no status is claimed.
    #[test]
    fn repeat_payment_without_a_payment_identifier_claims_no_status() {
        let router_data: RepeatPaymentRouterData = router_data(
            flow_data(Some(CUSTOMER), None),
            repeat_payment_data(connector_mandate(MANDATE)),
        );
        for body in [
            json!({ "status": "ok", "payment": { "payment_status": "approved" } }),
            json!({ "status": "ok", "payment": { "payment_status": "approved", "pnm_payment_identifier": "" } }),
            json!({ "status": "ok" }),
        ] {
            let report = handle(&router_data, body).expect_err("no status claimed");
            assert!(matches!(
                report.current_context(),
                ConnectorError::ResponseHandlingFailed { .. }
            ));
            let detail = report
                .frames()
                .find_map(|frame| match frame.downcast_ref::<ConnectorError>() {
                    Some(ConnectorError::ResponseDeserializationFailed { context }) => {
                        context.additional_context.clone()
                    }
                    _ => None,
                })
                .expect("response deserialization context");
            assert!(detail.contains("pnm_payment_identifier"), "{detail}");
            assert!(!detail.contains(ORDER), "the order id is not reported");
        }
    }

    /// An off-session charge is not refused for `auth_type = ThreeDs` (Hyperswitch
    /// sends the payment's stored authentication type on every Charge), only for
    /// 3-D Secure results PayNearMe could not carry. Guards run before any request.
    #[test]
    fn repeat_payment_refuses_authentication_data_but_not_three_ds_auth_type() {
        let mut three_ds = flow_data(Some(CUSTOMER), None);
        three_ds.auth_type = AuthenticationType::ThreeDs;
        let three_ds_charge: RepeatPaymentRouterData =
            router_data(three_ds, repeat_payment_data(connector_mandate(MANDATE)));
        let body = request_json(&three_ds_charge).expect("MIT body");
        assert_eq!(body["pnm_order_identifier"], ORDER);
        assert_eq!(body["payment_method_identifier"], TOKEN);
        assert_eq!(body["recurring"], "true");
        assert_eq!(body["payment_amount"], "25.00");

        let mut with_authentication = repeat_payment_data(connector_mandate(MANDATE));
        with_authentication.authentication_data = Some(authentication_data());
        let authenticated_charge: RepeatPaymentRouterData =
            router_data(flow_data(Some(CUSTOMER), None), with_authentication);
        assert!(matches!(
            request_json(&authenticated_charge)
                .expect_err("authentication data refused")
                .current_context(),
            IntegrationError::NotSupported { .. }
        ));

        let mut manual = repeat_payment_data(connector_mandate(MANDATE));
        manual.capture_method = Some(CaptureMethod::Manual);
        let manual_charge: RepeatPaymentRouterData =
            router_data(flow_data(Some(CUSTOMER), None), manual);
        assert!(matches!(
            request_json(&manual_charge)
                .expect_err("manual capture refused")
                .current_context(),
            IntegrationError::NotSupported { .. }
        ));
    }

    // =========================================================================
    // WALLETS
    // =========================================================================

    /// Both Apple Pay token forms are refused as `NotSupported`, each with its own
    /// message, and neither error carries the DPAN, cryptogram or encrypted token.
    #[test]
    fn refuses_apple_pay_in_either_token_form_without_leaking_token_data() {
        use domain_types::payment_method_data::{
            ApplePayCryptogramData, ApplePayDecryptedData, ApplepayPaymentMethod,
        };

        const DPAN: &str = "4111111111111111";
        const CRYPTOGRAM: &str = "AgAAAAAABk4DWZ4C28yUQAAAAAA=";
        const ENCRYPTED_TOKEN: &str = "eyJkYXRhIjoiYXBwbGUtZW5jcnlwdGVkLXRva2VuIn0=";

        let wallet = |payment_data| ApplePayWalletData {
            payment_data,
            payment_method: ApplepayPaymentMethod {
                display_name: "Visa 1111".to_string(),
                network: "Visa".to_string(),
                pm_type: "debit".to_string(),
            },
            transaction_identifier: "apple_pay_txn_001".to_string(),
        };
        let decrypted = wallet(ApplePayPaymentData::Decrypted(ApplePayDecryptedData {
            application_primary_account_number: CardNumber::try_from(DPAN.to_string())
                .expect("dpan"),
            application_expiration_month: Secret::new("12".to_string()),
            application_expiration_year: Secret::new("2030".to_string()),
            payment_data: ApplePayCryptogramData {
                online_payment_cryptogram: Secret::new(CRYPTOGRAM.to_string()),
                eci_indicator: Some("05".to_string()),
            },
        }));
        let encrypted = wallet(ApplePayPaymentData::Encrypted(ENCRYPTED_TOKEN.to_string()));

        for (apple_pay, variant) in [
            (decrypted, "Hyperswitch-decrypted"),
            (encrypted, "Apple-encrypted"),
        ] {
            let report = apple_pay_not_supported(&apple_pay);
            assert!(matches!(
                report.current_context(),
                IntegrationError::NotSupported { message, connector: PAYNEARME, .. }
                    if message.contains(variant)
            ));
            let rendered = format!("{report:?}");
            for token_data in [DPAN, CRYPTOGRAM, ENCRYPTED_TOKEN] {
                assert!(!rendered.contains(token_data));
            }
        }
    }

    /// Google Pay is refused as `NotSupported` in every token form: decrypted with a
    /// cryptogram and ECI (`CRYPTOGRAM_3DS`), decrypted without them (`PAN_ONLY`),
    /// and encrypted. The error points at the payment-methods matrix and carries
    /// none of the PAN, cryptogram or encrypted token.
    #[test]
    fn refuses_google_pay_in_every_token_form_without_leaking_token_data() {
        use domain_types::payment_method_data::{
            GooglePayDecryptedData, GooglePayPaymentMethodInfo, GpayEncryptedTokenizationData,
        };

        const DPAN: &str = "4761739001010010";
        const CRYPTOGRAM: &str = "AgAAAAAAAIR8CQrXcIhbQAAAAAA=";
        const ENCRYPTED_TOKEN: &str = "eyJzaWduZWRNZXNzYWdlIjoiZ29vZ2xlLWVuY3J5cHRlZCJ9";

        let wallet = |tokenization_data| GooglePayWalletData {
            pm_type: "CARD".to_string(),
            description: "Visa 0010".to_string(),
            info: GooglePayPaymentMethodInfo {
                card_network: "VISA".to_string(),
                card_details: "0010".to_string(),
                assurance_details: None,
            },
            tokenization_data,
        };
        let decrypted = |cryptogram: Option<&str>, eci_indicator: Option<&str>| {
            wallet(GpayTokenizationData::Decrypted(GooglePayDecryptedData {
                card_exp_month: Secret::new("12".to_string()),
                card_exp_year: Secret::new("2030".to_string()),
                application_primary_account_number: CardNumber::try_from(DPAN.to_string())
                    .expect("dpan"),
                cryptogram: cryptogram.map(|value| Secret::new(value.to_string())),
                eci_indicator: eci_indicator.map(str::to_string),
            }))
        };
        let cryptogram_3ds = decrypted(Some(CRYPTOGRAM), Some("05"));
        let pan_only = decrypted(None, None);
        let encrypted = wallet(GpayTokenizationData::Encrypted(
            GpayEncryptedTokenizationData {
                token_type: "PAYMENT_GATEWAY".to_string(),
                token: ENCRYPTED_TOKEN.to_string(),
            },
        ));

        for (google_pay, variant) in [
            (cryptogram_3ds, "Hyperswitch-decrypted"),
            (pan_only, "Hyperswitch-decrypted"),
            (encrypted, "Google-encrypted"),
        ] {
            let report = google_pay_not_supported(&google_pay);
            assert!(matches!(
                report.current_context(),
                IntegrationError::NotSupported { message, connector: PAYNEARME, context }
                    if message.starts_with("Google Pay")
                        && message.contains(variant)
                        && context.doc_url.as_deref()
                            == Some("https://apidocs.paynearme.com/devdocs/docs/payment-methods-matrix")
                        && context
                            .additional_context
                            .as_deref()
                            .is_some_and(|row| row.contains("GooglePay"))
                        && context.suggested_action.is_some()
            ));
            let rendered = format!("{report:?} {}", report.current_context());
            for token_data in [DPAN, CRYPTOGRAM, ENCRYPTED_TOKEN] {
                assert!(!rendered.contains(token_data));
            }
        }
    }

    // =========================================================================
    // CREATE ORDER
    // =========================================================================

    /// `setup_future_usage = OffSession` with a `customer.id` creates the standing
    /// order that SetupMandate and a customer-initiated mandate payment require:
    /// `any`, `order_is_standing="true"`, and the customer id itself as
    /// `site_customer_identifier`. The expected digest was computed outside this
    /// codebase (Python `hmac` + `hashlib`) with the secret from worked example #1.
    #[test]
    fn creates_a_standing_order_for_off_session() {
        let settings = PaynearmeOrderSettings::new(
            Some(FutureUsage::OffSession),
            Some("cus_pnm_0001"),
            "pay_pnm_0001_1",
        )
        .expect("standing order settings");
        let request = PaynearmeCreateOrderRequest::signed(
            auth("ab7b539ea1317cca67c63c552", "S2411573363"),
            "1702333839".to_string(),
            major_amount("0.00"),
            Currency::USD,
            settings,
            "pay_pnm_0001_1".to_string(),
        )
        .expect("request");

        let body = serde_json::to_value(&request).expect("body");
        assert_eq!(
            paynearme_string_to_sign(&body).expect("string_to_sign"),
            concat!(
                "order_amount0.00order_currencyUSD",
                "order_is_standingtrueorder_typeany",
                "return_minimal_infotrue",
                "site_customer_identifiercus_pnm_0001",
                "site_identifierS2411573363",
                "site_order_identifierpay_pnm_0001_1",
                "timestamp1702333839version3.0",
            )
        );
        assert_eq!(
            request.signature.peek(),
            "88479ad8cb089f79660f260dcaacc2f015ee726d696e0d13a9cb422b540b8089"
        );
    }

    /// A standing order without a customer would orphan the stored card, so it is
    /// refused before any request is built. An empty `customer.id` is refused the
    /// same way.
    #[test]
    fn refuses_a_standing_order_without_customer_id() {
        for customer_id in [None, Some("")] {
            let report = PaynearmeOrderSettings::new(
                Some(FutureUsage::OffSession),
                customer_id,
                "pay_pnm_0001_1",
            )
            .expect_err("standing order without customer.id refused");
            assert!(matches!(
                report.current_context(),
                IntegrationError::MissingRequiredField { field_name: "customer.id", context }
                    if context.additional_context.is_some()
                        && context.suggested_action.is_some()
            ));
        }
    }

    /// A one-time order is unchanged: the body below is byte for byte what the
    /// connector sent to the mock before CreateOrder carried a customer (request
    /// `S1a_create_order_one_time`), and the mock's independent HMAC check accepted
    /// its signature. `OnSession` creates the same one-time order as unset, and a
    /// supplied `customer.id` replaces only the attempt-reference fallback.
    #[test]
    fn one_time_order_body_is_unchanged() {
        for setup_future_usage in [None, Some(FutureUsage::OnSession)] {
            let settings = PaynearmeOrderSettings::new(setup_future_usage, None, "pay_u4_gp_001_1")
                .expect("one-time order settings");
            let request = PaynearmeCreateOrderRequest::signed(
                auth("dummy_api_secret_key", "S0000000001"),
                "1789435026".to_string(),
                major_amount("10.00"),
                Currency::USD,
                settings,
                "pay_u4_gp_001_1".to_string(),
            )
            .expect("request");
            assert_eq!(
                serde_json::to_string(&request).expect("body"),
                concat!(
                    r#"{"site_identifier":"S0000000001","timestamp":"1789435026","#,
                    r#""version":"3.0","#,
                    r#""signature":"01cd63b67b9e0774d20129e4a9bc112a2a7e58cb6c1cdc01db98be734aed0d0d","#,
                    r#""order_amount":"10.00","order_currency":"USD","#,
                    r#""site_customer_identifier":"pay_u4_gp_001_1","order_type":"exact","#,
                    r#""order_is_standing":"false","site_order_identifier":"pay_u4_gp_001_1","#,
                    r#""return_minimal_info":"true"}"#,
                )
            );
        }

        let with_customer =
            PaynearmeOrderSettings::new(None, Some("cus_pnm_0001"), "pay_u4_gp_001_1")
                .expect("one-time order settings");
        assert_eq!(with_customer.order_type, ORDER_TYPE_EXACT);
        assert_eq!(with_customer.order_is_standing, ORDER_IS_STANDING_FALSE);
        assert_eq!(
            with_customer.site_customer_identifier.peek(),
            "cus_pnm_0001"
        );
    }

    fn create_order_proto(
        customer_id: Option<&str>,
        setup_future_usage: Option<grpc::FutureUsage>,
    ) -> grpc::PaymentServiceCreateOrderRequest {
        grpc::PaymentServiceCreateOrderRequest {
            merchant_order_id: Some("pay_pnm_0001_1".to_string()),
            amount: Some(grpc::Money {
                minor_amount: 1000,
                currency: grpc::Currency::from_str_name("USD").expect("USD").into(),
            }),
            customer: customer_id.map(|id| grpc::Customer {
                id: Some(id.to_string()),
                name: Some("John Smith".to_string()),
                ..Default::default()
            }),
            setup_future_usage: setup_future_usage.map(i32::from),
            test_mode: Some(true),
            ..Default::default()
        }
    }

    /// The router data the gRPC CreateOrder handler builds from the proto.
    fn create_order_router_data(proto: grpc::PaymentServiceCreateOrderRequest) -> CreateOrderData {
        let request = PaymentCreateOrderData::foreign_try_from(proto.clone()).expect("order data");
        let common = PaymentFlowData::foreign_try_from((
            proto,
            Connectors::default(),
            &MaskedMetadata::default(),
        ))
        .expect("flow data");
        router_data(common, request)
    }

    /// The proto's `customer.id` becomes `PaymentFlowData.customer_id` and its
    /// `setup_future_usage` (with `FUTURE_USAGE_UNSPECIFIED` read as unset)
    /// becomes `PaymentCreateOrderData.setup_future_usage`; together they decide
    /// the PayNearMe order.
    #[test]
    fn create_order_proto_customer_and_setup_future_usage_decide_the_order() {
        for (customer_id, setup_future_usage, expected_usage, standing, order_type, owner) in [
            (
                Some(CUSTOMER),
                Some(grpc::FutureUsage::OffSession),
                Some(FutureUsage::OffSession),
                "true",
                "any",
                CUSTOMER,
            ),
            (
                Some(CUSTOMER),
                Some(grpc::FutureUsage::OnSession),
                Some(FutureUsage::OnSession),
                "false",
                "exact",
                CUSTOMER,
            ),
            (
                Some(CUSTOMER),
                Some(grpc::FutureUsage::Unspecified),
                None,
                "false",
                "exact",
                CUSTOMER,
            ),
            (None, None, None, "false", "exact", "pay_pnm_0001_1"),
        ] {
            let router_data =
                create_order_router_data(create_order_proto(customer_id, setup_future_usage));
            assert_eq!(router_data.request.setup_future_usage, expected_usage);
            assert_eq!(
                router_data
                    .resource_common_data
                    .customer_id
                    .as_ref()
                    .map(|id| id.get_string_repr()),
                customer_id
            );

            let body = request_json(&router_data).expect("create order body");
            assert_eq!(body["order_is_standing"], standing);
            assert_eq!(body["order_type"], order_type);
            assert_eq!(body["site_customer_identifier"], owner);
            assert_eq!(body["site_order_identifier"], "pay_pnm_0001_1");
        }

        let off_session_without_customer = create_order_router_data(create_order_proto(
            None,
            Some(grpc::FutureUsage::OffSession),
        ));
        assert!(matches!(
            request_json(&off_session_without_customer)
                .expect_err("standing order without customer.id")
                .current_context(),
            IntegrationError::MissingRequiredField {
                field_name: "customer.id",
                ..
            }
        ));
    }
}
