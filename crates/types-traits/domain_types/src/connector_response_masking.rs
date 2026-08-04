//! Selective, per-connector masking of the raw connector response.
//!
//! `raw_connector_response` is a `Secret<String>`, so any logger collapses the whole body into a
//! single placeholder. This module produces the sibling `unmasked_connector_response`: the same
//! body with **every key preserved** and **every value masked** unless that connector's configured
//! list names it.
//!
//! The output is emitted in the **same format** the gateway used — JSON in, JSON out; XML in, XML
//! out; form-encoded in, form-encoded out. The only thing the three paths share is the
//! per-connector key set.

use std::collections::{HashMap, HashSet};

use quick_xml::events::{BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer};
use serde::ser::{SerializeMap, SerializeSeq};
use serde::{Deserialize, Serialize, Serializer};
use serde_json::Value;

use common_utils::config_patch::Patch;

use crate::connector_types::ConnectorEnum;

/// Replacement written in place of a masked value.
pub const MASKED: &str = "***";

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Per-connector configuration controlling which response keys keep their value.
///
/// There is deliberately no global key list: a field name that is safe on one gateway is not
/// necessarily safe on another. A connector with no entry gets every value masked, with every key
/// still visible.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct ConnectorResponseMaskingConfig {
    /// Whether to populate `unmasked_connector_response` at all.
    pub enabled: bool,

    /// Connector -> comma-separated list of keys whose values stay visible.
    ///
    /// Keyed by [`ConnectorEnum`] so an unknown name in TOML or env aborts startup naming the bad
    /// key, rather than silently masking everything.
    ///
    /// The *value* stays a comma-separated string because it is the only shape that can be set per
    /// connector from the environment: env vars are always text, and the config crate only splits
    /// a key it has been told about by literal path — which cannot be done for an open-ended set
    /// of connectors. This is plain deserialized TOML, not a cache; nothing is derived ahead of
    /// time.
    #[serde(
        deserialize_with = "deserialize_connector_keys",
        serialize_with = "serialize_connector_keys"
    )]
    pub connector_keys: HashMap<ConnectorEnum, String>,
}

/// Parse map keys through `ConnectorEnum`'s `FromStr` rather than its serde derive.
///
/// `#[strum(serialize_all = "snake_case")]` governs `FromStr`/`Display`; serde would instead expect
/// the PascalCase variant names. Two reasons that matters: config files spell connectors in
/// lowercase, and the config crate lowercases environment-variable keys, so
/// `CS__…__CONNECTOR_KEYS__ADYEN` arrives as `adyen` and could never match `Adyen`.
///
/// This is the same route `WebhookSourceVerificationCall` takes (`deserialize_hashset`).
fn deserialize_connector_keys<'de, D>(
    deserializer: D,
) -> Result<HashMap<ConnectorEnum, String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    use std::str::FromStr;

    HashMap::<String, String>::deserialize(deserializer)?
        .into_iter()
        .map(|(name, keys)| {
            ConnectorEnum::from_str(&name.to_lowercase())
                .map(|connector| (connector, keys))
                .map_err(|_| D::Error::custom(format!("unknown connector `{name}`")))
        })
        .collect()
}

/// Mirror of [`deserialize_connector_keys`] — emit the snake_case name, not the variant name.
fn serialize_connector_keys<S>(
    connector_keys: &HashMap<ConnectorEnum, String>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    connector_keys
        .iter()
        .map(|(connector, keys)| (connector.to_string(), keys))
        .collect::<HashMap<_, _>>()
        .serialize(serializer)
}

