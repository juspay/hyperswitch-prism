use common_utils::metadata::MaskedMetadata;
use domain_types::{connector_types::ConnectorEnum, router_data::ConnectorSpecificConfig};

#[derive(Clone, Debug)]
pub struct FfiMetadataPayload {
    pub connector: ConnectorEnum,
    pub connector_config: ConnectorSpecificConfig,
}

#[derive(Debug)]
pub struct FfiRequestData<T> {
    pub payload: T,
    pub extracted_metadata: FfiMetadataPayload,
    pub masked_metadata: Option<MaskedMetadata>, // None when not provided
}
