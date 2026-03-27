use std::collections::HashMap;

use common_utils::Method;

#[derive(Debug, Eq, PartialEq, Clone, serde::Serialize, serde::Deserialize)]
pub enum RedirectForm {
    Form {
        endpoint: String,
        method: Method,
        form_fields: HashMap<String, String>,
    },
    Html {
        html_data: String,
    },
    BlueSnap {
        payment_fields_token: String, // payment-field-token
    },
    CybersourceAuthSetup {
        access_token: String,
        ddc_url: String,
        reference_id: String,
    },
    CybersourceConsumerAuth {
        access_token: String,
        step_up_url: String,
    },
    DeutschebankThreeDSChallengeFlow {
        acs_url: String,
        creq: String,
    },
    Payme,
    Braintree {
        client_token: String,
        card_token: String,
        bin: String,
        acs_url: String,
    },
    Nmi {
        amount: String,
        currency: common_enums::Currency,
        public_key: hyperswitch_masking::Secret<String>,
        customer_vault_id: String,
        order_id: String,
    },
    Mifinity {
        initialization_token: String,
    },
    WorldpayDDCForm {
        endpoint: url::Url,
        method: Method,
        form_fields: HashMap<String, String>,
        collection_id: Option<String>,
    },
    Uri {
        uri: String,
    },
}

impl From<(url::Url, Method)> for RedirectForm {
    fn from((mut redirect_url, method): (url::Url, Method)) -> Self {
        let form_fields = HashMap::from_iter(
            redirect_url
                .query_pairs()
                .map(|(key, value)| (key.to_string(), value.to_string())),
        );

        // Do not include query params in the endpoint
        redirect_url.set_query(None);

        Self::Form {
            endpoint: redirect_url.to_string(),
            method,
            form_fields,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Response {
    /// headers
    pub headers: Option<http::HeaderMap>,
    /// response
    pub response: bytes::Bytes,
    /// status code
    pub status_code: u16,
}

#[derive(Debug, Clone)]
pub struct VerifyWebhookSourceResponseData {
    pub verify_webhook_status: VerifyWebhookStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VerifyWebhookStatus {
    SourceVerified,
    SourceNotVerified,
}
