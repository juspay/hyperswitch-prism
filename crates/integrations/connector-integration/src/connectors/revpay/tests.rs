//! Unit tests for the revPAY transformers: known-answer signature vectors
//! from the Merchant API doc, response-code → status mappings, amount/expiry
//! formatting, PAN-BIN card-type inference and webhook verification.
#![cfg(test)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::panic)]
#![allow(clippy::indexing_slicing)]

use super::*;
use domain_types::payment_method_data::DefaultPCIHolder;

// Known-answer vectors from the revPAY Merchant API doc (§3.28 / §3.29)
// with sample key material Merchant Key "1OoL87PmJQ", Merchant ID
// "MER00000000001".

#[test]
fn error_response_parses_revpay_undocumented_422_body() {
    // Exact body captured from the UAT sandbox (HTTP 422) when
    // Transaction_Description is empty: integer `code`, capitalised
    // `Description`. Must parse into the error shape, not blow up.
    let body = r#"{"code":422,"Description":"Unprocessable Entity"}"#;
    let parsed: RevpayErrorResponse =
        serde_json::from_str(body).expect("422 error body must deserialize");
    assert_eq!(parsed.code.as_deref(), Some("422"));
    assert_eq!(parsed.message.as_deref(), Some("Unprocessable Entity"));
}

#[test]
fn error_response_parses_string_code_variant() {
    let body = r#"{"code":"422","Description":"Unprocessable Entity"}"#;
    let parsed: RevpayErrorResponse =
        serde_json::from_str(body).expect("string-code error body must deserialize");
    assert_eq!(parsed.code.as_deref(), Some("422"));
}

#[test]
fn payment_request_signature_matches_doc_vector() {
    let signature = generate_revpay_signature(
        "1OoL87PmJQ",
        &["MER00000000001", "RF201774111", "52.40", "MYR"],
    );
    assert_eq!(
        signature,
        "0716dc1863519f53d47a11ec603d402782222083b2178f596d210fcb93287e81412dd529fa2adddd8e0cdfa2ddf0bb5e8421f55a105196d4ee58961b253b98a1"
    );
}

#[test]
fn payment_response_signature_matches_doc_vector() {
    // Same formula shape the webhook path (separate task) will verify.
    let signature = generate_revpay_signature(
        "1OoL87PmJQ",
        &[
            "MER00000000001",
            "RT000000007222",
            "00",
            "RF201774111",
            "52.40",
            "MYR",
        ],
    );
    assert_eq!(
        signature,
        "fc8d556ae2e4fa12836e1192a4f0a793a9672af1d2348da502ef299a6b5169f4402fb6bb282a9e097fe1253b4bfd4f2fa365b7d593b564c585fc79b6c302ecad"
    );
}

#[test]
fn refund_request_signature_matches_offline_vector() {
    // Refund formula (§3.10): sha512(key + mid + Reference_Number +
    // Refund_Amount + Original_Reference_Number) — no Currency, original
    // reference appended. Self-computed offline:
    //   python3 -c 'import hashlib; print(hashlib.sha512(
    //       b"1OoL87PmJQMER00000000001RF20177411210.00RF201774111").hexdigest())'
    let signature = generate_revpay_signature(
        "1OoL87PmJQ",
        &["MER00000000001", "RF201774112", "10.00", "RF201774111"],
    );
    assert_eq!(
        signature,
        "981aa4c5b4109e97969e75916eb3c69476eea1180af60d87997916ae5f21f7aa13e384f4dcd7e3aebdd8ccd4571ba1c16e1dbf6f36c5a74b936a943d0fa820bf"
    );
}

#[test]
fn response_code_maps_to_refund_status() {
    // §2.5: only "00" is a synchronous acceptance; all else is declined.
    assert_eq!(
        RevpayResponseCode::Success.refund_status(),
        RefundStatus::Success
    );
    assert_eq!(
        RevpayResponseCode::RedirectRequired.refund_status(),
        RefundStatus::Failure
    );
    assert_eq!(
        RevpayResponseCode::PendingAuthorization.refund_status(),
        RefundStatus::Failure
    );
    assert_eq!(
        RevpayResponseCode::TransactionNotFound.refund_status(),
        RefundStatus::Failure
    );
    assert_eq!(
        RevpayResponseCode::Unknown("30".to_string()).refund_status(),
        RefundStatus::Failure
    );
}

