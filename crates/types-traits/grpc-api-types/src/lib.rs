#![allow(clippy::large_enum_variant)]
#![allow(clippy::uninlined_format_args)]
#![allow(legacy_derive_helpers)]

pub(crate) mod masking_serde {
    use hyperswitch_masking::Secret;
    use serde::Serializer;

    pub fn secret_string<S: Serializer>(value: &str, s: S) -> Result<S::Ok, S::Error> {
        serde::Serialize::serialize(&Secret::<String>::new(value.to_owned()), s)
    }

    pub fn optional_secret_string<S: Serializer>(
        value: &Option<String>,
        s: S,
    ) -> Result<S::Ok, S::Error> {
        serde::Serialize::serialize(&value.clone().map(Secret::<String>::new), s)
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
