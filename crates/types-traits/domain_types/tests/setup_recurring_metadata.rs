use common_utils::metadata::{HeaderMaskingConfig, MaskedMetadata};
use domain_types::{
    connector_types::PaymentFlowData,
    types::{Connectors, SetupRecurringRequest},
    utils::ForeignTryFrom,
};
use grpc_api_types::payments as grpc;

#[test]
fn setup_recurring_preserves_the_caller_attempt_identifier(
) -> Result<(), Box<dyn std::error::Error>> {
    for identifier in [Some("attempt_setup_1"), Some(""), None] {
        let mut headers = tonic::metadata::MetadataMap::new();
        if let Some(identifier) = identifier {
            headers.insert("x-resource-id", identifier.parse()?);
        }
        let metadata = MaskedMetadata::new(headers, HeaderMaskingConfig::default());
        let request = SetupRecurringRequest::from(grpc::PaymentServiceSetupRecurringRequest {
            merchant_recurring_payment_id: "not_the_attempt_identifier".into(),
            address: Some(grpc::PaymentAddress::default()),
            ..Default::default()
        });
        let data = PaymentFlowData::foreign_try_from((request, Connectors::default(), &metadata))?;
        let expected = identifier
            .filter(|id| !id.is_empty())
            .unwrap_or("IRRELEVANT_ATTEMPT_ID");
        if data.attempt_id != expected {
            return Err(format!("expected {expected}, got {}", data.attempt_id).into());
        }
    }
    Ok(())
}
