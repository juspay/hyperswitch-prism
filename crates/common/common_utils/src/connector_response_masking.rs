#![warn(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::as_conversions,
    clippy::unreachable
)]

use std::collections::{HashMap, HashSet};

#[cfg(feature = "log-transformations")]
use quick_xml::events::{BytesStart, BytesText, Event};
#[cfg(feature = "log-transformations")]
use quick_xml::{Reader, Writer};
#[cfg(feature = "log-transformations")]
use serde::ser::{SerializeMap, SerializeSeq};
#[cfg(feature = "log-transformations")]
use serde::Serializer;
use serde::{Deserialize, Serialize};
#[cfg(feature = "log-transformations")]
use serde_json::Value;

use crate::config_patch::Patch;

#[cfg(feature = "log-transformations")]
pub const MASKED: &str = "***";

/// Per-connector unmask lists for connector response bodies.
///
/// Connector names are **not** validated here. The connector enums live in `domain_types`, which
/// depends on this crate, so the arrow cannot be reversed; `ucs_env` checks them once at startup
/// via [`ConnectorResponseMaskingConfig::unknown_connectors`].
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct ConnectorResponseMaskingConfig {
    pub enabled: bool,

    #[serde(deserialize_with = "deserialize_connector_keys")]
    pub connector_keys: HashMap<Box<str>, String>,
}

/// Lowercases connector names so lookups match `ConnectorVariant::get_connector_name()`, which is
/// already snake_case.
fn deserialize_connector_keys<'de, D>(
    deserializer: D,
) -> Result<HashMap<Box<str>, String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(HashMap::<String, String>::deserialize(deserializer)?
        .into_iter()
        .map(|(name, keys)| (name.to_lowercase().into_boxed_str(), keys))
        .collect())
}

impl ConnectorResponseMaskingConfig {
    /// Configured connector names that `is_known` does not recognise.
    ///
    /// The predicate is supplied by the caller because only crates above `domain_types` can see
    /// the connector enums. Returns them all rather than the first, so a startup failure can name
    /// every bad key at once.
    pub fn unknown_connectors(&self, is_known: impl Fn(&str) -> bool) -> Vec<&str> {
        let mut unknown: Vec<&str> = self
            .connector_keys
            .keys()
            .map(AsRef::as_ref)
            .filter(|name| !is_known(name))
            .collect();
        // `HashMap` iteration order is unspecified; sort so the error message is reproducible.
        unknown.sort_unstable();
        unknown
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct ConnectorResponseMaskingConfigPatch {
    pub enabled: Option<bool>,
    #[serde(default, deserialize_with = "deserialize_optional_connector_keys")]
    pub connector_keys: Option<HashMap<Box<str>, String>>,
}

/// Runs on the per-request `x-config-override` header, so it must not reject anything: failing
/// here fails deserialization of the whole `ConfigPatch`, and the middleware then returns without
/// ever calling the handler — a typo in a diagnostic setting would cost a payment. An unresolvable
/// connector name simply never matches at lookup time and masks everything, which is the safe
/// direction. Names in a config *file* are still checked at startup, in `ucs_env`.
fn deserialize_optional_connector_keys<'de, D>(
    deserializer: D,
) -> Result<Option<HashMap<Box<str>, String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Some(
        HashMap::<String, String>::deserialize(deserializer)?
            .into_iter()
            .map(|(name, keys)| (name.to_lowercase().into_boxed_str(), keys))
            .collect(),
    ))
}

impl Patch<ConnectorResponseMaskingConfigPatch> for ConnectorResponseMaskingConfig {
    fn apply(&mut self, patch: ConnectorResponseMaskingConfigPatch) {
        if let Some(enabled) = patch.enabled {
            self.enabled = enabled;
        }
        if let Some(connector_keys) = patch.connector_keys {
            self.connector_keys = connector_keys;
        }
    }
}

#[cfg(feature = "log-transformations")]
const ALWAYS_MASKED_SUBSTRING: &[&str] = &[
    "cardnumber",
    "cardnum",
    "cardno",
    "accountnumber",
    "routingnumber",
    "sortcode",
    "cvv",
    "cvc",
    "cvn",
    "securitycode",
    "expmonth",
    "expyear",
    "expirydate",
    "expirymonth",
    "expiryyear",
    "expirationdate",
    "expirationmonth",
    "expirationyear",
    "track",
    "pinblock",
    "secret",
    "token",
    "password",
    "signature",
    "apikey",
    "privatekey",
    "credential",
    "checksum",
    "hmac",
];

#[cfg(feature = "log-transformations")]
const ALWAYS_MASKED_EXACT: &[&str] = &[
    "authorization",
    "pan",
    "iban",
    "bban",
    "pin",
    "ssn",
    "emv",
    "csc",
    "cid",
    "ksn",
    "jwt",
];

/// The parsed unmask list for a single connector.
#[derive(Debug, Default, Clone)]
pub struct MaskKeys {
    names: HashSet<Box<str>>,
    paths: Vec<Box<[Box<str>]>>,
}

impl MaskKeys {
    /// Parse one connector's comma-separated entry. An entry containing `.` is also registered as
    /// a dotted path, so `additionaldata.authcode` pins that one location as well as the bare name.
    fn parse(configured: &str) -> Self {
        let mut keys = Self::default();

        for entry in configured
            .split(',')
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
        {
            let entry = entry.to_lowercase();
            if entry.contains('.') {
                keys.paths.push(
                    entry
                        .split('.')
                        .map(|segment| segment.to_owned().into_boxed_str())
                        .collect(),
                );
            }
            keys.names.insert(entry.into_boxed_str());
        }

        keys
    }
}

/// Per-connector [`MaskKeys`], parsed once when config is loaded or patched rather than on every
/// connector response.
///
/// Mirrors `CompiledLogFieldsConfig` in [`crate::events`]: the type is always compiled, and only
/// the functions that consume it sit behind `log-transformations`.
#[derive(Debug, Clone, Default)]
pub struct CompiledMaskingKeys {
    /// Runtime kill-switch: when `false`, no masked view is built even though the
    /// `log-transformations` feature is compiled in.
    pub enabled: bool,
    pub keys: HashMap<Box<str>, MaskKeys>,
}

impl CompiledMaskingKeys {
    pub fn compile(config: &ConnectorResponseMaskingConfig) -> Self {
        Self {
            enabled: config.enabled,
            keys: config
                .connector_keys
                .iter()
                .map(|(connector, configured)| (connector.clone(), MaskKeys::parse(configured)))
                .collect(),
        }
    }
}

#[cfg(feature = "log-transformations")]
impl MaskKeys {
    fn names_key(&self, key: &str) -> bool {
        self.names.contains(key.to_ascii_lowercase().as_str())
            || key
                .rsplit_once(':')
                .is_some_and(|(_, local)| self.names.contains(local.to_ascii_lowercase().as_str()))
    }

    fn names_location(&self, segments: &[String]) -> bool {
        self.paths.iter().any(|entry| {
            entry.len() == segments.len()
                && entry
                    .iter()
                    .zip(segments)
                    .all(|(want, have)| have.eq_ignore_ascii_case(want))
        })
    }

    fn has_paths(&self) -> bool {
        !self.paths.is_empty()
    }
}

#[cfg(feature = "log-transformations")]
struct Path<'a> {
    parent: Option<&'a Self>,
    segment: &'a str,
}

