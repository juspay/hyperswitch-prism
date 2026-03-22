// Sanity / dev-only binary: allow lints that are acceptable for this script.
#![allow(
    clippy::panic_in_result_fn,
    clippy::print_stdout,
    clippy::panic,
    clippy::useless_conversion,
    clippy::unwrap_used
)]

use base64::{engine::general_purpose, Engine as _};
use common_utils::request::{Method, Request, RequestContent};
use domain_types::types::Proxy;
use external_services::service::call_connector_api;
use hyperswitch_masking::Maskable;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;

#[derive(Deserialize)]
struct Manifest {
    scenarios: Vec<Scenario>,
}

#[derive(Deserialize)]
struct Scenario {
    id: String,
    request: RequestDetails,
}

#[derive(Deserialize)]
struct RequestDetails {
    url: String,
    method: String,
    headers: HashMap<String, String>,
    body: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_str = fs::read_to_string("tests/sanity_manifest.json")?;
    let manifest: Manifest = serde_json::from_str(&manifest_str)?;

    let proxy = Proxy {
        http_url: None,
        https_url: None,
        idle_pool_connection_timeout: None,
        bypass_proxy_urls: vec![],
        mitm_proxy_enabled: false,
        mitm_ca_cert: None,
    };

    println!("🎨 [RUST]: Establishing Golden Truth from Manifest...");

    for scenario in manifest.scenarios {
        println!("   Scenario: {}", scenario.id);

        let method = match scenario.request.method.as_str() {
            "POST" => Method::Post,
            "GET" => Method::Get,
            "PUT" => Method::Put,
            "DELETE" => Method::Delete,
            "PATCH" => Method::Patch,
            _ => panic!("Unsupported method"),
        };

        let mut headers = std::collections::HashSet::new();
        for (k, v) in &scenario.request.headers {
            headers.insert((k.clone(), Maskable::Normal(v.clone())));
        }

        let source_id = format!("rust_{}", scenario.id);
        headers.insert((
            "x-source".to_string(),
            Maskable::Normal(source_id.clone().into()),
        ));
        headers.insert((
            "x-scenario-id".to_string(),
            Maskable::Normal(scenario.id.clone().into()),
        ));

        let body = scenario.request.body.map(|b| {
            if b.starts_with("base64:") {
                let raw = general_purpose::STANDARD
                    .decode(b.replace("base64:", ""))
                    .unwrap();
                RequestContent::RawBytes(raw)
            } else {
                RequestContent::RawBytes(b.into_bytes())
            }
        });

        let request = Request {
            url: scenario.request.url.clone(),
            method,
            headers,
            body,
            certificate: None,
            certificate_key: None,
            ca_certificate: None,
        };

        let _result = call_connector_api(&proxy, request, "sanity_gen", false).await?;

        // The Echo Server writes to tests/capture_{source_id}.json
        let capture_file = format!("tests/capture_{}.json", source_id);
        let golden_store = format!("tests/golden_{}.json", scenario.id);

        std::thread::sleep(std::time::Duration::from_millis(250));
        if std::path::Path::new(&capture_file).exists() {
            fs::rename(&capture_file, golden_store)?;
        }
    }
    Ok(())
}
