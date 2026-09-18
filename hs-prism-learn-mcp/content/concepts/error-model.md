---
{
  "slug": "error-model",
  "title": "Two kinds of failure: thrown errors vs payment errors",
  "tier": "operational",
  "audience": "engineer",
  "one_liner": "Failures split in two: SDK exceptions that are THROWN (network, integration bugs), and payment errors that come back INSIDE the response (declines). You handle them differently.",
  "analogy": "Two ways a delivery fails. The truck never arrives (a thrown exception). Or the truck arrives with a note 'recipient refused' (a payment error in the response). Same word 'failure', very different handling.",
  "depth": {
    "tldr": "Some failures throw (the call could not complete -- network down, bad integration). Other failures do NOT throw -- the call succeeded but the payment was declined, so the decline is reported in response.status/response.error. Always check the response; do not assume no-exception means success.",
    "standard": "Thrown errors -- IntegrationError, ConnectorError, NetworkError -- mean the request could not be completed and should be caught. Payment errors -- a decline like insufficient funds -- arrive as a normal response with a FAILURE status and details in response.error (often HTTP 200). Treating a decline as an exception (or ignoring response.status) is the most common integration bug. Check status first.",
    "deep": "The error model is documented in error-handling.md. Connectors map processor error details into the unified error structure (ErrorInfo in the proto). See status-codes for the numeric codes and the troubleshoot tool for symptom-based help."
  },
  "prerequisites": ["status-codes"],
  "related": ["status-codes", "flow-authorize"],
  "go_deeper": [
    {"path": "docs/architecture/concepts/error-handling.md", "why": "the full error model: exceptions vs payment errors"}
  ],
  "verify_anchors": []
}
---
