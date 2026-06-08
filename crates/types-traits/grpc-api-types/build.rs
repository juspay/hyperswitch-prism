use std::{env, path::PathBuf};

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

    // Add #[serde(default)] to repeated (Vec) fields in connector configs so missing
    // keys deserialize as empty rather than erroring (proto3 semantics).
    let fds = prost_build::Config::new().load_fds(&["proto/payment.proto"], &["proto"])?;
    add_serde_default_for_connector_configs(&mut config, &fds);

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

    // Use compile_protos_with_config which handles everything internally
    // including string enum support, serde derives, and descriptor set writing
    bridge_generator.compile_protos_with_config(
        config,
        &[
            "proto/services.proto",
            "proto/health_check.proto",
            "proto/payment.proto",
            "proto/composite_services.proto",
            "proto/composite_payment.proto",
            "proto/payment_methods.proto",
            "proto/sdk_config.proto",
            "proto/payouts.proto",
            "proto/surcharge.proto",
        ],
        &["proto"],
    )?;

    // prost_build::Config::new()
    //     .service_generator(Box::new(web_generator))
    //     .file_descriptor_set_path(out_dir.join("connector_service_descriptor.bin"))
    //     .type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]")
    //     .type_attribute(".", "#[allow(clippy::large_enum_variant)]")
    //     .compile_protos(
    //         &[
    //             "proto/services.proto",
    //             "proto/health_check.proto",
    //             "proto/payment.proto",
    //             "proto/composite_services.proto",
    //             "proto/composite_payment.proto",
    //             "proto/payment_methods.proto",
    //         ],
    //         &["proto"],
    //     )?;

    Ok(())
}

/// Adds `#[serde(default)]` to every repeated non-enum field in connector config messages.
fn add_serde_default_for_connector_configs(
    config: &mut prost_build::Config,
    fds: &prost_types::FileDescriptorSet,
) {
    let file = match fds.file.iter().find(|f| {
        f.message_type
            .iter()
            .any(|m| m.name() == "ConnectorSpecificConfig")
    }) {
        Some(f) => f,
        None => {
            eprintln!(
                "cargo:warning=build.rs: ConnectorSpecificConfig not found in payment.proto; \
                 no #[serde(default)] will be added for connector config repeated fields"
            );
            return;
        }
    };

    let package = file.package();
    let config_msg = file
        .message_type
        .iter()
        .find(|m| m.name() == "ConnectorSpecificConfig")
        .expect("already verified above");

    let connector_config_names: std::collections::HashSet<&str> = config_msg
        .field
        .iter()
        .filter_map(|f: &prost_types::FieldDescriptorProto| {
            let name = f.type_name().split('.').next_back().unwrap_or("");
            if name.is_empty() {
                None
            } else {
                Some(name)
            }
        })
        .collect();

    for message in &file.message_type {
        if connector_config_names.contains(message.name()) {
            add_repeated_default(config, package, message.name(), message);
        }
    }
}

/// Adds `#[serde(default)]` to every repeated non-enum field in a message and its nested types.
fn add_repeated_default(
    config: &mut prost_build::Config,
    package: &str,
    parent_path: &str,
    message: &prost_types::DescriptorProto,
) {
    use prost_types::field_descriptor_proto::{Label, Type};
    for field in &message.field {
        if field.label() == Label::Repeated && field.r#type() != Type::Enum {
            config.field_attribute(
                format!(".{package}.{parent_path}.{}", field.name()),
                "#[serde(default)]",
            );
        }
    }
    for nested in &message.nested_type {
        let nested_path = format!("{parent_path}.{}", nested.name());
        add_repeated_default(config, package, &nested_path, nested);
    }
}
