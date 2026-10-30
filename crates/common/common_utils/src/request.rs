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

    /// Render the parts as a `multipart/form-data` body.
    ///
    /// Returns `(body, boundary)`; the caller sets
    /// `Content-Type: multipart/form-data; boundary=<boundary>` itself.
    ///
    /// The encoding is the plain RFC 7578 layout. It used to be delegated to the
    /// unmaintained `multipart` crate, which transitively pulled `iron`,
    /// `typemap` and `buf_redux` into every build.
    pub fn render_as_bytes(&self) -> Result<(Vec<u8>, String), RequestError> {
        let boundary = format!("------------------------{}", uuid::Uuid::new_v4().simple());
        let mut body = Vec::new();

        for part in &self.parts {
            body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
            match part {
                FormDataPart::Text { name, value } => {
                    body.extend_from_slice(
                        format!(
                            "Content-Disposition: form-data; name=\"{}\"\r\n\r\n",
                            sanitize_quoted_value(name)
                        )
                        .as_bytes(),
                    );
                    body.extend_from_slice(value.as_bytes());
                    body.extend_from_slice(b"\r\n");
                }
                FormDataPart::File {
                    name,
                    filename,
                    bytes,
                    mime_type,
                } => {
                    body.extend_from_slice(
                        format!(
                            "Content-Disposition: form-data; name=\"{}\"; filename=\"{}\"\r\n",
                            sanitize_quoted_value(name),
                            sanitize_quoted_value(filename)
                        )
                        .as_bytes(),
                    );
                    let mime_type = if is_mime_type(mime_type) {
                        mime_type.as_str()
                    } else {
                        "application/octet-stream"
                    };
                    body.extend_from_slice(format!("Content-Type: {mime_type}\r\n\r\n").as_bytes());
                    body.extend_from_slice(bytes);
                    body.extend_from_slice(b"\r\n");
                }
            }
        }
        body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());

        Ok((body, boundary))
    }
}

/// Strip characters that would corrupt a quoted `Content-Disposition` parameter.
fn sanitize_quoted_value(value: &str) -> String {
    value
        .chars()
        .map(|c| match c {
            '\r' | '\n' => ' ',
            '"' => '\'',
            other => other,
        })
        .collect()
}

/// Whether `mime_type` is a usable `type/subtype` (optionally followed by
/// parameters). Anything else falls back to `application/octet-stream`.
fn is_mime_type(mime_type: &str) -> bool {
    let essence = mime_type.split(';').next().unwrap_or_default();
    let Some((media_type, subtype)) = essence.split_once('/') else {
        return false;
    };
    !media_type.is_empty()
        && !subtype.is_empty()
        && essence.chars().filter(|&c| c == '/').count() == 1
        && essence
            .chars()
            .all(|c| !c.is_whitespace() && !c.is_control())
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
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn render_as_bytes_emits_rfc7578_body() {
        let mut data = MultipartData::new();
        data.add_text("mandate_id", "mandat_123");
        data.add_file(
            "invoice",
            "invoice.pdf",
            b"%PDF-1.7".to_vec(),
            "application/pdf",
        );

        let (body, boundary) = data.render_as_bytes().unwrap();
        let body = String::from_utf8(body).unwrap();

        assert!(body.starts_with(&format!("--{boundary}\r\n")));
        assert!(body.ends_with(&format!("--{boundary}--\r\n")));
        assert!(body
            .contains("Content-Disposition: form-data; name=\"mandate_id\"\r\n\r\nmandat_123\r\n"));
        assert!(body.contains(
            "Content-Disposition: form-data; name=\"invoice\"; filename=\"invoice.pdf\"\r\n\
             Content-Type: application/pdf\r\n\r\n%PDF-1.7\r\n"
        ));
        // Boundary must not collide with the payload.
        assert_eq!(body.matches(&boundary).count(), 3);
    }

    #[test]
    fn render_as_bytes_falls_back_to_octet_stream() {
        let mut data = MultipartData::new();
        data.add_file("blob", "blob.bin", vec![0x00, 0xff], "not a mime type");

        let (body, boundary) = data.render_as_bytes().unwrap();
        let body = String::from_utf8_lossy(&body);

        assert!(body.contains("Content-Type: application/octet-stream\r\n"));
        assert!(body.ends_with(&format!("--{boundary}--\r\n")));
    }
}
