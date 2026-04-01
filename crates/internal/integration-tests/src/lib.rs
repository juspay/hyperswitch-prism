#![allow(
    clippy::expect_used,
    clippy::missing_panics_doc,
    clippy::panic,
    clippy::unwrap_used
)]

//! Library entrypoint for the connector integration test harness.
//!
//! Most runtime behavior lives under `harness`, while binaries under `src/bin`
//! wire CLI argument parsing and reporting around these reusable primitives.

pub mod harness;
