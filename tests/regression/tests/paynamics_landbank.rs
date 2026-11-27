//! Landbank-specific regression wiring: no credentials or gateway calls required.
use integration_tests::harness::{
    auto_gen::resolve_auto_generate,
    scenario_api::{apply_context_map, get_the_grpc_req_for_connector},
    scenario_loader::{
        load_scenario_for_connector, load_suite_spec_for_connector,
        load_supported_payment_methods_for_connector, scenario_matches_supported_payment_methods,
    },
};
use serde_json::json;

#[test]
fn landbank_authorize_uses_the_supported_bank_transfer_shape() {
    let scenario = load_scenario_for_connector(
        "PaymentService/Authorize",
        "no3ds_auto_capture_landbank",
        "paynamics",
    )
    .expect("Landbank scenario must load");
    let methods = load_supported_payment_methods_for_connector("paynamics")
        .expect("Paynamics capabilities must load");
    assert!(scenario_matches_supported_payment_methods(
        &scenario, &methods
    ));
    assert_eq!(
        scenario.grpc_req["payment_method"],
        json!({
            "local_bank_transfer": {"bank_code": "br_landbank_ph"}
        })
    );
    assert_eq!(scenario.grpc_req["capture_method"], "AUTOMATIC");
    assert_eq!(
        scenario.grpc_req["amount"],
        json!({"minor_amount": 10000, "currency": "PHP"})
    );

    let card = load_scenario_for_connector(
        "PaymentService/Authorize",
        "no3ds_auto_capture_credit_card",
        "paynamics",
    )
    .expect("baseline card scenario must load");
    assert!(!scenario_matches_supported_payment_methods(&card, &methods));
}

#[test]
fn landbank_sync_resolves_its_sale_and_preserves_the_complete_carrier() {
    let spec = load_suite_spec_for_connector("PaymentService/Get", "paynamics")
        .expect("Landbank Sync dependencies must resolve");
    assert_eq!(spec.depends_on.len(), 1);
    let dependency = &spec.depends_on[0];
    assert_eq!(dependency.suite(), "PaymentService/Authorize");
    assert_eq!(dependency.scenario(), Some("no3ds_auto_capture_landbank"));
    let map = dependency
        .context_map()
        .expect("Sync must map its sale context")
        .clone();
    let carrier = json!({"value": "{\"original_sale_request_id\":\"sale-123\",\"original_provider_id\":\"provider-456\"}"});
    let response =
        json!({"connector_transaction_id": "provider-456", "connector_feature_data": carrier});
    let request = json!({"merchant_transaction_id": "merchant-123", "amount": {"minor_amount": 10000, "currency": "PHP"}});
    for name in ["sync_payment", "sync_payment_with_handle_response"] {
        let mut sync = get_the_grpc_req_for_connector("PaymentService/Get", name, "paynamics")
            .expect("Sync template must load");
        apply_context_map(
            &[(map.clone(), request.clone(), response.clone())],
            &mut sync,
        );
        assert_eq!(sync["connector_transaction_id"], "provider-456");
        assert_eq!(sync["connector_feature_data"], carrier);
        assert_eq!(sync["amount"], request["amount"]);
        assert_eq!(sync["capture_method"], "AUTOMATIC");
        assert_eq!(sync["merchant_request_id"], "auto_generate");
        assert_eq!(sync["test_mode"], true);
        resolve_auto_generate(&mut sync, "paynamics")
            .expect("poll reference generation must succeed");
        let poll_id = sync["merchant_request_id"]
            .as_str()
            .expect("poll reference must remain a string");
        assert!(!poll_id.is_empty() && poll_id.len() <= 32);
        assert_ne!(poll_id, "auto_generate");
        assert_ne!(poll_id, "merchant-123");
        assert_eq!(sync["connector_feature_data"], carrier);
    }
}
