use std::{
    env, fs,
    path::{Path, PathBuf},
};

#[path = "codegen/auto_populate.rs"]
mod auto_populate;

/// Reorders item-level `#[serde(...)]` helper attributes so they follow the
/// `#[derive(...)]` attributes that introduce them.
///
/// prost-build emits registered type attributes in registration order and g2h
/// registers the serde derives when compilation starts — i.e. *after* the
/// `type_attribute` calls made above. rustc warns (future-incompat, and a hard
/// error in a future release) about a derive helper attribute used before its
/// derive, so the generated files are rewritten once compilation finished.
fn reorder_serde_helper_attrs(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let source = fs::read_to_string(path)?;
    let lines: Vec<&str> = source.lines().collect();

    let mut out: Vec<&str> = Vec::with_capacity(lines.len());
    let mut index = 0;
    while index < lines.len() {
        if !lines[index].trim_start().starts_with("#[serde(") {
            out.push(lines[index]);
            index += 1;
            continue;
        }

        // A run of item-level `#[serde(...)]` lines ...
        let run_start = index;
        while index < lines.len() && lines[index].trim_start().starts_with("#[serde(") {
            index += 1;
        }
        let run_end = index;

        // ... directly followed by the `#[derive(...)]` lines of the same item.
        while index < lines.len() && lines[index].trim_start().starts_with("#[derive(") {
            index += 1;
        }

        out.extend_from_slice(&lines[run_end..index]);
        out.extend_from_slice(&lines[run_start..run_end]);
    }

    let mut reordered = out.join("\n");
    if source.ends_with('\n') {
        reordered.push('\n');
    }
    if reordered != source {
        fs::write(path, reordered)?;
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=codegen/auto_populate.rs");
    println!("cargo:rerun-if-changed=proto");

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
    config.type_attribute(
        ".types.FlowStatus.status",
        "#[serde(rename_all = \"snake_case\")]",
    );
    config.type_attribute(
        ".types.PaymentMethodServiceRefreshRequest.payment_method",
        "#[serde(rename_all = \"snake_case\")]",
    );
    config.type_attribute(
        ".types.RefreshResult.result",
        "#[serde(rename_all = \"snake_case\")]",
    );

    // PaysafePaymentMethodDetails carries several optional per-currency account
    // maps (card, ach, apple_pay, interac, skrill, pay_safe_card, neteller). The
    // x-connector-config header only supplies the subset a merchant has
    // provisioned, so any omitted map must default to empty. Without this,
    // serde treats a missing (non-optional proto map) field as a hard error,
    // which would break backward compatibility when new account maps are added.
    // A partial config cannot silently send an empty account id: a missing map
    // resolves to `None` and an explicit `""` is rejected as empty by
    // `PaysafePaymentMethodDetails::get_account_id` (domain_types/router_data.rs).
    config.type_attribute(".types.PaysafePaymentMethodDetails", "#[serde(default)]");

    // Use compile_protos_with_config which handles everything internally
    // including string enum support, serde derives, and descriptor set writing
    bridge_generator.compile_protos_with_config(
        config,
        &[
            "proto/services.proto",
            "proto/health_check.proto",
            "proto/common_types.proto",
            "proto/domain.proto",
            "proto/payment.proto",
            "proto/events.proto",
            "proto/composite_services.proto",
            "proto/composite_payment.proto",
            "proto/composite_frm.proto",
            "proto/payment_methods.proto",
            "proto/sdk_config.proto",
            "proto/payouts.proto",
            "proto/surcharge.proto",
            "proto/frm.proto",
        ],
        &["proto"],
    )?;

    // Rewrite pass: move item-level `#[serde(...)]` helper attributes behind the
    // `#[derive(...)]` lines that introduce them (see the doc comment above).
    for entry in fs::read_dir(&out_dir)? {
        let path = entry?.path();
        if path.extension().is_some_and(|ext| ext == "rs") {
            reorder_serde_helper_attrs(&path)?;
        }
    }

    // Second codegen pass: read back the descriptor set just written above
    // and generate the `populate_<field>` trait impls (see
    // `codegen/auto_populate.rs` for the field -> setter declarations).
    let descriptor_bytes = std::fs::read(out_dir.join("connector_service_descriptor.bin"))?;
    let descriptor_set =
        <prost_types::FileDescriptorSet as prost::Message>::decode(descriptor_bytes.as_slice())?;
    let auto_populate_generated = auto_populate::generate(&descriptor_set);
    std::fs::write(
        out_dir.join("auto_populate_generated.rs"),
        auto_populate_generated,
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