#[test]
fn response_code_maps_to_refund_sync_status() {
    // Flow 4: "00" → Success, "09"/"99" → Pending (in flight — a status
    // query must stay resolvable). "76" is Pending, NOT Failure: whether
    // the inquiry resolves refund references is unverified, so not-found
    // must never terminally fail a refund the Refund call accepted.
    assert_eq!(
        RevpayResponseCode::Success.refund_sync_status(),
        RefundStatus::Success
    );
    assert_eq!(
        RevpayResponseCode::RedirectRequired.refund_sync_status(),
        RefundStatus::Pending
    );
    assert_eq!(
        RevpayResponseCode::PendingAuthorization.refund_sync_status(),
        RefundStatus::Pending
    );
    assert_eq!(
        RevpayResponseCode::TransactionNotFound.refund_sync_status(),
        RefundStatus::Pending
    );
    assert_eq!(
        RevpayResponseCode::Unknown("30".to_string()).refund_sync_status(),
        RefundStatus::Failure
    );
}

#[test]
fn amount_formats_major_with_two_decimals() {
    // RM 52.40 — the bytes revPAY signs must carry the literal dot.
    let amount = RevpayAmountConvertor::convert(&common_utils::types::Money {
        amount: MinorUnit::new(5240),
        currency: Currency::MYR,
    })
    .expect("amount conversion should succeed for MYR");
    assert_eq!(amount.get_amount_as_string(), "52.40");
    assert_eq!(
        serde_json::to_string(&amount).expect("amount serializes"),
        "\"52.40\""
    );
}

#[test]
fn payment_response_parses_uat_redirect_body_with_null_transaction_id() {
    // Captured verbatim from the UAT sandbox (22 Oct 2026) against the
    // gateway's test card: the "09 Request in progress" redirect response
    // has `Transaction_ID: null` plus many optional null fields. Must
    // parse and surface the redirect URL.
    let body = r#"{"Revpay_Merchant_ID":"MER00000006606","Payment_ID":2,"Bank_Code":null,"Transaction_ID":null,"Reference_Number":"azharamin-REVPAY1792669250-R1","Amount":"10.00","Currency":"MYR","Transaction_Description":"Payment azharamin-REVPAY1792669250-R1","Response_Code":"09","Error_Description":"Request in progress.","Key_Index":1,"Signature":"e4b159","Settlement_Amount":null,"Settlement_Currency":null,"Settlement_FX_Rate":null,"Request_Datetime":null,"Response_Datetime":null,"Ecomm_Marketplace":null,"Promo_Code":null,"Funding_Pan_Mask":null,"Funding_Bin":null,"Funding_Exp_Date":null,"Token_Pan":null,"Token_Exp_Date":null,"Fraud_Score":null,"Bank_Reference_Number":null,"Bank_Redirect_URL":"https://stg-mpg.revpay-sandbox.com.my/v1/payment/intents/3936cb80-bf75-4943-92c4-ff7620f70f36/abc"}"#;
    let parsed: RevpayPaymentsResponse =
        serde_json::from_str(body).expect("UAT redirect body must deserialize");
    assert_eq!(parsed.transaction_id, None);
    assert!(matches!(
        parsed.response_code,
        RevpayResponseCode::RedirectRequired
    ));
    assert_eq!(
        parsed.bank_redirect_url.as_deref(),
        Some("https://stg-mpg.revpay-sandbox.com.my/v1/payment/intents/3936cb80-bf75-4943-92c4-ff7620f70f36/abc")
    );
}

