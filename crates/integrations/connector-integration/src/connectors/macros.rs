use std::marker::PhantomData;

use common_enums::DynamicContentType;
use common_utils::{errors::CustomResult, ext_traits::BytesExt};
use domain_types::{errors, router_data_v2::RouterDataV2};
use error_stack::ResultExt;

use crate::types;

/// Trait for connectors that need to dynamically select content types
/// based on runtime conditions such as payment method, flow type, etc.
pub trait ContentTypeSelector<F, FCD, Req, Res> {
    /// Determines the appropriate content type for the given request
    fn get_dynamic_content_type(
        &self,
        req: &RouterDataV2<F, FCD, Req, Res>,
    ) -> CustomResult<DynamicContentType, errors::ConnectorError>;
}

pub trait FlowTypes {
    type Flow;
    type FlowCommonData;
    type Request;
    type Response;
}

impl<F, FCD, Req, Resp> FlowTypes for RouterDataV2<F, FCD, Req, Resp> {
    type Flow = F;
    type FlowCommonData = FCD;
    type Request = Req;
    type Response = Resp;
}

impl<F, FCD, Req, Resp> FlowTypes for &RouterDataV2<F, FCD, Req, Resp> {
    type Flow = F;
    type FlowCommonData = FCD;
    type Request = Req;
    type Response = Resp;
}

pub trait GetFormData {
    fn get_form_data(&self) -> common_utils::request::MultipartData;
}

/// Trait for converting request structures into SOAP XML format
/// Implement this trait for request types that need to be sent as SOAP/XML
pub trait GetSoapXml {
    /// Converts the implementing type into a SOAP XML string
    /// The output should be properly formatted and escaped XML
    fn to_soap_xml(&self) -> String;
}

/// Helper function to validate XML is well-formed
/// This catches malformed XML before sending to the connector
#[inline]
pub(crate) fn validate_xml_structure(xml_str: &str) -> Result<(), String> {
    use quick_xml::{events::Event, Reader};

    let mut reader = Reader::from_str(xml_str);
    reader.trim_text(true);
    reader.check_end_names(true); // Verify open/close tags match

    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("XML validation failed: {e}")),
            _ => {}
        }
        buf.clear();
    }

    Ok(())
}

pub struct NoRequestBody;
pub struct NoRequestBodyTemplating;

impl<F, FCD, Req, Resp> TryFrom<RouterDataV2<F, FCD, Req, Resp>> for NoRequestBody {
    type Error = error_stack::Report<errors::ConnectorError>;

    fn try_from(_value: RouterDataV2<F, FCD, Req, Resp>) -> Result<Self, Self::Error> {
        Ok(Self)
    }
}

type RouterDataType<T> = RouterDataV2<
    <T as FlowTypes>::Flow,
    <T as FlowTypes>::FlowCommonData,
    <T as FlowTypes>::Request,
    <T as FlowTypes>::Response,
>;

type ResponseRouterDataType<T, R> = types::ResponseRouterData<
    R,
    RouterDataV2<
        <T as FlowTypes>::Flow,
        <T as FlowTypes>::FlowCommonData,
        <T as FlowTypes>::Request,
        <T as FlowTypes>::Response,
    >,
>;

pub trait BridgeRequestResponse: Send + Sync {
    type RequestBody;
    type ResponseBody;
    type ConnectorInputData: FlowTypes;
    fn request_body(
        &self,
        rd: Self::ConnectorInputData,
    ) -> CustomResult<Self::RequestBody, errors::ConnectorError>
    where
        Self::RequestBody:
            TryFrom<Self::ConnectorInputData, Error = error_stack::Report<errors::ConnectorError>>,
    {
        Self::RequestBody::try_from(rd)
    }

    fn response(
        &self,
        bytes: bytes::Bytes,
    ) -> CustomResult<Self::ResponseBody, errors::ConnectorError>
    where
        Self::ResponseBody: for<'a> serde::Deserialize<'a>,
    {
        if bytes.is_empty() {
            serde_json::from_str("{}")
                .change_context(errors::ConnectorError::ResponseDeserializationFailed)
        } else {
            bytes
                .parse_struct(std::any::type_name::<Self::ResponseBody>())
                .change_context(errors::ConnectorError::ResponseDeserializationFailed)
        }
    }

    fn router_data(
        &self,
        response: ResponseRouterDataType<Self::ConnectorInputData, Self::ResponseBody>,
    ) -> CustomResult<RouterDataType<Self::ConnectorInputData>, errors::ConnectorError>
    where
        RouterDataType<Self::ConnectorInputData>: TryFrom<
            ResponseRouterDataType<Self::ConnectorInputData, Self::ResponseBody>,
            Error = error_stack::Report<errors::ConnectorError>,
        >,
    {
        RouterDataType::<Self::ConnectorInputData>::try_from(response)
    }
}

