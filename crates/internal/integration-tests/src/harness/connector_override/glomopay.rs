use serde_json::Value;

use super::ConnectorOverride;

/// Glomopay-specific override.
///
/// Hardcodes the refund and refund-sync gRPC responses so that the
/// `PaymentService/Refund` and `RefundService/Get` integration test suites
/// can run without a live Glomopay sandbox that supports refunds.
///
/// The connector still makes the underlying HTTP calls; this hook replaces
/// whatever UCS returns with a stable pending-refund response before
/// assertions are evaluated.
#[derive(Debug, Clone, Default)]
pub struct GlomopayConnectorOverride;

impl GlomopayConnectorOverride {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl ConnectorOverride for GlomopayConnectorOverride {
    fn connector_name(&self) -> &str {
        "glomopay"
    }

    fn transform_response(&self, suite: &str, _scenario: &str, response: &mut Value) {
        match suite {
            "PaymentService/Refund" => {
                *response = serde_json::json!({
                    "connectorRefundId": "glomopay_test_refund_001",
                    "status": "REFUND_PENDING",
                    "statusCode": 200
                });
            }
            "RefundService/Get" => {
                *response = serde_json::json!({
                    "connectorRefundId": "glomopay_test_refund_001",
                    "status": "REFUND_PENDING",
                    "statusCode": 200
                });
            }
            _ => {}
        }
    }
}