#[test]
fn redirect_without_transaction_id_falls_back_to_reference_number() {
    let response = RevpayPaymentsResponse {
        revpay_merchant_id: None,
        payment_id: Some(2),
        transaction_id: None,
        reference_number: Some("reference".to_string()),
        amount: Some(RevpayAmount::Text("10.00".to_string())),
        currency: Some("MYR".to_string()),
        response_code: RevpayResponseCode::RedirectRequired,
        error_description: Some("Request in progress.".to_string()),
        bank_redirect_url: Some("https://example.com/redirect".to_string()),
        key_index: Some(1),
        signature: Some("signature".to_string()),
        bank_reference_number: None,
    };

    // "09" carries `Transaction_ID: null`; the interim resource id is the
    // `Reference_Number` we sent (PSync later overwrites it with the real
    // Transaction_ID).
    assert!(matches!(
        authorize_resource_id(&response, 200).expect("reference number is a valid interim id"),
        ResponseId::ConnectorTransactionId(id) if id == "reference"
    ));
}

#[test]
fn response_without_any_identifier_is_rejected() {
    let response = RevpayPaymentsResponse {
        revpay_merchant_id: None,
        payment_id: Some(2),
        transaction_id: None,
        reference_number: None,
        amount: Some(RevpayAmount::Text("10.00".to_string())),
        currency: Some("MYR".to_string()),
        response_code: RevpayResponseCode::RedirectRequired,
        error_description: Some("Request in progress.".to_string()),
        bank_redirect_url: Some("https://example.com/redirect".to_string()),
        key_index: Some(1),
        signature: Some("signature".to_string()),
        bank_reference_number: None,
    };

    assert!(authorize_resource_id(&response, 200).is_err());
}

fn expiry_card(month: &str, year: &str) -> Card<DefaultPCIHolder> {
    Card {
        card_number: RawCardNumber(
            cards::CardNumber::from_str("4111111111111111").expect("test card number"),
        ),
        card_exp_month: Secret::new(month.to_string()),
        card_exp_year: Secret::new(year.to_string()),
        card_cvc: Secret::new("123".to_string()),
        card_issuer: None,
        card_network: None,
        card_type: None,
        card_issuing_country: None,
        bank_code: None,
        nick_name: None,
        card_holder_name: None,
        co_badged_card_data: None,
    }
}

#[test]
fn exp_date_formats_strict_yyyymm() {
    // revPAY `Funding_Exp_Date` is YYYYMM — the UAT gateway's own sample
    // is "203901" = January 2039.
    let exp =
        revpay_funding_exp_date(&expiry_card("01", "2039")).expect("valid expiry must format");
    assert_eq!(exp.peek(), "203901");
    let exp = revpay_funding_exp_date(&expiry_card("1", "39")).expect("valid expiry must format");
    assert_eq!(exp.peek(), "203901");
}

#[test]
fn exp_date_rejects_malformed_values() {
    // Anything but M/MM + YY/YYYY must fail locally instead of surfacing
    // as revPAY Response_Code 14 "Invalid buyer account or card
    // information" (the gateway rejects a malformed Funding_Exp_Date).
    for (month, year) in [
        ("13", "2039"),
        ("0", "2039"),
        ("01", "01/2039"),
        ("01", ""),
        ("Jan", "2039"),
        ("01", "039"),
        ("01", "20391"),
    ] {
        assert!(
            revpay_funding_exp_date(&expiry_card(month, year)).is_err(),
            "month={month} year={year} must be rejected"
        );
    }
}

#[test]
fn card_type_infers_from_pan_bin() {
    // UCS `card_network` is absent on the euler /txns path, so `Card_Type`
    // (Appendix H) is inferred from the PAN; a missing Card_Type is a
    // gateway-confirmed cause of Response_Code 14.
    assert!(matches!(
        card_type_from_pan("4111111111111111"),
        Some(RevpayCardType::Visa)
    ));
    // The gateway's own UAT test card is a MasterCard BIN.
    assert!(matches!(
        card_type_from_pan("5123450000000008"),
        Some(RevpayCardType::Mastercard)
    ));
    assert!(matches!(
        card_type_from_pan("5123-4500 0000-0008"),
        Some(RevpayCardType::Mastercard)
    ));
    assert!(matches!(
        card_type_from_pan("2221000000000009"),
        Some(RevpayCardType::Mastercard)
    ));
    assert!(matches!(
        card_type_from_pan("378282246310005"),
        Some(RevpayCardType::Amex)
    ));
    // BIN 62 is UnionPay; explicit network selection handles MyDebit co-badging.
    assert!(matches!(
        card_type_from_pan("6250941234567890"),
        Some(RevpayCardType::UnionPay)
    ));
    assert!(card_type_from_pan("9999").is_none());
}