#[cfg(feature = "log-transformations")]
impl Path<'_> {
    fn is(&self, segments: &[Box<str>]) -> bool {
        let mut here = Some(self);
        let mut rest = segments;
        while let Some((last, head)) = rest.split_last() {
            let Some(node) = here else { return false };
            if !node.segment.eq_ignore_ascii_case(last) {
                return false;
            }
            here = node.parent;
            rest = head;
        }
        here.is_none()
    }
}

#[cfg(feature = "log-transformations")]
fn key_looks_like_card_data(key: &str) -> bool {
    key.chars().filter(char::is_ascii_digit).count() >= 12
        && key
            .chars()
            .all(|character| character.is_ascii_digit() || matches!(character, ' ' | '-' | '_'))
}

#[cfg(feature = "log-transformations")]
fn emitted_key(key: &str) -> &str {
    if key_looks_like_card_data(key) {
        MASKED
    } else {
        key
    }
}

#[cfg(feature = "log-transformations")]
fn normalize(key: &str) -> String {
    key.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|character| character.to_ascii_lowercase())
        .collect()
}

#[cfg(feature = "log-transformations")]
fn is_always_masked(key: &str) -> bool {
    let normalized = normalize(key);
    ALWAYS_MASKED_EXACT
        .iter()
        .any(|needle| normalized == *needle)
        || ALWAYS_MASKED_SUBSTRING
            .iter()
            .any(|needle| normalized.contains(needle))
}

#[cfg(feature = "log-transformations")]
fn allowed(keys: &MaskKeys, key: &str, pinned: bool) -> bool {
    (pinned || keys.names_key(key)) && !is_always_masked(key)
}

#[cfg(feature = "log-transformations")]
struct Masked<'a> {
    value: &'a Value,
    keys: &'a MaskKeys,
    at: Option<&'a Path<'a>>,
    mask: bool,
}

#[cfg(feature = "log-transformations")]
impl Serialize for Masked<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.value {
            Value::Object(map) => {
                let mut state = serializer.serialize_map(Some(map.len()))?;
                for (key, value) in map {
                    let here = Path {
                        parent: self.at,
                        segment: key,
                    };
                    let pinned =
                        self.keys.has_paths() && self.keys.paths.iter().any(|entry| here.is(entry));
                    state.serialize_entry(
                        emitted_key(key),
                        &Masked {
                            value,
                            keys: self.keys,
                            at: Some(&here),
                            mask: !allowed(self.keys, key, pinned),
                        },
                    )?;
                }
                state.end()
            }
            Value::Array(items) => {
                let mut state = serializer.serialize_seq(Some(items.len()))?;
                for value in items {
                    state.serialize_element(&Self {
                        value,
                        keys: self.keys,
                        at: self.at,
                        mask: true,
                    })?;
                }
                state.end()
            }
            Value::Null => self.value.serialize(serializer),
            _ if self.mask => serializer.serialize_str(MASKED),
            other => other.serialize(serializer),
        }
    }
}

#[cfg(feature = "log-transformations")]
fn mask_json(body: &[u8], keys: &MaskKeys) -> Option<Value> {
    let value: Value = serde_json::from_slice(body).ok()?;
    serde_json::to_value(Masked {
        value: &value,
        keys,
        at: None,
        mask: true,
    })
    .ok()
}

#[cfg(feature = "log-transformations")]
fn mask_attributes(tag: &BytesStart<'_>, keys: &MaskKeys) -> BytesStart<'static> {
    let name = String::from_utf8_lossy(tag.name().as_ref()).into_owned();
    let mut rebuilt = BytesStart::new(name);
    // quick-xml's duplicate-attribute check is quadratic in attributes-per-element, and the error
    // it produces is exactly what `flatten` discards. Skip it.
    let mut attributes = tag.attributes();
    attributes.with_checks(false);

    for attribute in attributes.flatten() {
        let key = String::from_utf8_lossy(attribute.key.as_ref()).into_owned();
        if allowed(keys, &key, false) {
            let value = attribute.unescape_value().unwrap_or_default();
            rebuilt.push_attribute((key.as_str(), value.as_ref()));
        } else {
            rebuilt.push_attribute((key.as_str(), MASKED));
        }
    }
    rebuilt
}

#[cfg(feature = "log-transformations")]
fn mask_xml(body: &[u8], keys: &MaskKeys) -> Option<String> {
    let text = std::str::from_utf8(body).ok()?;
    let mut reader = Reader::from_str(text);
    reader.check_end_names(false);

    let mut writer = Writer::new(Vec::new());
    let mut current: Option<String> = None;
    let mut open: Vec<String> = Vec::new();
    let mut first_event = true;

    fn pinned(keys: &MaskKeys, open: &[String]) -> bool {
        keys.has_paths() && keys.names_location(open)
    }

    loop {
        match reader.read_event().ok()? {
            Event::Eof => break,
            Event::Start(tag) => {
                let name = String::from_utf8_lossy(tag.name().as_ref()).into_owned();
                open.push(name.clone());
                current = Some(name);
                writer
                    .write_event(Event::Start(mask_attributes(&tag, keys)))
                    .ok()?;
            }
            Event::Empty(tag) => {
                current = None;
                writer
                    .write_event(Event::Empty(mask_attributes(&tag, keys)))
                    .ok()?;
            }
            Event::End(tag) => {
                current = None;
                open.pop();
                writer.write_event(Event::End(tag)).ok()?;
            }
            Event::Text(text) => {
                let keep = text.iter().all(u8::is_ascii_whitespace)
                    || current
                        .as_deref()
                        .is_some_and(|name| allowed(keys, name, pinned(keys, &open)));
                if keep {
                    writer.write_event(Event::Text(text)).ok()?;
                } else {
                    writer
                        .write_event(Event::Text(BytesText::new(MASKED)))
                        .ok()?;
                }
            }
            Event::CData(data) => {
                if current
                    .as_deref()
                    .is_some_and(|name| allowed(keys, name, pinned(keys, &open)))
                {
                    writer.write_event(Event::CData(data)).ok()?;
                } else {
                    writer
                        .write_event(Event::Text(BytesText::new(MASKED)))
                        .ok()?;
                }
            }
            Event::Decl(declaration) if first_event => {
                writer.write_event(Event::Decl(declaration)).ok()?;
            }
            Event::Comment(_) => {
                current = None;
                writer
                    .write_event(Event::Comment(BytesText::new(MASKED)))
                    .ok()?;
            }
            _ => {}
        }
        first_event = false;
    }

    String::from_utf8(writer.into_inner()).ok()
}

#[cfg(feature = "log-transformations")]
fn is_form_key_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b'[' | b']' | b'%' | b'+')
}

#[cfg(feature = "log-transformations")]
const MAX_FORM_KEY_LEN: usize = 128;

#[cfg(feature = "log-transformations")]
fn is_pair_shaped(body: &[u8]) -> bool {
    let mut saw_value = false;
    for segment in body.split(|byte| *byte == b'&') {
        if segment.is_empty() {
            continue;
        }
        let Some(equals) = segment.iter().position(|byte| *byte == b'=') else {
            return false;
        };
        let (key, value) = segment.split_at(equals);
        if key.is_empty()
            || key.len() > MAX_FORM_KEY_LEN
            || !key.iter().all(|byte| is_form_key_byte(*byte))
        {
            return false;
        }
        if value.len() > 1 {
            saw_value = true;
        }
    }
    saw_value
}

