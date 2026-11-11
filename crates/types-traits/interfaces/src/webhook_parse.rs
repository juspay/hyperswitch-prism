//! Compatibility names for the original ParseEvent trait and boxed connector.
//!
//! Existing implementations only need `parse_event`; new webhook methods have defaults.
// TODO(webhook-migration): remove this alias after all callers adopt Webhook.
pub use crate::webhook::{BoxedWebhookConnector, Webhook as WebhookParse};