#[test]
fn authorize_attempt_status_is_decided_by_code_and_redirect_url() {
    assert_eq!(
        authorize_attempt_status(&RevpayResponseCode::Success, None),
        AttemptStatus::Charged
    );
    assert_eq!(
        authorize_attempt_status(&RevpayResponseCode::Success, Some("https://bank/redirect")),
        AttemptStatus::Charged
    );
    assert_eq!(
        authorize_attempt_status(
            &RevpayResponseCode::RedirectRequired,
            Some("https://bank/redirect")
        ),
        AttemptStatus::AuthenticationPending
    );
    // A redirect reply without its URL is a failed attempt, not pending.
    assert_eq!(
        authorize_attempt_status(&RevpayResponseCode::RedirectRequired, None),
        AttemptStatus::AuthorizationFailed
    );
    // FPX B2B settles asynchronously — still resolvable.
    assert_eq!(
        authorize_attempt_status(&RevpayResponseCode::PendingAuthorization, None),
        AttemptStatus::Pending
    );
    // One-shot sale: any decline is an authorization failure.
    for code in [
        RevpayResponseCode::TransactionNotFound,
        RevpayResponseCode::Unknown("1A".to_string()),
        RevpayResponseCode::Unknown("01".to_string()),
    ] {
        assert_eq!(
            authorize_attempt_status(&code, Some("https://bank/redirect")),
            AttemptStatus::AuthorizationFailed,
            "{code:?}"
        );
        assert_eq!(
            authorize_attempt_status(&code, None),
            AttemptStatus::AuthorizationFailed,
            "{code:?}"
        );
    }
}

#[test]
fn unionpay_explicit_network_maps_to_cnp_rail() {
    let (payment_id, card_type) =
        card_payment_details(Some(&CardNetwork::UnionPay), "6250941234567890");
    assert_eq!(payment_id, UNIONPAY_CNP_PAYMENT_ID);
    assert!(matches!(card_type, Some(RevpayCardType::UnionPay)));
}

#[test]
fn card_type_serializes_to_appendix_h_codes() {
    assert_eq!(
        serde_json::to_string(&RevpayCardType::Visa).expect("serialize"),
        "\"02\""
    );
    assert_eq!(
        serde_json::to_string(&RevpayCardType::Mastercard).expect("serialize"),
        "\"03\""
    );
    assert_eq!(
        serde_json::to_string(&RevpayCardType::Amex).expect("serialize"),
        "\"04\""
    );
    assert_eq!(
        serde_json::to_string(&RevpayCardType::MyDebit).expect("serialize"),
        "\"05\""
    );
}

#[test]
fn issuer_country_maps_to_alpha3() {
    // Gateway sample sends "MYS" (ISO 3166-1 alpha-3).
    assert_eq!(
        issuer_country_alpha3(Some("mys"), None).as_deref(),
        Some("MYS")
    );
    assert_eq!(
        issuer_country_alpha3(None, Some(CountryAlpha2::MY)).as_deref(),
        Some("MYS")
    );
    assert_eq!(
        issuer_country_alpha3(Some("IN"), Some(CountryAlpha2::MY)).as_deref(),
        Some("MYS"),
        "a two-letter card country must fall back to billing"
    );
    assert_eq!(issuer_country_alpha3(None, None), None);
}

#[test]
fn amount_round_trips_through_wire_form() {
    let amount = RevpayAmountConvertor::convert(&common_utils::types::Money {
        amount: MinorUnit::new(1000),
        currency: Currency::MYR,
    })
    .expect("amount conversion should succeed");
    assert_eq!(amount.get_amount_as_string(), "10.00");
    let back = RevpayAmountConvertor::convert_back(amount, Currency::MYR)
        .expect("convert_back should succeed");
    assert_eq!(back.amount, MinorUnit::new(1000));

    // Zero-exponent currency: no minor units, still rendered with two decimals.
    let jpy = RevpayAmountConvertor::convert(&common_utils::types::Money {
        amount: MinorUnit::new(1000),
        currency: Currency::JPY,
    })
    .expect("JPY conversion should succeed");
    assert_eq!(jpy.get_amount_as_string(), "1000.00");
    let back = RevpayAmountConvertor::convert_back(jpy, Currency::JPY)
        .expect("convert_back should succeed");
    assert_eq!(back.amount, MinorUnit::new(1000));
}

