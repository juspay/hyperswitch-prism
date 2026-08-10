//! Builds `masked_connector_response`: the connector's reply with every key preserved and every
//! value masked unless that connector's configured list names it. Emitted in the same format the
//! gateway used — JSON, XML or form-encoded.
//!
//! Every input here is gateway-controlled, and this must never be able to fail a payment, so the
//! module opts itself into the panic lints. `domain_types` has no `[lints] workspace = true`
//! stanza, unlike its sibling crates, so without this the file sits outside the checks CI enforces
//! as errors everywhere else. Scoped to this module rather than the crate because `types.rs` alone
//! is ~17k lines and cleaning it is separate work.
#![warn(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::as_conversions,
    clippy::unreachable
)]

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
            // Dotted entries land here too — see the field's documentation.
            keys.names.insert(entry.into_boxed_str());
        }

        keys
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
    // Track data carries the PAN, the expiry and the discretionary data together. Also masks
    // `trackingNumber`, which is over-masking we accept.
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

/// Never revealed, matched **exactly**. Two reasons a name lands here rather than above:
///
/// - `authorization` would otherwise also block `authorizationCode`, which operators legitimately
///   reveal.
/// - The rest are too short to match as substrings without catching innocent words — `pan` occurs
///   inside `company` and `japan`, `pin` inside `shipping`, `cid` inside `acidic`.
///
/// `pan` matters most: `peachpayments` names its card-number field exactly that, so without this
/// entry a single allowlist line would hand back a full PAN.
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

/// The keys one connector's configuration unmasks.
///
/// Two kinds of entry, because operators need both. A bare name — `authcode` — unmasks that key
/// wherever it appears, at any depth. A dotted entry — `card.last4` — pins one exact location,
/// counted from the body's root, which is how an operator reveals a generically-named field
/// without also revealing every namesake elsewhere in the body.
#[derive(Debug, Default, Clone)]
pub struct MaskKeys {
    /// Bare entries, matched against a single key name at any depth.
    ///
    /// Dotted entries are inserted here verbatim as well, because some gateways genuinely return
    /// key names containing dots — Adyen's `bankAccount.iban` and `retry.attempt1.rawResponse` are
    /// flat keys, not nesting. An entry that pins no path can still name one of those.
    names: HashSet<Box<str>>,
    /// Dotted entries split into segments, matched against the whole path from the root.
    paths: Vec<Box<[Box<str>]>>,
}

impl MaskKeys {
    /// Whether a bare entry names this key.
    fn names_key(&self, key: &str) -> bool {
        self.names.contains(key.to_ascii_lowercase().as_str())
            // XML names may be prefixed (`s:authCode`); accept the local name too.
            || key.rsplit_once(':').is_some_and(|(_, local)| {
                self.names.contains(local.to_ascii_lowercase().as_str())
            })
    }

    /// Whether a dotted entry pins exactly this location, given as segments from the root.
    fn names_location(&self, segments: &[&str]) -> bool {
        self.paths.iter().any(|entry| {
            entry.len() == segments.len()
                && entry
                    .iter()
                    .zip(segments)
                    .all(|(want, have)| have.eq_ignore_ascii_case(want))
        })
    }

    /// Whether this connector unmasks nothing at all.
    #[cfg(test)]
    fn is_empty(&self) -> bool {
        self.names.is_empty() && self.paths.is_empty()
    }

    /// Whether any dotted entry is configured at all. Lets the maskers skip position tracking
    /// entirely for the common case of a name-only configuration.
    fn has_paths(&self) -> bool {
        !self.paths.is_empty()
    }
}

/// Where a key sits in a JSON body, as a chain back to the root.
///
/// A chain rather than a `Vec` so descending a level allocates nothing; matching walks up, which
/// is the order the segments are already in.
struct Path<'a> {
    parent: Option<&'a Path<'a>>,
    segment: &'a str,
}

impl Path<'_> {
    /// Whether this location is exactly `segments`, read from the root.
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
        // Anything left above means the entry named a deeper location than this one.
        here.is_none()
    }
}

