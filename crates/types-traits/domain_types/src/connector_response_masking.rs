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

    /// Whether the masked view may reach our own logs at all. Separate from [`Self::enabled`] so
    /// the caller can be sent the field without a copy being retained here, keeping a mistaken
    /// allowlist entry contained to whoever configured it.
    ///
    /// While this is off the value is stripped from every gRPC-level log sink, not just the
    /// dedicated `response.masked_body` span field: it is also removed from `response_body` and
    /// from the event payload, both of which otherwise serialize the whole response
    /// (`grpc-server::utils::response_for_logging`). Being a plain `String` rather than a
    /// `Secret<String>`, it has no type-level masking of its own to fall back on.
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

/// Validate map keys via `FromStr`, not the serde derive.
///
/// `#[strum(serialize_all = "snake_case")]` governs `FromStr` *and* the derived `Display` — which
/// is why `get_connector_name()` yields `adyen` and a lowercase config key resolves at all. It does
/// not reach serde: the connector enums carry no `#[serde(rename_all)]`, so their serde derive
/// would demand `Adyen`, while both TOML keys and the config crate's env-var keys arrive
/// lowercased (`CS__…__CONNECTOR_KEYS__ADYEN` → `adyen`). Same route as
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

/// Serializes a [`Value`], substituting `"***"` for scalars whose key is not allowed.
///
/// `mask` is the decision for a value that has no key of its own — the body's root, and every
/// array element. It is only ever cleared by an object entry whose key the allowlist names, so
/// allowing a key reveals that key's own scalar and nothing else: objects beneath it re-decide
/// per key, arrays beneath it stay masked.
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
            // Elements carry no key of their own, so an operator can never name one. Inheriting
            // the parent's decision would let a single allowed key reveal every scalar beneath
            // it, which is the invariant the object arm above upholds.
            Value::Array(items) => {
                let mut state = serializer.serialize_seq(Some(items.len()))?;
                for value in items {
                    state.serialize_element(&Self {
                        value,
                        keys: self.keys,
                        mask: true,
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
        // The root has no key to gate on — a bare scalar or an array of scalars must not be
        // revealed just because nothing named it. An object re-decides per key immediately.
        value: &value,
        keys,
        mask: true,
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
            // The XML declaration is structural — version and encoding, never data.
            Event::Decl(declaration) => {
                writer.write_event(Event::Decl(declaration)).ok()?;
            }
            // Comments are free text with no element name to gate on, so a gateway that echoes
            // the request into one would leak it. Keep the fact that a comment was there;
            // discard what it said.
            Event::Comment(_) => {
                writer
                    .write_event(Event::Comment(BytesText::new(MASKED)))
                    .ok()?;
            }
            // Processing instructions and DOCTYPE (whose internal subset can carry entity
            // values) are dropped. Fail-closed: anything this match does not recognise —
            // including a variant a future `quick-xml` adds — is dropped rather than forwarded.
            _ => {}
        }
    }

    String::from_utf8(writer.into_inner()).ok()
}

/// Whether `body` is genuinely a sequence of `key=value` pairs.
///
/// `serde_urlencoded` never *fails* on arbitrary bytes: a segment with no `=` parses as
/// `(whole_segment, "")`. Keys are emitted verbatim by design, so without this check a body that
/// is not form-encoded at all comes back out as one giant unmasked key — a `text/plain` decline
/// message or a CSV row would be re-emitted in full. Requiring every segment to carry a non-empty
/// key sends those to the size-only stub instead.
///
/// Deliberately all-or-nothing: a real form body with one valueless segment (`a=1&flag&b=2`) is
/// stubbed whole rather than partly emitted. No gateway is known to send that shape, and losing a
/// diagnostic is the cheaper failure here — relax it if one turns up.
fn is_pair_shaped(body: &[u8]) -> bool {
    let mut saw_pair = false;
    for segment in body.split(|byte| *byte == b'&') {
        // A trailing or doubled separator is not a pair either way.
        if segment.is_empty() {
            continue;
        }
        match segment.iter().position(|byte| *byte == b'=') {
            // No `=` at all, or nothing before it: no key to keep.
            None | Some(0) => return false,
            Some(_) => saw_pair = true,
        }
    }
    saw_pair
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

    // Checked after normalisation so a newline-separated body is judged on its real pairs, and
    // before parsing because parsing is what silently accepts a non-form body.
    if !is_pair_shaped(&normalised) {
        return None;
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    const PAYSAFE: &str = "paysafe";
    const ADYEN: &str = "adyen";
    const ELAVON: &str = "elavon";
    const PAYU: &str = "payu";
    const FIUU: &str = "fiuu";

    /// A PAN that must never appear in any output below. Asserting on its absence is the
    /// property under test; asserting on the exact shape only pins how it is spelt.
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

    /// The size-only stub emitted when a body matches no structured format.
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

    // -- key set -----------------------------------------------------------

    #[test]
    fn keys_for_splits_trims_and_lowercases() {
        let cfg = config(&[(PAYSAFE, " id , authCode ,, MerchantRefNum ")]);
        let keys = cfg.keys_for(PAYSAFE);
        assert_eq!(keys.len(), 3);
        assert!(keys.contains("id"));
        assert!(keys.contains("authcode"));
        assert!(keys.contains("merchantrefnum"));
    }

    #[test]
    fn unknown_connector_yields_an_empty_set() {
        let cfg = config(&[(PAYSAFE, "id")]);
        assert!(cfg.keys_for(ADYEN).is_empty());
    }

    // -- JSON: keyed values ------------------------------------------------

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
        // `authorization` is exact-match only: substring-matching it would permanently block
        // `authorizationCode`, which connectors return and operators legitimately reveal.
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

    // -- JSON: the invariant that allowing a key never reveals a subtree ----

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
        // Elements carry no key of their own, so an operator can never name them. Allowing
        // `success` reveals the scalar it names, not everything nested beneath it.
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

    // -- JSON: bodies with no key to gate on -------------------------------

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
        // `[` sniffs as JSON, so a missing Content-Type takes the same path.
        let body = format!(r#"["{PAN}"]"#);
        let out = mask_connector_response(body.as_bytes(), None, PAYSAFE, &config(&[])).unwrap();
        assert_eq!(out, r#"["***"]"#);
    }

    // -- XML ---------------------------------------------------------------

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
    fn xml_masks_attribute_values_and_keeps_namespaces() {
        let cfg = config(&[(ELAVON, "id")]);
        let out = mask_xml_body(
            &format!(r#"<root xmlns:s="urn:x"><item id="42" pan="{PAN}"/></root>"#),
            ELAVON,
            &cfg,
        );

        assert!(out.contains(r#"xmlns:s="urn:x""#), "namespace kept: {out}");
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
        // A gateway that echoes the request inside a comment must not leak it.
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

    // -- form-urlencoded ---------------------------------------------------

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
        // Fiuu separates pairs with newlines; left as-is, urlencoded parsing folds the whole
        // body into the first pair's value.
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

    // -- bodies that match no structured format ----------------------------

    #[test]
    fn a_plain_text_body_is_not_re_emitted_as_a_form_key() {
        // Keys are emitted verbatim by design, so a body that parses as one giant key would
        // come back out in the clear.
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
        // The declared Content-Type is not enough: the body still has to be pair-shaped.
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

    // -- detection and framing ---------------------------------------------

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
        // Authorize.Net prefixes responses with a UTF-8 BOM; every parser below rejects it, and
        // it would also misroute the sniff.
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

    // -- config ------------------------------------------------------------

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
        assert!(cfg.keys_for(PAYSAFE).contains("authcode"));
        assert!(cfg.keys_for(ADYEN).contains("pspreference"));
        assert!(cfg.keys_for("stripe").is_empty());
    }

    #[test]
    fn config_accepts_a_name_from_any_connector_enum() {
        // No single enum spans all five flow families, so validating against one would reject
        // real connectors.
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
    fn an_unknown_connector_name_is_rejected_at_load() {
        // A typo must abort startup rather than silently mask everything.
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
    fn a_patch_takes_effect_without_any_rebuild_step() {
        let mut cfg = config(&[(PAYSAFE, "id")]);
        assert!(cfg.keys_for(PAYSAFE).contains("id"));

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
        assert!(keys.contains("status"));
        assert!(!keys.contains("id"), "stale key survived the patch");
    }
}