#[derive(Clone)]
pub struct Bridge<Q, S, T>(pub PhantomData<(Q, S, T)>);

macro_rules! expand_fn_get_request_body {
    ($connector: ident, $curl_res: ty, $flow: ident, $resource_common_data: ty, $request: ident, $response: ty) => {
        paste::paste! {
            fn get_request_body(
                &self,
                _req: &RouterDataV2<$flow, $resource_common_data, $request, $response>,
            ) -> CustomResult<Option<macro_types::RequestContent>, macro_types::ConnectorError>
            {
                // always return None
                Ok(None)
            }
        }
    };
    (
        $connector: ident,
        $curl_req: ty,
        FormData,
        $curl_res: ty,
        $flow: ident,
        $resource_common_data: ty,
        $request: ty,
        $response: ty
    ) => {
        paste::paste! {
            fn get_request_body(
                &self,
                req: &RouterDataV2<$flow, $resource_common_data, $request, $response>,
            ) -> CustomResult<Option<macro_types::RequestContent>, macro_types::ConnectorError>
            {
                let bridge = self.[< $flow:snake >];
                let input_data = [<$connector RouterData>] {
                    connector: self.to_owned(),
                    router_data: req.clone(),
                };
                let request = bridge.request_body(input_data)?;
                let form_data = <$curl_req as GetFormData>::get_form_data(&request);
                Ok(Some(macro_types::RequestContent::FormData(form_data)))
            }
        }
    };
    (
        $connector: ident,
        $curl_req: ty,
        SoapXml,
        $curl_res: ty,
        $flow: ident,
        $resource_common_data: ty,
        $request: ty,
        $response: ty
    ) => {
        paste::paste! {
            fn get_request_body(
                &self,
                req: &RouterDataV2<$flow, $resource_common_data, $request, $response>,
            ) -> CustomResult<Option<macro_types::RequestContent>, macro_types::ConnectorError>
            {
                let bridge = self.[< $flow:snake >];
                let input_data = [<$connector RouterData>] {
                    connector: self.to_owned(),
                    router_data: req.clone(),
                };
                let request = bridge.request_body(input_data)?;
                let soap_xml = <$curl_req as GetSoapXml>::to_soap_xml(&request);

                // Validate XML structure before sending
                crate::connectors::macros::validate_xml_structure(&soap_xml)
                    .map_err(|e| {
                        error_stack::report!(errors::ConnectorError::RequestEncodingFailed)
                            .attach_printable(e)
                    })?;

                Ok(Some(macro_types::RequestContent::RawBytes(soap_xml.into_bytes())))
            }
        }
    };
    (
        $connector: ty,
        $curl_req: ty,
        Dynamic,
        $curl_res: ty,
        $flow: ident,
        $resource_common_data: ty,
        $request: ty,
        $response: ty
    ) => {
        paste::paste! {
            fn get_request_body(
                &self,
                req: &RouterDataV2<$flow, $resource_common_data, $request, $response>,
            ) -> CustomResult<Option<macro_types::RequestContent>, macro_types::ConnectorError>
            {
                use crate::connectors::macros::ContentTypeSelector;

                let bridge = self.[< $flow:snake >];
                let input_data = [< $connector RouterData >] {
                    connector: self.to_owned(),
                    router_data: req.clone(),
                };
                let request = bridge.request_body(input_data)?;

                // Get dynamic content type based on runtime conditions
                let content_type = self.get_dynamic_content_type(req)?;

                match content_type {
                    common_enums::DynamicContentType::Json => {
                        Ok(Some(macro_types::RequestContent::Json(Box::new(request))))
                    }
                    common_enums::DynamicContentType::FormUrlEncoded => {
                        Ok(Some(macro_types::RequestContent::FormUrlEncoded(Box::new(request))))
                    }
                    common_enums::DynamicContentType::FormData => {
                        let form_data = <$curl_req as GetFormData>::get_form_data(&request);
                        Ok(Some(macro_types::RequestContent::FormData(form_data)))
                    }
                }
            }
        }
    };
    (
        $connector: ty,
        $curl_req: ty,
        $content_type: ident,
        $curl_res: ty,
        $flow: ident,
        $resource_common_data: ty,
        $request: ty,
        $response: ty
    ) => {
        paste::paste! {
            fn get_request_body(
                &self,
                req: &RouterDataV2<$flow, $resource_common_data, $request, $response>,
            ) -> CustomResult<Option<macro_types::RequestContent>, macro_types::ConnectorError>
            {
                let bridge = self.[< $flow:snake >];
                let input_data = [< $connector RouterData >] {
                    connector: self.to_owned(),
                    router_data: req.clone(),
                };
                let request = bridge.request_body(input_data)?;
                Ok(Some(macro_types::RequestContent::$content_type(Box::new(request))))
            }
        }
    };
}
pub(crate) use expand_fn_get_request_body;

