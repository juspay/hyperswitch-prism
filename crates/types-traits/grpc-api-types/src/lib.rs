#![allow(clippy::large_enum_variant)]
#![allow(clippy::uninlined_format_args)]
#![allow(legacy_derive_helpers)]

pub const FILE_DESCRIPTOR_SET: &[u8] =
    tonic::include_file_descriptor_set!("connector_service_descriptor");

mod types {
    tonic::include_proto!("types");
}

/// serde `serialize_with` helpers for secret-bearing proto `string` fields (see build.rs).
/// Plain serializers see the raw value; `hyperswitch_masking::masked_serialize` masks it.
pub mod masked_serde {
    use hyperswitch_masking::Secret;
    use serde::{Serialize, Serializer};

    pub fn string<S: Serializer>(value: &str, serializer: S) -> Result<S::Ok, S::Error> {
        Secret::<String>::new(value.to_owned()).serialize(serializer)
    }

    pub fn option_string<S: Serializer>(
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
