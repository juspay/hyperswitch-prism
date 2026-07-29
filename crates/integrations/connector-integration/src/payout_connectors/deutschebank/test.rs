#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
#[allow(clippy::panic)]
mod tests {
    use domain_types::errors::IntegrationError;
    use hyperswitch_masking::PeekInterface;

    use super::super::transformers::split_pem_bundle;

    fn invalid_config_detail(report: &error_stack::Report<IntegrationError>) -> String {
        match report.current_context() {
            IntegrationError::InvalidConnectorConfig { config, context } => {
                assert_eq!(*config, "client_certificate_bundle");
                context
                    .additional_context
                    .clone()
                    .expect("additional_context should be set")
            }
            other => panic!("expected InvalidConnectorConfig, got: {other:?}"),
        }
    }

    const CERT_A: &str = "-----BEGIN CERTIFICATE-----\nMIIBcert+A\n-----END CERTIFICATE-----";
    const CERT_B: &str = "-----BEGIN CERTIFICATE-----\nMIIBcert+B\n-----END CERTIFICATE-----";
    const KEY_PKCS8: &str = "-----BEGIN PRIVATE KEY-----\nMIIEpkcs8\n-----END PRIVATE KEY-----";
    const KEY_PKCS1: &str =
        "-----BEGIN RSA PRIVATE KEY-----\nMIIEpkcs1\n-----END RSA PRIVATE KEY-----";
    const KEY_SEC1: &str = "-----BEGIN EC PRIVATE KEY-----\nMIIEsec1\n-----END EC PRIVATE KEY-----";

    #[test]
    fn cert_then_key() {
        let bundle = format!("{CERT_A}\n{KEY_PKCS8}\n");
        let (cert, key) = split_pem_bundle(&bundle).unwrap();
        assert!(cert.contains("MIIBcert+A"));
        assert!(key.peek().contains("MIIEpkcs8"));
    }

    #[test]
    fn key_then_cert_is_order_independent() {
        let bundle = format!("{KEY_PKCS8}\n{CERT_A}\n");
        let (cert, key) = split_pem_bundle(&bundle).unwrap();
        assert!(cert.contains("MIIBcert+A"));
        assert!(key.peek().contains("MIIEpkcs8"));
    }

    #[test]
    fn accepts_pkcs1_rsa_key() {
        let bundle = format!("{CERT_A}\n{KEY_PKCS1}\n");
        let (_, key) = split_pem_bundle(&bundle).unwrap();
        assert!(key.peek().contains("MIIEpkcs1"));
        assert!(key.peek().contains("RSA PRIVATE KEY"));
    }

    #[test]
    fn accepts_sec1_ec_key() {
        let bundle = format!("{CERT_A}\n{KEY_SEC1}\n");
        let (_, key) = split_pem_bundle(&bundle).unwrap();
        assert!(key.peek().contains("MIIEsec1"));
        assert!(key.peek().contains("EC PRIVATE KEY"));
    }

    #[test]
    fn concatenates_cert_chain() {
        let bundle = format!("{CERT_A}\n{CERT_B}\n{KEY_PKCS8}\n");
        let (chain, _) = split_pem_bundle(&bundle).unwrap();
        assert!(chain.contains("MIIBcert+A"));
        assert!(chain.contains("MIIBcert+B"));
    }

    #[test]
    fn rejects_missing_certificate() {
        let err = split_pem_bundle(KEY_PKCS8).unwrap_err();
        let detail = invalid_config_detail(&err);
        assert!(
            detail.contains("missing CERTIFICATE block"),
            "unexpected detail: {detail}"
        );
    }

    #[test]
    fn rejects_missing_key() {
        let err = split_pem_bundle(CERT_A).unwrap_err();
        let detail = invalid_config_detail(&err);
        assert!(
            detail.contains("missing PRIVATE KEY block"),
            "unexpected detail: {detail}"
        );
    }

    #[test]
    fn rejects_multiple_keys_as_ambiguous() {
        let bundle = format!("{CERT_A}\n{KEY_PKCS8}\n{KEY_PKCS1}\n");
        let err = split_pem_bundle(&bundle).unwrap_err();
        let detail = invalid_config_detail(&err);
        assert!(
            detail.contains("private-key blocks"),
            "unexpected detail: {detail}"
        );
    }

    #[test]
    fn ignores_unrelated_pem_blocks() {
        let stray = "-----BEGIN PUBLIC KEY-----\nMIIBpub\n-----END PUBLIC KEY-----";
        let bundle = format!("{CERT_A}\n{stray}\n{KEY_PKCS8}\n");
        let (cert, key) = split_pem_bundle(&bundle).unwrap();
        assert!(!cert.contains("MIIBpub"));
        assert!(!key.peek().contains("MIIBpub"));
    }