macro_rules! expand_fn_handle_response {
    // When preprocess_response is enabled - only for connectors that explicitly set it
    ($connector: ident, $flow: ident, $resource_common_data: ty, $request: ty, $response: ty, preprocess_enabled) => {
        fn handle_response_v2(
            &self,
            data: &RouterDataV2<$flow, $resource_common_data, $request, $response>,
            event_builder: Option<&mut events::Event>,
            res: Response,
        ) -> CustomResult<
            RouterDataV2<$flow, $resource_common_data, $request, $response>,
            macro_types::ConnectorError,
        > {
            use error_stack::ResultExt;
            paste::paste! {let bridge = self.[< $flow:snake >];}

            // Apply preprocessing if specified in the macro
            let response_bytes = self
                .preprocess_response_bytes(data, res.response)
                .change_context(errors::ConnectorError::ResponseDeserializationFailed)?;

            let response_body = bridge.response(response_bytes)?;
            event_builder.map(|i| i.set_connector_response(&response_body));
            let response_router_data = ResponseRouterData {
                response: response_body,
                router_data: data.clone(),
                http_code: res.status_code,
            };
            let result = bridge.router_data(response_router_data)?;
            Ok(result)
        }
    };

    // When preprocess_response is disabled or default
    ($connector: ident, $flow: ident, $resource_common_data: ty, $request: ty, $response: ty, $preprocess_flag:tt) => {
        fn handle_response_v2(
            &self,
            data: &RouterDataV2<$flow, $resource_common_data, $request, $response>,
            event_builder: Option<&mut events::Event>,
            res: Response,
        ) -> CustomResult<
            RouterDataV2<$flow, $resource_common_data, $request, $response>,
            macro_types::ConnectorError,
        > {
            paste::paste! {let bridge = self.[< $flow:snake >];}
            let response_body = bridge.response(res.response)?;
            event_builder.map(|i| i.set_connector_response(&response_body));
            let response_router_data = ResponseRouterData {
                response: response_body,
                router_data: data.clone(),
                http_code: res.status_code,
            };
            let result = bridge.router_data(response_router_data)?;
            Ok(result)
        }
    };
}
pub(crate) use expand_fn_handle_response;

macro_rules! expand_default_functions {
    (
        function: get_headers,
        flow_name:$flow: ident,
        resource_common_data:$resource_common_data: ty,
        flow_request:$request: ty,
        flow_response:$response: ty,
    ) => {
        fn get_headers(
            &self,
            req: &RouterDataV2<$flow, $resource_common_data, $request, $response>,
        ) -> macro_types::CustomResult<
            Vec<(String, macro_types::Maskable<String>)>,
            macro_types::ConnectorError,
        > {
            self.build_headers(req)
        }
    };
    (
        function: get_content_type,
        flow_name:$flow: ident,
        resource_common_data:$resource_common_data: ty,
        flow_request:$request: ty,
        flow_response:$response: ty,
    ) => {
        fn get_content_type(&self) -> &'static str {
            self.common_get_content_type()
        }
    };
    (
        function: get_error_response_v2,
        flow_name:$flow: ident,
        resource_common_data:$resource_common_data: ty,
        flow_request:$request: ty,
        flow_response:$response: ty,
    ) => {
        fn get_error_response_v2(
            &self,
            res: Response,
            event_builder: Option<&mut events::Event>,
        ) -> CustomResult<ErrorResponse, macro_types::ConnectorError> {
            self.build_error_response(res, event_builder)
        }
    };
}
pub(crate) use expand_default_functions;

