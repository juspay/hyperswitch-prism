use std::sync::Arc;

use common_utils::SuperpositionConfig;
use grpc_server::{self, app};
use ucs_env::{configs, logger};

#[allow(clippy::unwrap_in_result)]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(debug_assertions)]
    verify_other_config_files();

    #[allow(clippy::expect_used)]
    let mut config = configs::Config::new().expect("Failed while parsing config");

    // Stamp the compiled build version onto runtime metadata so every event carries it (A/B groups
    // on `version`). Sourced from the binary via `git_describe!()` — the authoritative running
    // build, not a config/env value. `application_name`/`deployment_id`/`pod_name` come from config.
    config.runtime_metadata.version = ucs_env::git_describe!().to_string();

    // Install the déjà runtime hook before `logger::setup` and before any instrumented
    // call: the hook cell latches on first peek. Record misconfiguration fails open
    // (disabled hook + stderr note, boot continues); replay misconfiguration aborts boot.
    #[cfg(feature = "deja")]
    let deja_report = grpc_server::deja::boot::install(
        &config.deja,
        Some(&config.events.brokers),
        config.runtime_metadata.pod_name.as_deref(),
    )
    .map_err(|error| format!("deja replay configuration error: {error}"))?;

    let _guard = logger::setup(
        &config.log,
        ucs_env::service_name!(),
        [ucs_env::service_name!(), "grpc_server", "tower_http"],
    );

    // Now that the logger is up, surface how the déjà hook resolved.
    #[cfg(feature = "deja")]
    tracing::info!(
        mode = deja_report.mode,
        run_id = ?deja_report.run_id,
        detail = ?deja_report.detail,
        "deja runtime hook installed"
    );

    // Build the Superposition provider — the remote workspace when `[superposition]`
    // enables it (the baked file as its init-time fallback), else the baked file,
    // watched. AFTER `logger::setup`, deliberately: this block logs through `tracing`,
    // and events emitted before the subscriber is installed are discarded, not
    // buffered — the source line and the fallback warning below are the signals that
    // say where policy comes from, and they must not vanish. (Only the déjà hook
    // install above genuinely needs to precede the logger.)
    let superposition_config_path = format!(
        "{}/config/superposition.toml",
        configs::workspace_path().display()
    );
    match SuperpositionConfig::new(&config.superposition, &superposition_config_path).await {
        Ok(sp_config) => {
            // `experiments_supported = false` is the loud answer to "why does my
            // experiment never sample anything": a file source carries none.
            tracing::info!(
                source = %sp_config.source(),
                experiments_supported = sp_config.experiments_supported(),
                workspace = %config.superposition.workspace_id,
                polling_interval_secs = config.superposition.polling_interval,
                path = %superposition_config_path,
                "superposition initialised"
            );
            external_services::shared_metrics::SUPERPOSITION_SOURCE
                .with_label_values(&[&sp_config.source().to_string()])
                .set(1);
            config.superposition_config = Some(Arc::new(sp_config));
        }
        Err(e) => {
            tracing::warn!(
                "Failed to load superposition.toml from {}: {}. Connector URLs will use defaults from sandbox.toml",
                superposition_config_path,
                e
            );
        }
    }

    // Optionally push metrics over OTLP to an OpenTelemetry Collector (mirrors the
    // hyperswitch app). Additive to the Prometheus /metrics scrape endpoint.
    #[cfg(feature = "otel")]
    logger::metrics::setup_metrics_pipeline(&config.metrics.otel, ucs_env::service_name!());

    let metrics_server = app::metrics_server_builder(config.clone());
    let server = app::server_builder(config);

    #[allow(clippy::expect_used)]
    tokio::try_join!(metrics_server, server)?;

    Ok(())
}

#[cfg(debug_assertions)]
fn verify_other_config_files() {
    use std::path::PathBuf;

    use crate::configs;
    let config_file_names = vec!["production.toml", "sandbox.toml"];
    let mut config_path = PathBuf::new();
    config_path.push(configs::workspace_path());
    let config_directory: String = "config".into();
    config_path.push(config_directory);
    for config_file_name in config_file_names {
        config_path.push(config_file_name);
        #[allow(clippy::panic)]
        let _ = configs::Config::new_with_config_path(Some(config_path.clone()))
            .unwrap_or_else(|_| panic!("Update {config_file_name} with the default config values"));
        config_path.pop();
    }
}
