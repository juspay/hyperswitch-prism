//! Déjà record/replay wiring for the gRPC server.
//!
//! Feature-gated and **inert until a boot hook is installed** (a later commit): with no
//! hook, `deja::process_runtime_mode()` is `Disabled`, every predicate here is false, and
//! the ingress boundary (added next) is a pure passthrough. So even feature-on, the server
//! behaves exactly as feature-off until recording is deliberately switched on.

pub mod descriptors;

/// Whether the process is recording or replaying.
///
/// The ingress boundary gates on this **boot-time process mode**, never the per-request
/// recording decision — the ingress is what *pushes* that decision, so gating on it would
/// be circular (nothing would ever record). See [`deja::process_runtime_mode`].
pub fn process_is_active() -> bool {
    !deja::process_runtime_mode().is_disabled()
}

/// Whether the process is in record mode (so a per-request sampling decision is pushed).
pub fn process_is_record_mode() -> bool {
    deja::process_runtime_mode().is_record()
}
