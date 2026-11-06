//! Compatibility names for the original ParseEvent trait and boxed connector.
//!
//! Existing implementations only need `parse_event`; new common methods have defaults.
pub use crate::webhook_common::{BoxedWebhookConnector, WebhookCommon as WebhookParse};
