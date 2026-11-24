#![allow(clippy::unwrap_used)]

use prost::Message;
use ucs_cards::{CardNumber, NetworkToken};

fn encoded(value: &str) -> Vec<u8> {
    let mut buf = Vec::new();
    prost::encoding::string::encode(1, &value.to_string(), &mut buf);
    buf
}

#[test]
fn valid_card_number_decodes() {
    assert!(CardNumber::decode(encoded("4111111111111111").as_slice()).is_ok());
}

#[test]
fn invalid_card_number_is_rejected_on_decode() {
    // Fails the Luhn check, and is not a known test card.
    assert!(CardNumber::decode(encoded("4111111111111112").as_slice()).is_err());
    assert!(CardNumber::decode(encoded("not-a-card").as_slice()).is_err());
}

#[test]
fn invalid_network_token_is_rejected_on_decode() {
    assert!(NetworkToken::decode(encoded("4111111111111111").as_slice()).is_ok());
    assert!(NetworkToken::decode(encoded("abc").as_slice()).is_err());
}
