//! Builds `masked_connector_response`: the connector's reply with every key preserved and every
//! value masked unless that connector's configured list names it. Emitted in the same format the
//! gateway used — JSON, XML or form-encoded.

use std::collections::{HashMap, HashSet};

use quick_xml::events::{BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer};
use serde::ser::{SerializeMap, SerializeSeq};
use serde::{Deserialize, Serialize, Serializer};
use serde_json::Value;

use common_utils::config_patch::Patch;

use crate::connector_types::{
    AuthenticatorConnectorEnum, ConnectorEnum, FrmConnectorEnum, PayoutConnectorEnum,
    SurchargeConnectorEnum,
};

/// Replacement written in place of a masked value.
pub const MASKED: &str = "***";

/// Per-connector configuration controlling which response keys keep their value.
///
/// No global list: a field name safe on one gateway is not necessarily safe on another.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct ConnectorResponseMaskingConfig {
    /// Whether to populate `masked_connector_response` at all.
    pub enabled: bool,

    /// Whether to *also* record the masked view on the outgoing span. Separate from
    /// [`Self::enabled`] so the caller can be sent the field without a copy being retained in our
    /// own logs, keeping a mistaken allowlist entry contained to whoever configured it.
    pub log_to_span: bool,

    /// Connector name -> comma-separated list of keys whose values stay visible.
    ///
    /// Comma-separated rather than a list because that is the only shape settable per connector
    /// from the environment. Keyed by name rather than a typed enum — see [`is_known_connector`].
    #[serde(deserialize_with = "deserialize_connector_keys")]
    pub connector_keys: HashMap<Box<str>, String>,
}

/// Whether any connector enum recognises this snake_case name.
///
/// Ingress resolves a connector per flow family — `x-connector` against [`ConnectorEnum`],
/// `x-payout-connector` against [`PayoutConnectorEnum`], and so on
/// (`ucs_interface_common::metadata::connector_variant_from_metadata`). No single enum spans all
/// five, so validating against one would reject real connectors: `interpayments`, `deutschebank`
/// and `plaid` have no [`ConnectorEnum`] counterpart.
fn is_known_connector(name: &str) -> bool {
    use std::str::FromStr;

    ConnectorEnum::from_str(name).is_ok()
        || SurchargeConnectorEnum::from_str(name).is_ok()
        || PayoutConnectorEnum::from_str(name).is_ok()
        || FrmConnectorEnum::from_str(name).is_ok()
        || AuthenticatorConnectorEnum::from_str(name).is_ok()
}

/// Validate map keys via `FromStr`, not the serde derive: strum's `snake_case` applies to
/// `FromStr` only, and the config crate lowercases env-var keys. Same route as
/// `WebhookSourceVerificationCall`.
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
    /// Keys whose values stay visible for `connector_name`. Built per request rather than cached,
    /// so a runtime config patch can never leave a stale set behind. No entry yields an empty set,
    /// as does an unrecognised name — masking every value is right either way.
    pub fn keys_for(&self, connector_name: &str) -> HashSet<Box<str>> {
        self.connector_keys
            .get(connector_name)
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
    /// See [`ConnectorResponseMaskingConfig::log_to_span`].
    pub log_to_span: Option<bool>,
    /// See [`ConnectorResponseMaskingConfig::connector_keys`].
    #[serde(default, deserialize_with = "deserialize_optional_connector_keys")]
    pub connector_keys: Option<HashMap<Box<str>, String>>,
}

fn deserialize_optional_connector_keys<'de, D>(
    deserializer: D,
) -> Result<Option<HashMap<Box<str>, String>>, D::Error>
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
        if let Some(log_to_span) = patch.log_to_span {
            self.log_to_span = log_to_span;
        }
        if let Some(connector_keys) = patch.connector_keys {
            self.connector_keys = connector_keys;
        }
        // Nothing derived to rebuild — the next request reads the new lists directly.
    }
}

