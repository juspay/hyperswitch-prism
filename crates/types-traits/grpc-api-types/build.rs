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