/// Whether the key *itself* looks like card data rather than a field name.
///
/// Keys are emitted verbatim in every format — that is what makes the output readable — so a
/// gateway that puts the value in key position, `{"declines":{"<PAN>":"expired"}}`, would echo it
/// however well the values are masked. A long run of digits is the one key shape worth refusing:
/// no real field name is a dozen consecutive digits, so the false-positive cost is nil.
fn key_looks_like_card_data(key: &str) -> bool {
    key.chars().filter(char::is_ascii_digit).count() >= 12
        && key
            .chars()
            .all(|character| character.is_ascii_digit() || matches!(character, ' ' | '-' | '_'))
}

/// The key as it should appear in the output.
fn emitted_key(key: &str) -> &str {
    if key_looks_like_card_data(key) {
        MASKED
    } else {
        key
    }
}

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
///
/// `pinned` is whether a dotted entry names this exact location. The caller supplies it because
/// JSON and XML track position differently — a chain for one, an element stack for the other —
/// and form bodies are flat, so there is no position to speak of.
///
/// Allowlist first: the denylist only ever overrides a key the allowlist would have revealed, so
/// the keys it rejects are masked either way. Checking membership first skips the substring scan
/// for the large majority of fields.
fn allowed(keys: &MaskKeys, key: &str, pinned: bool) -> bool {
    (pinned || keys.names_key(key)) && !is_always_masked(key)
}

