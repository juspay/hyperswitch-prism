//! Preserve the destination payload when unwrapping a vault proxy response.
use serde_json::Value;
use std::collections::HashMap;

pub(crate) fn is_xml_media_type(value: &str) -> bool {
    matches!(
        value
            .split(';')
            .next()
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str(),
        "text/xml" | "application/xml"
    )
}

pub(crate) fn destination_headers(response: &Value) -> Option<HashMap<String, String>> {
    response
        .get("response_headers")?
        .as_object()
        .map(|headers| {
            headers
                .iter()
                .filter_map(|(name, value)| {
                    value.as_str().map(|value| (name.clone(), value.to_owned()))
                })
                .collect()
        })
}

pub(crate) fn body_bytes(
    response: &Value,
    xml_request: bool,
) -> Result<Vec<u8>, serde_json::Error> {
    let body = response.get("response").unwrap_or(response);
    let xml_response = destination_headers(response).is_some_and(|headers| {
        headers.iter().any(|(key, value)| {
            key.eq_ignore_ascii_case("content-type") && is_xml_media_type(value)
        })
    });
    if xml_request || xml_response {
        if let Value::String(text) = body {
            return Ok(text.as_bytes().to_vec());
        }
    }
    serde_json::to_vec(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn xml_is_not_json_quoted_and_destination_headers_are_used() {
        let response = serde_json::json!({"response":"<?xml version=\"1.0\"?><reply>A &amp; B</reply>","status_code":400,"response_headers":{"Content-Type":"text/xml; charset=UTF-8"}});
        assert_eq!(
            body_bytes(&response, false).unwrap(),
            b"<?xml version=\"1.0\"?><reply>A &amp; B</reply>"
        );
        assert_eq!(
            destination_headers(&response).unwrap()["Content-Type"],
            "text/xml; charset=UTF-8"
        );
        assert_eq!(
            body_bytes(
                &serde_json::json!({"response":"<html>blocked</html>"}),
                true
            )
            .unwrap(),
            b"<html>blocked</html>"
        );
    }
    #[test]
    fn json_body_contract_is_unchanged() {
        assert_eq!(
            body_bytes(&serde_json::json!({"response":{"ok":true}}), false).unwrap(),
            br#"{"ok":true}"#
        );
        assert_eq!(
            body_bytes(&serde_json::json!({"response":"text"}), false).unwrap(),
            br#""text""#
        );
    }
}
