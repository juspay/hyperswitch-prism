#![allow(clippy::large_enum_variant)]
#![allow(clippy::uninlined_format_args)]
#![allow(legacy_derive_helpers)]

pub const FILE_DESCRIPTOR_SET: &[u8] =
    tonic::include_file_descriptor_set!("connector_service_descriptor");

pub mod payments {
    tonic::include_proto!("types");
}

pub mod health_check {
    tonic::include_proto!("grpc.health.v1");
}

pub mod payouts {
    tonic::include_proto!("types");
}
