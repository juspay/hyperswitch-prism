#![allow(clippy::large_enum_variant)]
#![allow(clippy::uninlined_format_args)]
#![allow(legacy_derive_helpers)]

/// Serde helpers the generated `WebhookSecrets` type references: keep the
/// `secret`/`additional_secret` proto wire as plain `string` while redacting
/// the value whenever serialization goes through
/// `hyperswitch_masking::masked_serialize` (the gRPC-server golden-log
/// `request_body` span field). Mirrors the masking crate's `pii_serialize`:
/// mask only when the serializer is the crate's PII serializer; on every
/// other serializer (serde_json, bincode, tonic JSON) values serialize
/// exposed, matching the behaviour of `Secret<String>`-typed fields.
pub mod masked {
    use serde::Serializer;

    const MASKED: &str = "***MASKED***";

    /// `type_name` of hyperswitch_masking's internal PII serializer. The type
    /// itself is crate-private, so comparison goes through its fully-qualified
    /// name — the same identity the masking crate's own `pii_serialize` holds.
    const PII_SERIALIZER: &str = "hyperswitch_masking::serde::pii_serializer::PIISerializer";

    pub fn serialize_secret_string<S: Serializer>(
        value: &str,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        if std::any::type_name::<S>() == PII_SERIALIZER {
            serializer.serialize_str(MASKED)
        } else {
            serializer.serialize_str(value)
        }
    }

    pub fn serialize_opt_secret_string<S: Serializer>(
        value: &Option<String>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match value {
            Some(_) if std::any::type_name::<S>() == PII_SERIALIZER => {
                serializer.serialize_some(MASKED)
            }
            Some(value) => serializer.serialize_some(value),
            None => serializer.serialize_none(),
        }
    }
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
