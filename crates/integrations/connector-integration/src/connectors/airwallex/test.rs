//! Serialization tests for the Airwallex request bodies.
//!
//! Everything here is pure `Serialize`/`Deserialize` logic — the untagged enums, the
//! `payment_method_options` selection and the `mandate_metadata` round-trip — which the grpcurl
//! sandbox runs exercise only indirectly and which would otherwise drift silently.

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use hyperswitch_masking::Secret;
    use serde_json::json;

    use crate::connectors::airwallex::transformers::{
        build_payment_method_options, AirwallexAtomeData, AirwallexAtomeDetails,
        AirwallexAuthorizeRequest, AirwallexCardData, AirwallexCardDetails,
        AirwallexCompleteRequest, AirwallexGooglePayData, AirwallexGooglePayDetails,
        AirwallexGpayPaymentDataType, AirwallexKlarnaData, AirwallexKlarnaDetails,
        AirwallexMandateMetadata, AirwallexPayLaterData, AirwallexPaymentMethod,
        AirwallexPaymentRequest, AirwallexPaymentType, AirwallexThreeDsData, AirwallexThreeDsType,
        AirwallexWalletData,
    };

    fn card_method() -> AirwallexPaymentMethod {
        AirwallexPaymentMethod::Card(AirwallexCardData {
            card: AirwallexCardDetails {
                number: Secret::new("4242424242424242".to_string()),
                expiry_month: Secret::new("03".to_string()),
                expiry_year: Secret::new("2030".to_string()),
                cvc: Secret::new("737".to_string()),
                name: None,
            },
            payment_method_type: AirwallexPaymentType::Card,
        })
    }

    fn googlepay_method() -> AirwallexPaymentMethod {
        AirwallexPaymentMethod::Wallets(AirwallexWalletData::GooglePay(AirwallexGooglePayData {
            googlepay: AirwallexGooglePayDetails {
                encrypted_payment_token: Secret::new("tok_encrypted".to_string()),
                payment_data_type: AirwallexGpayPaymentDataType::EncryptedPaymentToken,
            },
            payment_method_type: AirwallexPaymentType::Googlepay,
        }))
    }

    fn atome_method() -> AirwallexPaymentMethod {
        AirwallexPaymentMethod::PayLater(AirwallexPayLaterData::Atome(AirwallexAtomeData {
            atome: AirwallexAtomeDetails {
                shopper_phone: Secret::new("+6591234567".to_string()),
            },
            payment_method_type: AirwallexPaymentType::Atome,
        }))
    }

    fn klarna_method() -> AirwallexPaymentMethod {
        AirwallexPaymentMethod::PayLater(AirwallexPayLaterData::Klarna(Box::new(
            AirwallexKlarnaData {
                klarna: AirwallexKlarnaDetails {
                    country_code: common_enums::CountryAlpha2::DE,
                    billing: None,
                },
                payment_method_type: AirwallexPaymentType::Klarna,
            },
        )))
    }

    /// The untagged wallet enum has to serialize as its inner body — a nested object keyed by the
    /// wallet name plus a sibling `type` discriminator, not as an enum wrapper.
    #[test]
    fn wallet_data_serializes_untagged_with_type_discriminator() {
        let json = serde_json::to_value(googlepay_method()).unwrap();
        assert_eq!(
            json,
            json!({
                "googlepay": {
                    "encrypted_payment_token": "tok_encrypted",
                    "payment_data_type": "encrypted_payment_token"
                },
                "type": "googlepay"
            })
        );
    }

    /// Same contract for PayLater. `Klarna` is boxed, which must not change the wire shape.
    #[test]
    fn paylater_data_serializes_untagged_with_type_discriminator() {
        let json = serde_json::to_value(klarna_method()).unwrap();
        assert_eq!(
            json,
            json!({ "klarna": { "country_code": "DE" }, "type": "klarna" })
        );
    }

    /// The Authorize body is untagged over the two legs: leg 1 confirms the intent, leg 2 finishes
    /// card 3DS. Each has to serialize as its inner body with no variant name in sight.
    #[test]
    fn authorize_request_confirm_continue_leg_serializes_as_three_ds_continue() {
        let request = AirwallexAuthorizeRequest::ConfirmContinue(AirwallexCompleteRequest {
            request_id: "req_1".to_string(),
            three_ds: AirwallexThreeDsData {
                acs_response: Some(Secret::new("acs".to_string())),
            },
            three_ds_type: AirwallexThreeDsType::ThreeDSContinue,
        });

        assert_eq!(
            serde_json::to_value(request).unwrap(),
            json!({
                "request_id": "req_1",
                "three_ds": { "acs_response": "acs" },
                "type": "3ds_continue"
            })
        );
    }

    /// `payment_method_options` and `device_data` must be omitted rather than sent as an explicit
    /// `null` — an APM intent that ships `"payment_method_options": null` is not at parity with
    /// the reference connector.
    #[test]
    fn absent_options_and_device_data_are_omitted_not_null() {
        let request = AirwallexPaymentRequest {
            request_id: "req_1".to_string(),
            payment_method: googlepay_method(),
            payment_method_options: None,
            return_url: Some("https://example.com/return".to_string()),
            device_data: None,
            payment_consent: None,
            customer_id: None,
        };

        let json = serde_json::to_value(request).unwrap();
        let object = json.as_object().unwrap();
        assert!(!object.contains_key("payment_method_options"));
        assert!(!object.contains_key("device_data"));
        assert!(!object.contains_key("payment_consent"));
        assert!(!object.contains_key("customer_id"));
    }

    /// Options are emitted for Card, Klarna and Atome only, each under its own key, mirroring the
    /// reference connector. Wallets, bank redirects and bank transfers get no options block.
    #[test]
    fn payment_method_options_are_built_per_method() {
        let card = build_payment_method_options(&card_method(), true).unwrap();
        assert_eq!(card.card.unwrap().auto_capture, Some(true));

        let klarna = build_payment_method_options(&klarna_method(), false).unwrap();
        assert_eq!(klarna.klarna.unwrap().auto_capture, Some(false));
        assert!(klarna.card.is_none());
        assert!(klarna.atome.is_none());

        let atome = build_payment_method_options(&atome_method(), true).unwrap();
        assert_eq!(atome.atome.unwrap().auto_capture, Some(true));
        assert!(atome.card.is_none());
        assert!(atome.klarna.is_none());

        assert!(build_payment_method_options(&googlepay_method(), true).is_none());
    }

    /// hyperswitch overwrites `payment_method_id` with its own id, so the Airwallex token is
    /// round-tripped through `mandate_metadata` as `{"id": ...}`. If this shape drifts, MIT loses
    /// `payment_method.id` and Airwallex rejects the replay.
    #[test]
    fn mandate_metadata_round_trips_the_payment_method_token() {
        let stored = json!({ "id": "pm_abc123" });
        let parsed: AirwallexMandateMetadata = serde_json::from_value(stored).unwrap();
        assert_eq!(parsed.id.as_deref(), Some("pm_abc123"));

        // Mandates stored before the metadata round-trip existed have no `id`; the MIT
        // transformer falls back to payment_method_id rather than erroring on parse.
        let legacy: AirwallexMandateMetadata = serde_json::from_value(json!({})).unwrap();
        assert!(legacy.id.is_none());
    }
}