    #[test]
    fn does_not_echo_input_bytes_on_failure() {
        let canary = "SECRET_LEAK_CANARY_xyz123";
        let err = split_pem_bundle(canary).unwrap_err();
        let detail = invalid_config_detail(&err);
        let rendered = format!("{err:?}");
        assert!(
            !detail.contains(canary),
            "additional_context leaked input bytes: {detail}"
        );
        assert!(
            !rendered.contains(canary),
            "Debug-rendered error leaked input bytes: {rendered}"
        );
    }

    #[test]
    fn server_ca_bundle_absent_or_blank_is_none() {
        use super::super::server_ca_pem;

        assert!(server_ca_pem(None).unwrap().is_none());
        assert!(server_ca_pem(Some("")).unwrap().is_none());
        assert!(server_ca_pem(Some(" \n\t ")).unwrap().is_none());
    }

    #[test]
    fn server_ca_bundle_pem_round_trips_through_base64() {
        use base64::Engine as _;
        use hyperswitch_masking::ExposeInterface;

        use super::super::server_ca_pem;

        let encoded = server_ca_pem(Some(CERT_A))
            .unwrap()
            .expect("non-blank bundle should produce a CA cert")
            .expose();
        let decoded = common_utils::consts::BASE64_ENGINE.decode(encoded).unwrap();
        assert_eq!(decoded, format!("{CERT_A}\n").into_bytes());
    }

    #[test]
    fn connector_payout_id_round_trips() {
        use super::super::transformers::{decode_connector_payout_id, encode_connector_payout_id};
        use hyperswitch_masking::Secret;

        let end_to_end_id = "E2EABC123";
        let iban = Secret::new("DE89370400440532013000".to_string());
        let encoded = encode_connector_payout_id(end_to_end_id, &iban);

        let (decoded_e2e, decoded_iban) = decode_connector_payout_id(&encoded).unwrap();
        assert_eq!(decoded_e2e, end_to_end_id);
        assert_eq!(decoded_iban.peek(), iban.peek());
    }

    #[test]
    fn decode_connector_payout_id_rejects_missing_separator() {
        use super::super::transformers::decode_connector_payout_id;
        assert!(decode_connector_payout_id("no-separator-present").is_err());
    }

    #[test]
    fn eligible_vop_carries_connector_payout_id() {
        use super::super::transformers::{
            build_eligibility_response, DeutschebankVopMatchStatus, DeutschebankVopResponse,
        };
        use common_enums::PayoutStatus;

        let resp = DeutschebankVopResponse {
            match_status: Some(DeutschebankVopMatchStatus::Mtch),
            additional_info: None,
        };
        let out = build_eligibility_response(resp, "vop-123".to_string(), 200).unwrap();
        assert_eq!(out.payout_eligible, Some(true));
        assert_eq!(out.connector_payout_id.as_deref(), Some("vop-123"));
        assert_eq!(out.payout_status, PayoutStatus::RequiresFulfillment);
    }

    #[test]
    fn ineligible_vop_drops_connector_payout_id() {
        use super::super::transformers::{
            build_eligibility_response, DeutschebankVopMatchStatus, DeutschebankVopResponse,
        };
        use common_enums::PayoutStatus;

        let resp = DeutschebankVopResponse {
            match_status: Some(DeutschebankVopMatchStatus::Nmtc),
            additional_info: None,
        };
        let out = build_eligibility_response(resp, "vop-123".to_string(), 200).unwrap();
        assert_eq!(out.payout_eligible, Some(false));
        assert_eq!(out.connector_payout_id, None);
        assert_eq!(out.payout_status, PayoutStatus::Ineligible);
    }

    #[test]
    fn vop_without_match_status_is_error() {
        use super::super::transformers::{build_eligibility_response, DeutschebankVopResponse};

        let resp = DeutschebankVopResponse {
            match_status: None,
            additional_info: None,
        };
        assert!(build_eligibility_response(resp, "vop-123".to_string(), 200).is_err());
    }

    #[test]
    fn error_response_captures_unmodeled_fields() {
        use super::super::transformers::DeutschebankErrorResponse;

        let raw = r#"{"code":"APP-RULE","detail":"IBAN failed scheme rule","violationId":42}"#;
        let parsed: DeutschebankErrorResponse = serde_json::from_str(raw).unwrap();

        assert_eq!(parsed.code.as_deref(), Some("APP-RULE"));
        assert!(parsed.message.is_none());
        assert!(parsed.additional.contains_key("detail"));
        assert!(parsed.additional.contains_key("violationId"));
    }
}
