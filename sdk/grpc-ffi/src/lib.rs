//! hyperswitch-grpc-ffi — thin C ABI wrapper around the connector-service gRPC API.
//!
//! Exports two symbols:
//!   - `hyperswitch_grpc_call`  — make one gRPC call (blocking; embeds a tokio runtime)
//!   - `hyperswitch_grpc_free`  — free the buffer returned by `hyperswitch_grpc_call`
//!
//! The returned buffer always starts with a 1-byte tag:
//!   - `0x00` → success; remaining bytes are a protobuf-encoded response
//!   - `0x01` → error;   remaining bytes are a UTF-8 error message
//!
//! # Channel caching
//! A tonic `Channel` (HTTP-2 connection) is cached per `endpoint` URL and reused
//! across calls.  This avoids the connect/disconnect cycling that causes transport
//! errors when the caller creates a new logical client for each RPC.

use std::{
    collections::HashMap,
    ffi::c_char,
    sync::{Arc, Mutex},
};

use grpc_api_types::payments::{
    customer_service_client::CustomerServiceClient, event_service_client::EventServiceClient,
    merchant_authentication_service_client::MerchantAuthenticationServiceClient,
    payment_method_authentication_service_client::PaymentMethodAuthenticationServiceClient,
    payment_method_service_client::PaymentMethodServiceClient,
    payment_service_client::PaymentServiceClient,
    recurring_payment_service_client::RecurringPaymentServiceClient, CustomerServiceCreateRequest,
    EventServiceHandleRequest, MerchantAuthenticationServiceCreateAccessTokenRequest,
    MerchantAuthenticationServiceCreateSdkSessionTokenRequest,
    MerchantAuthenticationServiceCreateSessionTokenRequest,
    PaymentMethodAuthenticationServiceAuthenticateRequest,
    PaymentMethodAuthenticationServicePostAuthenticateRequest,
    PaymentMethodAuthenticationServicePreAuthenticateRequest, PaymentMethodServiceTokenizeRequest,
    PaymentServiceAuthorizeRequest, PaymentServiceCaptureRequest, PaymentServiceGetRequest,
    PaymentServiceRefundRequest, PaymentServiceReverseRequest, PaymentServiceSetupRecurringRequest,
    PaymentServiceVoidRequest, RecurringPaymentServiceChargeRequest,
};
use prost::Message;
use serde::Deserialize;
use tonic::{
    metadata::{MetadataKey, MetadataValue},
    transport::Channel,
    Request,
};

// ── Embedded tokio runtime (one per process) ──────────────────────────────────

// current_thread avoids spawning background tokio threads that can conflict
// with Node.js / libuv on macOS (competing kqueue registrations).
// All async work runs on the calling thread inside each `block_on`.
static RT: std::sync::LazyLock<tokio::runtime::Runtime> = std::sync::LazyLock::new(|| {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("hyperswitch-grpc-ffi: failed to create tokio runtime")
});

// ── Channel cache (one HTTP-2 connection per endpoint) ────────────────────────

static CHANNEL_CACHE: std::sync::LazyLock<Mutex<HashMap<String, Channel>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

fn get_channel(endpoint: &str) -> Result<Channel, String> {
    // Fast path: already connected.
    {
        let cache = CHANNEL_CACHE.lock().map_err(|e| e.to_string())?;
        if let Some(ch) = cache.get(endpoint) {
            return Ok(ch.clone());
        }
    }
    // Slow path: create a lazy channel (TCP connect deferred to first RPC,
    // which happens inside block_on where the tokio I/O driver is active).
    let ch = Channel::from_shared(endpoint.to_owned())
        .map_err(|e| format!("invalid endpoint URI: {e}"))?
        .connect_lazy();
    {
        let mut cache = CHANNEL_CACHE.lock().map_err(|e| e.to_string())?;
        cache.insert(endpoint.to_owned(), ch.clone());
    }
    Ok(ch)
}

