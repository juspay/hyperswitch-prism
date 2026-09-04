// Test file placeholder for Cashfree connector
// Tests will be implemented once the basic connector is working

#[cfg(test)]
mod tests {
    use domain_types::payment_method_data::DefaultPCIHolder;
    use interfaces::api::ConnectorCommon;

    use crate::connectors;

    #[test]
    fn test_cashfree_connector_creation() {
        // Basic test to ensure connector can be created
        let connector: &connectors::cashfree::Cashfree<DefaultPCIHolder> =
            super::super::Cashfree::new();
        assert_eq!(connector.id(), "cashfree");
    }

    /// The customer email must still reach Cashfree verbatim, while the
    /// structured logs (which serialize the typed struct via
    /// `masked_serialize`) must never carry the address.
    #[test]
    fn test_customer_email_masked_in_logs_but_verbatim_on_the_wire() {
        use hyperswitch_masking::Secret;

        use crate::connectors::cashfree::transformers::CashfreeCustomerDetails;

        let details = CashfreeCustomerDetails {
            customer_id: "cust_1".to_string(),
            customer_email: Some(Secret::new("jane.doe@example.com".to_string())),
            customer_phone: Secret::new("9999999999".to_string()),
            customer_name: Some("Jane Doe".to_string()),
        };

        let wire = serde_json::to_string(&details).expect("wire serialization");
        assert!(
            wire.contains("jane.doe@example.com"),
            "connector payload must be unchanged, got: {wire}"
        );

        let masked = hyperswitch_masking::masked_serialize(&details)
            .expect("masked serialization")
            .to_string();
        assert!(
            !masked.contains("jane.doe@example.com"),
            "email leaked into the log view: {masked}"
        );
        assert!(
            masked.contains("@example.com"),
            "EmailStrategy should retain the domain, got: {masked}"
        );
    }
}
