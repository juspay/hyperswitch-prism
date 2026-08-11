#![warn(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::as_conversions,
    clippy::unreachable
)]

use std::collections::HashMap;
#[cfg(feature = "connector-response-masking")]
use std::collections::HashSet;

#[cfg(feature = "connector-response-masking")]
use quick_xml::events::{BytesStart, BytesText, Event};
#[cfg(feature = "connector-response-masking")]
use quick_xml::{Reader, Writer};
#[cfg(feature = "connector-response-masking")]
use serde::ser::{SerializeMap, SerializeSeq};
#[cfg(feature = "connector-response-masking")]
use serde::Serializer;
use serde::{Deserialize, Serialize};
#[cfg(feature = "connector-response-masking")]
use serde_json::Value;

#[cfg(feature = "connector-response-masking")]
use crate::connector_types::RawConnectorRequestResponse;
#[cfg(feature = "connector-response-masking")]
use crate::router_response_types::Response;

use common_utils::config_patch::Patch;

use crate::connector_types::{
    AuthenticatorConnectorEnum, ConnectorEnum, FrmConnectorEnum, PayoutConnectorEnum,
    SurchargeConnectorEnum,
};

#[cfg(feature = "connector-response-masking")]
pub const MASKED: &str = "***";

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct ConnectorResponseMaskingConfig {
    pub enabled: bool,

    pub log_to_span: bool,

    #[serde(deserialize_with = "deserialize_connector_keys")]
    pub connector_keys: HashMap<Box<str>, String>,
}

fn is_known_connector(name: &str) -> bool {
    use std::str::FromStr;

    ConnectorEnum::from_str(name).is_ok()
        || SurchargeConnectorEnum::from_str(name).is_ok()
        || PayoutConnectorEnum::from_str(name).is_ok()
        || FrmConnectorEnum::from_str(name).is_ok()
        || AuthenticatorConnectorEnum::from_str(name).is_ok()
}

fn deserialize_connector_keys<'de, D>(
    deserializer: D,
) -> Result<HashMap<Box<str>, String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;

    HashMap::<String, String>::deserialize(deserializer)?
        .into_iter()
        .map(|(name, keys)| {
            let normalized = name.to_lowercase();
            if is_known_connector(&normalized) {
                Ok((normalized.into_boxed_str(), keys))
            } else {
                Err(D::Error::custom(format!("unknown connector `{name}`")))
            }
        })
        .collect()
}

impl ConnectorResponseMaskingConfig {
    #[cfg(feature = "connector-response-masking")]
    pub fn keys_for(&self, connector_name: &str) -> MaskKeys {
        let mut keys = MaskKeys::default();
        let Some(configured) = self.connector_keys.get(connector_name) else {
            return keys;
        };

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

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct ConnectorResponseMaskingConfigPatch {
    pub enabled: Option<bool>,
    pub log_to_span: Option<bool>,
    #[serde(default, deserialize_with = "deserialize_optional_connector_keys")]
    pub connector_keys: Option<HashMap<Box<str>, String>>,
}

/// Deliberately does not validate, unlike [`deserialize_connector_keys`].
///
/// This runs on the per-request `x-config-override` header. Rejecting a name here fails
/// deserialization of the whole `ConfigPatch`, and the middleware then returns without ever calling
/// the handler — so a typo in a diagnostic setting would cost a payment. Names are checked in
/// [`Patch::apply`] instead, where a bad one can be dropped harmlessly. A typo in a config *file*
/// still aborts startup, via the strict deserializer on the base config.
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
        // A name we do not recognise means the caller is describing a connector we cannot resolve,
        // so the rest of the block cannot be trusted either: drop it whole and keep our own config.
        // Warn rather than fail — this arrives on a request header, and masking is a diagnostic.
        if let Some(unknown) = patch
            .connector_keys
            .as_ref()
            .and_then(|keys| keys.keys().find(|name| !is_known_connector(name)))
        {
            tracing::warn!(
                connector = %unknown,
                "unknown connector in connector_response_masking override; ignoring the whole block"
            );
            return;
        }

        if let Some(enabled) = patch.enabled {
            self.enabled = enabled;
        }
        if let Some(log_to_span) = patch.log_to_span {
            self.log_to_span = log_to_span;
        }
        if let Some(connector_keys) = patch.connector_keys {
            self.connector_keys = connector_keys;
        }
    }
}

#[cfg(feature = "connector-response-masking")]
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

#[cfg(feature = "connector-response-masking")]
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

#[derive(Debug, Default, Clone)]
#[cfg(feature = "connector-response-masking")]
pub struct MaskKeys {
    names: HashSet<Box<str>>,
    paths: Vec<Box<[Box<str>]>>,
}

#[cfg(feature = "connector-response-masking")]
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

    #[cfg(test)]
    fn is_empty(&self) -> bool {
        self.names.is_empty() && self.paths.is_empty()
    }

    fn has_paths(&self) -> bool {
        !self.paths.is_empty()
    }
}

#[cfg(feature = "connector-response-masking")]
struct Path<'a> {
    parent: Option<&'a Path<'a>>,
    segment: &'a str,
}