macro_rules! macro_connector_implementation {
    // MOST SPECIFIC PATTERNS FIRST - Version with preprocess_response: true and curl_request
    (
        connector_default_implementations: [$($function_name: ident), *],
        connector: $connector: ident,
        curl_request: $content_type:ident($curl_req: ty),
        curl_response:$curl_res: ty,
        flow_name:$flow: ident,
        resource_common_data:$resource_common_data: ty,
        flow_request:$request: ty,
        flow_response:$response: ty,
        http_method: $http_method_type:ident,
        preprocess_response: true,
        generic_type: $generic_type:tt,
        [$($bounds:tt)*],
        other_functions: {
            $($function_def: tt)*
        }
    ) => {
        impl <$generic_type: $($bounds)*>
            ConnectorIntegrationV2<
                $flow,
                $resource_common_data,
                $request,
                $response,
            > for $connector<$generic_type>
        {
            fn get_http_method(&self) -> common_utils::request::Method {
                common_utils::request::Method::$http_method_type
            }
            $($function_def)*
            $(
                macros::expand_default_functions!(
                    function: $function_name,
                    flow_name:$flow,
                    resource_common_data:$resource_common_data,
                    flow_request:$request,
                    flow_response:$response,
                );
            )*
            macros::expand_fn_get_request_body!(
                $connector,
                $curl_req,
                $content_type,
                $curl_res,
                $flow,
                $resource_common_data,
                $request,
                $response
            );
            macros::expand_fn_handle_response!(
                $connector,
                $flow,
                $resource_common_data,
                $request,
                $response,
                preprocess_enabled
            );
        }
    };

    // Version with preprocess_response: true but no curl_request
    (
        connector_default_implementations: [$($function_name: ident), *],
        connector: $connector: ident,
        curl_response:$curl_res: ty,
        flow_name:$flow: ident,
        resource_common_data:$resource_common_data: ty,
        flow_request:$request: ty,
        flow_response:$response: ty,
        http_method: $http_method_type:ident,
        preprocess_response: true,
        generic_type: $generic_type:tt,
        [$($bounds:tt)*],
        other_functions: {
            $($function_def: tt)*
        }
    ) => {
        impl <$generic_type: $($bounds)*>
            ConnectorIntegrationV2<
                $flow,
                $resource_common_data,
                $request,
                $response,
            > for $connector<$generic_type>
        {
            fn get_http_method(&self) -> common_utils::request::Method {
                common_utils::request::Method::$http_method_type
            }
            $($function_def)*
            $(
                macros::expand_default_functions!(
                    function: $function_name,
                    flow_name:$flow,
                    resource_common_data:$resource_common_data,
                    flow_request:$request,
                    flow_response:$response,
                );
            )*
            macros::expand_fn_get_request_body!(
                $connector,
                $curl_res,
                $flow,
                $resource_common_data,
                $request,
                $response
            );
            macros::expand_fn_handle_response!(
                $connector,
                $flow,
                $resource_common_data,
                $request,
                $response,
                preprocess_enabled
            );
        }
    };

    // Version with Dynamic content type selection
    (
        connector_default_implementations: [$($function_name:ident), *],
        connector: $connector:ident,
        curl_request: Dynamic($curl_req:ty),
        curl_response: $curl_res:ty,
        flow_name: $flow:ident,
        resource_common_data:$resource_common_data:ty,
        flow_request: $request:ty,
        flow_response: $response:ty,
        http_method: $http_method_type:ident,
        generic_type: $generic_type:tt,
        [$($bounds:tt)*],
        other_functions: {
            $($function_def: tt)*
        }
    ) => {
        impl<$generic_type: $($bounds)*>
            ConnectorIntegrationV2<
                $flow,
                $resource_common_data,
                $request,
                $response,
            > for $connector<$generic_type>
        {
            fn get_http_method(&self) -> common_utils::request::Method {
                common_utils::request::Method::$http_method_type
            }
            $($function_def)*
            $(
                macros::expand_default_functions!(
                    function: $function_name,
                    flow_name:$flow,
                    resource_common_data:$resource_common_data,
                    flow_request:$request,
                    flow_response:$response,
                );
            )*
            macros::expand_fn_get_request_body!(
                $connector,
                $curl_req,
                Dynamic,
                $curl_res,
                $flow,
                $resource_common_data,
                $request,
                $response
            );
            macros::expand_fn_handle_response!(
                $connector,
                $flow,
                $resource_common_data,
                $request,
                $response,
                no_preprocess
            );
        }
    };

    //Version without preprocess_response parameter (defaults to false)
    (
        connector_default_implementations: [$($function_name:ident), *],
        connector: $connector:ident,
        curl_request: $content_type:ident($curl_req:ty),
        curl_response: $curl_res:ty,
        flow_name: $flow:ident,
        resource_common_data:$resource_common_data:ty,
        flow_request: $request:ty,
        flow_response: $response:ty,
        http_method: $http_method_type:ident,
        generic_type: $generic_type:tt,
        [$($bounds:tt)*],
        other_functions: {
            $($function_def: tt)*
        }
    ) => {
        impl<$generic_type: $($bounds)*>
            ConnectorIntegrationV2<
                $flow,
                $resource_common_data,
                $request,
                $response,
            > for $connector<$generic_type>
        {
            fn get_http_method(&self) -> common_utils::request::Method {
                common_utils::request::Method::$http_method_type
            }
            $($function_def)*
            $(
                macros::expand_default_functions!(
                    function: $function_name,
                    flow_name:$flow,
                    resource_common_data:$resource_common_data,
                    flow_request:$request,
                    flow_response:$response,
                );
            )*
            macros::expand_fn_get_request_body!(
                $connector,
                $curl_req,
                $content_type,
                $curl_res,
                $flow,
                $resource_common_data,
                $request,
                $response
            );
            macros::expand_fn_handle_response!(
                $connector,
                $flow,
                $resource_common_data,
                $request,
                $response,
                no_preprocess
            );
        }
    };
    (
        connector_default_implementations: [$($function_name:ident), *],
        connector: $connector:ident,
        curl_response: $curl_res:ty,
        flow_name: $flow:ident,
        resource_common_data:$resource_common_data:ty,
        flow_request: $request:ident,
        flow_response: $response:ty,
        http_method: $http_method_type:ident,
        generic_type: $generic_type:tt,
        [$($bounds:tt)*],
        other_functions: {
            $($function_def: tt)*
        }
    ) => {
        impl<$generic_type: $($bounds)*>
            ConnectorIntegrationV2<
                $flow,
                $resource_common_data,
                $request,
                $response,
            > for $connector<$generic_type>
        {
            fn get_http_method(&self) -> common_utils::request::Method {
                common_utils::request::Method::$http_method_type
            }
            $($function_def)*
            $(
                macros::expand_default_functions!(
                    function: $function_name,
                    flow_name:$flow,
                    resource_common_data:$resource_common_data,
                    flow_request:$request,
                    flow_response:$response,
                );
            )*
            macros::expand_fn_get_request_body!(
                $connector,
                $curl_res,
                $flow,
                $resource_common_data,
                $request,
                $response
            );
            macros::expand_fn_handle_response!(
                $connector,
                $flow,
                $resource_common_data,
                $request,
                $response,
                no_preprocess
            );
        }
    };
    (
        connector_default_implementations: [$($function_name:ident), *],
        connector: $connector:ident,
        curl_response: $curl_res:ty,
        flow_name: $flow:ident,
        resource_common_data:$resource_common_data:ty,
        flow_request: $request:ty,
        flow_response: $response:ty,
        http_method: $http_method_type:ident,
        preprocess_response: $preprocess_response: expr,
        generic_type: $generic_type:tt,
        [$($bounds:tt)*],
        other_functions: {
            $($function_def: tt)*
        }
    ) => {
        impl<$generic_type: $($bounds)*>
            ConnectorIntegrationV2<
                $flow,
                $resource_common_data,
                $request,
                $response,
            > for $connector<$generic_type>
        {
            fn get_http_method(&self) -> common_utils::request::Method {
                common_utils::request::Method::$http_method_type
            }
            $($function_def)*
            $(
                macros::expand_default_functions!(
                    function: $function_name,
                    flow_name:$flow,
                    resource_common_data:$resource_common_data,
                    flow_request:$request,
                    flow_response:$response,
                );
            )*
            macros::expand_fn_get_request_body!(
                $connector,
                $curl_res,
                $flow,
                $resource_common_data,
                $request,
                $response
            );
            macros::expand_fn_handle_response!(
                $connector,
                $flow,
                $resource_common_data,
                $request,
                $response,
                preprocess_enabled
            );
        }
    };
}
pub(crate) use macro_connector_implementation;