/// Never revealed regardless of config. Substring match after stripping non-alphanumerics, so
/// `card_number`, `cardNumber` and `ssl_card_number` all match `cardnumber`.
///
/// Scope is full PAN, CVV, expiry and credentials — **not** every card-derived field. Truncated
/// values such as `cardSummary`, `last4` and `cardBin` are deliberately absent: they are not PAN,
/// they appear on receipts, and a connector that needs them for reconciliation can name them in
/// its own list.
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

/// Never revealed, matched **exactly**. `authorization` is here rather than above so it does not
/// also block `authorizationCode`, which operators legitimately reveal.
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

/// Whether the connector's configured list names this key.
fn in_allowlist(keys: &HashSet<Box<str>>, key: &str) -> bool {
    keys.contains(key.to_ascii_lowercase().as_str())
        // XML names may be prefixed (`s:authCode`); accept the local name too.
        || key
            .rsplit_once(':')
            .is_some_and(|(_, local)| keys.contains(local.to_ascii_lowercase().as_str()))
}

/// Whether this key's scalar value keeps its value.
///
/// Allowlist first: the denylist only ever overrides a key the allowlist would have revealed, so
/// the keys it rejects are masked either way. Checking membership first skips the substring scan
/// for the large majority of fields.
fn allowed(keys: &HashSet<Box<str>>, key: &str) -> bool {
    in_allowlist(keys, key) && !is_always_masked(key)
}

/// Namespace declarations are structural: masking them would break prefix resolution, and they
/// never carry secrets.
fn is_namespace_declaration(name: &str) -> bool {
    name == "xmlns" || name.starts_with("xmlns:")
}

/// Serializes a [`Value`], substituting `"***"` for scalars whose key is not allowed. `mask`
/// carries the parent key's decision so array elements inherit it.
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

/// Rebuild a tag, masking attribute values whose name is not allowed. Values are unescaped first
/// because `push_attribute` re-escapes.
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
                // Whitespace between elements is layout, not data — never mask it.
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

fn mask_form(body: &[u8], keys: &HashSet<Box<str>>) -> Option<String> {
    // Some connectors (Fiuu) separate pairs with newlines rather than `&`. Left as-is, urlencoded
    // parsing folds the entire body into the first pair's value, so an allowlisted first key would
    // reveal every later line. Only literal newline bytes are rewritten, so a percent-encoded
    // `%0A` inside a genuine form value is untouched.
    let normalised: Vec<u8> = body
        .iter()
        .map(|byte| match byte {
            b'\n' | b'\r' => b'&',
            other => *other,
        })
        .collect();

    // A Vec rather than a map so repeated keys survive.
    let pairs: Vec<(String, String)> = serde_urlencoded::from_bytes(&normalised).ok()?;
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

/// Mask `body` for `connector_name` and re-emit it in the same format.
///
/// Returns `None` when masking is disabled or the body is empty. A body that cannot be parsed
/// yields a labelled stub carrying only its size — never its content.
pub fn mask_connector_response(
    body: &[u8],
    content_type: Option<&str>,
    connector_name: &str,
    config: &ConnectorResponseMaskingConfig,
) -> Option<String> {
    if !config.enabled || body.is_empty() {
        return None;
    }

    // Some connectors (Authorize.Net) prefix responses with a UTF-8 BOM, which every parser below
    // rejects. The connector strips it in `preprocess_response_bytes`, but that runs inside
    // `handle_response_v2` — after this point. Must precede `detect`: a BOM makes the first
    // meaningful byte `0xEF`, so sniffing would misroute before reaching the `{`.
    let body = common_utils::bytes_utils::strip_utf8_bom(body);

    let keys = config.keys_for(connector_name);

    let masked = match detect(content_type, body) {
        Some(Format::Json) => mask_json(body, &keys),
        Some(Format::Xml) => mask_xml(body, &keys),
        Some(Format::Form) => mask_form(body, &keys),
        None => None,
    };

    // Emitted whole: the full body is the point, so there is no truncation.
    Some(masked.unwrap_or_else(|| format!(r#"{{"_format":"unparsable","_bytes":{}}}"#, body.len())))
}
