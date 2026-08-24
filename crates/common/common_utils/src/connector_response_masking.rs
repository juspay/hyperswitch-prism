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