#[cfg(feature = "log-transformations")]
fn mask_form(body: &[u8], keys: &MaskKeys) -> Option<String> {
    let normalised: Vec<u8> = body
        .iter()
        .map(|byte| match byte {
            b'\n' | b'\r' => b'&',
            other => *other,
        })
        .collect();

    if !is_pair_shaped(&normalised) {
        return None;
    }

    let pairs: Vec<(String, String)> = serde_urlencoded::from_bytes(&normalised).ok()?;
    let masked = pairs
        .into_iter()
        .map(|(key, value)| {
            let value = if allowed(keys, &key, false) {
                value
            } else {
                MASKED.to_string()
            };
            (emitted_key(&key).to_string(), value)
        })
        .collect::<Vec<_>>();
    serde_urlencoded::to_string(masked).ok()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(feature = "log-transformations")]
enum Format {
    Json,
    Xml,
    Form,
}

#[cfg(feature = "log-transformations")]
fn detect(content_type: Option<&str>, body: &[u8]) -> Option<Format> {
    let sniffed = match body.iter().find(|byte| !byte.is_ascii_whitespace()) {
        Some(b'{' | b'[') => Format::Json,
        Some(b'<') => Format::Xml,
        Some(_) => Format::Form,
        None => return None,
    };

    if let Some(content_type) = content_type {
        let lowered = content_type.to_ascii_lowercase();
        if lowered.contains("json") {
            return Some(Format::Json);
        }
        if lowered.contains("xml") || lowered.contains("soap") {
            return Some(Format::Xml);
        }
        if lowered.contains("x-www-form-urlencoded") {
            return Some(match sniffed {
                Format::Json | Format::Xml => sniffed,
                Format::Form => Format::Form,
            });
        }
    }

    Some(sniffed)
}

/// Build the masked view of a connector response body, ready to hand to
/// [`crate::events::record_json_fields_on_span`].
///
/// JSON bodies come back as a `Value::Object`, so the log formatter emits them as real nested JSON
/// rather than one escaped blob. XML and form bodies keep their original text and come back as
/// `Value::String`, since neither round-trips through JSON without losing structure.
#[cfg(feature = "log-transformations")]
pub fn mask_connector_response(
    body: &[u8],
    content_type: Option<&str>,
    connector_name: &str,
    compiled: &CompiledMaskingKeys,
) -> Option<Value> {
    if !compiled.enabled || body.is_empty() {
        return None;
    }

    let body = crate::bytes_utils::strip_utf8_bom(body);

    // A connector with no configured entry gets every value masked, so the empty set is the
    // correct fallback rather than an error.
    let empty = MaskKeys::default();
    let keys = compiled.keys.get(connector_name).unwrap_or(&empty);

    let masked = match detect(content_type, body) {
        Some(Format::Json) => mask_json(body, keys),
        Some(Format::Xml) => mask_xml(body, keys).map(Value::String),
        Some(Format::Form) => mask_form(body, keys).map(Value::String),
        None => None,
    };

    // A body matching no structured format is replaced by a size-only stub rather than emitted:
    // with no keys to gate on, there is nothing to decide what would be safe to reveal.
    Some(
        masked.unwrap_or_else(
            || serde_json::json!({ "_format": "unparsable", "_bytes": body.len() }),
        ),
    )
}

