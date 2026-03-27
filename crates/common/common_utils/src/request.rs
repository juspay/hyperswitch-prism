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
    pub ca_certificate: Option<Secret<String>>,
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

        Ok((finished_bytes, boundary))
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
        self.certificate = certificate_key;
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
            ca_certificate: self.ca_certificate,
        }
    }
}

impl Default for RequestBuilder {
    fn default() -> Self {
        Self::new()
    }
}
