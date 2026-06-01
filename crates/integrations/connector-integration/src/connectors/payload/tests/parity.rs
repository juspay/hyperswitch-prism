use common_utils::types::FloatMajorUnit;
use domain_types::payment_method_data::DefaultPCIHolder;

use crate::connectors::payload::requests::{
    self, PayloadPaymentRequestData, PayloadRefundLedgerEntry, PayloadRefundRequest,
};

#[test]
fn test_parity_16476_setup_mandate_json_serialization() {
    let card_number =
        <domain_types::payment_method_data::RawCardNumber<DefaultPCIHolder>>::default();

    let request: PayloadPaymentRequestData<DefaultPCIHolder> = PayloadPaymentRequestData {
        amount: FloatMajorUnit::zero(),
        payment_method: requests::PayloadPaymentMethod {
            method: requests::PayloadPaymentMethods::Card(requests::PayloadCard {
                card: requests::PayloadCardData {
                    card_number,
                    expiry: "12/30".to_string().into(),
                    card_code: "123".to_string().into(),
                },
            }),
            billing_address: Some(requests::BillingAddress {
                city: "New York".to_string().into(),
                country_code: common_enums::CountryAlpha2::US,
                postal_code: "10001".to_string().into(),
                state_province: "New York".to_string().into(),
                street_address: "123".to_string().into(),
            }),
            keep_active: true,
        },
        transaction_types: requests::TransactionTypes::Payment,
        status: None,
        processing_id: None,
        customer_id: Some("acct_3fHzDw8e1IDHNO77OBaMj".to_string()),
    };

    let json = serde_json::to_value(&request).expect("must serialize to JSON");

    assert!(json.is_object(), "top-level must be a JSON object");
    assert_eq!(json["type"], "payment");
    assert_eq!(
        json["customer_id"], "acct_3fHzDw8e1IDHNO77OBaMj",
        "customer_id at top level"
    );

    let pm = &json["payment_method"];
    assert!(pm.is_object(), "payment_method must be a nested object");
    assert_eq!(pm["type"], "card", "tagged enum produces type:card");
    assert_eq!(pm["keep_active"], true);

    let card = &pm["card"];
    assert!(
        card.is_object(),
        "card must be a nested object inside payment_method"
    );
    assert_eq!(card["expiry"], "12/30");
    assert_eq!(card["card_code"], "123");
    assert!(
        card.get("card_number").is_some(),
        "card_number field must exist"
    );

    let ba = &pm["billing_address"];
    assert!(ba.is_object(), "billing_address nested inside payment_method");
    assert_eq!(ba["city"], "New York");
    assert_eq!(ba["country_code"], "US");
    assert_eq!(ba["postal_code"], "10001");
    assert_eq!(ba["state_province"], "New York");
    assert_eq!(ba["street_address"], "123");

    assert!(
        json.get("payment_method[card][card_number]").is_none(),
        "must NOT have bracket-notation keys (form-urlencoded artifact)"
    );
    assert!(
        json.get("payment_method[type]").is_none(),
        "must NOT have bracket-notation keys"
    );
}

#[test]
fn test_parity_16476_refund_ledger_nested() {
    let request = PayloadRefundRequest {
        transaction_type: requests::TransactionTypes::Refund,
        amount: FloatMajorUnit::zero(),
        ledger: vec![PayloadRefundLedgerEntry {
            assoc_transaction_id: "txn_abc123".to_string(),
        }],
    };

    let json = serde_json::to_value(&request).expect("must serialize to JSON");
    assert_eq!(json["type"], "refund");

    let ledger = &json["ledger"];
    assert!(ledger.is_array(), "ledger must be a JSON array");
    assert_eq!(ledger[0]["assoc_transaction_id"], "txn_abc123");

    assert!(
        json.get("ledger[0][assoc_transaction_id]").is_none(),
        "must NOT have bracket-notation key"
    );
}
