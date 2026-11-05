//! Temporary Serde bridge for deprecated plain protobuf fields. Normal Serde
//! preserves their HTTP representation; the existing masking serializer sees
//! Secret values. Remove these adapters when the legacy fields are retired.
use std::{collections::HashMap, fmt};

use hyperswitch_masking::Secret;
use serde::{Serialize, Serializer};

#[allow(clippy::ptr_arg)] // Borrow the exact SerializableSecret type without copying it.
pub(crate) fn string<S: Serializer>(value: &String, serializer: S) -> Result<S::Ok, S::Error> {
    Secret::<_>::new(value).serialize(serializer)
}

pub(crate) fn optional_string<S: Serializer>(
    value: &Option<String>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    value.as_ref().map(Secret::<_>::new).serialize(serializer)
}

#[allow(clippy::ptr_arg)] // Slices do not implement SerializableSecret in this crate version.
pub(crate) fn bytes<S: Serializer>(value: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error> {
    Secret::<_>::new(value).serialize(serializer)
}

pub(crate) fn headers<S: Serializer>(
    value: &HashMap<String, String>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    use serde::ser::SerializeMap;
    let mut map = serializer.serialize_map(Some(value.len()))?;
    for (name, value) in value {
        map.serialize_entry(name, &Secret::<_>::new(value))?;
    }
    map.end()
}

// Prost normally derives Debug over the unwrapped legacy values. Do not let
// a Debug fallback bypass their Serde masking. New fields use Secret directly.
impl fmt::Debug for crate::types::RequestDetails {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RequestDetails")
            .field("method", &self.method)
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for crate::types::WebhookSecrets {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WebhookSecrets").finish_non_exhaustive()
    }
}

impl fmt::Debug for crate::types::RedirectResponseSecrets {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RedirectResponseSecrets")
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for crate::types::EventAckResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EventAckResponse")
            .field("status_code", &self.status_code)
            .finish_non_exhaustive()
    }
}
