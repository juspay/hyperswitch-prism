# Connector Implementation Guide

This guide provides step-by-step instructions for adding support for a new connector and implementing its authorization flow in the connector service.

## Table of Contents
1. [Adding a New Connector](#adding-a-new-connector)
2. [Implementing Authorization Flow](#implementing-authorization-flow)
3. [Best Practices](#best-practices)

## Adding a NewConnectorName (NewConnectorName=the connector name which is needed to be integrated)

### 1. Support in framework
Add support for introducing the new connector to the following files:

1. `backend/domain_types/src/connector_types.rs`:
```rust
#[derive(Clone, Debug, strum::EnumString)]
#[strum(serialize_all = "snake_case")]
pub enum ConnectorEnum {
    Adyen,
    Razorpay,
    NewConnectorName, // Add your connector here
}

impl ForeignTryFrom<i32> for ConnectorEnum {
    type Error = ApplicationErrorResponse;

    fn foreign_try_from(connector: i32) -> Result<Self, error_stack::Report<Self::Error>> {
        match connector {
            2 => Ok(Self::Adyen),
            68 => Ok(Self::Razorpay),
            NEW_CONNECTOR_ID => Ok(Self::NewConnectorName), // Add your connector ID
            _ => Err(ApplicationErrorResponse::BadRequest(ApiError {
                sub_code: "INVALID_CONNECTOR".to_owned(),
                error_identifier: 401,
                error_message: format!("Invalid value for authenticate_by: {}", connector),
                error_object: None,
            })
            .into()),
        }
    }
}


```
2. `backend/domain_types/src/types.rs`:
```rust
#[derive(Clone, serde::Deserialize, Debug)]
pub struct Connectors {
    pub adyen: ConnectorParams,
    pub razorpay: ConnectorParams,
    pub new_connector: ConnectorParams, // Add your connector params
}
```

3. `backend/connector-integration/src/types.rs` :
```rust
use crate::connectors::{Adyen, Razorpay, NewConnectorName};

    fn convert_connector(connector_name: ConnectorEnum) -> BoxedConnector {
        match connector_name {
            ConnectorEnum::Adyen => Box::new(Adyen::new()),
            ConnectorEnum::Razorpay => Box::new(Razorpay::new()),
            ConnectorEnum::NewConnectorName => Box::new(NewConnectorName::new()),
        }
    }
```

4. `/backend/connector-integration/src/connectors.rs` : 

```rust
    pub mod new_connector_name;
    pub use self::new_connector_name::NewConnectorName;
```

### 2. Add base_url for new connector in config/development.toml

Note here NEW_CONNECTOR_ID will be taken from  `backend/grpc-api-types/proto/payment.proto`:
```protobuf
enum Connector {
    // ... existing connectors ...
    NEWCONNECTOR = NEW_CONNECTOR_ID; // pick from here.
}
```
#Add baseurl for new_connector in config/development.toml



### 3. Create Connector Implementation

1. Create a new directory in `backend/connector-integration/src/connectors/` for your connector:
```
backend/connector-integration/src/connectors/new_connector/
├── transformers.rs
```

```
backend/connector-integration/src/connectors/
├── new_connector.rs
```

### Note: Don't create any new file apart from this


2. Implement the connector in `new_connector.rs`:
You can take reference from `adyen.rs` and `razorpay.rs`
```rust
use domain_types::connector_types::ConnectorServiceTrait;

pub struct NewConnectorName;

impl NewConnectorName {
    pub fn new() -> Self {
        Self
    }
}

impl ConnectorServiceTrait for NewConnectorName {
    // Implement required traits
}
```

3. Add connector to `backend/connector-integration/src/types.rs`:


```rust
use crate::connectors::{Adyen, Razorpay, NewConnectorName};

impl ConnectorData {
    fn convert_connector(connector_name: ConnectorEnum) -> BoxedConnector {
        match connector_name {
            ConnectorEnum::Adyen => Box::new(Adyen::new()),
            ConnectorEnum::Razorpay => Box::new(Razorpay::new()),
            ConnectorEnum::NewConnectorName => Box::new(NewConnectorName::new()),
        }
    }
}
```

### 3. Implement Required Traits
You can take reference from `adyen.rs` and `razorpay.rs`


1. Add necessary imports in your connector's `new_connector.rs`: //Authorize is imported
```rust
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, RSync, Refund, Void},
    connector_types::{
        EventType, PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, RefundFlowData, RefundsData, RefundsResponseData,
    },
};
use error_stack::ResultExt;
use hyperswitch_api_models::enums::{self, AttemptStatus, RefundStatus};
use hyperswitch_common_utils::{
    errors::CustomResult,
    ext_traits::ByteSliceExt,
    request::Method,
    types::MinorUnit,
};
use hyperswitch_domain_models::{
    payment_method_data::{Card, PaymentMethodData},
    router_data::{ConnectorAuthType, ErrorResponse, RouterData},
    router_data_v2::RouterDataV2,
    router_request_types::ResponseId,
    router_response_types::{MandateReference, RedirectForm},
};
use hyperswitch_interfaces::{
    api::ConnectorCommon,
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    connector_integration_v2::ConnectorIntegrationV2,
    errors,
};
use hyperswitch_masking::{ExposeInterface, Secret};
use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};
use url::Url;
```

2. Add imports in `transformers.rs`:
```rust
use domain_types::{
    connector_flow::{Authorize, Capture, Refund, Void},
    connector_types::{
        EventType, PaymentFlowData, PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData,
        PaymentsResponseData, RefundFlowData, RefundsData, RefundsResponseData,
    },
};
use error_stack::ResultExt;
use hyperswitch_api_models::enums::{self, AttemptStatus, RefundStatus};
use hyperswitch_common_utils::{
    errors::CustomResult,
    ext_traits::ByteSliceExt,
    request::Method,
    types::MinorUnit,
};
use hyperswitch_domain_models::{
    payment_method_data::{Card, PaymentMethodData},
    router_data::{ConnectorAuthType, ErrorResponse, RouterData},
    router_data_v2::RouterDataV2,
    router_request_types::ResponseId,
    router_response_types::{MandateReference, RedirectForm},
};
use hyperswitch_interfaces::{
    consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE},
    errors,
};
use hyperswitch_masking::{ExposeInterface, Secret};
use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};
use url::Url;
```



3. Common type definitions to add in `transformers.rs`:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Amount {
    pub currency: enums::Currency,
    pub value: MinorUnit,
}

#[derive(Debug, Serialize, PartialEq)]
pub enum ConnectorError {
    ParsingFailed,
    NotImplemented,
    FailedToObtainAuthType,
}

#[derive(Debug, Serialize)]
pub struct NewConnectorRouterData<T> {
    pub amount: MinorUnit,
    pub router_data: T,
}

pub struct NewConnectorAuthType {
    pub(super) key_id: Secret<String>,
    pub(super) secret_key: Secret<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PaymentType {
    Card,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PaymentStatus {
    // Add your connector's payment statuses
    Succeeded,
    Failed,
    Pending,
    // ... other statuses
}
```

4. Required trait implementations with imports:
```rust
use hyperswitch_interfaces::{
    api::ConnectorCommon,
    connector_integration_v2::ConnectorIntegrationV2,
    errors::ConnectorError,
};

impl ConnectorCommon for NewConnectorName {
    fn id() -> &'static str {
        "new_connector"
    }

    fn common_get_content_type() -> &'static str {
        "application/json"
    }
}

impl ConnectorServiceTrait for NewConnectorName {}
impl PaymentAuthorizeV2 for NewConnectorName {}
impl PaymentSyncV2 for NewConnectorName {}
impl PaymentOrderCreate for NewConnectorName {}
impl PaymentVoidV2 for NewConnectorName {}
impl RefundSyncV2 for NewConnectorName {}
impl RefundV2 for NewConnectorName {}
impl PaymentCapture for NewConnectorName {}

impl ConnectorIntegrationV2<Authorize, PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData>
    for NewConnectorName
{
    fn get_headers(
        &self,
        req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData>,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::ConnectorError>
    where
        Self: ConnectorIntegrationV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData,
            PaymentsResponseData,
        >,
    {
        let mut header = vec![(
            headers::CONTENT_TYPE.to_string(),
            "application/json".to_string().into(),
        )];
        let mut api_key = self.get_auth_header(&req.connector_auth_type)?;
        header.append(&mut api_key);
        Ok(header)
    }

    fn get_url(
        &self,
        req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData>,
        ) -> CustomResult<String, errors::ConnectorError> {
        Ok(format!(
            "{}{}/payments",
            req.resource_common_data.connectors.new_connector.base_url,
            "v1" // Replace with actual API version
        ))
    }

    fn get_request_body(
        &self,
        req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData>,
    ) -> CustomResult<Option<RequestContent>, errors::ConnectorError> {
        let connector_router_data = NewConnectorRouterData::try_from((req.request.minor_amount, req))?;
        let connector_req = NewConnectorPaymentRequest::try_from(&connector_router_data)?;
        Ok(Some(RequestContent::Json(Box::new(connector_req))))
    }

    fn handle_response_v2(
        &self,
        data: &RouterDataV2<
            Authorize,
            PaymentFlowData,
            PaymentsAuthorizeData,
            PaymentsResponseData,
        >,
        event_builder: Option<&mut ConnectorEvent>,
        res: Response,
    ) -> CustomResult<
     RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData>,
        errors::ConnectorError,
    > {
        let response: NewConnectorPaymentResponse = res
            .response
            .parse_struct("NewConnectorPaymentResponse")
            .map_err(|_| errors::ConnectorError::ResponseDeserializationFailed)?;

        with_response_body!(event_builder, response);

        RouterDataV2::foreign_try_from((
            response,
            data.clone(),
            res.status_code,
            data.request.capture_method,
            false,
            data.request.payment_method_type,
        ))
        .change_context(errors::ConnectorError::ResponseHandlingFailed)
    }

    fn get_error_response_v2(
        &self,
        res: Response,
        event_builder: Option<&mut ConnectorEvent>,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        self.build_error_response(res, event_builder)
    }
     fn get_5xx_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut ConnectorEvent>,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        self.build_error_response(res, event_builder)
    }
}

impl ConnectorIntegrationV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>
    for NewConnectorName
{}
impl
    ConnectorIntegrationV2<
        CreateOrder,
        PaymentFlowData,
        PaymentCreateOrderData,
        PaymentCreateOrderResponse,
    > for NewConnectorName
{}
impl ConnectorIntegrationV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>
    for NewConnectorName
{}
impl IncomingWebhook for NewConnectorName {}
impl ConnectorIntegrationV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>
    for NewConnectorName
{
}

impl ConnectorIntegrationV2<Refund, RefundFlowData, RefundsData, RefundsResponseData> for NewConnectorName {
}
impl ConnectorIntegrationV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>
    for NewConnectorName
{}


These imports and type definitions are based on the current connector implementations in the codebase. Make sure to:

1. Only import what you need for your specific connector
2. Follow the same patterns as existing connectors
3. Keep the code organized and maintainable
4. Add proper error handling and type safety
5. Document any custom types or implementations

Remember to follow the existing patterns in the codebase and maintain consistency with other connector implementations. When in doubt, refer to the existing connectors as examples. 