macro_rules! impl_templating {

    (
        connector: $connector: ident,
        curl_request: $curl_req: ident,
        curl_response: $curl_res: ident,
        router_data: $router_data: ty,
        generic_type: $generic_type: tt,
    ) => {
        paste::paste!{
            pub struct [<$curl_req Templating>];
            pub struct [<$curl_res Templating>];

            impl<$generic_type: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + serde::Serialize> BridgeRequestResponse for Bridge<[<$curl_req Templating>], [<$curl_res Templating>], $generic_type>{
                type RequestBody = $curl_req;
                type ResponseBody = $curl_res;
                type ConnectorInputData = [<$connector RouterData>]<$router_data, $generic_type>;
            }
        }
    };
    (
        connector: $connector: ident,
        curl_response: $curl_res: ident,
        router_data: $router_data: ty,
        generic_type: $generic_type:tt,
    ) => {
        paste::paste!{
            pub struct [<$curl_res Templating>];

            impl<$generic_type: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + serde::Serialize> BridgeRequestResponse for Bridge<NoRequestBodyTemplating, [<$curl_res Templating>], $generic_type> {
                type RequestBody = NoRequestBody;
                type ResponseBody = $curl_res;
                type ConnectorInputData = [<$connector RouterData>]<$router_data, $generic_type>;
            }
        }
    };
}
pub(crate) use impl_templating;

