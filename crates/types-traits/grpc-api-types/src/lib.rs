#![allow(clippy::large_enum_variant)]
#![allow(clippy::uninlined_format_args)]
#![allow(legacy_derive_helpers)]

// TODO: Remove these serializers and their build.rs attributes with the legacy webhook fields.
fn serialize_legacy_secret<T, S>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
where
    T: hyperswitch_masking::SerializableSecret + std::fmt::Debug,
    S: serde::Serializer,
{
    serde::Serialize::serialize(&hyperswitch_masking::Secret::<_>::new(value), serializer)
}

fn serialize_optional_legacy_secret<T, S>(
    value: &Option<T>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    T: hyperswitch_masking::SerializableSecret + std::fmt::Debug,
    S: serde::Serializer,
{
    serde::Serialize::serialize(
        &value.as_ref().map(hyperswitch_masking::Secret::<_>::new),
        serializer,
    )
}

fn serialize_legacy_headers<S>(
    value: &std::collections::HashMap<String, String>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use serde::ser::SerializeMap;

    let mut map = serializer.serialize_map(Some(value.len()))?;
    for (key, value) in value {
        map.serialize_entry(key, &hyperswitch_masking::Secret::<_>::new(value))?;
    }
    map.end()
}

pub const FILE_DESCRIPTOR_SET: &[u8] =
    tonic::include_file_descriptor_set!("connector_service_descriptor");

mod types {
    tonic::include_proto!("types");
}

pub mod payments {
    pub use super::types::*;
}

pub mod health_check {
    tonic::include_proto!("grpc.health.v1");
}

pub mod payouts {
    pub use super::types::*;
}

pub mod surcharge {
    pub use super::types::*;
}

pub mod frm {
    pub use super::types::*;
}

pub mod auto_populate {
    include!(concat!(env!("OUT_DIR"), "/auto_populate_generated.rs"));
    pub use generated::*;
}
