// Kount unit tests. Production modules (`kount.rs`, `transformers.rs`) stay
// free of `#[cfg(test)]` blocks — connector-specific coverage lives here only.

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
#[allow(clippy::indexing_slicing)]
#[allow(clippy::panic)]
#[allow(clippy::unreachable)]
mod tests {
    use hyperswitch_masking::Secret;

    use super::super::transformers::kount_custom_fields;

    fn metadata(json: &str) -> Secret<String> {
        Secret::new(json.to_owned())
    }

    /// `metadata.customFields` parsing: absent is silent `None`, a valid
    /// object of native scalars is kept verbatim, and any malformed shape
    /// (non-object block, nested values, oversized key/value) drops the whole
    /// block strict + loud — never a silently mangled subset.
    #[test]
    fn metadata_custom_fields_parsing() {
        // Missing key, missing metadata, and explicit JSON null are not errors.
        assert_eq!(
            kount_custom_fields(Some(&metadata(r#"{"merchantOrderId": "o1"}"#))),
            None
        );
        assert_eq!(kount_custom_fields(None), None);
        assert_eq!(
            kount_custom_fields(Some(&metadata(r#"{"customFields": null}"#))),
            None
        );

        // Valid: object of native scalars, passed through verbatim.
        let parsed = kount_custom_fields(Some(&metadata(
            r#"{"customFields": {"mdf1": "gold", "AdultCount": 2, "isGift": true}, "userIp": "1.2.3.4"}"#,
        )))
        .expect("valid customFields must parse");
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed.get("mdf1").unwrap(), &serde_json::json!("gold"));
        assert_eq!(parsed.get("AdultCount").unwrap(), &serde_json::json!(2));
        assert_eq!(parsed.get("isGift").unwrap(), &serde_json::json!(true));

        // Malformed: the whole block is dropped, never a subset.
        assert_eq!(
            kount_custom_fields(Some(&metadata(r#"{"customFields": "oops"}"#))),
            None
        );
        assert_eq!(
            kount_custom_fields(Some(&metadata(r#"{"customFields": ["a"]}"#))),
            None
        );
        assert_eq!(
            kount_custom_fields(Some(&metadata(
                r#"{"customFields": {"bad": {"nested": 1}, "ok": 1}}"#
            ))),
            None
        );
        assert_eq!(
            kount_custom_fields(Some(&metadata(
                r#"{"customFields": {"bad": [1, 2], "ok": 1}}"#
            ))),
            None
        );
        // One oversized key poisons the block (33 chars) — no silent truncation.
        assert_eq!(
            kount_custom_fields(Some(&metadata(
                r#"{"customFields": {"this-key-is-far-too-long-for-kount": 1, "ok": 1}}"#
            ))),
            None
        );
        // Oversized string value poisons the block too.
        let long_value = "x".repeat(257);
        assert_eq!(
            kount_custom_fields(Some(&metadata(&format!(
                r#"{{"customFields": {{"ok": "{long_value}"}}}}"#
            )))),
            None
        );
    }
}
