use std::collections::HashMap;

use base64::{engine::general_purpose::STANDARD as BASE64_ENGINE, Engine};
use serde::{Deserialize, Serialize};

pub type JsonValue = serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "tag", content = "contents")]
pub enum RecordingEntry {
    #[serde(rename = "MetadataEntryT")]
    Metadata(MetadataEntry),
    #[serde(rename = "TimeStampEntryT", alias = "TimestampEntryT")]
    Timestamp(TimestampEntry),
    #[serde(rename = "UuidEntryT", alias = "UUIDEntryT")]
    Uuid(UuidEntry),
    #[serde(rename = "RandomRIOEntryT")]
    RandomRio(RandomRioEntry),
    #[serde(rename = "RandomBytesEntryT")]
    RandomBytes(RandomBytesEntry),
    #[serde(rename = "CallAPIEntryT")]
    CallApi(CallApiEntry),
    #[serde(rename = "CallAPIEntryPIIT", alias = "CallAPIEntryPII")]
    CallApiPii(CallApiEntry),
    #[serde(rename = "IncomingApiEntryT")]
    IncomingApi(IncomingApiEntry),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MetadataEntry {
    pub tag: String,
    pub metadata: JsonValue,
}

impl MetadataEntry {
    pub fn new(tag: impl Into<String>, metadata: JsonValue) -> Self {
        Self {
            tag: tag.into(),
            metadata,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TimestampEntry {
    #[serde(rename = "functionName")]
    pub function_name: String,
    pub timestamp: JsonValue,
    pub tag: String,
}

impl TimestampEntry {
    pub fn new(timestamp: JsonValue, tag: impl Into<String>) -> Self {
        Self {
            function_name: "getCurrentTime".to_string(),
            timestamp,
            tag: tag.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UuidEntry {
    #[serde(rename = "functionName")]
    pub function_name: String,
    pub uuid: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
}

impl UuidEntry {
    pub fn new(
        function_name: impl Into<String>,
        uuid: impl Into<String>,
        _tag: impl Into<String>,
    ) -> Self {
        Self {
            function_name: function_name.into(),
            uuid: uuid.into(),
            tag: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RandomRioEntry {
    #[serde(rename = "functionName")]
    pub function_name: String,
    pub range: JsonValue,
    pub value: JsonValue,
    pub tag: String,
}

impl RandomRioEntry {
    pub fn new(range: JsonValue, value: JsonValue, tag: impl Into<String>) -> Self {
        Self {
            function_name: "randomRIO".to_string(),
            range,
            value,
            tag: tag.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RandomBytesEntry {
    #[serde(rename = "functionName")]
    pub function_name: String,
    pub input: usize,
    pub value: JsonValue,
    pub tag: String,
}

impl RandomBytesEntry {
    pub fn from_bytes(bytes: Vec<u8>, tag: impl Into<String>) -> Self {
        let input = bytes.len();
        Self {
            function_name: "getRandomBytes".to_string(),
            input,
            value: JsonValue::String(BASE64_ENGINE.encode(bytes)),
            tag: tag.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CallApiEntry {
    #[serde(rename = "jsonRequest")]
    pub json_request: HttpRequestEntry,
    #[serde(rename = "jsonResult")]
    pub json_result: Either<ErrorPayload, JsonValue>,
    #[serde(rename = "apiTag")]
    pub api_tag: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Either<L, R> {
    #[serde(rename = "Left")]
    Left(L),
    #[serde(rename = "Right")]
    Right(R),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ErrorPayload {
    #[serde(rename = "isError")]
    pub is_error: bool,
    #[serde(rename = "errorMessage")]
    pub error_message: String,
    #[serde(rename = "userMessage")]
    pub user_message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HttpRequestEntry {
    #[serde(rename = "getRequestMethod")]
    pub get_request_method: String,
    #[serde(rename = "getRequestHeaders")]
    pub get_request_headers: HashMap<String, String>,
    #[serde(rename = "getRequestBody")]
    pub get_request_body: Option<String>,
    #[serde(rename = "getRequestURL")]
    pub get_request_url: String,
    #[serde(rename = "getRequestTimeout")]
    pub get_request_timeout: Option<i64>,
    #[serde(rename = "getRequestRedirects")]
    pub get_request_redirects: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HttpResponseEntry {
    #[serde(rename = "getResponseBody")]
    pub get_response_body: String,
    #[serde(rename = "getResponseCode")]
    pub get_response_code: i32,
    #[serde(rename = "getResponseHeaders")]
    pub get_response_headers: HashMap<String, String>,
    #[serde(rename = "getResponseStatus")]
    pub get_response_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IncomingApiEntry {
    #[serde(rename = "apiRequest")]
    pub api_request: IncomingApiRequestEntry,
    #[serde(rename = "apiResponse")]
    pub api_response: IncomingApiResponseEntry,
    #[serde(rename = "apiTag")]
    pub api_tag: String,
    pub hostname: String,
    #[serde(rename = "startTime")]
    pub start_time: JsonValue,
    #[serde(rename = "endTime")]
    pub end_time: JsonValue,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IncomingApiRequestEntry {
    #[serde(rename = "apiReqBody")]
    pub api_req_body: JsonValue,
    #[serde(rename = "apiReqUrl")]
    pub api_req_url: String,
    #[serde(rename = "apiReqMethod")]
    pub api_req_method: String,
    #[serde(rename = "apiReqHeaders")]
    pub api_req_headers: HashMap<String, String>,
    #[serde(rename = "apiReqQueryParams")]
    pub api_req_query_params: HashMap<String, String>,
    #[serde(rename = "apiReqRouteParams")]
    pub api_req_route_params: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IncomingApiResponseEntry {
    #[serde(rename = "apiResBody")]
    pub api_res_body: JsonValue,
    #[serde(rename = "apiResHeaders")]
    pub api_res_headers: HashMap<String, String>,
    #[serde(rename = "apiResCode")]
    pub api_res_code: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CsvRecording {
    #[serde(rename = "sessId")]
    pub sess_id: String,
    #[serde(rename = "merchId")]
    pub merch_id: String,
    #[serde(rename = "ordId")]
    pub ord_id: String,
    pub counter: i32,
    #[serde(rename = "valType")]
    pub val_type: String,
    #[serde(rename = "recEntry")]
    pub rec_entry: String,
}

impl CsvRecording {
    pub fn to_euler_csv_row(&self) -> String {
        format!(
            "{},{},{},{},{},{}",
            self.sess_id, self.merch_id, self.ord_id, self.counter, self.val_type, self.rec_entry
        )
    }
}
