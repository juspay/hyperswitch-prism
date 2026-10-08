#![allow(clippy::large_enum_variant)]
#![allow(clippy::uninlined_format_args)]
#![allow(legacy_derive_helpers)]

pub const FILE_DESCRIPTOR_SET: &[u8] =
    tonic::include_file_descriptor_set!("connector_service_descriptor");

mod types {
    tonic::include_proto!("types");
}

/// `serialize_with` helpers for proto `string` fields that hold a secret (see `build.rs`,
/// `WebhookSecrets`). They serialize through `Secret<String>`: masked under
/// `hyperswitch_masking::masked_serialize`, exposed under any other serializer.
pub mod secret_serde {
    use hyperswitch_masking::Secret;
    use serde::Serialize;

    #[allow(clippy::ptr_arg)]
    pub fn string<S: serde::Serializer>(value: &String, serializer: S) -> Result<S::Ok, S::Error> {
        Secret::<String>::new(value.clone()).serialize(serializer)
    }

    pub fn optional_string<S: serde::Serializer>(
        value: &Option<String>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        value
            .clone()
            .map(Secret::<String>::new)
            .serialize(serializer)
    }
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