macro_rules! impl_templating_mixed {
    // Pattern for generic request types like AdyenPaymentRequest<T>
    (
        connector: $connector: ident,
        curl_request: $base_req: ident<$req_generic: ident>,
        curl_response: $curl_res: ident,
        router_data: $router_data: ty,
        generic_type: $generic_type: tt,
    ) => {
        paste::paste!{
            pub struct [<$base_req Templating>];
            pub struct [<$curl_res Templating>];

            impl<$generic_type: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + serde::Serialize> BridgeRequestResponse for Bridge<[<$base_req Templating>], [<$curl_res Templating>], $generic_type>{
                type RequestBody = $base_req<$generic_type>;
                type ResponseBody = $curl_res;
                type ConnectorInputData = [<$connector RouterData>]<$router_data, $generic_type>;
            }
        }
    };

    // Pattern for non-generic request types like AdyenRedirectRequest
    (
        connector: $connector: ident,
        curl_request: $base_req: ident,
        curl_response: $curl_res: ident,
        router_data: $router_data: ty,
        generic_type: $generic_type: tt,
    ) => {
        paste::paste!{
            pub struct [<$base_req Templating>];
            pub struct [<$curl_res Templating>];

            impl<$generic_type: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + serde::Serialize> BridgeRequestResponse for Bridge<[<$base_req Templating>], [<$curl_res Templating>], $generic_type>{
                type RequestBody = $base_req;
                type ResponseBody = $curl_res;
                type ConnectorInputData = [<$connector RouterData>]<$router_data, $generic_type>;
            }
        }
    };

    // Pattern for generic request with XML response parsing
    (
        connector: $connector: ident,
        curl_request: $base_req: ident<$req_generic: ident>,
        curl_response: $curl_res: ident,
        router_data: $router_data: ty,
        generic_type: $generic_type: tt,
        response_format: xml,
    ) => {
        paste::paste!{
            pub struct [<$base_req Templating>];
            pub struct [<$curl_res Templating>];

            impl<$generic_type: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + serde::Serialize> BridgeRequestResponse for Bridge<[<$base_req Templating>], [<$curl_res Templating>], $generic_type>{
                type RequestBody = $base_req<$generic_type>;
                type ResponseBody = $curl_res;
                type ConnectorInputData = [<$connector RouterData>]<$router_data, $generic_type>;

                fn response(
                    &self,
                    bytes: bytes::Bytes,
                ) -> CustomResult<Self::ResponseBody, errors::ConnectorError> {
                    use common_utils::ext_traits::XmlExt;
                    use error_stack::ResultExt;

                    if bytes.is_empty() {
                        return Err(errors::ConnectorError::ResponseDeserializationFailed.into());
                    }

                    let response_str = String::from_utf8(bytes.to_vec())
                        .change_context(errors::ConnectorError::ResponseDeserializationFailed)
                        .attach_printable("Failed to convert response bytes to UTF-8 string")?;

                    response_str
                        .as_str()
                        .parse_xml::<Self::ResponseBody>()
                        .change_context(errors::ConnectorError::ResponseDeserializationFailed)
                        .attach_printable("Failed to parse XML response")
                }
            }
        }
    };

    // Pattern for non-generic request with XML response parsing
    (
        connector: $connector: ident,
        curl_request: $base_req: ident,
        curl_response: $curl_res: ident,
        router_data: $router_data: ty,
        generic_type: $generic_type: tt,
        response_format: xml,
    ) => {
        paste::paste!{
            pub struct [<$base_req Templating>];
            pub struct [<$curl_res Templating>];

            impl<$generic_type: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + serde::Serialize> BridgeRequestResponse for Bridge<[<$base_req Templating>], [<$curl_res Templating>], $generic_type>{
                type RequestBody = $base_req;
                type ResponseBody = $curl_res;
                type ConnectorInputData = [<$connector RouterData>]<$router_data, $generic_type>;

                fn response(
                    &self,
                    bytes: bytes::Bytes,
                ) -> CustomResult<Self::ResponseBody, errors::ConnectorError> {
                    use common_utils::ext_traits::XmlExt;
                    use error_stack::ResultExt;

                    if bytes.is_empty() {
                        return Err(errors::ConnectorError::ResponseDeserializationFailed.into());
                    }

                    let response_str = String::from_utf8(bytes.to_vec())
                        .change_context(errors::ConnectorError::ResponseDeserializationFailed)
                        .attach_printable("Failed to convert response bytes to UTF-8 string")?;

                    response_str
                        .as_str()
                        .parse_xml::<Self::ResponseBody>()
                        .change_context(errors::ConnectorError::ResponseDeserializationFailed)
                        .attach_printable("Failed to parse XML response")
                }
            }
        }
    };
}
pub(crate) use impl_templating_mixed;