impl ConnectorResponseMaskingConfig {
    /// Build the set of keys whose values stay visible for `connector`.
    ///
    /// Called once per request, for the single connector in play — a split plus a handful of small
    /// allocations, against a gateway call measured in hundreds of milliseconds. Building on demand
    /// rather than caching means a runtime config patch can never leave a stale set behind.
    ///
    /// A connector with no entry yields an empty set: every value masked, every key still visible.
    pub fn keys_for(&self, connector: &ConnectorEnum) -> HashSet<Box<str>> {
        self.connector_keys
            .get(connector)
            .map(|keys| {
                keys.split(',')
                    .map(str::trim)
                    .filter(|key| !key.is_empty())
                    .map(|key| key.to_lowercase().into_boxed_str())
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// Partial override for [`ConnectorResponseMaskingConfig`].
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct ConnectorResponseMaskingConfigPatch {
    /// See [`ConnectorResponseMaskingConfig::enabled`].
    pub enabled: Option<bool>,
    /// See [`ConnectorResponseMaskingConfig::connector_keys`].
    #[serde(default, deserialize_with = "deserialize_optional_connector_keys")]
    pub connector_keys: Option<HashMap<ConnectorEnum, String>>,
}

fn deserialize_optional_connector_keys<'de, D>(
    deserializer: D,
) -> Result<Option<HashMap<ConnectorEnum, String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_connector_keys(deserializer).map(Some)
}

impl Patch<ConnectorResponseMaskingConfigPatch> for ConnectorResponseMaskingConfig {
    fn apply(&mut self, patch: ConnectorResponseMaskingConfigPatch) {
        if let Some(enabled) = patch.enabled {
            self.enabled = enabled;
        }
        if let Some(connector_keys) = patch.connector_keys {
            self.connector_keys = connector_keys;
        }
        // Nothing derived to rebuild — the next request reads the new lists directly.
    }
}

// ---------------------------------------------------------------------------
// Key policy
// ---------------------------------------------------------------------------

/// Keys never revealed whatever the configuration says, matched as a **substring** after stripping
/// non-alphanumerics — so `card_number`, `cardNumber`, `card-number` and `ssl_card_number` all
/// match `cardnumber`. Every entry here must be safe to match mid-word.
const ALWAYS_MASKED_SUBSTRING: &[&str] = &[
    "cardnumber",
    "cardnum",
    "accountnumber",
    "cvv",
    "cvc",
    "cvn",
    "expmonth",
    "expyear",
    "expirydate",
    "secret",
    "token",
    "password",
    "signature",
    "apikey",
];

/// Keys never revealed, matched **exactly**.
///
/// `authorization` is here rather than above because substring-matching it would also block
/// `authorizationCode` — a routine, non-sensitive field that connectors return and operators will
/// legitimately want visible. Blocking it would be unfixable from config, and would present as the
/// same "I configured it but it is still `***`" confusion this feature exists to remove.
const ALWAYS_MASKED_EXACT: &[&str] = &["authorization"];

/// Normalise a key for comparison: lowercase, alphanumerics only.
fn normalize(key: &str) -> String {
    key.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|character| character.to_ascii_lowercase())
        .collect()
}

/// Whether a key is on either never-reveal list.
fn is_always_masked(key: &str) -> bool {
    let normalized = normalize(key);
    ALWAYS_MASKED_EXACT
        .iter()
        .any(|needle| normalized == *needle)
        || ALWAYS_MASKED_SUBSTRING
            .iter()
            .any(|needle| normalized.contains(needle))
}

/// Whether this key's scalar value keeps its value.
fn allowed(keys: &HashSet<Box<str>>, key: &str) -> bool {
    if is_always_masked(key) {
        return false;
    }
    if keys.contains(key.to_ascii_lowercase().as_str()) {
        return true;
    }
    // XML names may be prefixed (`s:authCode`); configuring the local name is what a
    // reader expects, so accept that too.
    key.rsplit_once(':')
        .is_some_and(|(_, local)| keys.contains(local.to_ascii_lowercase().as_str()))
}

/// Namespace declarations are structural: masking them would break prefix resolution, and they
/// never carry secrets.
fn is_namespace_declaration(name: &str) -> bool {
    name == "xmlns" || name.starts_with("xmlns:")
}

// ---------------------------------------------------------------------------
// JSON — mask while serializing, never mutate the tree
// ---------------------------------------------------------------------------

/// Serializes a [`Value`], substituting `"***"` for scalars whose key is not allowed.
///
/// `mask` carries the decision made about the *parent key*, which is what lets array elements
/// inherit it — masking a tree in place would leave array scalars untouched, because they reach
/// the walker with no key in scope.
struct Masked<'a> {
    value: &'a Value,
    keys: &'a HashSet<Box<str>>,
    mask: bool,
}

impl Serialize for Masked<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.value {
            // Containers always recurse: allowing a key must never reveal a whole subtree.
            Value::Object(map) => {
                let mut state = serializer.serialize_map(Some(map.len()))?;
                for (key, value) in map {
                    state.serialize_entry(
                        key,
                        &Self {
                            value,
                            keys: self.keys,
                            mask: !allowed(self.keys, key),
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
                        mask: self.mask,
                    })?;
                }
                state.end()
            }
            // An explicit null carries no secret and is worth seeing.
            Value::Null => self.value.serialize(serializer),
            _ if self.mask => serializer.serialize_str(MASKED),
            other => other.serialize(serializer),
        }
    }
}

fn mask_json(body: &[u8], keys: &HashSet<Box<str>>) -> Option<String> {
    let value: Value = serde_json::from_slice(body).ok()?;
    serde_json::to_string(&Masked {
        value: &value,
        keys,
        mask: false,
    })
    .ok()
}

// ---------------------------------------------------------------------------
// XML — copy the event stream, rewrite only values
// ---------------------------------------------------------------------------

