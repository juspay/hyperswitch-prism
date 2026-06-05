use std::{env, path::PathBuf};

use prost_types::{
    field_descriptor_proto::{Label, Type},
    DescriptorProto, FileDescriptorSet,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);

    // Create the bridge generator with string enums
    let bridge_generator = g2h::BridgeGenerator::with_tonic_build()
        .with_string_enums()
        .file_descriptor_set_path(out_dir.join("connector_service_descriptor.bin"));

    // Create a basic prost config and add your extern_path configuration
    let mut config = prost_build::Config::new();
    config.extern_path(".types.CardNumberType", "::cards::CardNumber");
    config.extern_path(".types.NetworkTokenType", "::cards::NetworkToken");
    config.extern_path(
        ".types.SecretString",
        "::hyperswitch_masking::Secret<String>",
    );

    // Add serde rename_all = "snake_case" for oneof enum types to output proper proto JSON
    // This ensures variant names like "ApplePay" serialize as "apple_pay"
    config.type_attribute(
        ".types.PaymentMethod.payment_method",
        "#[serde(rename_all = \"snake_case\")]",
    );
    config.type_attribute(
        ".types.AppleWallet.PaymentData.payment_data",
        "#[serde(rename_all = \"snake_case\")]",
    );
    config.type_attribute(
        ".types.GoogleWallet.TokenizationData.tokenization_data",
        "#[serde(rename_all = \"snake_case\")]",
    );

    let protos: &[&str] = &[
        "proto/services.proto",
        "proto/health_check.proto",
        "proto/payment.proto",
        "proto/composite_services.proto",
        "proto/composite_payment.proto",
        "proto/payment_methods.proto",
        "proto/sdk_config.proto",
        "proto/payouts.proto",
        "proto/surcharge.proto",
    ];

    // g2h's add_skip_null_attribute_static does not add #[serde(default)] for repeated
    // (Vec<T>) proto fields. Protobuf semantics say repeated fields default to empty,
    // so serde(default) is always correct. Load the file descriptors and patch the config
    // before handing it to g2h.
    let fds = prost_build::Config::new().load_fds(protos, &["proto"])?;
    add_serde_default_for_repeated_fields(&mut config, &fds);

    // Use compile_protos_with_config which handles everything internally
    // including string enum support, serde derives, and descriptor set writing
    bridge_generator.compile_protos_with_config(config, protos, &["proto"])?;

    Ok(())
}

/// Add `#[serde(default)]` for all repeated non-enum proto fields.
///
/// g2h already adds `default` for repeated enum fields via its enum serializer attributes,
/// so we skip those to avoid duplicate serde(default) which would cause a compile error.
fn add_serde_default_for_repeated_fields(
    config: &mut prost_build::Config,
    fds: &FileDescriptorSet,
) {
    for file in &fds.file {
        for message in &file.message_type {
            add_serde_default_recursive(config, message);
        }
    }
}

fn add_serde_default_recursive(config: &mut prost_build::Config, message: &DescriptorProto) {
    let message_name = message.name.as_deref().unwrap_or_default();
    for field in &message.field {
        if field.label() == Label::Repeated && field.r#type() != Type::Enum {
            let field_name = field.name.as_deref().unwrap_or_default();
            config.field_attribute(format!("{message_name}.{field_name}"), "#[serde(default)]");
        }
    }
    for nested in &message.nested_type {
        add_serde_default_recursive(config, nested);
    }
}