#[cfg(feature = "connector-response-masking")]
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

#[cfg(feature = "connector-response-masking")]
fn key_looks_like_card_data(key: &str) -> bool {
    key.chars().filter(char::is_ascii_digit).count() >= 12
        && key
            .chars()
            .all(|character| character.is_ascii_digit() || matches!(character, ' ' | '-' | '_'))
}

#[cfg(feature = "connector-response-masking")]
fn emitted_key(key: &str) -> &str {
    if key_looks_like_card_data(key) {
        MASKED
    } else {
        key
    }
}

#[cfg(feature = "connector-response-masking")]
fn normalize(key: &str) -> String {
    key.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|character| character.to_ascii_lowercase())
        .collect()
}

#[cfg(feature = "connector-response-masking")]
fn is_always_masked(key: &str) -> bool {
    let normalized = normalize(key);
    ALWAYS_MASKED_EXACT
        .iter()
        .any(|needle| normalized == *needle)
        || ALWAYS_MASKED_SUBSTRING
            .iter()
            .any(|needle| normalized.contains(needle))
}

#[cfg(feature = "connector-response-masking")]
fn allowed(keys: &MaskKeys, key: &str, pinned: bool) -> bool {
    (pinned || keys.names_key(key)) && !is_always_masked(key)
}

#[cfg(feature = "connector-response-masking")]
struct Masked<'a> {
    value: &'a Value,
    keys: &'a MaskKeys,
    at: Option<&'a Path<'a>>,
    mask: bool,
}

#[cfg(feature = "connector-response-masking")]
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

#[cfg(feature = "connector-response-masking")]
fn mask_json(body: &[u8], keys: &MaskKeys) -> Option<String> {
    let value: Value = serde_json::from_slice(body).ok()?;
    serde_json::to_string(&Masked {
        value: &value,
        keys,
        at: None,
        mask: true,
    })
    .ok()
}

#[cfg(feature = "connector-response-masking")]
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

#[cfg(feature = "connector-response-masking")]
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

#[cfg(feature = "connector-response-masking")]
fn is_form_key_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b'[' | b']' | b'%' | b'+')
}

#[cfg(feature = "connector-response-masking")]
const MAX_FORM_KEY_LEN: usize = 128;

#[cfg(feature = "connector-response-masking")]
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

#[cfg(feature = "connector-response-masking")]
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
#[cfg(feature = "connector-response-masking")]
enum Format {
    Json,
    Xml,
    Form,
}

#[cfg(feature = "connector-response-masking")]
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