/// Serializes a [`Value`], substituting `"***"` for scalars whose key is not allowed.
///
/// `mask` is the decision for a value that has no key of its own — the body's root, and every
/// array element. It is only ever cleared by an object entry whose key the allowlist names, so
/// allowing a key reveals that key's own scalar and nothing else: objects beneath it re-decide
/// per key, arrays beneath it stay masked.
struct Masked<'a> {
    value: &'a Value,
    keys: &'a MaskKeys,
    /// Where this value sits, for dotted entries. `None` at the root.
    at: Option<&'a Path<'a>>,
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
                    let here = Path {
                        parent: self.at,
                        segment: key,
                    };
                    // Only pay for position tracking when a dotted entry could use it.
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
            // Elements carry no key of their own, so an operator can never name one. Inheriting
            // the parent's decision would let a single allowed key reveal every scalar beneath
            // it, which is the invariant the object arm above upholds.
            Value::Array(items) => {
                let mut state = serializer.serialize_seq(Some(items.len()))?;
                for value in items {
                    state.serialize_element(&Self {
                        value,
                        keys: self.keys,
                        // An element has no name, so it cannot extend a dotted entry's path.
                        at: self.at,
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

fn mask_json(body: &[u8], keys: &MaskKeys) -> Option<String> {
    let value: Value = serde_json::from_slice(body).ok()?;
    serde_json::to_string(&Masked {
        // The root has no key to gate on — a bare scalar or an array of scalars must not be
        // revealed just because nothing named it. An object re-decides per key immediately.
        value: &value,
        keys,
        at: None,
        mask: true,
    })
    .ok()
}

/// Rebuild a tag, masking attribute values whose name is not allowed. Values are unescaped first
/// because `push_attribute` re-escapes.
///
/// `xmlns:*` declarations get no exemption. They were once passed through untouched, reasoning
/// that masking them breaks prefix resolution and that they never carry secrets — but the name is
/// gateway-controlled, so `xmlns:cardnumber="<PAN>"` sailed past the denylist with an empty
/// allowlist. A masked namespace URI costs nothing here: this output is a diagnostic artefact that
/// nothing re-parses.
fn mask_attributes(tag: &BytesStart<'_>, keys: &MaskKeys) -> BytesStart<'static> {
    let name = String::from_utf8_lossy(tag.name().as_ref()).into_owned();
    let mut rebuilt = BytesStart::new(name);
    for attribute in tag.attributes().flatten() {
        let key = String::from_utf8_lossy(attribute.key.as_ref()).into_owned();
        if allowed(keys, &key, false) {
            let value = attribute.unescape_value().unwrap_or_default();
            rebuilt.push_attribute((emitted_key(&key), value.as_ref()));
        } else {
            rebuilt.push_attribute((emitted_key(&key), MASKED));
        }
    }
    rebuilt
}

fn mask_xml(body: &[u8], keys: &MaskKeys) -> Option<String> {
    let text = std::str::from_utf8(body).ok()?;
    let mut reader = Reader::from_str(text);
    // Lenient: this is a diagnostic artefact, not a validator.
    reader.check_end_names(false);

    let mut writer = Writer::new(Vec::new());
    // The element currently open; a Text/CData event belongs to it.
    let mut current: Option<String> = None;
    // Elements still open, outermost first — the position a dotted entry pins.
    let mut open: Vec<String> = Vec::new();
    // Only the very first event may be a declaration; see the `Decl` arm.
    let mut first_event = true;

    // Whether a dotted entry names the element currently open.
    fn pinned(keys: &MaskKeys, open: &[String]) -> bool {
        keys.has_paths() && {
            let segments: Vec<&str> = open.iter().map(String::as_str).collect();
            keys.names_location(&segments)
        }
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
                // An empty element holds no text of its own, so anything following it belongs to
                // the parent. Leaving `current` alone would attribute that text to this element's
                // enclosing one: `<status>OK<pan/><PAN></status>` would inherit an allowed
                // `status`. There is no parent name to fall back to, so mask.
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
                // Whitespace between elements is layout, not data — never mask it.
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
            // The XML declaration is structural — version and encoding, never data — but only the
            // real one, at the top. `quick-xml` classifies *any* `<?xml …?>` as a declaration
            // wherever it appears, and writes it through verbatim, so a gateway emitting
            // `<r><?xml pan="<PAN>"?>` mid-document got a free pass. Later ones are dropped like
            // any other processing instruction.
            Event::Decl(declaration) if first_event => {
                writer.write_event(Event::Decl(declaration)).ok()?;
            }
            // Comments are free text with no element name to gate on, so a gateway that echoes
            // the request into one would leak it. Keep the fact that a comment was there;
            // discard what it said. Clears `current` for the same reason `Empty` does.
            Event::Comment(_) => {
                current = None;
                writer
                    .write_event(Event::Comment(BytesText::new(MASKED)))
                    .ok()?;
            }
            // Processing instructions and DOCTYPE (whose internal subset can carry entity
            // values) are dropped. Fail-closed: anything this match does not recognise —
            // including a variant a future `quick-xml` adds — is dropped rather than forwarded.
            _ => {}
        }
        first_event = false;
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
/// Bytes a form key may contain. Anything else — a space, comma, quote, brace, colon — means the
/// body is prose, JSON or some other payload that merely happens to contain an `=`, and keys are
/// emitted verbatim, so believing it would echo that payload in the clear.
fn is_form_key_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b'[' | b']' | b'%' | b'+')
}

/// Longest key we will believe. Real form keys are short; a long unbroken run of key-legal bytes is
/// a blob (base64, a JWT), not a key.
const MAX_FORM_KEY_LEN: usize = 128;

fn is_pair_shaped(body: &[u8]) -> bool {
    // Tracks a pair with a *non-empty value*, not merely a pair. Base64 ends in `=` padding, which
    // otherwise parses as one giant key with an empty value and gets echoed whole.
    let mut saw_value = false;
    for segment in body.split(|byte| *byte == b'&') {
        // A trailing or doubled separator is not a pair either way.
        if segment.is_empty() {
            continue;
        }
        // No `=` at all: no key to keep.
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
        // `value` still carries the `=` it was split on.
        if value.len() > 1 {
            saw_value = true;
        }
    }
    saw_value
}

fn mask_form(body: &[u8], keys: &MaskKeys) -> Option<String> {
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

/// Wire format of a connector response body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Json,
    Xml,
    Form,
}

/// Prefer the declared `Content-Type`; fall back to sniffing the first meaningful byte.
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
            // A declared form type does not override a body that plainly is not one. The form
            // masker emits keys verbatim, so a gateway mislabelling its JSON or XML would
            // otherwise get that body echoed in the clear.
            return Some(match sniffed {
                Format::Json | Format::Xml => sniffed,
                Format::Form => Format::Form,
            });
        }
    }

    Some(sniffed)
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
// Tests assert, so they are exempt from the module's panic lints.
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
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
    fn xml_masks_attribute_values_including_namespace_declarations() {
        // Namespace declarations used to be passed through untouched, on the reasoning that they
        // are structural and carry no data. The name is gateway-controlled, so that was a hole:
        // see `xml_a_namespace_declaration_cannot_smuggle_a_value` below. The prefix is still
        // visible; only the URI is masked, and nothing re-parses this output.
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
        assert!(cfg.keys_for(PAYSAFE).names_key("authcode"));
        assert!(cfg.keys_for(ADYEN).names_key("pspreference"));
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

    // -- bodies that are not really the format they were routed to ---------
    //
    // Keys are emitted verbatim, so believing a body is form-encoded when it is not echoes it.
    // Each of these once came back in the clear with no configuration at all.

    #[test]
    fn prose_containing_an_equals_sign_is_not_a_form_body() {
        let body = format!("Declined: card {PAN}, retry=true");
        let out = mask_connector_response(body.as_bytes(), None, PAYSAFE, &config(&[])).unwrap();
        assert!(!out.contains(PAN), "{out}");
        assert_is_stub(&out, body.len());
    }

    #[test]
    fn a_base64_blob_is_not_a_form_body() {
        // The `=` padding satisfies "has a key and an `=`", so only a non-empty value tells the
        // two apart. The blob decodes to a PAN, so asserting on the literal PAN is not enough.
        let body = "eyJwYW4iOiI0MTExMTExMTExMTExMTExIn0=";
        let out = mask_connector_response(body.as_bytes(), None, PAYSAFE, &config(&[])).unwrap();
        assert!(!out.contains("eyJwYW4"), "blob echoed: {out}");
        assert_is_stub(&out, body.len());
    }

    #[test]
    fn a_declared_form_type_does_not_override_a_json_body() {
        // A gateway mislabelling its own JSON must not route it to the masker that keeps keys.
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
        // The guard above must not reject genuine form bodies — Fiuu's sync response shape.
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

    // -- data in key position ----------------------------------------------

    #[test]
    fn a_key_that_is_itself_card_data_is_masked() {
        let body = format!(r#"{{"declines":{{"{PAN}":"expired"}}}}"#);
        let out = mask_json_body(&body, PAYSAFE, &config(&[]));
        assert!(!out.contains(PAN), "{out}");
    }

    // -- the denylist beats the configured list, for every entry -----------

    #[test]
    fn short_denylist_entries_survive_being_allowlisted() {
        // `peachpayments` names its card-number field exactly `pan`, so this is the one that
        // matters most. Each is exact-match: as substrings they would catch `company`,
        // `shipping`, `acidic`.
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

    // -- XML holes ---------------------------------------------------------

    #[test]
    fn xml_a_namespace_declaration_cannot_smuggle_a_value() {
        // The attribute *name* is gateway-controlled, so passing `xmlns:*` through untouched let
        // it carry anything with an empty allowlist.
        let out = mask_xml_body(
            &format!(r#"<r xmlns:cardnumber="{PAN}"><a>x</a></r>"#),
            ELAVON,
            &config(&[]),
        );
        assert!(!out.contains(PAN), "{out}");
    }

    #[test]
    fn xml_only_a_leading_declaration_is_written_through() {
        // quick-xml calls any `<?xml …?>` a declaration wherever it appears, and declarations are
        // emitted verbatim while processing instructions are dropped.
        let out = mask_xml_body(
            &format!(r#"<r><?xml pan="{PAN}"?><a>b</a></r>"#),
            ELAVON,
            &config(&[]),
        );
        assert!(!out.contains(PAN), "{out}");

        // The real one, at the top, still survives.
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
        // `<pan/>` and `<pan></pan>` carry the same data; both must mask what follows.
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

    // -- allowlist semantics: bare name vs dotted path ---------------------

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
        // Adyen returns `bankAccount.iban` and `retry.attempt1.rawResponse` as flat key names,
        // not as nesting, so an entry that pins no path must still be able to name one.
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
}