/// Rebuild a start/empty tag with the same name, masking attribute values whose name is not
/// allowed. Values are unescaped before being pushed back, because `push_attribute` re-escapes.
fn mask_attributes(tag: &BytesStart<'_>, keys: &HashSet<Box<str>>) -> BytesStart<'static> {
    let name = String::from_utf8_lossy(tag.name().as_ref()).into_owned();
    let mut rebuilt = BytesStart::new(name);
    for attribute in tag.attributes().flatten() {
        let key = String::from_utf8_lossy(attribute.key.as_ref()).into_owned();
        if is_namespace_declaration(&key) || allowed(keys, &key) {
            let value = attribute.unescape_value().unwrap_or_default();
            rebuilt.push_attribute((key.as_str(), value.as_ref()));
        } else {
            rebuilt.push_attribute((key.as_str(), MASKED));
        }
    }
    rebuilt
}

fn mask_xml(body: &[u8], keys: &HashSet<Box<str>>) -> Option<String> {
    let text = std::str::from_utf8(body).ok()?;
    let mut reader = Reader::from_str(text);
    // Lenient: this is a diagnostic artefact, not a validator.
    reader.check_end_names(false);

    let mut writer = Writer::new(Vec::new());
    // The element currently open; a Text/CData event belongs to it.
    let mut current: Option<String> = None;

    loop {
        match reader.read_event().ok()? {
            Event::Eof => break,
            Event::Start(tag) => {
                current = Some(String::from_utf8_lossy(tag.name().as_ref()).into_owned());
                writer
                    .write_event(Event::Start(mask_attributes(&tag, keys)))
                    .ok()?;
            }
            Event::Empty(tag) => {
                writer
                    .write_event(Event::Empty(mask_attributes(&tag, keys)))
                    .ok()?;
            }
            Event::End(tag) => {
                current = None;
                writer.write_event(Event::End(tag)).ok()?;
            }
            Event::Text(text) => {
                // Whitespace between elements is layout, not data — never mask it, or
                // pretty-printed XML turns into a wall of markers.
                let keep = text.iter().all(u8::is_ascii_whitespace)
                    || current.as_deref().is_some_and(|name| allowed(keys, name));
                if keep {
                    writer.write_event(Event::Text(text)).ok()?;
                } else {
                    writer
                        .write_event(Event::Text(BytesText::new(MASKED)))
                        .ok()?;
                }
            }
            Event::CData(data) => {
                if current.as_deref().is_some_and(|name| allowed(keys, name)) {
                    writer.write_event(Event::CData(data)).ok()?;
                } else {
                    writer
                        .write_event(Event::Text(BytesText::new(MASKED)))
                        .ok()?;
                }
            }
            // Declaration, comments, processing instructions, doctype: structural, copied as-is.
            other => {
                writer.write_event(other).ok()?;
            }
        }
    }

    String::from_utf8(writer.into_inner()).ok()
}

// ---------------------------------------------------------------------------
// Form-urlencoded
// ---------------------------------------------------------------------------

fn mask_form(body: &[u8], keys: &HashSet<Box<str>>) -> Option<String> {
    // A Vec rather than a map so repeated keys survive.
    let pairs: Vec<(String, String)> = serde_urlencoded::from_bytes(body).ok()?;
    let masked = pairs
        .into_iter()
        .map(|(key, value)| {
            if allowed(keys, &key) {
                (key, value)
            } else {
                (key, MASKED.to_string())
            }
        })
        .collect::<Vec<_>>();
    serde_urlencoded::to_string(masked).ok()
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Wire format of a connector response body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Json,
    Xml,
    Form,
}

/// Prefer the declared `Content-Type`; fall back to sniffing the first meaningful byte.
fn detect(content_type: Option<&str>, body: &[u8]) -> Option<Format> {
    if let Some(content_type) = content_type {
        let lowered = content_type.to_ascii_lowercase();
        if lowered.contains("json") {
            return Some(Format::Json);
        }
        if lowered.contains("xml") || lowered.contains("soap") {
            return Some(Format::Xml);
        }
        if lowered.contains("x-www-form-urlencoded") {
            return Some(Format::Form);
        }
    }

    match body.iter().find(|byte| !byte.is_ascii_whitespace()) {
        Some(b'{' | b'[') => Some(Format::Json),
        Some(b'<') => Some(Format::Xml),
        Some(_) => Some(Format::Form),
        None => None,
    }
}

/// Mask `body` for `connector` and re-emit it in the same format.
///
/// Returns `None` when masking is disabled or the body is empty. A body that cannot be parsed
/// yields a labelled stub carrying only its size — never its content.
pub fn mask_connector_response(
    body: &[u8],
    content_type: Option<&str>,
    connector: &ConnectorEnum,
    config: &ConnectorResponseMaskingConfig,
) -> Option<String> {
    if !config.enabled || body.is_empty() {
        return None;
    }

    // Built here, for this connector only. Empty if it has no configured list, which still shows
    // every key with every value masked.
    let keys = config.keys_for(connector);

    let masked = match detect(content_type, body) {
        Some(Format::Json) => mask_json(body, &keys),
        Some(Format::Xml) => mask_xml(body, &keys),
        Some(Format::Form) => mask_form(body, &keys),
        None => None,
    };

    // Emitted whole: the full body is the point, so there is no truncation.
    Some(masked.unwrap_or_else(|| format!(r#"{{"_format":"unparsable","_bytes":{}}}"#, body.len())))
}
