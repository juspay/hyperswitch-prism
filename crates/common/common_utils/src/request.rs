use crate::events::MaskedSerdeValue;
use hyperswitch_masking::{Maskable, Secret};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub type Headers = std::collections::HashSet<(String, Maskable<String>)>;

#[derive(Debug, thiserror::Error)]
pub enum RequestError {
    #[error("Multipart rendering failed: {0}")]
    MultipartRenderingFailed(String),
    #[error("Failed to read multipart stream: {0}")]
    MultipartReadFailed(String),
}

#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    Deserialize,
    Serialize,
    strum::Display,
    strum::EnumString,
    ToSchema,
)]
#[serde(rename_all = "UPPERCASE")]
#[strum(serialize_all = "UPPERCASE")]
pub enum Method {
    Get,
    Post,
    Put,
    Delete,
    Patch,
}

#[derive(Deserialize, Serialize, Debug)]
pub enum ContentType {
    Json,
    FormUrlEncoded,
    FormData,
    Xml,
}

fn default_request_headers() -> [(String, Maskable<String>); 1] {
    use http::header;

    [(header::VIA.to_string(), "HyperSwitch".to_string().into())]
}

#[derive(Debug)]
pub struct Request {
    pub url: String,
    pub headers: Headers,
    pub method: Method,
    pub certificate: Option<Secret<String>>,
    pub certificate_key: Option<Secret<String>>,
    pub body: Option<RequestContent>,
    pub typed_connector_request_value: Option<serde_json::Value>,
    pub ca_certificate: Option<Secret<String>>,
}

/// Kafka request payload for pushing payments to a Kafka queue.
/// Contains only per-message data (topic, key, headers, payload).
#[derive(Debug)]
pub struct KafkaRecord {
    pub topic: String,
    pub key: Option<String>,
    pub headers: Headers,
    pub payload: Option<RequestContent>,
}

/// Enum representing different connector request transport formats.
#[derive(Debug)]
pub enum TransportType {
    /// Standard HTTP request to a connector API.
    Http,
    /// Push payment message to a Kafka queue.
    Kafka,
}

impl std::fmt::Debug for RequestContent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Json(_) => "JsonRequestBody",
            Self::FormUrlEncoded(_) => "FormUrlEncodedRequestBody",
            Self::FormData(_) => "FormDataRequestBody",
            Self::Xml(_) => "XmlRequestBody",
            Self::RawBytes(_) => "RawBytesRequestBody",
        })
    }
}
#[derive(Serialize)]
pub enum RequestContent {
    Json(Box<dyn hyperswitch_masking::ErasedMaskSerialize + Send>),
    FormUrlEncoded(Box<dyn hyperswitch_masking::ErasedMaskSerialize + Send>),
    FormData(MultipartData),
    Xml(Box<dyn hyperswitch_masking::ErasedMaskSerialize + Send>),
    RawBytes(Vec<u8>),
}

/// Bundles the wire-format request body with its typed observability payload.
///
/// The macro-generated `get_request_body` returns this so that the typed
/// connector request struct is serialized *before* conversion to FormData or
/// RawBytes (which would otherwise lose the typed information).
#[derive(Debug)]
pub struct ConnectorRequestData {
    /// The actual request content sent over the wire.
    pub content: RequestContent,
    /// Masked serialization of the connector's typed request struct, for observability.
    pub typed_request: Option<MaskedSerdeValue>,
}

