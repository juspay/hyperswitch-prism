use std::{cell::Cell, marker::PhantomData, sync::Arc};

use common_enums::{Currency, RefundStatus};
use common_utils::{
    crypto::{HmacSha256, SignMessage},
    types::{MinorUnit, Money},
};
use domain_types::payment_method_data::DefaultPCIHolder;
use hyperswitch_masking::{ExposeInterface, Secret};

use super::*;
use base64::Engine;
use boost::BASE64_ENGINE;

thread_local! {
    static CLOCK_READS: Cell<u32> = const { Cell::new(0) };
}

fn advancing_timestamp() -> String {
    CLOCK_READS.with(|reads| {
        let second = reads.get();
        reads.set(second + 1);
        format!("2026-11-23T00:00:{second:02}.000Z")
    })
}

struct ResetClock;
impl Drop for ResetClock {
    fn drop(&mut self) {
        boost::TEST_CREATED_TIMESTAMP.with(|clock| clock.set(None));
    }
}

#[test]
fn refund_signs_the_sent_body_when_the_clock_advances() {
    CLOCK_READS.with(|reads| reads.set(0));
    boost::TEST_CREATED_TIMESTAMP.with(|clock| clock.set(Some(advancing_timestamp)));
    let _reset = ResetClock;
    let mut connectors = Connectors::default();
    connectors.boost.base_url = "https://example.com/gateway".to_string();
    let data = RouterDataV2 {
        flow: PhantomData::<Refund>,
        resource_common_data: RefundFlowData {
            merchant_id: Default::default(),
            status: RefundStatus::Pending,
            refund_id: None,
            connectors: Arc::new(connectors),
            connector_request_reference_id: "refund-reference".to_string(),
            raw_connector_response: None,
            typed_connector_response: None,
            connector_response_headers: None,
            raw_connector_request: None,
            typed_connector_request: None,
            access_token: None,
            connector_feature_data: None,
            test_mode: None,
            payment_method: None,
            merchant_request_id: Some("4a736f10-462d-4e31-b5e2-3fdac3117f24".to_string()),
            raw_connector_status: None,
        },
        connector_config: ConnectorSpecificConfig::Boost {
            client_id: Secret::new("test-client".to_string()),
            merchant_secret: Secret::new("test-secret".to_string()),
            public_key: None,
            base_url: None,
        },
        request: RefundsData {
            connector_transaction_id: "payment-uuid".to_string(),
            refund_money: Money::from_minor_unit(MinorUnit::new(100), Currency::MYR),
            currency: Currency::MYR,
            ..Default::default()
        },
        response: Err(ErrorResponse::default()),
    };
    let request = <Boost<DefaultPCIHolder> as ConnectorIntegrationV2<
        Refund,
        RefundFlowData,
        RefundsData,
        RefundsResponseData,
    >>::build_request_v2(Boost::new(), &data)
    .expect("build refund request")
    .expect("outbound request");
    let body = request
        .body
        .as_ref()
        .expect("refund body")
        .get_inner_value()
        .expose();
    let signature = HmacSha256
        .sign_message(
            b"test-secret",
            format!("POST/v1/reversals{body}").as_bytes(),
        )
        .expect("compute expected HMAC");
    let expected = format!(
        "Basic {}",
        BASE64_ENGINE.encode(format!("test-client:{}", BASE64_ENGINE.encode(signature)))
    );
    let authorization = request
        .headers
        .iter()
        .find(|(name, _)| name == headers::AUTHORIZATION)
        .expect("authorization header")
        .1
        .clone()
        .into_inner();
    assert_eq!(authorization, expected);
    assert_eq!(
        CLOCK_READS.with(Cell::get),
        1,
        "generate the body only once"
    );
    let json: serde_json::Value = serde_json::from_str(&body).expect("valid JSON");
    assert_eq!(json["uuid"], "4a736f10-462d-4e31-b5e2-3fdac3117f24");
    assert!(request.typed_connector_request_value.is_some());
}
