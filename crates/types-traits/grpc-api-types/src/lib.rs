#![allow(clippy::large_enum_variant)]
#![allow(clippy::uninlined_format_args)]
#![allow(legacy_derive_helpers)]

pub const FILE_DESCRIPTOR_SET: &[u8] =
    tonic::include_file_descriptor_set!("connector_service_descriptor");

/// `serialize_with` for secret-bearing proto `string` fields: serializes through
/// `Secret<String>`, which is exposed by ordinary serializers and masked by
/// `hyperswitch_masking::masked_serialize`.
pub fn serialize_as_secret<S>(value: &str, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serde::Serialize::serialize(
        &hyperswitch_masking::Secret::<String>::new(value.to_owned()),
        serializer,
    )
}

/// As [`serialize_as_secret`], for `optional string` fields.
pub fn serialize_as_optional_secret<S>(
    value: &Option<String>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serde::Serialize::serialize(
        &value
            .clone()
            .map(hyperswitch_masking::Secret::<String>::new),
        serializer,
    )
}

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