// ============================================================================
// WEBHOOK TESTS (Flow 5) — doc-vector-based, key "1OoL87PmJQ"
// ============================================================================
//
// Positive vector recomputed offline (matches the doc's §3.29 response
// signature KA-vector):
//   python3 -c 'import hashlib; print(hashlib.sha512(
//       b"1OoL87PmJQMER00000000001RT00000000722200RF20177411152.40MYR"
//   ).hexdigest())'
const WEBHOOK_DOC_SIGNATURE: &str = "fc8d556ae2e4fa12836e1192a4f0a793a9672af1d2348da502ef299a6b5169f4402fb6bb282a9e097fe1253b4bfd4f2fa365b7d593b564c585fc79b6c302ecad";

fn webhook_auth() -> RevpayAuthType {
    RevpayAuthType {
        merchant_id: Secret::new("MER00000000001".to_string()),
        merchant_key: Secret::new("1OoL87PmJQ".to_string()),
        key_index: Secret::new("1".to_string()),
    }
}

fn webhook_form_body(amount: &str, key_index: &str, signature: &str) -> String {
    format!(
        "Revpay_Merchant_ID=MER00000000001&Payment_ID=2&Bank_Code=MB2U&\
         Transaction_ID=RT000000007222&Reference_Number=RF201774111&\
         Bank_Reference_Number=BR000001&Amount={amount}&Currency=MYR&\
         Transaction_Description=Order+RF201774111&Fraud_Score=0&\
         Response_Code=00&Error_Description=&Key_Index={key_index}&\
         Signature={signature}&Request_Datetime=2020-01-01+12%3A00%3A00&\
         Response_Datetime=2020-01-01+12%3A00%3A05"
    )
}

#[test]
fn webhook_body_parses_form_encoded() {
    let body: RevpayWebhookBody =
        serde_urlencoded::from_str(&webhook_form_body("52.40", "1", WEBHOOK_DOC_SIGNATURE))
            .expect("form-encoded webhook body parses");
    assert_eq!(body.revpay_merchant_id, "MER00000000001");
    assert_eq!(body.payment_id, Some(2));
    assert_eq!(body.transaction_id, "RT000000007222");
    assert_eq!(body.reference_number, "RF201774111");
    assert_eq!(body.amount, RevpayAmount::Text("52.40".into()));
    assert_eq!(body.currency, Currency::MYR);
    assert_eq!(body.response_code, RevpayResponseCode::Success);
    assert_eq!(body.error_description, Some(String::new()));
    assert_eq!(body.key_index, "1");
    assert_eq!(body.signature, WEBHOOK_DOC_SIGNATURE);
}

#[test]
fn webhook_signature_verify_accepts_doc_vector() {
    let body: RevpayWebhookBody =
        serde_urlencoded::from_str(&webhook_form_body("52.40", "1", WEBHOOK_DOC_SIGNATURE))
            .expect("form-encoded webhook body parses");
    assert!(body.verifies_against(&webhook_auth()));
}

#[test]
fn webhook_signature_verify_rejects_tampered_amount() {
    // Same body, same signature, but Amount flipped 52.40 -> 52.41: the
    // recomputed sha512 preimage differs, so verification MUST fail.
    let body: RevpayWebhookBody =
        serde_urlencoded::from_str(&webhook_form_body("52.41", "1", WEBHOOK_DOC_SIGNATURE))
            .expect("form-encoded webhook body parses");
    assert!(!body.verifies_against(&webhook_auth()));
}

#[test]
fn webhook_signature_verify_rejects_unknown_key_index() {
    // A Key_Index we do not hold cannot verify — we keep exactly one key.
    let body: RevpayWebhookBody =
        serde_urlencoded::from_str(&webhook_form_body("52.40", "2", WEBHOOK_DOC_SIGNATURE))
            .expect("form-encoded webhook body parses");
    assert!(!body.verifies_against(&webhook_auth()));
}