#[cfg(feature = "connector-response-masking")]
pub fn mask_connector_response(
    body: &[u8],
    content_type: Option<&str>,
    connector_name: &str,
    config: &ConnectorResponseMaskingConfig,
) -> Option<String> {
    if !config.enabled || body.is_empty() {
        return None;
    }

    let body = common_utils::bytes_utils::strip_utf8_bom(body);

    let keys = config.keys_for(connector_name);

    let masked = match detect(content_type, body) {
        Some(Format::Json) => mask_json(body, &keys),
        Some(Format::Xml) => mask_xml(body, &keys),
        Some(Format::Form) => mask_form(body, &keys),
        None => None,
    };

    Some(masked.unwrap_or_else(|| format!(r#"{{"_format":"unparsable","_bytes":{}}}"#, body.len())))
}

/// Build the masked view of a connector response and stash it on the flow data.
///
/// Lives here rather than in `external-services` so the whole feature sits behind one crate's
/// `#[cfg]`: everything it touches — the trait, `Response`, the config — is already in this crate.
#[cfg(feature = "connector-response-masking")]
pub fn record_masked_connector_response<ResourceCommonData>(
    resource_common_data: &mut ResourceCommonData,
    body: &Response,
    connector_name: &str,
    config: &ConnectorResponseMaskingConfig,
) where
    ResourceCommonData: RawConnectorRequestResponse,
{
    let content_type = body
        .headers
        .as_ref()
        .and_then(|headers| headers.get("content-type"))
        .and_then(|value| value.to_str().ok());

    let masked = mask_connector_response(&body.response, content_type, connector_name, config);

    // Metadata only. The body itself goes no further than `log_to_span` allows, which is the whole
    // point of that flag; content type and size are what explain an unexpected stub.
    tracing::debug!(
        connector = connector_name,
        content_type = content_type.unwrap_or("<none>"),
        masked_bytes = masked.as_deref().map(str::len),
        "built masked connector response"
    );

    if config.log_to_span {
        if let Some(masked) = masked.as_deref() {
            tracing::Span::current()
                .record("response.masked_body", tracing::field::display(masked));
        }
    }

    resource_common_data.set_masked_connector_response(masked);
}

#[cfg(all(test, feature = "connector-response-masking"))]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    const PAYSAFE: &str = "paysafe";
    const ADYEN: &str = "adyen";
    const ELAVON: &str = "elavon";
    const PAYU: &str = "payu";
    const FIUU: &str = "fiuu";

    const PAN: &str = "4111111111111111";

    fn config(pairs: &[(&str, &str)]) -> ConnectorResponseMaskingConfig {
        ConnectorResponseMaskingConfig {
            enabled: true,
            log_to_span: false,
            connector_keys: pairs
                .iter()
                .map(|(connector, keys)| ((*connector).into(), (*keys).to_string()))
                .collect(),
        }
    }

    fn mask_json_body(body: &str, connector: &str, cfg: &ConnectorResponseMaskingConfig) -> String {
        mask_connector_response(body.as_bytes(), Some("application/json"), connector, cfg)
            .unwrap_or_default()
    }

    fn mask_xml_body(body: &str, connector: &str, cfg: &ConnectorResponseMaskingConfig) -> String {
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
        let keys = cfg.keys_for(PAYSAFE);
        assert_eq!(keys.names.len(), 3);
        assert!(keys.names_key("id"));
        assert!(keys.names_key("authcode"));
        assert!(keys.names_key("merchantrefnum"));
    }

    #[test]
    fn unknown_connector_yields_an_empty_set() {
        let cfg = config(&[(PAYSAFE, "id")]);
        assert!(cfg.keys_for(ADYEN).is_empty());
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
        let out = mask_json_body(&format!(r#""{PAN}""#), PAYSAFE, &cfg);
        assert_eq!(out, r#""***""#);
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

    #[test]
    fn deserializes_from_the_toml_shape_used_in_config_files() {
        let toml = r#"
            enabled = true
            log_to_span = false
            [connector_keys]
            paysafe = "id,status,authCode"
            adyen = "pspReference,resultCode"
        "#;
        let cfg: ConnectorResponseMaskingConfig =
            toml::from_str(toml).expect("config section must deserialize");

        assert!(cfg.enabled);
        assert!(!cfg.log_to_span);
        assert!(cfg.keys_for(PAYSAFE).names_key("authcode"));
        assert!(cfg.keys_for(ADYEN).names_key("pspreference"));
        assert!(cfg.keys_for("stripe").is_empty());
    }

    #[test]
    fn config_accepts_a_name_from_any_connector_enum() {
        let toml = r#"
            enabled = true
            [connector_keys]
            adyen = "resultcode"
            interpayments = "id"
            deutschebank = "id"
            kount = "id"
            plaid = "id"
        "#;
        let cfg: ConnectorResponseMaskingConfig =
            toml::from_str(toml).expect("every flow family's names must load");

        for name in ["adyen", "interpayments", "deutschebank", "kount", "plaid"] {
            assert!(!cfg.keys_for(name).is_empty(), "`{name}` did not resolve");
        }
    }

    #[test]
    fn a_patch_takes_effect_without_any_rebuild_step() {
        let mut cfg = config(&[(PAYSAFE, "id")]);
        assert!(cfg.keys_for(PAYSAFE).names_key("id"));

        cfg.apply(ConnectorResponseMaskingConfigPatch {
            enabled: None,
            log_to_span: None,
            connector_keys: Some(
                [(PAYSAFE.into(), "status".to_string())]
                    .into_iter()
                    .collect(),
            ),
        });

        let keys = cfg.keys_for(PAYSAFE);
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

    #[test]
    fn an_unknown_connector_name_is_rejected_at_load() {
        let toml = r#"
            enabled = true
            [connector_keys]
            paysafee = "id,status"
        "#;
        let err = toml::from_str::<ConnectorResponseMaskingConfig>(toml)
            .expect_err("an unknown connector name must not deserialize");
        assert!(
            err.to_string().contains("paysafee"),
            "error should name the bad key: {err}"
        );
    }

    #[test]
    fn defaults_are_off() {
        let cfg = ConnectorResponseMaskingConfig::default();
        assert!(!cfg.enabled);
        assert!(!cfg.log_to_span);
        assert!(cfg.connector_keys.is_empty());
    }

    #[test]
    fn an_unknown_connector_in_an_override_never_fails_deserialization() {
        // The same name that aborts startup above must not error here: this path is the
        // `x-config-override` header, and an error would reject the request before the handler
        // runs, so a typo in a diagnostic setting would cost a payment.
        let patch: ConnectorResponseMaskingConfigPatch =
            serde_json::from_str(r#"{"enabled":true,"connector_keys":{"paysafee":"id"}}"#)
                .expect("a per-request override must never fail to deserialize");

        let mut cfg = ConnectorResponseMaskingConfig::default();
        cfg.apply(patch);

        assert!(
            !cfg.enabled,
            "the unknown name should discard the whole block"
        );
        assert!(cfg.connector_keys.is_empty());
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
    fn an_override_mixing_known_and_unknown_applies_nothing() {
        // Whole-block, not per-entry: a half-applied override is harder to reason about than none.
        let patch: ConnectorResponseMaskingConfigPatch = serde_json::from_str(
            r#"{"enabled":true,"connector_keys":{"adyen":"resultCode","paysafee":"id"}}"#,
        )
        .expect("must still deserialize");

        let mut cfg = ConnectorResponseMaskingConfig::default();
        cfg.apply(patch);

        assert!(!cfg.enabled);
        assert!(cfg.connector_keys.is_empty());
    }
}
