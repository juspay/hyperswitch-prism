use bytes::Bytes;
use domain_types::errors;
use serde_json::{Map, Value};

/// Processes XML response bytes by converting to properly structured JSON.
///
/// This function:
/// 1. Takes XML data as `Bytes` input
/// 2. Converts it to a UTF-8 string and trims whitespace
/// 3. Checks for XML declarations and removes them if present
/// 4. Parses the XML into a JSON structure
/// 5. Flattens nested "$text" fields to create a clean key-value structure
/// 6. Returns the processed JSON data as `Bytes`
pub fn preprocess_xml_response_bytes(xml_data: Bytes) -> Result<Bytes, errors::ConnectorError> {
    // Log raw bytes for debugging
    tracing::info!(bytes=?xml_data, "Raw XML bytes received for preprocessing");

    // Convert to UTF-8 string
    let response_str = std::str::from_utf8(&xml_data)
        .map_err(|_| errors::ConnectorError::ResponseDeserializationFailed)?
        .trim();

    // Handle XML declarations by removing them if present
    let cleaned_response = if response_str.starts_with("<?xml") {
        // Find the end of the XML declaration and skip it
        match response_str.find("?>") {
            Some(pos) => {
                let substring = &response_str[pos + 2..];
                let cleaned = substring.trim();
                tracing::info!("Removed XML declaration: {}", cleaned);
                cleaned
            }
            None => {
                tracing::warn!("XML declaration start found but no closing '?>' tag");
                response_str
            }
        }
    } else {
        tracing::info!("No XML declaration found, using as-is");
        response_str
    };

    // Ensure the XML has a txn wrapper if needed
    let final_xml = if !cleaned_response.starts_with("<txn>")
        && (cleaned_response.contains("<ssl_") || cleaned_response.contains("<error"))
    {
        format!("<txn>{cleaned_response}</txn>")
    } else {
        cleaned_response.to_string()
    };

    // Parse XML to a generic JSON Value
    let json_value: Value = match quick_xml::de::from_str(&final_xml) {
        Ok(val) => {
            tracing::info!("Successfully converted XML to JSON structure");
            val
        }
        Err(err) => {
            tracing::error!(error=?err, "Failed to parse XML to JSON structure");

            // Create a basic JSON structure with error information
            return Err(errors::ConnectorError::ResponseDeserializationFailed);
        }
    };

    // Extract and flatten the JSON structure
    let flattened_json = flatten_json_structure(json_value);

    // Convert JSON Value to string and then to bytes
    let json_string = serde_json::to_string(&flattened_json).map_err(|e| {
        tracing::error!(error=?e, "Failed to convert to JSON string");
        errors::ConnectorError::ResponseDeserializationFailed
    })?;

    tracing::info!(json=?json_string, "Flattened JSON structure");

    // Return JSON as bytes
    Ok(Bytes::from(json_string.into_bytes()))
}

/// Flattens a nested JSON structure, extracting values from "$text" fields
pub fn flatten_json_structure(json_value: Value) -> Value {
    let mut flattened = Map::new();

    // Extract txn object if present
    let txn_obj = if let Some(obj) = json_value.as_object() {
        if let Some(txn) = obj.get("txn") {
            txn.as_object()
        } else {
            Some(obj)
        }
    } else {
        None
    };

    // Process the fields
    if let Some(obj) = txn_obj {
        for (key, value) in obj {
            // Handle nested "$text" fields
            if let Some(value_obj) = value.as_object() {
                if let Some(text_value) = value_obj.get("$text") {
                    // Extract the value from "$text" field
                    flattened.insert(key.clone(), text_value.clone());
                } else if value_obj.is_empty() {
                    // Empty object, insert empty string
                    flattened.insert(key.clone(), Value::String("".to_string()));
                } else {
                    // Use the value as is
                    flattened.insert(key.clone(), value.clone());
                }
            } else {
                // Use the value as is
                flattened.insert(key.clone(), value.clone());
            }
        }
    }

    Value::Object(flattened)
}