#[cfg(all(test, feature = "log-transformations"))]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    const PAYSAFE: &str = "paysafe";
    const ADYEN: &str = "adyen";
    const ELAVON: &str = "elavon";
    const PAYU: &str = "payu";
    const FIUU: &str = "fiuu";

    const PAN: &str = "4111111111111111";

    fn config(pairs: &[(&str, &str)]) -> CompiledMaskingKeys {
        CompiledMaskingKeys::compile(&ConnectorResponseMaskingConfig {
            enabled: true,
            connector_keys: pairs
                .iter()
                .map(|(connector, keys)| ((*connector).into(), (*keys).to_string()))
                .collect(),
        })
    }

    /// Text view of a masked body, for assertions.
    ///
    /// JSON comes back as a `Value` and re-serialises to the bytes the connector would have sent
    /// (`serde_json`'s `preserve_order` is on workspace-wide, so key order survives). XML and form
    /// bodies are already text inside a `Value::String` and must be unwrapped rather than
    /// re-quoted.
    fn as_text(value: Value) -> String {
        match value {
            Value::String(text) => text,
            other => other.to_string(),
        }
    }

    /// Shadows [`super::mask_connector_response`] so every assertion below reads plain text
    /// instead of unwrapping a `Value` at each of the ~20 call sites. Same function, same
    /// arguments — only the rendering differs.
    fn mask_connector_response(
        body: &[u8],
        content_type: Option<&str>,
        connector_name: &str,
        cfg: &CompiledMaskingKeys,
    ) -> Option<String> {
        super::mask_connector_response(body, content_type, connector_name, cfg).map(as_text)
    }

    fn mask_json_body(body: &str, connector: &str, cfg: &CompiledMaskingKeys) -> String {
        mask_connector_response(body.as_bytes(), Some("application/json"), connector, cfg)
            .unwrap_or_default()
    }

    fn mask_xml_body(body: &str, connector: &str, cfg: &CompiledMaskingKeys) -> String {
        mask_connector_response(body.as_bytes(), Some("text/xml"), connector, cfg)
            .unwrap_or_default()
    }

    fn assert_is_stub(out: &str, bytes: usize) {
        assert!(
            out.contains(r#""_format":"unparsable""#),
            "not a stub: {out}"
        );
        assert!(
            out.contains(&format!(r#""_bytes":{bytes}"#)),
            "stub should carry the size: {out}"
        );
    }

    #[test]
    fn keys_for_splits_trims_and_lowercases() {
        let cfg = config(&[(PAYSAFE, " id , authCode ,, MerchantRefNum ")]);
        let keys = &cfg.keys[PAYSAFE];
        assert_eq!(keys.names.len(), 3);
        assert!(keys.names_key("id"));
        assert!(keys.names_key("authcode"));
        assert!(keys.names_key("merchantrefnum"));
    }

    #[test]
    fn unknown_connector_yields_an_empty_set() {
        let cfg = config(&[(PAYSAFE, "id")]);
        assert!(!cfg.keys.contains_key(ADYEN));
        // and therefore masks everything, rather than erroring
        assert_eq!(
            mask_json_body(r#"{"id":"1003044460"}"#, ADYEN, &cfg),
            r#"{"id":"***"}"#
        );
    }

    #[test]
    fn listed_keys_keep_their_value_others_are_masked() {
        let cfg = config(&[(PAYSAFE, "id,status")]);
        let out = mask_json_body(
            r#"{"id":"1003044460","status":"COMPLETED","amount":1000}"#,
            PAYSAFE,
            &cfg,
        );
        assert_eq!(
            out,
            r#"{"id":"1003044460","status":"COMPLETED","amount":"***"}"#
        );
    }

    #[test]
    fn nested_objects_keep_their_inner_key_names() {
        let cfg = config(&[(PAYSAFE, "status")]);
        let out = mask_json_body(
            r#"{"status":"OK","card":{"holder":"A","last4":"1111"}}"#,
            PAYSAFE,
            &cfg,
        );
        assert_eq!(
            out,
            r#"{"status":"OK","card":{"holder":"***","last4":"***"}}"#
        );
    }

    #[test]
    fn key_matching_ignores_case() {
        let cfg = config(&[(PAYSAFE, "authcode")]);
        assert_eq!(
            mask_json_body(r#"{"authCode":"727050"}"#, PAYSAFE, &cfg),
            r#"{"authCode":"727050"}"#
        );
    }

    #[test]
    fn explicit_nulls_are_preserved() {
        let cfg = config(&[(PAYSAFE, "")]);
        assert_eq!(
            mask_json_body(r#"{"reason":null}"#, PAYSAFE, &cfg),
            r#"{"reason":null}"#
        );
    }

    #[test]
    fn unconfigured_connector_masks_every_value_and_keeps_every_key() {
        let cfg = config(&[]);
        assert_eq!(
            mask_json_body(r#"{"id":"1","status":"OK"}"#, ADYEN, &cfg),
            r#"{"id":"***","status":"***"}"#
        );
    }

    #[test]
    fn denylist_overrides_the_configured_list() {
        let cfg = config(&[(PAYSAFE, "cardnumber,card_number,cvv,status")]);
        let out = mask_json_body(
            &format!(r#"{{"status":"OK","card_number":"{PAN}","cvv":"123"}}"#),
            PAYSAFE,
            &cfg,
        );
        assert_eq!(out, r#"{"status":"OK","card_number":"***","cvv":"***"}"#);
        assert!(!out.contains(PAN));
    }

    #[test]
    fn authorization_code_is_configurable_but_bare_authorization_is_not() {
        let cfg = config(&[(PAYSAFE, "authorizationcode,authorization")]);
        assert_eq!(
            mask_json_body(
                r#"{"authorizationCode":"A1B2C3","authorization":"Bearer xyz"}"#,
                PAYSAFE,
                &cfg,
            ),
            r#"{"authorizationCode":"A1B2C3","authorization":"***"}"#
        );
    }

    #[test]
    fn deeply_nested_input_does_not_blow_the_stack() {
        let depth = 120; // serde_json refuses beyond 128
        let body = format!("{}{}{}", "{\"a\":".repeat(depth), "1", "}".repeat(depth));
        let out = mask_json_body(&body, PAYSAFE, &config(&[]));
        assert!(out.ends_with(&"}".repeat(depth)));
    }

    #[test]
    fn a_listed_key_holding_an_object_still_recurses() {
        let cfg = config(&[(PAYSAFE, "card,holder")]);
        assert_eq!(
            mask_json_body(r#"{"card":{"holder":"A","last4":"1111"}}"#, PAYSAFE, &cfg),
            r#"{"card":{"holder":"A","last4":"***"}}"#
        );
    }

    #[test]
    fn objects_inside_arrays_are_decided_per_key() {
        let cfg = config(&[(PAYSAFE, "code")]);
        assert_eq!(
            mask_json_body(
                r#"{"errors":[{"code":"51","detail":"no funds"}]}"#,
                PAYSAFE,
                &cfg
            ),
            r#"{"errors":[{"code":"51","detail":"***"}]}"#
        );
    }

    #[test]
    fn scalars_in_an_array_under_a_denied_key_are_masked() {
        let cfg = config(&[(PAYSAFE, "status")]);
        let out = mask_json_body(
            &format!(r#"{{"status":"OK","tags":["{PAN}","secret"]}}"#),
            PAYSAFE,
            &cfg,
        );
        assert_eq!(out, r#"{"status":"OK","tags":["***","***"]}"#);
        assert!(!out.contains(PAN));
    }

    #[test]
    fn scalars_in_an_array_under_an_allowed_key_are_masked_too() {
        let cfg = config(&[(ADYEN, "success")]);
        let out = mask_json_body(&format!(r#"{{"success":["{PAN}"]}}"#), ADYEN, &cfg);
        assert_eq!(out, r#"{"success":["***"]}"#);
        assert!(!out.contains(PAN));
    }

    #[test]
    fn an_allowed_key_holding_a_scalar_is_unaffected_by_that_rule() {
        let cfg = config(&[(ADYEN, "success")]);
        assert_eq!(
            mask_json_body(r#"{"success":true}"#, ADYEN, &cfg),
            r#"{"success":true}"#
        );
    }

    #[test]
    fn a_top_level_array_of_scalars_is_masked() {
        let cfg = config(&[(PAYSAFE, "status")]);
        let out = mask_json_body(&format!(r#"["{PAN}","x"]"#), PAYSAFE, &cfg);
        assert_eq!(out, r#"["***","***"]"#);
        assert!(!out.contains(PAN));
    }

    #[test]
    fn a_bare_top_level_scalar_is_masked() {
        let cfg = config(&[(PAYSAFE, "status")]);
        // Asserted on the `Value` rather than through `as_text`, which cannot tell a JSON body
        // that happens to be a string from the text of an XML or form body.
        let out = super::mask_connector_response(
            format!(r#""{PAN}""#).as_bytes(),
            Some("application/json"),
            PAYSAFE,
            &cfg,
        );
        assert_eq!(out, Some(Value::String(MASKED.to_string())));
    }

    #[test]
    fn a_top_level_array_is_reached_by_sniffing_too() {
        let body = format!(r#"["{PAN}"]"#);
        let out = mask_connector_response(body.as_bytes(), None, PAYSAFE, &config(&[])).unwrap();
        assert_eq!(out, r#"["***"]"#);
    }

    #[test]
    fn xml_stays_xml_and_masks_element_text() {
        let cfg = config(&[(ELAVON, "ssl_result")]);
        let out = mask_xml_body(
            &format!(
                r#"<?xml version="1.0"?><txn><ssl_result>0</ssl_result><ssl_card_number>{PAN}</ssl_card_number></txn>"#
            ),
            ELAVON,
            &cfg,
        );

        assert!(out.starts_with("<?xml"), "declaration preserved: {out}");
        assert!(out.contains("<ssl_result>0</ssl_result>"));
        assert!(out.contains("<ssl_card_number>***</ssl_card_number>"));
        assert!(!out.contains(PAN));
    }

    #[test]
    fn xml_masks_attribute_values_including_namespace_declarations() {
        let cfg = config(&[(ELAVON, "id")]);
        let out = mask_xml_body(
            &format!(r#"<root xmlns:s="urn:x"><item id="42" pan="{PAN}"/></root>"#),
            ELAVON,
            &cfg,
        );

        assert!(out.contains(r#"xmlns:s="***""#), "namespace masked: {out}");
        assert!(out.contains(r#"id="42""#));
        assert!(out.contains(r#"pan="***""#));
        assert!(!out.contains(PAN));
    }

    #[test]
    fn xml_whitespace_between_elements_is_not_masked() {
        let cfg = config(&[(ELAVON, "")]);
        assert_eq!(
            mask_xml_body("<txn>\n  <a>1</a>\n</txn>", ELAVON, &cfg),
            "<txn>\n  <a>***</a>\n</txn>"
        );
    }

    #[test]
    fn xml_cdata_is_masked() {
        let cfg = config(&[(ELAVON, "")]);
        let out = mask_xml_body(
            &format!("<txn><note><![CDATA[{PAN}]]></note></txn>"),
            ELAVON,
            &cfg,
        );
        assert!(!out.contains(PAN), "{out}");
    }

    #[test]
    fn xml_comments_do_not_carry_their_content_through() {
        let cfg = config(&[(ELAVON, "a")]);
        let out = mask_xml_body(&format!("<r><!-- pan {PAN} --><a>b</a></r>"), ELAVON, &cfg);
        assert!(!out.contains(PAN), "{out}");
        assert!(
            out.contains("<a>b</a>"),
            "allowed text still revealed: {out}"
        );
    }

    #[test]
    fn xml_doctype_does_not_carry_its_content_through() {
        let cfg = config(&[(ELAVON, "a")]);
        let out = mask_xml_body(
            &format!(r#"<!DOCTYPE r [<!ENTITY pan "{PAN}">]><r><a>b</a></r>"#),
            ELAVON,
            &cfg,
        );
        assert!(!out.contains(PAN), "{out}");
    }

    #[test]
    fn xml_processing_instructions_do_not_carry_their_content_through() {
        let cfg = config(&[(ELAVON, "a")]);
        let out = mask_xml_body(&format!("<r><?echo {PAN}?><a>b</a></r>"), ELAVON, &cfg);
        assert!(!out.contains(PAN), "{out}");
    }

    #[test]
    fn form_stays_form_and_keeps_repeated_keys() {
        let cfg = config(&[(PAYU, "status")]);
        let out = mask_connector_response(
            format!("status=success&tag=a&tag=b&pan={PAN}").as_bytes(),
            Some("application/x-www-form-urlencoded"),
            PAYU,
            &cfg,
        )
        .unwrap();

        assert_eq!(out, "status=success&tag=***&tag=***&pan=***");
    }

    #[test]
    fn a_newline_separated_form_body_is_masked_pair_wise() {
        let cfg = config(&[(FIUU, "status")]);
        let out = mask_connector_response(
            format!("status=success\npan={PAN}\ntranID=123").as_bytes(),
            Some("application/x-www-form-urlencoded"),
            FIUU,
            &cfg,
        )
        .unwrap();

        assert_eq!(out, "status=success&pan=***&tranID=***");
        assert!(!out.contains(PAN));
    }

    #[test]
    fn a_plain_text_body_is_not_re_emitted_as_a_form_key() {
        let body = format!("Transaction declined for card {PAN}");
        let out =
            mask_connector_response(body.as_bytes(), Some("text/plain"), PAYSAFE, &config(&[]))
                .unwrap();

        assert!(!out.contains(PAN), "{out}");
        assert_is_stub(&out, body.len());
    }

    #[test]
    fn a_csv_shaped_body_is_not_re_emitted_as_form_keys() {
        let body = format!("1,{PAN},SUPERSECRET");
        let out = mask_connector_response(body.as_bytes(), None, PAYSAFE, &config(&[])).unwrap();

        assert!(!out.contains(PAN), "{out}");
        assert!(!out.contains("SUPERSECRET"), "{out}");
        assert_is_stub(&out, body.len());
    }

    #[test]
    fn a_binary_body_is_not_re_emitted_as_a_form_key() {
        let body: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x1A, 0x0A, 0xFF, 0xFE];
        let out = mask_connector_response(body, None, PAYSAFE, &config(&[])).unwrap();
        assert_is_stub(&out, body.len());
    }

    #[test]
    fn a_declared_form_body_that_is_not_form_shaped_yields_the_stub() {
        let out = mask_connector_response(
            PAN.as_bytes(),
            Some("application/x-www-form-urlencoded"),
            PAYSAFE,
            &config(&[]),
        )
        .unwrap();

        assert!(!out.contains(PAN), "{out}");
        assert_is_stub(&out, PAN.len());
    }

    #[test]
    fn an_unparsable_xml_body_yields_a_stub_carrying_only_its_size() {
        let body = format!("<<<not really anything {PAN}");
        let out = mask_connector_response(body.as_bytes(), Some("text/xml"), PAYSAFE, &config(&[]))
            .unwrap();

        assert!(!out.contains(PAN), "{out}");
        assert_is_stub(&out, body.len());
    }

    #[test]
    fn format_is_sniffed_when_content_type_is_absent() {
        let cfg = config(&[(PAYSAFE, "id")]);
        assert_eq!(
            mask_connector_response(br#"{"id":"1","x":"y"}"#, None, PAYSAFE, &cfg).unwrap(),
            r#"{"id":"1","x":"***"}"#
        );
    }

    #[test]
    fn a_bom_prefixed_body_is_still_parsed() {
        let cfg = config(&[(PAYSAFE, "id")]);
        let mut body = vec![0xEF, 0xBB, 0xBF];
        body.extend_from_slice(format!(r#"{{"id":"1","pan":"{PAN}"}}"#).as_bytes());

        let out = mask_connector_response(&body, None, PAYSAFE, &cfg).unwrap();
        assert_eq!(out, r#"{"id":"1","pan":"***"}"#);
    }

    #[test]
    fn disabled_or_empty_yields_nothing() {
        let mut cfg = config(&[(PAYSAFE, "id")]);
        cfg.enabled = false;
        assert!(mask_connector_response(br#"{"id":"1"}"#, None, PAYSAFE, &cfg).is_none());

        cfg.enabled = true;
        assert!(mask_connector_response(b"", None, PAYSAFE, &cfg).is_none());
    }

    /// The config section is plain serde, so a JSON fixture exercises the same deserializer the
    /// TOML files go through. Connector names are *not* validated here — `ucs_env` does that once
    /// at startup, where the connector enums are in scope.
    #[test]
    fn deserializes_from_the_shape_used_in_config_files() {
        let raw = r#"{
            "enabled": true,
            "connector_keys": {
                "paysafe": "id,status,authCode",
                "Adyen": "pspReference,resultCode"
            }
        }"#;
        let cfg: ConnectorResponseMaskingConfig =
            serde_json::from_str(raw).expect("config section must deserialize");
        let compiled = CompiledMaskingKeys::compile(&cfg);

        assert!(compiled.enabled);
        assert!(compiled.keys[PAYSAFE].names_key("authcode"));
        // `Adyen` is lowercased on the way in, so it matches `get_connector_name()`
        assert!(compiled.keys[ADYEN].names_key("pspreference"));
        assert!(!compiled.keys.contains_key("stripe"));
    }

    #[test]
    fn an_unresolvable_connector_name_deserializes_and_simply_never_matches() {
        // Rejecting here would fail the whole `ConfigPatch` on an `x-config-override` header and
        // cost a payment; masking everything is the safe direction.
        let raw = r#"{"enabled": true, "connector_keys": {"paysafee": "id"}}"#;
        let cfg: ConnectorResponseMaskingConfig =
            serde_json::from_str(raw).expect("a typo must not fail deserialization");

        assert_eq!(cfg.unknown_connectors(|name| name == PAYSAFE), ["paysafee"]);
        assert_eq!(
            mask_json_body(
                r#"{"id":"1"}"#,
                PAYSAFE,
                &CompiledMaskingKeys::compile(&cfg)
            ),
            r#"{"id":"***"}"#
        );
    }

    #[test]
    fn a_patch_takes_effect_once_the_keys_are_recompiled() {
        let mut cfg = ConnectorResponseMaskingConfig {
            enabled: true,
            connector_keys: [(PAYSAFE.into(), "id".to_string())].into_iter().collect(),
        };
        assert!(CompiledMaskingKeys::compile(&cfg).keys[PAYSAFE].names_key("id"));

        cfg.apply(ConnectorResponseMaskingConfigPatch {
            enabled: None,
            connector_keys: Some(
                [(PAYSAFE.into(), "status".to_string())]
                    .into_iter()
                    .collect(),
            ),
        });

        // Keys are parsed once, so a patch is only live after the recompile step that
        // `Config::post_patch_processing` performs.
        let keys = &CompiledMaskingKeys::compile(&cfg).keys[PAYSAFE];
        assert!(keys.names_key("status"));
        assert!(!keys.names_key("id"), "stale key survived the patch");
    }

    #[test]
    fn prose_containing_an_equals_sign_is_not_a_form_body() {
        let body = format!("Declined: card {PAN}, retry=true");
        let out = mask_connector_response(body.as_bytes(), None, PAYSAFE, &config(&[])).unwrap();
        assert!(!out.contains(PAN), "{out}");
        assert_is_stub(&out, body.len());
    }

    #[test]
    fn a_base64_blob_is_not_a_form_body() {
        let body = "eyJwYW4iOiI0MTExMTExMTExMTExMTExIn0=";
        let out = mask_connector_response(body.as_bytes(), None, PAYSAFE, &config(&[])).unwrap();
        assert!(!out.contains("eyJwYW4"), "blob echoed: {out}");
        assert_is_stub(&out, body.len());
    }

    #[test]
    fn a_declared_form_type_does_not_override_a_json_body() {
        let body = format!(r#"{{"pan":"{PAN}","sig":"a=b"}}"#);
        let out = mask_connector_response(
            body.as_bytes(),
            Some("application/x-www-form-urlencoded"),
            PAYSAFE,
            &config(&[]),
        )
        .unwrap();
        assert_eq!(out, r#"{"pan":"***","sig":"***"}"#);
    }

    #[test]
    fn a_real_form_body_still_masks_as_a_form() {
        let cfg = config(&[(FIUU, "status,tranid")]);
        let out = mask_connector_response(
            b"status=00&tranID=31530063&amount=1.00",
            Some("application/x-www-form-urlencoded"),
            FIUU,
            &cfg,
        )
        .unwrap();
        assert_eq!(out, "status=00&tranID=31530063&amount=***");
    }

    #[test]
    fn a_key_that_is_itself_card_data_is_masked() {
        let body = format!(r#"{{"declines":{{"{PAN}":"expired"}}}}"#);
        let out = mask_json_body(&body, PAYSAFE, &config(&[]));
        assert!(!out.contains(PAN), "{out}");
    }

    #[test]
    fn short_denylist_entries_survive_being_allowlisted() {
        for key in [
            "pan", "iban", "bban", "pin", "ssn", "emv", "csc", "cid", "ksn", "jwt",
        ] {
            let out = mask_json_body(
                &format!(r#"{{"{key}":"{PAN}"}}"#),
                PAYSAFE,
                &config(&[(PAYSAFE, key)]),
            );
            assert_eq!(out, format!(r#"{{"{key}":"***"}}"#), "`{key}` was revealed");
        }
    }

    #[test]
    fn spelling_variants_of_card_fields_survive_being_allowlisted() {
        for key in [
            "card_no",
            "cardNo",
            "expiry_month",
            "expiry_year",
            "expirationDate",
            "track2",
            "trackData",
            "securityCode",
            "routingNumber",
            "sortCode",
            "pinBlock",
            "hmac",
        ] {
            let out = mask_json_body(
                &format!(r#"{{"{key}":"{PAN}"}}"#),
                PAYSAFE,
                &config(&[(PAYSAFE, key)]),
            );
            assert!(!out.contains(PAN), "`{key}` was revealed: {out}");
        }
    }

    #[test]
    fn xml_a_namespace_declaration_cannot_smuggle_a_value() {
        let out = mask_xml_body(
            &format!(r#"<r xmlns:cardnumber="{PAN}"><a>x</a></r>"#),
            ELAVON,
            &config(&[]),
        );
        assert!(!out.contains(PAN), "{out}");
    }

    #[test]
    fn xml_only_a_leading_declaration_is_written_through() {
        let out = mask_xml_body(
            &format!(r#"<r><?xml pan="{PAN}"?><a>b</a></r>"#),
            ELAVON,
            &config(&[]),
        );
        assert!(!out.contains(PAN), "{out}");

        let leading = mask_xml_body(
            &format!(r#"<?xml version="1.0"?><txn><a>{PAN}</a></txn>"#),
            ELAVON,
            &config(&[]),
        );
        assert!(
            leading.starts_with("<?xml"),
            "declaration dropped: {leading}"
        );
        assert!(!leading.contains(PAN));
    }

    #[test]
    fn xml_text_after_a_self_closing_tag_does_not_inherit_the_parent() {
        let cfg = config(&[(ELAVON, "status")]);
        let empty = mask_xml_body(&format!("<status>OK<pan/>{PAN}</status>"), ELAVON, &cfg);
        assert!(!empty.contains(PAN), "self-closing sibling leaked: {empty}");

        let paired = mask_xml_body(
            &format!("<status>OK<pan></pan>{PAN}</status>"),
            ELAVON,
            &cfg,
        );
        assert!(!paired.contains(PAN), "paired sibling leaked: {paired}");
    }

    #[test]
    fn a_bare_entry_unmasks_that_name_at_any_depth() {
        let body = r#"{"z":"1","a":{"z":"2","y":"3"},"b":[{"z":"4"}]}"#;
        assert_eq!(
            mask_json_body(body, PAYSAFE, &config(&[(PAYSAFE, "z")])),
            r#"{"z":"1","a":{"z":"2","y":"***"},"b":[{"z":"4"}]}"#
        );
    }

    #[test]
    fn a_dotted_entry_unmasks_only_that_location() {
        let body = r#"{"z":"1","a":{"z":"2","y":"3"},"b":[{"z":"4"}]}"#;
        assert_eq!(
            mask_json_body(body, PAYSAFE, &config(&[(PAYSAFE, "a.z")])),
            r#"{"z":"***","a":{"z":"2","y":"***"},"b":[{"z":"***"}]}"#
        );
    }

    #[test]
    fn a_dotted_entry_also_matches_a_literal_dotted_key_name() {
        let body = r#"{"additionalData":{"retry.attempt1.acquirer":"acq","authCode":"1234"}}"#;
        let out = mask_json_body(
            body,
            PAYSAFE,
            &config(&[(PAYSAFE, "retry.attempt1.acquirer")]),
        );
        assert!(out.contains(r#""retry.attempt1.acquirer":"acq""#), "{out}");
        assert!(out.contains(r#""authCode":"***""#), "{out}");
    }

    #[test]
    fn a_dotted_entry_resolves_as_a_path_when_one_exists() {
        let body = r#"{"additionalData":{"authCode":"1234","other":"x"}}"#;
        let out = mask_json_body(
            body,
            PAYSAFE,
            &config(&[(PAYSAFE, "additionaldata.authcode")]),
        );
        assert!(out.contains(r#""authCode":"1234""#), "{out}");
        assert!(out.contains(r#""other":"***""#), "{out}");
    }

    #[test]
    fn a_multipart_body_yields_the_stub() {
        let body = format!(
            "--x9Y\r\nContent-Disposition: form-data; name=\"pan\"\r\n\r\n{PAN}\r\n--x9Y--\r\n"
        );
        let out = mask_connector_response(
            body.as_bytes(),
            Some("multipart/form-data; boundary=x9Y"),
            PAYSAFE,
            &config(&[]),
        )
        .unwrap();

        assert!(!out.contains(PAN), "{out}");
        assert_is_stub(&out, body.len());
    }

    /// A boundary may legally contain `=`, `-`, `_`, `+` and `.` — every one of which
    /// [`is_form_key_byte`] accepts. The part header is what stops it being read as a form.
    #[test]
    fn a_multipart_body_whose_boundary_contains_an_equals_still_stubs() {
        let body = format!(
            "--a=b\r\nContent-Disposition: form-data; name=\"pan\"\r\n\r\n{PAN}\r\n--a=b--\r\n"
        );
        let out = mask_connector_response(
            body.as_bytes(),
            Some("multipart/form-data; boundary=a=b"),
            PAYSAFE,
            &config(&[]),
        )
        .unwrap();

        assert!(!out.contains(PAN), "{out}");
        assert_is_stub(&out, body.len());
    }

    /// `pinned` used to rebuild a `Vec<&str>` of the whole open-element stack per text node, which
    /// is quadratic in depth. `check_end_names(false)` means these tags never close, so the stack
    /// only grows. Without the fix this does not finish.
    #[test]
    fn deeply_nested_xml_with_a_configured_path_stays_linear() {
        let body = format!("<r>{}", "<a>x".repeat(20_000));
        let out = mask_xml_body(&body, ELAVON, &config(&[(ELAVON, "r.a")]));
        assert!(!out.is_empty());
    }

    #[test]
    fn an_allowlisted_attribute_value_survives_an_unresolvable_entity() {
        let cfg = config(&[(ELAVON, "redirecturl")]);
        let out = mask_xml_body(
            r#"<r><item redirectUrl="https://acs.test/3ds?a=1&amp;b=2"/></r>"#,
            ELAVON,
            &cfg,
        );
        assert!(out.contains("acs.test"), "value should survive: {out}");
    }

    /// XML names cannot legally be card data, so they are emitted as-is rather than substituted —
    /// `***` is not a valid XML name and is not injective.
    #[test]
    fn a_card_shaped_xml_attribute_name_is_emitted_verbatim_not_as_a_mask() {
        let out = mask_xml_body(
            &format!(r#"<r><item {PAN}="secret" {PAN}9="other"/></r>"#),
            ELAVON,
            &config(&[]),
        );
        assert!(out.contains(&format!("{PAN}=")), "name kept: {out}");
        assert!(!out.contains("***=\"***\""), "no collapsed name: {out}");
        assert!(!out.contains("secret"), "value still masked: {out}");
    }
}

// Config parsing and patching do not need the engine, so these run in a default build too — which
// is the build where a rejected override would fail a payment.
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod config_tests {
    use super::*;

    /// Stands in for the connector enums, which live in `domain_types` and cannot be reached from
    /// this crate. `ucs_env` supplies the real predicate.
    fn is_known(name: &str) -> bool {
        matches!(name, "adyen" | "paysafe")
    }

    #[test]
    fn defaults_are_off() {
        let cfg = ConnectorResponseMaskingConfig::default();
        assert!(!cfg.enabled);
        assert!(cfg.connector_keys.is_empty());
        assert!(!CompiledMaskingKeys::compile(&cfg).enabled);
    }

    #[test]
    fn unknown_connectors_reports_every_bad_name_sorted() {
        let cfg: ConnectorResponseMaskingConfig = serde_json::from_str(
            r#"{"connector_keys":{"zzz":"id","adyen":"resultCode","paysafee":"id"}}"#,
        )
        .unwrap();

        // Sorted and complete, so a startup failure can name them all at once.
        assert_eq!(cfg.unknown_connectors(is_known), ["paysafee", "zzz"]);
    }

    #[test]
    fn names_are_lowercased_so_lookups_match_get_connector_name() {
        let cfg: ConnectorResponseMaskingConfig =
            serde_json::from_str(r#"{"connector_keys":{"Adyen":"resultCode"}}"#).unwrap();

        assert!(cfg.connector_keys.contains_key("adyen"));
        assert!(cfg.unknown_connectors(is_known).is_empty());
    }

    #[test]
    fn an_unknown_connector_in_an_override_never_fails_deserialization() {
        // This path is the `x-config-override` header: an error would reject the request before
        // the handler runs, so a typo in a diagnostic setting would cost a payment.
        let patch: ConnectorResponseMaskingConfigPatch =
            serde_json::from_str(r#"{"enabled":true,"connector_keys":{"paysafee":"id"}}"#)
                .expect("a per-request override must never fail to deserialize");

        let mut cfg = ConnectorResponseMaskingConfig::default();
        cfg.apply(patch);

        // The patch applies; the unresolvable name simply never matches a connector at lookup
        // time, which masks everything — the safe direction.
        assert!(cfg.enabled);
        assert_eq!(cfg.unknown_connectors(is_known), ["paysafee"]);
    }

    #[test]
    fn an_override_naming_only_known_connectors_still_applies() {
        let patch: ConnectorResponseMaskingConfigPatch =
            serde_json::from_str(r#"{"enabled":true,"connector_keys":{"adyen":"resultCode"}}"#)
                .expect("valid override");

        let mut cfg = ConnectorResponseMaskingConfig::default();
        cfg.apply(patch);

        assert!(cfg.enabled);
        assert_eq!(
            cfg.connector_keys.get("adyen").map(String::as_str),
            Some("resultCode")
        );
    }

    #[test]
    fn an_override_replaces_rather_than_merges_the_key_lists() {
        let mut cfg = ConnectorResponseMaskingConfig {
            enabled: true,
            connector_keys: [("adyen".into(), "resultCode".to_string())]
                .into_iter()
                .collect(),
        };

        cfg.apply(
            serde_json::from_str(r#"{"connector_keys":{"paysafe":"id"}}"#).expect("valid override"),
        );

        assert!(
            !cfg.connector_keys.contains_key("adyen"),
            "the TOML entry should go dark, not merge"
        );
        assert!(cfg.connector_keys.contains_key("paysafe"));
    }
}

/// Edge cases for the surface that changed when masking became log-only:
/// `mask_connector_response` returning a `serde_json::Value` instead of a `String`, and the
/// allowlist being parsed once into [`CompiledMaskingKeys`] instead of on every response.
#[cfg(all(test, feature = "log-transformations"))]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod value_shape_tests {
    use super::*;

    const PAYSAFE: &str = "paysafe";
    const PAN: &str = "4111111111111111";

    fn compiled(pairs: &[(&str, &str)]) -> CompiledMaskingKeys {
        CompiledMaskingKeys::compile(&ConnectorResponseMaskingConfig {
            enabled: true,
            connector_keys: pairs
                .iter()
                .map(|(connector, keys)| ((*connector).into(), (*keys).to_string()))
                .collect(),
        })
    }

    fn mask(body: &str, content_type: Option<&str>, cfg: &CompiledMaskingKeys) -> Option<Value> {
        mask_connector_response(body.as_bytes(), content_type, PAYSAFE, cfg)
    }

    // ---- shape per input format -------------------------------------------------------------

    /// The whole point of returning `Value`: a JSON body reaches the log formatter as a real
    /// object, so it is queryable in Loki instead of one escaped blob.
    #[test]
    fn a_json_body_is_emitted_as_an_object_not_a_string() {
        let out = mask(
            r#"{"id":"1","amount":100}"#,
            Some("application/json"),
            &compiled(&[(PAYSAFE, "id")]),
        )
        .unwrap();
        assert!(matches!(out, Value::Object(_)), "got {out:?}");
        assert_eq!(out["id"], Value::String("1".into()));
        assert_eq!(out["amount"], Value::String(MASKED.into()));
    }

    #[test]
    fn a_json_root_array_stays_an_array() {
        let out = mask(
            &format!(r#"["{PAN}","x"]"#),
            Some("application/json"),
            &compiled(&[]),
        )
        .unwrap();
        assert_eq!(out, serde_json::json!([MASKED, MASKED]));
    }

    /// XML has no JSON representation, so it stays text — but as a `Value::String`, it must not
    /// come back double-quoted or backslash-escaped.
    #[test]
    fn an_xml_body_is_a_plain_string_not_re_escaped() {
        let out = mask(r#"<r><a>secret</a></r>"#, Some("text/xml"), &compiled(&[])).unwrap();
        // A `Value::String`, so the XML is carried verbatim rather than re-encoded as JSON.
        assert_eq!(out, Value::String("<r><a>***</a></r>".into()));
        assert!(
            !out.as_str().unwrap_or_default().contains('\\'),
            "escaped: {out:?}"
        );
    }

    #[test]
    fn a_form_body_is_a_plain_string() {
        let out = mask(
            "status=1&amount=100",
            Some("application/x-www-form-urlencoded"),
            &compiled(&[(PAYSAFE, "status")]),
        )
        .unwrap();
        assert_eq!(out, Value::String("status=1&amount=***".into()));
    }

    /// The stub is structured too, so `_bytes` stays a queryable number rather than text.
    #[test]
    fn the_unparsable_stub_is_an_object_with_a_numeric_size() {
        let body = "not, a, structured, body";
        let out = mask(body, Some("text/plain"), &compiled(&[])).unwrap();
        assert_eq!(out["_format"], Value::String("unparsable".into()));
        assert_eq!(out["_bytes"], Value::Number(body.len().into()));
        assert!(out.get("_bytes").is_some_and(Value::is_number));
    }

    // ---- things `to_value` could plausibly have broken ---------------------------------------

    /// `serde_json`'s `preserve_order` is on, so `to_value` keeps the connector's field order.
    /// Without it `Map` is a `BTreeMap` and every logged body would come out alphabetised.
    #[test]
    fn key_order_follows_the_connector_not_the_alphabet() {
        let out = mask(
            r#"{"zebra":"1","apple":"2","mango":"3"}"#,
            Some("application/json"),
            &compiled(&[]),
        )
        .unwrap();
        let keys: Vec<&str> = out
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(keys, ["zebra", "apple", "mango"]);
    }

    /// An allowlisted value keeps its JSON *type*. Going through a string would have turned
    /// numbers and booleans into text and made them useless for numeric queries.
    #[test]
    fn an_allowed_value_keeps_its_json_type() {
        let out = mask(
            r#"{"amount":1000,"captured":true,"reason":null,"note":"hi"}"#,
            Some("application/json"),
            &compiled(&[(PAYSAFE, "amount,captured,reason,note")]),
        )
        .unwrap();
        assert_eq!(out["amount"], Value::Number(1000.into()));
        assert_eq!(out["captured"], Value::Bool(true));
        assert_eq!(out["reason"], Value::Null);
        assert_eq!(out["note"], Value::String("hi".into()));
    }

    /// `null` is structural, not a value to hide, and must survive under a *masked* key too —
    /// otherwise the shape of the connector's reply is lost.
    #[test]
    fn null_survives_under_a_masked_key() {
        let out = mask(
            r#"{"secretish":null}"#,
            Some("application/json"),
            &compiled(&[]),
        )
        .unwrap();
        assert_eq!(out["secretish"], Value::Null);
    }

    #[test]
    fn nesting_survives_as_nesting() {
        let out = mask(
            r#"{"a":{"b":{"c":"deep"}}}"#,
            Some("application/json"),
            &compiled(&[(PAYSAFE, "c")]),
        )
        .unwrap();
        assert_eq!(out["a"]["b"]["c"], Value::String("deep".into()));
        assert!(out["a"]["b"].is_object());
    }

    // ---- CompiledMaskingKeys / kill-switch ---------------------------------------------------

    #[test]
    fn nothing_is_built_when_disabled_even_with_keys_configured() {
        let mut cfg = compiled(&[(PAYSAFE, "id")]);
        cfg.enabled = false;
        assert!(mask(r#"{"id":"1"}"#, Some("application/json"), &cfg).is_none());
    }

    #[test]
    fn an_empty_body_produces_nothing_rather_than_a_stub() {
        assert!(mask_connector_response(b"", None, PAYSAFE, &compiled(&[])).is_none());
    }

    #[test]
    fn a_connector_with_no_entry_masks_everything() {
        let cfg = compiled(&[("adyen", "id")]);
        let out = mask(r#"{"id":"1"}"#, Some("application/json"), &cfg).unwrap();
        assert_eq!(out["id"], Value::String(MASKED.into()));
    }

    /// Separators only: must parse to an empty set, not to a phantom `""` key that would then
    /// match every empty-named field.
    #[test]
    fn a_separator_only_entry_allowlists_nothing() {
        let keys = MaskKeys::parse(" , ,, ");
        assert!(keys.names.is_empty() && keys.paths.is_empty());
    }

    /// A dotted entry registers both the bare name and the pinned location, so `a.b` reveals
    /// `b` under `a` via the path and `b` anywhere via the name.
    #[test]
    fn a_dotted_entry_registers_both_a_name_and_a_path() {
        let keys = MaskKeys::parse("additionaldata.authcode");
        assert!(keys.has_paths());
        assert!(keys.names_key("additionaldata.authcode"));
    }

    /// Pins the caching contract: keys are parsed once, so a mutated config is only live after
    /// the recompile that `Config::post_patch_processing` performs on every override.
    #[test]
    fn a_config_change_is_invisible_until_recompiled() {
        let mut raw = ConnectorResponseMaskingConfig {
            enabled: true,
            connector_keys: [(PAYSAFE.into(), "id".to_string())].into_iter().collect(),
        };
        let stale = CompiledMaskingKeys::compile(&raw);

        raw.connector_keys
            .insert(PAYSAFE.into(), "amount".to_string());

        // stale copy still reflects the old list...
        assert!(stale.keys[PAYSAFE].names_key("id"));
        assert!(!stale.keys[PAYSAFE].names_key("amount"));
        // ...and only the recompile picks up the change.
        assert!(CompiledMaskingKeys::compile(&raw).keys[PAYSAFE].names_key("amount"));
    }

    /// The `config` crate lowercases env-var keys, so `CS__..__CONNECTOR_KEYS__ADYEN` arrives as
    /// `adyen`. Lookups use `get_connector_name()`, which is already snake_case, so both must meet
    /// in the middle regardless of how the operator cased it.
    #[test]
    fn an_env_var_style_uppercase_name_still_resolves() {
        let cfg: ConnectorResponseMaskingConfig =
            serde_json::from_str(r#"{"enabled":true,"connector_keys":{"PAYSAFE":"id"}}"#).unwrap();
        let out = mask_connector_response(
            br#"{"id":"1"}"#,
            Some("application/json"),
            PAYSAFE,
            &CompiledMaskingKeys::compile(&cfg),
        )
        .unwrap();
        assert_eq!(out["id"], Value::String("1".into()));
    }

    /// The denylist is the backstop against a careless allowlist entry, and it must still win
    /// now that the output is a `Value` rather than a serialised string.
    #[test]
    fn the_denylist_still_beats_an_operator_allowlist() {
        let out = mask(
            &format!(r#"{{"cardNumber":"{PAN}","cvv":"123","authCode":"ok"}}"#),
            Some("application/json"),
            &compiled(&[(PAYSAFE, "cardnumber,cvv,authcode")]),
        )
        .unwrap();
        assert_eq!(out["cardNumber"], Value::String(MASKED.into()));
        assert_eq!(out["cvv"], Value::String(MASKED.into()));
        assert_eq!(out["authCode"], Value::String("ok".into()));
        assert!(!out.to_string().contains(PAN));
    }
}
