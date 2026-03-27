use common_utils::errors::CustomResult;
use domain_types::errors::ApplicationErrorResponse;
use std::sync::Arc;
use ucs_env::configs::Config;

use crate::config::merge_config_with_override;

/// Shared config extraction logic, transport-agnostic.
///
/// Given an optional config override header value and the base config,
/// returns the effective config (merged or base).
pub fn extract_and_merge_config(
    config_override_header: Option<&str>,
    base_config: &Arc<Config>,
) -> CustomResult<Arc<Config>, ApplicationErrorResponse> {
    match config_override_header {
        Some(override_str) if !override_str.trim().is_empty() => {
            merge_config_with_override(override_str.to_owned(), (*base_config.as_ref()).clone())
        }
        _ => Ok(Arc::clone(base_config)),
    }
}