macro_rules! resolve_request_body_type {
    // Generic type like AdyenPaymentRequest<T>
    ($base_req: ident<$req_generic: ident>, $generic_type: tt) => {
        $base_req<$generic_type>
    };
    // Non-generic type like AdyenRedirectRequest
    ($base_req: ident, $generic_type: tt) => {
        $base_req
    };
}
pub(crate) use resolve_request_body_type;

macro_rules! resolve_templating_type {
    // Generic type like AdyenPaymentRequest<T>
    ($base_req: ident<$req_generic: ident>) => {
        paste::paste! { [<$base_req Templating>] }
    };
    // Non-generic type like AdyenRedirectRequest
    ($base_req: ident) => {
        paste::paste! { [<$base_req Templating>] }
    };
}
pub(crate) use resolve_templating_type;

macro_rules! expand_connector_input_data {
    ($connector: ident, $generics: tt) => {
        paste::paste! {
            pub struct [<$connector RouterData>]<RD: FlowTypes, $generics: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + serde::Serialize> {
                pub connector: $connector<$generics>,
                pub router_data: RD,
            }
            impl<RD: FlowTypes, $generics: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + serde::Serialize> FlowTypes for [<$connector RouterData>]<RD, $generics> { //here too
                type Flow = RD::Flow;
                type FlowCommonData = RD::FlowCommonData;
                type Request = RD::Request;
                type Response = RD::Response;
            }
        }
    };
}
pub(crate) use expand_connector_input_data;

macro_rules! create_all_prerequisites {
    (
        connector_name: $connector: ident,
        generic_type: $generic_type:tt,
        api: [
            $(
                (
                    flow: $flow_name: ident,
                    $(request_body: $flow_request: ident $(<$generic_param: ident>)?,)?
                    response_body: $flow_response: ident,
                    $(response_format: $response_format:ident,)?
                    router_data: $router_data_type: ty,
                )
            ),*
        ],
        amount_converters: [
            $($converter_name:ident : $amount_unit:ty),*
        ],
        member_functions: {
            $($function_def: tt)*
        }
    ) => {
        crate::connectors::macros::expand_imports!();
        crate::connectors::macros::expand_connector_input_data!($connector, $generic_type);
        $(
            crate::connectors::macros::create_all_prerequisites_impl_templating!(
                connector: $connector,
                $(request_body: $flow_request $(<$generic_param>)?,)?
                response_body: $flow_response,
                $(response_format: $response_format,)?
                router_data: $router_data_type,
                generic_type: $generic_type,
            );
        )*
        paste::paste! {
            pub struct $connector<$generic_type: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + serde::Serialize> {
                $(
                    pub $converter_name: &'static (dyn common_utils::types::AmountConvertor<Output = $amount_unit> + Sync),
                )*
                $(
                    [<$flow_name:snake>]: &'static dyn BridgeRequestResponse<
                        RequestBody = crate::connectors::macros::create_all_prerequisites_resolve_request_body_type!($(request_body: $flow_request $(<$generic_param>)?,)? generic_type: $generic_type),
                        ResponseBody = $flow_response,
                        ConnectorInputData = [<$connector RouterData>]<$router_data_type, $generic_type>,
                    >,
                )*
            }

            impl<$generic_type: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + serde::Serialize> Clone for $connector<$generic_type> {
                fn clone(&self) -> Self {
                    Self {
                        $(
                            $converter_name: self.$converter_name,
                        )*
                        $(
                            [<$flow_name:snake>]: self.[<$flow_name:snake>],
                        )*
                    }
                }
            }

            impl<$generic_type: PaymentMethodDataTypes + std::fmt::Debug + std::marker::Sync + std::marker::Send + 'static + serde::Serialize>  $connector<$generic_type> {
                pub const fn new() -> &'static Self {
                    &Self{
                        $(
                            $converter_name: &common_utils::types::[<$amount_unit ForConnector>],
                        )*
                        $(
                            [<$flow_name:snake>]: &Bridge::<
                                    crate::connectors::macros::create_all_prerequisites_resolve_templating_type!($(request_body: $flow_request $(<$generic_param>)?,)?),
                                    [<$flow_response Templating>], $generic_type
                                >(PhantomData),
                        )*
                    }
                }
                $($function_def)*
            }
        }
    };
}
pub(crate) use create_all_prerequisites;