impl ConnectorRequestData {
    pub fn new(content: RequestContent, typed_request: Option<MaskedSerdeValue>) -> Self {
        Self {
            content,
            typed_request,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MultipartData {
    pub parts: Vec<FormDataPart>,
}

#[derive(Debug, Clone, Serialize)]
pub enum FormDataPart {
    Text {
        name: String,
        value: String,
    },
    File {
        name: String,
        filename: String,
        bytes: Vec<u8>,
        mime_type: String,
    },
}

impl MultipartData {
    pub fn new() -> Self {
        Self { parts: Vec::new() }
    }

    pub fn add_text(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.parts.push(FormDataPart::Text {
            name: name.into(),
            value: value.into(),
        });
    }

    pub fn add_file(
        &mut self,
        name: impl Into<String>,
        filename: impl Into<String>,
        bytes: Vec<u8>,
        mime_type: impl Into<String>,
    ) {
        self.parts.push(FormDataPart::File {
            name: name.into(),
            filename: filename.into(),
            bytes,
            mime_type: mime_type.into(),
        });
    }

    pub fn render_as_bytes(&self) -> Result<(Vec<u8>, String), RequestError> {
        use std::io::Read;
        let mut builder = multipart::client::lazy::Multipart::new();

        for part in &self.parts {
            match part {
                FormDataPart::Text { name, value } => builder.add_text(name, value),
                FormDataPart::File {
                    name,
                    filename,
                    bytes,
                    mime_type,
                } => {
                    let mime = if !mime_type.is_empty() {
                        mime_type.parse().ok()
                    } else {
                        None
                    };
                    builder.add_stream(name, std::io::Cursor::new(bytes), Some(filename), mime)
                }
            };
        }

        let mut prepared = builder
            .prepare()
            .map_err(|e| RequestError::MultipartRenderingFailed(e.to_string()))?;
        let boundary = prepared.boundary().to_string();

        let mut finished_bytes = Vec::new();
        prepared
            .read_to_end(&mut finished_bytes)
            .map_err(|e| RequestError::MultipartReadFailed(e.to_string()))?;

        strip_multipart_preamble(&mut finished_bytes);

        Ok((finished_bytes, boundary))
    }
}

/// Drop the CRLF preamble the `multipart` crate writes before the first boundary.
///
/// RFC 2046 permits a preamble, but strict gateways reject it: HiPay's Secure Vault and
/// order endpoints answer a body that begins `\r\n--boundary` with HTTP 400 ("Your browser
/// sent a request that this server could not understand") and accept the byte-identical
/// body when it begins at the boundary. Every mainstream client (curl, browsers, reqwest's
/// own multipart writer) starts the body at the first boundary, so the preamble is dropped
/// here rather than worked around per connector.
///
/// A body that already starts at the boundary is left untouched.
fn strip_multipart_preamble(body: &mut Vec<u8>) {
    if body.starts_with(b"\r\n") {
        body.drain(..2);
    }
}

impl Default for MultipartData {
    fn default() -> Self {
        Self::new()
    }
}

impl RequestContent {
    pub fn get_inner_value(&self) -> Secret<String> {
        match self {
            Self::Json(i) => serde_json::to_string(&i).unwrap_or_default().into(),
            Self::FormUrlEncoded(i) => serde_urlencoded::to_string(i).unwrap_or_default().into(),
            Self::Xml(i) => quick_xml::se::to_string(&i).unwrap_or_default().into(),
            Self::FormData(_) => String::new().into(),
            // For RawBytes (e.g., SOAP XML), convert to UTF-8 string for logging
            Self::RawBytes(bytes) => String::from_utf8(bytes.clone()).unwrap_or_default().into(),
        }
    }

    /// Masked-serialize the inner value for Json/FormUrlEncoded/Xml variants.
    /// Returns `(Value, String)` — Value for event logging, String for typed_connector_request.
    /// Returns `None` for FormData/RawBytes.
    pub fn masked_serialize_inner(&self) -> Option<(serde_json::Value, String)> {
        match self {
            Self::Json(i) | Self::FormUrlEncoded(i) | Self::Xml(i) => (**i)
                .masked_serialize()
                .inspect_err(|error| {
                    tracing::warn!(
                        error = %error,
                        "failed to masked-serialize connector request"
                    );
                })
                .ok()
                .and_then(|value| {
                    serde_json::to_string(&value)
                        .inspect_err(|error| {
                            tracing::warn!(
                                error = %error,
                                "failed to stringify masked connector request"
                            );
                        })
                        .ok()
                        .map(|s| (value, s))
                }),
            Self::FormData(_) | Self::RawBytes(_) => None,
        }
    }

    pub fn get_body_bytes(&self) -> Result<(Option<Vec<u8>>, Option<String>), RequestError> {
        use hyperswitch_masking::ExposeInterface;
        match self {
            Self::RawBytes(bytes) => Ok((Some(bytes.clone()), None)),
            Self::Json(_) | Self::FormUrlEncoded(_) | Self::Xml(_) => {
                Ok((Some(self.get_inner_value().expose().into_bytes()), None))
            }
            Self::FormData(data) => {
                let (bytes, boundary) = data.render_as_bytes()?;
                Ok((Some(bytes), Some(boundary)))
            }
        }
    }
}

impl Request {
    pub fn new(method: Method, url: &str) -> Self {
        Self {
            method,
            url: String::from(url),
            headers: std::collections::HashSet::new(),
            certificate: None,
            certificate_key: None,
            body: None,
            typed_connector_request_value: None,
            ca_certificate: None,
        }
    }

    /// Converts the request headers into a simple HashMap with lowercase keys.
    /// This ensures global parity across all language SDKs.
    pub fn get_headers_map(&self) -> std::collections::HashMap<String, String> {
        use hyperswitch_masking::ExposeInterface;
        self.headers
            .iter()
            .map(|(k, v)| {
                let value = match v {
                    Maskable::Normal(val) => val.clone(),
                    Maskable::Masked(val) => val.clone().expose(),
                };
                (k.to_lowercase(), value)
            })
            .collect()
    }

    pub fn set_body<T: Into<RequestContent>>(&mut self, body: T) {
        self.body.replace(body.into());
    }

    pub fn add_default_headers(&mut self) {
        self.headers.extend(default_request_headers());
    }

    pub fn add_header(&mut self, header: &str, value: Maskable<String>) {
        self.headers.insert((String::from(header), value));
    }

    pub fn add_certificate(&mut self, certificate: Option<Secret<String>>) {
        self.certificate = certificate;
    }

    pub fn add_certificate_key(&mut self, certificate_key: Option<Secret<String>>) {
        self.certificate_key = certificate_key;
    }
}

#[derive(Debug)]
pub struct RequestBuilder {
    pub url: String,
    pub headers: Headers,
    pub method: Method,
    pub certificate: Option<Secret<String>>,
    pub certificate_key: Option<Secret<String>>,
    pub body: Option<RequestContent>,
    pub typed_connector_request_value: Option<serde_json::Value>,
    pub ca_certificate: Option<Secret<String>>,
}

impl RequestBuilder {
    pub fn new() -> Self {
        Self {
            method: Method::Get,
            url: String::with_capacity(1024),
            headers: std::collections::HashSet::new(),
            certificate: None,
            certificate_key: None,
            body: None,
            typed_connector_request_value: None,
            ca_certificate: None,
        }
    }

    pub fn url(mut self, url: &str) -> Self {
        self.url = url.into();
        self
    }

    pub fn method(mut self, method: Method) -> Self {
        self.method = method;
        self
    }

    pub fn attach_default_headers(mut self) -> Self {
        self.headers.extend(default_request_headers());
        self
    }

    pub fn header(mut self, header: &str, value: &str) -> Self {
        self.headers.insert((header.into(), value.into()));
        self
    }

    pub fn headers(mut self, headers: Vec<(String, Maskable<String>)>) -> Self {
        self.headers.extend(headers);
        self
    }

    pub fn set_optional_body<T: Into<RequestContent>>(mut self, body: Option<T>) -> Self {
        body.map(|body| self.body.replace(body.into()));
        self
    }

    pub fn set_typed_connector_request(mut self, request: Option<serde_json::Value>) -> Self {
        self.typed_connector_request_value = request;
        self
    }

    pub fn set_body<T: Into<RequestContent>>(mut self, body: T) -> Self {
        self.body.replace(body.into());
        self
    }

    pub fn add_certificate(mut self, certificate: Option<Secret<String>>) -> Self {
        self.certificate = certificate;
        self
    }

    pub fn add_certificate_key(mut self, certificate_key: Option<Secret<String>>) -> Self {
        self.certificate_key = certificate_key;
        self
    }

    pub fn add_ca_certificate_pem(mut self, ca_certificate: Option<Secret<String>>) -> Self {
        self.ca_certificate = ca_certificate;
        self
    }

    pub fn build(self) -> Request {
        Request {
            method: self.method,
            url: self.url,
            headers: self.headers,
            certificate: self.certificate,
            certificate_key: self.certificate_key,
            body: self.body,
            typed_connector_request_value: self.typed_connector_request_value,
            ca_certificate: self.ca_certificate,
        }
    }
}

impl Default for RequestBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct KafkaRecordBuilder {
    pub topic: String,
    pub key: Option<String>,
    pub headers: Headers,
    pub payload: Option<RequestContent>,
}

impl KafkaRecordBuilder {
    pub fn new() -> Self {
        Self {
            topic: String::with_capacity(1024),
            key: None,
            headers: std::collections::HashSet::new(),
            payload: None,
        }
    }

    pub fn topic(mut self, topic: &str) -> Self {
        self.topic = topic.into();
        self
    }

    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }

    pub fn attach_default_headers(mut self) -> Self {
        self.headers.extend(default_request_headers());
        self
    }

    pub fn header(mut self, header: &str, value: &str) -> Self {
        self.headers.insert((header.into(), value.into()));
        self
    }

    pub fn headers(mut self, headers: Vec<(String, Maskable<String>)>) -> Self {
        self.headers.extend(headers);
        self
    }

    pub fn set_payload<T: Into<RequestContent>>(mut self, payload: T) -> Self {
        self.payload.replace(payload.into());
        self
    }

    pub fn set_optional_payload<T: Into<RequestContent>>(mut self, payload: Option<T>) -> Self {
        payload.map(|payload| self.payload.replace(payload.into()));
        self
    }

    pub fn build(self) -> KafkaRecord {
        KafkaRecord {
            topic: self.topic,
            key: self.key,
            headers: self.headers,
            payload: self.payload,
        }
    }
}

impl Default for KafkaRecordBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod multipart_tests {
    #![allow(clippy::expect_used)]

    use super::*;

    /// The rendered body must begin at the first boundary. `multipart`'s writer emits a
    /// CRLF preamble ahead of it, which HiPay answers with HTTP 400.
    #[test]
    fn rendered_body_starts_at_the_first_boundary() {
        let mut form = MultipartData::new();
        form.add_text("card_number", "4111111111111111");
        form.add_text("card_holder", "John Doe");

        let (body, boundary) = form.render_as_bytes().expect("multipart rendering");

        assert!(
            !body.starts_with(b"\r\n"),
            "body must not begin with a CRLF preamble"
        );
        assert!(
            body.starts_with(format!("--{boundary}").as_bytes()),
            "body must begin with the opening boundary delimiter"
        );
        // The parts themselves are untouched by the preamble strip.
        assert!(body.windows(16).any(|window| window == b"4111111111111111"));
        assert!(body.ends_with(format!("--{boundary}--").as_bytes()));
    }

    /// Only a leading CRLF is removed, and only once.
    #[test]
    fn preamble_strip_removes_exactly_one_leading_crlf() {
        let mut body =
            b"\r\n--BOUNDARY\r\nContent-Disposition: form-data\r\n\r\nvalue\r\n--BOUNDARY--"
                .to_vec();

        strip_multipart_preamble(&mut body);

        assert_eq!(
            body,
            b"--BOUNDARY\r\nContent-Disposition: form-data\r\n\r\nvalue\r\n--BOUNDARY--".to_vec()
        );
    }

    /// A body that already starts at the boundary is returned byte-identical.
    #[test]
    fn preamble_strip_leaves_a_well_formed_body_untouched() {
        let original =
            b"--BOUNDARY\r\nContent-Disposition: form-data\r\n\r\nvalue\r\n--BOUNDARY--".to_vec();
        let mut body = original.clone();

        strip_multipart_preamble(&mut body);

        assert_eq!(body, original);
    }

    /// A body whose *content* happens to start with a bare LF, or which is empty, must not
    /// be truncated.
    #[test]
    fn preamble_strip_ignores_a_body_that_does_not_begin_with_crlf() {
        let mut empty: Vec<u8> = Vec::new();
        strip_multipart_preamble(&mut empty);
        assert!(empty.is_empty());

        let mut bare_lf = b"\n--BOUNDARY--".to_vec();
        strip_multipart_preamble(&mut bare_lf);
        assert_eq!(bare_lf, b"\n--BOUNDARY--".to_vec());
    }
}
