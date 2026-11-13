//! Validate every registered standalone provider, including payout-only providers.

use std::collections::BTreeMap;

use connector_integration::webhook_connectors;
use domain_types::connector_types::WebhookProvider;

use crate::types::{ConnectorResult, FlowResult, SamplePayload};

pub(crate) fn probe_registered_provider(name: &str) -> Option<FlowResult> {
    let provider = WebhookProvider::new(name)?;
    let connector = webhook_connectors::get_connector(&provider)?;
    Some(probe_provider_sample(connector.as_ref()))
}

fn probe_provider_sample(connector: &dyn interfaces::webhook::Webhook) -> FlowResult {
    let request = connector.sample_webhook_request();
    let sample = SamplePayload {
        url: request.uri.clone().unwrap_or_default(),
        method: format!("{:?}", request.method),
        headers: request.headers.clone().into_iter().collect(),
        body: String::from_utf8(request.body.clone()).ok(),
    };
    match connector.parse_event(&request) {
        Ok(_) => FlowResult {
            status: "supported".to_string(),
            sample: Some(sample),
            ..Default::default()
        },
        Err(error) => FlowResult {
            status: "error".to_string(),
            error: Some(format!("{error:?}")),
            sample: Some(sample),
            ..Default::default()
        },
    }
}

pub(crate) fn probe_registered_providers(results: &mut Vec<ConnectorResult>) {
    for provider in webhook_connectors::registered_providers() {
        let name = provider.as_str();
        if let Some(result) = probe_registered_provider(name) {
            let flows = BTreeMap::from([(
                "parse_event".to_string(),
                BTreeMap::from([("default".to_string(), result)]),
            )]);
            if let Some(existing) = results.iter_mut().find(|result| result.connector == name) {
                existing.flows.extend(flows);
            } else {
                results.push(ConnectorResult {
                    connector: name.to_string(),
                    flows,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain_types::{
        connector_types::{EventType, HttpMethod, RequestDetails, WebhookParseResponse},
        errors::WebhookError,
    };
    use interfaces::webhook::Webhook;

    struct Provider {
        valid: bool,
    }
    impl Webhook for Provider {
        fn sample_webhook_request(&self) -> RequestDetails {
            RequestDetails {
                method: HttpMethod::Post,
                uri: Some("/webhook".to_string()),
                headers: [("signature".to_string(), "sample".to_string())].into(),
                body: b"sample".to_vec(),
                query_params: None,
            }
        }
        fn parse_event(
            &self,
            request: &RequestDetails,
        ) -> common_utils::CustomResult<WebhookParseResponse, WebhookError> {
            if self.valid && request.headers.get("signature").map(String::as_str) == Some("sample")
            {
                Ok(WebhookParseResponse {
                    event_type: EventType::PayoutSuccess,
                    reference: None,
                })
            } else {
                Err(WebhookError::WebhookEventTypeNotFound.into())
            }
        }
    }

    #[test]
    fn provider_sample_parse_errors_are_failures() {
        assert_eq!(
            probe_provider_sample(&Provider { valid: false }).status,
            "error"
        );
        assert_eq!(
            probe_provider_sample(&Provider { valid: true }).status,
            "supported"
        );
    }
}