#[test]
fn webhook_event_and_status_mapping_follows_response_code() {
    let parse = |code: &str| RevpayWebhookBody {
        revpay_merchant_id: "MER00000000001".to_string(),
        payment_id: None,
        bank_code: None,
        transaction_id: "RT000000007222".to_string(),
        reference_number: "RF201774111".to_string(),
        bank_reference_number: None,
        amount: RevpayAmount::Text("52.40".to_string()),
        currency: Currency::MYR,
        transaction_description: None,
        fraud_score: None,
        response_code: serde_json::from_str::<RevpayResponseCode>(&format!("\"{code}\""))
            .expect("response code parses"),
        error_description: (code != "00" && code != "09" && code != "99")
            .then(|| "Declined".to_string()),
        key_index: "1".to_string(),
        signature: WEBHOOK_DOC_SIGNATURE.to_string(),
        request_datetime: None,
        response_datetime: None,
    };

    // "00" — final success for a payment that left Authorize pending.
    let body = parse("00");
    assert_eq!(body.event_type(), EventType::PaymentIntentSuccess);
    assert_eq!(body.attempt_status(), AttemptStatus::Charged);
    assert_eq!(body.failure_details(), None);

    // "09"/"99" — still resolvable; NOT a failure and NOT terminal.
    let body = parse("99");
    assert_eq!(body.event_type(), EventType::PaymentIntentProcessing);
    assert_eq!(body.attempt_status(), AttemptStatus::Pending);
    assert_eq!(body.failure_details(), None);

    // Terminal failure carries the raw code + description.
    let body = parse("1A");
    assert_eq!(body.event_type(), EventType::PaymentIntentFailure);
    assert_eq!(body.attempt_status(), AttemptStatus::Failure);
    assert_eq!(
        body.failure_details(),
        Some(("1A".to_string(), "Declined".to_string()))
    );
}

#[test]
fn response_code_deserializes_known_and_unknown_codes() {
    assert_eq!(
        serde_json::from_str::<RevpayResponseCode>("\"00\"").expect("00 parses"),
        RevpayResponseCode::Success
    );
    assert_eq!(
        serde_json::from_str::<RevpayResponseCode>("\"09\"").expect("09 parses"),
        RevpayResponseCode::RedirectRequired
    );
    assert_eq!(
        serde_json::from_str::<RevpayResponseCode>("\"99\"").expect("99 parses"),
        RevpayResponseCode::PendingAuthorization
    );
    // Inquiry-side mapping round-trip for the documented pending codes.
    assert_eq!(
        RevpayResponseCode::Success.psync_status(),
        AttemptStatus::Charged
    );
    assert_eq!(
        RevpayResponseCode::RedirectRequired.psync_status(),
        AttemptStatus::Pending
    );
    assert_eq!(
        RevpayResponseCode::PendingAuthorization.psync_status(),
        AttemptStatus::Pending
    );
    assert_eq!(
        RevpayResponseCode::Unknown("76".to_string()).psync_status(),
        AttemptStatus::Failure
    );
    assert_eq!(
        serde_json::from_str::<RevpayResponseCode>("\"1A\"").expect("1A parses"),
        RevpayResponseCode::Unknown("1A".to_string())
    );
}

#[test]
fn webhook_typed_amount_preserves_signed_text() {
    let body: RevpayWebhookBody =
        serde_urlencoded::from_str(&webhook_form_body("52.4", "1", WEBHOOK_DOC_SIGNATURE))
            .expect("webhook parses");
    assert_eq!(body.amount, RevpayAmount::Text("52.4".into()));
    // Same numeric value, different signed bytes: never normalize to 52.40.
    assert!(!body.verifies_against(&webhook_auth()));
    let mut numeric = body;
    numeric.amount = RevpayAmount::Number(52.4);
    assert!(!numeric.verifies_against(&webhook_auth()));
}

#[test]
fn webhook_rejects_unknown_currency() {
    let form = webhook_form_body("52.40", "1", WEBHOOK_DOC_SIGNATURE)
        .replace("Currency=MYR", "Currency=INVALID");
    assert!(serde_urlencoded::from_str::<RevpayWebhookBody>(&form).is_err());
}
