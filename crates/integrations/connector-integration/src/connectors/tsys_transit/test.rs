#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
mod tests {
    use domain_types::connector_types::{
        ConnectorMandateReferenceId, MandateIds, MandateReferenceId, NetworkTokenWithNTIRef,
    };
    use hyperswitch_masking::PeekInterface;

    use crate::connectors::{
        macros::validate_xml_structure,
        tsys_transit::{
            profile::{CofPhase, MitKind},
            transformers::{
                decode_mandate_dispatch, format_mm_yy, generate_xml_or_fallback, MandateDispatch,
            },
        },
    };

    fn connector_mandate(id: Option<&str>) -> MandateIds {
        MandateIds {
            mandate_id: None,
            mandate_reference_id: Some(MandateReferenceId::ConnectorMandateId(
                ConnectorMandateReferenceId::new(
                    id.map(str::to_string),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                ),
            )),
        }
    }

    fn network_token_mandate() -> MandateIds {
        MandateIds {
            mandate_id: None,
            mandate_reference_id: Some(MandateReferenceId::NetworkTokenWithNTI(
                NetworkTokenWithNTIRef {
                    network_transaction_id: "NTI123".to_string(),
                    transaction_link_id: None,
                    token_exp_month: None,
                    token_exp_year: None,
                },
            )),
        }
    }

    #[test]
    fn network_token_with_nti_dispatches_the_nti() {
        match decode_mandate_dispatch(Some(&network_token_mandate())).unwrap() {
            MandateDispatch::Ntid { ntid } => assert_eq!(ntid, "NTI123"),
            other => panic!("expected Ntid, got {other:?}"),
        }
    }

    #[test]
    fn network_token_with_nti_is_an_mit() {
        assert_eq!(
            CofPhase::derive(Some(&network_token_mandate()), None, None, Some(true)),
            CofPhase::Mit(MitKind::Unscheduled)
        );
    }

    #[test]
    fn vault_connector_mandate_id_decodes() {
        assert!(matches!(
            decode_mandate_dispatch(Some(&connector_mandate(Some("cust:C1:W1")))).unwrap(),
            MandateDispatch::Vault { .. }
        ));
    }

    #[test]
    fn undecodable_connector_mandate_id_is_refused() {
        assert!(decode_mandate_dispatch(Some(&connector_mandate(Some("txn_123")))).is_err());
        assert!(decode_mandate_dispatch(Some(&connector_mandate(None))).is_err());
    }

    #[test]
    fn no_mandate_is_no_dispatch() {
        assert!(matches!(
            decode_mandate_dispatch(None).unwrap(),
            MandateDispatch::None
        ));
    }

    #[test]
    fn expiry_is_zero_padded_mm_yy() {
        assert_eq!(format_mm_yy("3", "2025").peek(), "03/25");
        assert_eq!(format_mm_yy("12", "28").peek(), "12/28");
    }

    #[test]
    fn serialization_failure_is_not_sendable_xml() {
        // A map with a non-string key cannot be serialized as XML.
        let unserializable = std::collections::HashMap::from([(1u8, 1u8)]);
        let xml = generate_xml_or_fallback(&unserializable, "Sale");
        assert!(
            validate_xml_structure(&xml).is_err(),
            "got sendable XML: {xml}"
        );
    }
}
