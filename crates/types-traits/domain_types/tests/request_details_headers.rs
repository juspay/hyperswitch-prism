#![allow(clippy::unwrap_used)]

use std::collections::HashMap;

use domain_types::{connector_types::RequestDetails, utils::ForeignTryFrom};

#[test]
fn incoming_header_names_are_lowercased() {
    let request = grpc_api_types::payments::RequestDetails {
        method: grpc_api_types::payments::HttpMethod::Post as i32,
        uri: None,
        headers: HashMap::from([
            ("Cko-Signature".to_string(), "sig".to_string()),
            ("x-already-lower".to_string(), "v".to_string()),
        ]),
        body: b"{}".to_vec(),
        query_params: None,
    };
    let details = RequestDetails::foreign_try_from(request).unwrap();
    assert_eq!(
        details.headers.get("cko-signature").map(String::as_str),
        Some("sig")
    );
    assert_eq!(
        details.headers.get("x-already-lower").map(String::as_str),
        Some("v")
    );
    assert!(!details.headers.contains_key("Cko-Signature"));
}
