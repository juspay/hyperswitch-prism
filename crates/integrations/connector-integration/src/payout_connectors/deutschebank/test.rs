#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
mod tests {
    use domain_types::errors::IntegrationError;

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
        assert!(key.contains("MIIEpkcs8"));
    }

    #[test]
    fn key_then_cert_is_order_independent() {
        let bundle = format!("{KEY_PKCS8}\n{CERT_A}\n");
        let (cert, key) = split_pem_bundle(&bundle).unwrap();
        assert!(cert.contains("MIIBcert+A"));
        assert!(key.contains("MIIEpkcs8"));
    }

    #[test]
    fn accepts_pkcs1_rsa_key() {
        let bundle = format!("{CERT_A}\n{KEY_PKCS1}\n");
        let (_, key) = split_pem_bundle(&bundle).unwrap();
        assert!(key.contains("MIIEpkcs1"));
        assert!(key.contains("RSA PRIVATE KEY"));
    }

    #[test]
    fn accepts_sec1_ec_key() {
        let bundle = format!("{CERT_A}\n{KEY_SEC1}\n");
        let (_, key) = split_pem_bundle(&bundle).unwrap();
        assert!(key.contains("MIIEsec1"));
        assert!(key.contains("EC PRIVATE KEY"));
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
        assert!(!key.contains("MIIBpub"));
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
}