macro_rules! create_all_prerequisites_impl_templating {
    // Pattern with request body and XML response format
    (
        connector: $connector: ident,
        request_body: $flow_request: ident $(<$generic_param: ident>)?,
        response_body: $flow_response: ident,
        response_format: xml,
        router_data: $router_data_type: ty,
        generic_type: $generic_type: tt,
    ) => {
        crate::connectors::macros::impl_templating_mixed!(
            connector: $connector,
            curl_request: $flow_request $(<$generic_param>)?,
            curl_response: $flow_response,
            router_data: $router_data_type,
            generic_type: $generic_type,
            response_format: xml,
        );
    };

    // Pattern with request body (default JSON response format)
    (
        connector: $connector: ident,
        request_body: $flow_request: ident $(<$generic_param: ident>)?,
        response_body: $flow_response: ident,
        router_data: $router_data_type: ty,
        generic_type: $generic_type: tt,
    ) => {
        crate::connectors::macros::impl_templating_mixed!(
            connector: $connector,
            curl_request: $flow_request $(<$generic_param>)?,
            curl_response: $flow_response,
            router_data: $router_data_type,
            generic_type: $generic_type,
        );
    };

    // Pattern without request body
    (
        connector: $connector: ident,
        response_body: $flow_response: ident,
        router_data: $router_data_type: ty,
        generic_type: $generic_type: tt,
    ) => {
        crate::connectors::macros::impl_templating!(
            connector: $connector,
            curl_response: $flow_response,
            router_data: $router_data_type,
            generic_type: $generic_type,
        );
    };
}
pub(crate) use create_all_prerequisites_impl_templating;

macro_rules! create_all_prerequisites_resolve_request_body_type {
    // Pattern with request body
    (
        request_body: $flow_request: ident $(<$generic_param: ident>)?,
        generic_type: $generic_type: tt
    ) => {
        crate::connectors::macros::resolve_request_body_type!($flow_request $(<$generic_param>)?, $generic_type)
    };

    // Pattern without request body
    (
        generic_type: $generic_type: tt
    ) => {
        NoRequestBody
    };
}
pub(crate) use create_all_prerequisites_resolve_request_body_type;

macro_rules! create_all_prerequisites_resolve_templating_type {
    // Pattern with request body
    (
        request_body: $flow_request: ident $(<$generic_param: ident>)?,
    ) => {
        crate::connectors::macros::resolve_templating_type!($flow_request $(<$generic_param>)?)
    };

    // Pattern without request body
    () => {
        NoRequestBodyTemplating
    };
}
pub(crate) use create_all_prerequisites_resolve_templating_type;

macro_rules! expand_imports {
    () => {
        use std::marker::PhantomData;

        #[allow(unused_imports)]
        use crate::connectors::macros::{
            Bridge, BridgeRequestResponse, FlowTypes, GetFormData, NoRequestBody,
            NoRequestBodyTemplating,
        };
        #[allow(unused_imports)]
        mod macro_types {
            // pub(super) use domain_models::{
            //     AuthenticationInitiation, Confirmation, PostAuthenticationSync, PreAuthentication,
            // };
            pub(super) use common_utils::{errors::CustomResult, events, request::RequestContent};
            pub(super) use domain_types::{
                errors::ConnectorError, router_data::ErrorResponse, router_data_v2::RouterDataV2,
                router_response_types::Response,
            };
            pub(super) use hyperswitch_masking::Maskable;

            pub(super) use crate::types::*;
        }
    };
}
pub(crate) use expand_imports;

macro_rules! create_amount_converter_wrapper {
    (connector_name: $connector_name:ident, amount_type: $amount_type:ty) => {
        paste::paste! {
            #[derive(Default, Debug, Clone, Copy, PartialEq)]
            pub struct [<$connector_name AmountConvertor>];

            impl [<$connector_name AmountConvertor>] {
                pub fn convert(
                    amount: common_utils::types::MinorUnit,
                    currency: common_enums::Currency,
                ) -> Result<common_utils::types::$amount_type, error_stack::Report<errors::ConnectorError>> {
                    domain_types::utils::convert_amount(
                        &common_utils::types::[<$amount_type ForConnector>],
                        amount,
                        currency,
                    )
                }

                pub fn convert_back(
                    amount: common_utils::types::$amount_type,
                    currency: common_enums::Currency,
                ) -> Result<common_utils::types::MinorUnit, error_stack::Report<errors::ConnectorError>> {
                    domain_types::utils::convert_back_amount_to_minor_units(
                        &common_utils::types::[<$amount_type ForConnector>],
                        amount,
                        currency,
                    )
                }
            }
        }
    };
}
pub(crate) use create_amount_converter_wrapper;
