//! Shared harness modules used by CLI binaries and tests.
//!
//! The harness is responsible for loading scenario definitions, applying
//! connector-specific overrides, executing gRPC/SDK flows, asserting responses,
//! and producing JSON/Markdown reports.

pub mod auto_gen;
pub mod connector_override;
pub mod cred_masking;
pub mod credentials;
pub mod executor;
pub mod metadata;
pub mod report;
pub mod scenario_api;
pub mod scenario_assert;
pub mod scenario_loader;
pub mod scenario_types;
pub mod sdk_executor;
pub mod server;