// ── Config ────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct GrpcConfigInput {
    endpoint: String,
    connector: String,
    /// Connector-specific configuration for x-connector-config header.
    /// Format: {"config": {"ConnectorName": {"api_key": "...", ...}}}
    connector_config: serde_json::Value,
}

fn build_headers(cfg: &GrpcConfigInput) -> Arc<HashMap<String, String>> {
    let mut h = HashMap::new();
    h.insert("x-connector".into(), cfg.connector.clone());
    // Serialize connector_config to JSON string for x-connector-config header
    let config_json =
        serde_json::to_string(&cfg.connector_config).unwrap_or_else(|_| "{}".to_string());
    h.insert("x-connector-config".into(), config_json);
    Arc::new(h)
}

fn inject<T>(payload: T, headers: &HashMap<String, String>) -> Request<T> {
    let mut req = Request::new(payload);
    for (k, v) in headers {
        if let (Ok(key), Ok(val)) = (
            MetadataKey::from_bytes(k.as_bytes()),
            MetadataValue::try_from(v.as_str()),
        ) {
            req.metadata_mut().insert(key, val);
        }
    }
    req
}

// ── Response encoding helpers ─────────────────────────────────────────────────

fn encode_ok(bytes: Vec<u8>) -> Vec<u8> {
    let mut out = vec![0u8];
    out.extend_from_slice(&bytes);
    out
}

fn encode_err(msg: &str) -> Vec<u8> {
    let mut out = vec![1u8];
    out.extend_from_slice(msg.as_bytes());
    out
}

// ── gRPC dispatch (reuses cached channel) ────────────────────────────────────

async fn dispatch(method: &str, cfg: GrpcConfigInput, req_bytes: &[u8]) -> Result<Vec<u8>, String> {
    let headers = build_headers(&cfg);
    let channel = get_channel(&cfg.endpoint)?;

    macro_rules! call {
        ($stub:ident, $rpc:ident, $req_ty:ty) => {{
            let req = <$req_ty>::decode(req_bytes).map_err(|e| e.to_string())?;
            let mut client = $stub::new(channel.clone());
            let res = client
                .$rpc(inject(req, &headers))
                .await
                .map_err(|s| s.to_string())?;
            Ok(res.into_inner().encode_to_vec())
        }};
    }

    match method {
        "payment/authorize" => call!(
            PaymentServiceClient,
            authorize,
            PaymentServiceAuthorizeRequest
        ),
        "payment/capture" => call!(PaymentServiceClient, capture, PaymentServiceCaptureRequest),
        "payment/void" => call!(PaymentServiceClient, void, PaymentServiceVoidRequest),
        "payment/get" => call!(PaymentServiceClient, get, PaymentServiceGetRequest),
        "payment/refund" => call!(PaymentServiceClient, refund, PaymentServiceRefundRequest),
        "payment/reverse" => call!(PaymentServiceClient, reverse, PaymentServiceReverseRequest),
        "payment/setup_recurring" => call!(
            PaymentServiceClient,
            setup_recurring,
            PaymentServiceSetupRecurringRequest
        ),
        "customer/create" => call!(CustomerServiceClient, create, CustomerServiceCreateRequest),
        "payment_method/tokenize" => call!(
            PaymentMethodServiceClient,
            tokenize,
            PaymentMethodServiceTokenizeRequest
        ),
        "payment_method_authentication/pre_authenticate" => call!(
            PaymentMethodAuthenticationServiceClient,
            pre_authenticate,
            PaymentMethodAuthenticationServicePreAuthenticateRequest
        ),
        "payment_method_authentication/authenticate" => call!(
            PaymentMethodAuthenticationServiceClient,
            authenticate,
            PaymentMethodAuthenticationServiceAuthenticateRequest
        ),
        "payment_method_authentication/post_authenticate" => call!(
            PaymentMethodAuthenticationServiceClient,
            post_authenticate,
            PaymentMethodAuthenticationServicePostAuthenticateRequest
        ),
        "event/handle_event" => call!(EventServiceClient, handle_event, EventServiceHandleRequest),
        "merchant_authentication/create_access_token" => call!(
            MerchantAuthenticationServiceClient,
            create_access_token,
            MerchantAuthenticationServiceCreateAccessTokenRequest
        ),
        "merchant_authentication/create_session_token" => call!(
            MerchantAuthenticationServiceClient,
            create_session_token,
            MerchantAuthenticationServiceCreateSessionTokenRequest
        ),
        "merchant_authentication/create_sdk_session_token" => call!(
            MerchantAuthenticationServiceClient,
            create_sdk_session_token,
            MerchantAuthenticationServiceCreateSdkSessionTokenRequest
        ),
        "recurring_payment/charge" => call!(
            RecurringPaymentServiceClient,
            charge,
            RecurringPaymentServiceChargeRequest
        ),
        other => Err(format!("unknown gRPC method: \"{other}\"")),
    }
}

