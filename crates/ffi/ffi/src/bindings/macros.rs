//! Macros for defining FFI flow transformers.
//!
//! Provides the `define_ffi_flow!` macro that generates `#[uniffi::export]`
//! function pairs for request and response transformation.

/// Generates a `#[uniffi::export]` `{flow}_req_transformer` and
/// `{flow}_res_transformer` function pair backed by the generic runners.
///
/// # Arguments
/// - `$flow`        — snake_case flow name (used as identifier prefix)
/// - `$req_type`    — protobuf request type to decode from bytes
/// - `$req_handler` — handler fn: `(FfiRequestData<Req>, Option<Environment>) -> Result<Option<Request>, IntegrationError>`
/// - `$res_handler` — handler fn: `(FfiRequestData<Req>, Response, Option<Environment>) -> Result<Res, ConnectorError>`
#[macro_export]
macro_rules! define_ffi_flow {
    ($flow:ident, $req_type:ty, $req_handler:path, $res_handler:path) => {
        paste::paste! {
            #[uniffi::export]
            pub fn [<$flow _req_transformer>](
                request_bytes: Vec<u8>,
                options_bytes: Vec<u8>,
            ) -> Vec<u8> {
                $crate::bindings::uniffi::run_req_transformer::<$req_type>(
                    request_bytes,
                    options_bytes,
                    $req_handler,
                )
            }

            #[uniffi::export]
            pub fn [<$flow _res_transformer>](
                response_bytes: Vec<u8>,
                request_bytes: Vec<u8>,
                options_bytes: Vec<u8>,
            ) -> Vec<u8> {
                $crate::bindings::uniffi::run_res_transformer::<$req_type, _>(
                    response_bytes,
                    request_bytes,
                    options_bytes,
                    $res_handler,
                )
            }
        }
    };
}
