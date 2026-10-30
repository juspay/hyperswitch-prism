#![allow(clippy::large_enum_variant)]
#![allow(clippy::uninlined_format_args)]
#![allow(legacy_derive_helpers)]

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

/// serde `serialize_with` helpers for proto `string` fields that carry secrets
/// (see build.rs). Serializing through `Secret<String>` keeps the value for normal
/// serializers and masks it under `hyperswitch_masking::masked_serialize`.
pub mod masked_serde {
    use hyperswitch_masking::Secret;
    use serde::Serialize;

    #[allow(clippy::ptr_arg)]
    pub fn secret_string<S: serde::Serializer>(
        value: &String,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        Secret::<String>::new(value.clone()).serialize(serializer)
    }

    pub fn optional_secret_string<S: serde::Serializer>(
        value: &Option<String>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        value
            .clone()
            .map(Secret::<String>::new)
            .serialize(serializer)
    }
}