// ── C ABI helpers ─────────────────────────────────────────────────────────────

unsafe fn to_raw_buf(bytes: Vec<u8>, out_len: *mut u32) -> *mut u8 {
    unsafe {
        *out_len = bytes.len() as u32;
    }
    let boxed: Box<[u8]> = bytes.into_boxed_slice();
    Box::into_raw(boxed) as *mut u8
}

// ── Public C ABI ─────────────────────────────────────────────────────────────

/// Call a gRPC method on the connector-service.
///
/// | Param        | Description                                                  |
/// |--------------|--------------------------------------------------------------|
/// | `method_ptr` | Null-terminated UTF-8 method key, e.g. `"payment/authorize"` |
/// | `config_ptr` | JSON-encoded `GrpcConfigInput`                               |
/// | `config_len` | Byte length of `config_ptr`                                  |
/// | `req_ptr`    | Protobuf-encoded gRPC request message                        |
/// | `req_len`    | Byte length of `req_ptr`                                     |
/// | `out_len`    | Written with the byte length of the returned buffer          |
///
/// Returns a heap-allocated buffer; first byte is the tag:
/// - `0x00` success — remaining bytes are the protobuf-encoded response
/// - `0x01` error   — remaining bytes are a UTF-8 error string
///
/// Free with [`hyperswitch_grpc_free`].
///
/// # Safety
/// All pointer parameters must be non-null and valid for the given lengths.
#[no_mangle]
pub unsafe extern "C" fn hyperswitch_grpc_call(
    method_ptr: *const c_char,
    config_ptr: *const u8,
    config_len: u32,
    req_ptr: *const u8,
    req_len: u32,
    out_len: *mut u32,
) -> *mut u8 {
    let method = match unsafe { std::ffi::CStr::from_ptr(method_ptr) }.to_str() {
        Ok(s) => s,
        Err(e) => {
            return unsafe {
                to_raw_buf(encode_err(&format!("invalid method string: {e}")), out_len)
            }
        }
    };

    let config_bytes = unsafe { std::slice::from_raw_parts(config_ptr, config_len as usize) };
    let req_bytes = unsafe { std::slice::from_raw_parts(req_ptr, req_len as usize) };

    let cfg: GrpcConfigInput = match serde_json::from_slice(config_bytes) {
        Ok(c) => c,
        Err(e) => {
            return unsafe { to_raw_buf(encode_err(&format!("invalid config JSON: {e}")), out_len) }
        }
    };

    let res = RT.block_on(dispatch(method, cfg, req_bytes));

    let bytes = match res {
        Ok(b) => encode_ok(b),
        Err(e) => encode_err(&e),
    };

    unsafe { to_raw_buf(bytes, out_len) }
}

/// Free a buffer returned by [`hyperswitch_grpc_call`].
///
/// # Safety
/// `ptr` must have been returned by `hyperswitch_grpc_call` and `len` must
/// match the `out_len` value written by that call.
#[no_mangle]
pub unsafe extern "C" fn hyperswitch_grpc_free(ptr: *mut u8, len: u32) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(
                ptr,
                len as usize,
            )));
        }
    }
}
