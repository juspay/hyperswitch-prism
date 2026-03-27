#![allow(clippy::expect_used, clippy::missing_panics_doc, clippy::panic)]

//! Library entrypoint for the UCS connector test harness.
//!
//! Most runtime behavior lives under `harness`, while binaries under `src/bin`
//! wire CLI argument parsing and reporting around these reusable primitives.

pub mod harness;
