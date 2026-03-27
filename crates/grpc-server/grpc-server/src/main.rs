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

    // Load superposition.toml for connector URL resolution
    let superposition_config_path = format!(
        "{}/config/superposition.toml",
        configs::workspace_path().display()
    );
    match SuperpositionConfig::from_file(&superposition_config_path) {
        Ok(sp_config) => {
            tracing::info!(
                "Successfully loaded superposition.toml from {}",
                superposition_config_path
            );
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

    let _guard = logger::setup(
        &config.log,
        ucs_env::service_name!(),
        [ucs_env::service_name!(), "grpc_server", "tower_http"],
    );

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
