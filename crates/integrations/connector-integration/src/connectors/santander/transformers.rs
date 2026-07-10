use serde::{Deserialize, Deserializer, Serialize};

// ===== ERROR RESPONSE =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SantanderErrorResponse {
    #[serde(
        default,
        alias = "code",
        alias = "errorCode",
        alias = "_errorCode",
        deserialize_with = "deserialize_optional_string"
    )]
    pub code: Option<String>,
    #[serde(alias = "message", alias = "errorMessage", alias = "_message")]
    pub message: Option<String>,
    #[serde(alias = "httpStatus")]
    pub http_status: Option<String>,
    #[serde(alias = "details", alias = "_details")]
    pub details: Option<String>,
    #[serde(
        default,
        alias = "timestamp",
        alias = "_timestamp",
        deserialize_with = "deserialize_optional_string"
    )]
    pub timestamp: Option<String>,
    #[serde(alias = "traceId", alias = "_traceId", alias = "trackingId")]
    pub trace_id: Option<String>,
    #[serde(default, alias = "_errors")]
    pub errors: Vec<SantanderErrorDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SantanderErrorDetail {
    #[serde(
        default,
        alias = "code",
        alias = "_code",
        alias = "errorCode",
        deserialize_with = "deserialize_optional_string"
    )]
    pub code: Option<String>,
    #[serde(alias = "field", alias = "_field")]
    pub field: Option<String>,
    #[serde(alias = "message", alias = "_message", alias = "errorMessage")]
    pub message: Option<String>,
}

fn deserialize_optional_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(value.and_then(|value| match value {
        serde_json::Value::Null => None,
        serde_json::Value::String(value) if value.is_empty() => None,
        serde_json::Value::String(value) => Some(value),
        serde_json::Value::Number(value) => Some(value.to_string()),
        serde_json::Value::Bool(value) => Some(value.to_string()),
        other => Some(other.to_string()),
    }))
}

impl SantanderErrorResponse {
    pub fn error_code(&self, status_code: u16) -> String {
        self.code
            .clone()
            .or_else(|| self.errors.iter().find_map(|error| error.code.clone()))
            .unwrap_or_else(|| status_code.to_string())
    }

    pub fn error_message(&self, status_code: u16) -> String {
        self.message
            .clone()
            .or_else(|| self.http_status.clone())
            .or_else(|| self.errors.iter().find_map(|error| error.message.clone()))
            .unwrap_or_else(|| format!("Santander error response with status code {status_code}"))
    }

    pub fn error_reason(&self) -> Option<String> {
        let mut reasons = Vec::new();

        if let Some(details) = self.details.clone() {
            reasons.push(details);
        }

        reasons.extend(self.errors.iter().filter_map(|error| {
            match (&error.field, &error.code, &error.message) {
                (Some(field), Some(code), Some(message)) => {
                    Some(format!("{field}: {message} ({code})"))
                }
                (Some(field), None, Some(message)) => Some(format!("{field}: {message}")),
                (None, Some(code), Some(message)) => Some(format!("{code}: {message}")),
                (None, None, Some(message)) => Some(message.clone()),
                (_, Some(code), None) => Some(code.clone()),
                _ => None,
            }
        }));

        if let Some(trace_id) = self.trace_id.clone() {
            reasons.push(format!("trace_id: {trace_id}"));
        }

        if let Some(timestamp) = self.timestamp.clone() {
            reasons.push(format!("timestamp: {timestamp}"));
        }

        if reasons.is_empty() {
            None
        } else {
            Some(reasons.join("; "))
        }
    }
}
