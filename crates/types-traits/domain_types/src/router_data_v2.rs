use std::marker::PhantomData;

use crate::router_data::{ConnectorSpecificConfig, ErrorResponse};

#[derive(Debug, Clone)]
pub struct RouterDataV2<Flow, ResourceCommonData, FlowSpecificRequest, FlowSpecificResponse> {
    pub flow: PhantomData<Flow>,
    // pub tenant_id: id_type::TenantId, // TODO: Should we add this
    pub resource_common_data: ResourceCommonData,
    /// Typed connector integration config used to derive auth and request-scoped connector metadata.
    ///
    /// URL resolution should continue to come from `resource_common_data.connectors`, which is
    /// already the post-override runtime config.
    pub connector_config: ConnectorSpecificConfig,
    /// Contains flow-specific data required to construct a request and send it to the connector.
    pub request: FlowSpecificRequest,
    /// Contains flow-specific data that the connector responds with.
    pub response: Result<FlowSpecificResponse, ErrorResponse>,
}
