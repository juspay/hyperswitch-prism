//! Thin Cucumber step definitions for HTTP client sanity tests.
//!
//! Execute the SDK request and write actual JSON. All assertion/normalization
//! logic is delegated to the shared judge_scenario.js (single source of truth).

use base64::{engine::general_purpose, Engine as _};
use common_utils::request::Method;
use cucumber::{gherkin::Step, given, then, when, World};
use hyperswitch_payments_client::http_client::{HttpClient, HttpOptions, HttpRequest, ProxyConfig};
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::LazyLock;

const LANG: &str = "rust";

fn artifacts_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("tests")
        .join("client_sanity")
        .join("artifacts")
}

fn judge_script() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("tests")
        .join("client_sanity")
        .join("judge_scenario.js")
}

fn manifest_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("tests")
        .join("client_sanity")
        .join("manifest.json")
}

/// Build title → id lookup once.
static TITLE_TO_ID: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
    let data = fs::read_to_string(manifest_path()).expect("Failed to read manifest.json");
    let manifest: serde_json::Value = serde_json::from_str(&data).expect("Failed to parse manifest");
    manifest["scenarios"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|s| {
            let title = s["title"].as_str()?.to_string();
            let id = s["id"].as_str()?.to_string();
            Some((title, id))
        })
        .collect()
});

/// Scenarios from manifest.json that are skipped for Rust.
static SKIP_IDS: LazyLock<Vec<String>> = LazyLock::new(|| {
    let data = fs::read_to_string(manifest_path()).expect("Failed to read manifest.json");
    let manifest: serde_json::Value = serde_json::from_str(&data).expect("Failed to parse manifest");
    manifest["scenarios"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|s| {
            let skip_langs = s["skip_langs"].as_array()?;
            if skip_langs.iter().any(|l| l.as_str() == Some("rust")) {
                Some(s["id"].as_str()?.to_string())
            } else {
                None
            }
        })
        .collect()
});

#[derive(Debug, Default, World)]
pub struct SanityWorld {
    base_url: String,
    method: String,
    url: String,
    headers: HashMap<String, String>,
    query_params: Vec<(String, String)>,
    body: Option<String>,
    proxy_url: Option<String>,
    response_timeout_ms: Option<u32>,
    scenario_id: String,
    source_id: String,
    skipped: bool,
    judged: bool,
}

impl SanityWorld {
    /// Resolve the full URL from base + path + query params.
    fn resolve_url(&self) -> String {
        let mut url = if self.url.starts_with('/') {
            format!("{}{}", self.base_url, self.url)
        } else {
            self.url.clone()
        };
        if !self.query_params.is_empty() {
            let qs: Vec<String> = self.query_params.iter().map(|(k, v)| format!("{}={}", k, v)).collect();
            url = format!("{}?{}", url, qs.join("&"));
        }
        url
    }
}

#[derive(Serialize)]
struct RunnerOutput {
    #[serde(skip_serializing_if = "Option::is_none")]
    response: Option<SdkResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<SdkError>,
}

#[derive(Serialize)]
struct SdkResponse {
    #[serde(rename = "statusCode")]
    status_code: u16,
    headers: HashMap<String, String>,
    body: String,
}

#[derive(Serialize)]
struct SdkError {
    code: String,
    message: String,
}

// ── Given ───────────────────────────────────────────────────────

#[given(expr = "the echo server is running on port {int}")]
async fn echo_server_running(w: &mut SanityWorld, port: i32) {
    w.base_url = format!("http://localhost:{}", port);
}

#[given(expr = "a {string} request to {string}")]
async fn set_request(w: &mut SanityWorld, method: String, url: String) {
    w.method = method;
    w.url = url;
}

#[given(expr = "query parameter {string} is {string}")]
async fn set_query_param(w: &mut SanityWorld, name: String, value: String) {
    w.query_params.push((name, value));
}

#[given(expr = "header {string} is {string}")]
async fn set_header(w: &mut SanityWorld, name: String, value: String) {
    w.headers.insert(name, value);
}

/// Unescape \uXXXX to actual Unicode in body strings (e.g. \uFEFF → BOM).
fn unescape_unicode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' && chars.peek() == Some(&'u') {
            chars.next();
            let hex: String = chars.by_ref().take(4).collect();
            if hex.len() == 4 {
                if let Ok(code) = u32::from_str_radix(&hex, 16) {
                    if let Some(u) = char::from_u32(code) {
                        out.push(u);
                        continue;
                    }
                }
            }
            out.push('\\');
            out.push('u');
            out.push_str(&hex);
        } else {
            out.push(c);
        }
    }
    out
}

#[given(expr = "body is {string}")]
async fn set_body(w: &mut SanityWorld, body: String) {
    let s = body.replace("\\r\\n", "\r\n").replace("\\n", "\n");
    w.body = Some(unescape_unicode(&s));
}

#[given("body is:")]
async fn set_body_docstring(w: &mut SanityWorld, step: &Step) {
    let text = step.docstring.as_ref().expect("Missing doc string for 'body is:'").as_str().trim();
    let ct = w.headers.get("Content-Type").map(|s| s.to_lowercase()).unwrap_or_default();
    if ct.contains("multipart/") {
        w.body = Some(text.replace('\n', "\r\n") + "\r\n");
    } else {
        w.body = Some(text.to_string());
    }
}

#[given(expr = "a response timeout of {int} ms")]
async fn set_timeout(w: &mut SanityWorld, ms: i32) {
    w.response_timeout_ms = Some(ms as u32);
}

#[given(expr = "the proxy is {string}")]
async fn set_proxy(w: &mut SanityWorld, url: String) {
    w.proxy_url = Some(url);
}

// ── When (thin: execute + write actual JSON) ────────────────────

#[when("the request is sent")]
async fn execute_request(w: &mut SanityWorld) {
    if w.scenario_id.is_empty() {
        panic!("Could not resolve scenario ID from Gherkin title");
    }

    if SKIP_IDS.contains(&w.scenario_id) {
        w.skipped = true;
        return;
    }

    let actual_file = artifacts_dir().join(format!("actual_{}.json", w.source_id));
    let capture_file = artifacts_dir().join(format!("capture_{}.json", w.source_id));
    let _ = fs::remove_file(&actual_file);
    let _ = fs::remove_file(&capture_file);

    let full_url = w.resolve_url();

    let method = match w.method.as_str() {
        "POST" => Method::Post,
        "GET" => Method::Get,
        "PUT" => Method::Put,
        "DELETE" => Method::Delete,
        "PATCH" => Method::Patch,
        other => {
            write_output(&actual_file, &RunnerOutput {
                response: None,
                error: Some(SdkError { code: "RUNNER_CRASH".into(), message: format!("Unsupported: {}", other) }),
            });
            return;
        }
    };

    let mut headers = w.headers.clone();
    headers.insert("x-source".into(), w.source_id.clone());
    headers.insert("x-scenario-id".into(), w.scenario_id.clone());

    let body = w.body.as_ref().map(|b| {
        if b.starts_with("base64:") {
            general_purpose::STANDARD.decode(b.trim_start_matches("base64:")).unwrap_or_default()
        } else {
            b.as_bytes().to_vec()
        }
    });

    let request = HttpRequest { url: full_url, method, headers, body };

    let proxy_cfg = w.proxy_url.as_ref().map(|url| ProxyConfig {
        http_url: Some(url.clone()),
        https_url: None,
        bypass_urls: vec![],
    });

    let options = HttpOptions {
        proxy: proxy_cfg,
        response_timeout_ms: w.response_timeout_ms,
        ..Default::default()
    };

    let client = match HttpClient::new(options) {
        Ok(c) => c,
        Err(e) => {
            write_output(&actual_file, &RunnerOutput {
                response: None,
                error: Some(SdkError { code: e.error_code().to_string(), message: e.to_string() }),
            });
            return;
        }
    };

    let output = match client.execute(request, None).await {
        Ok(resp) => {
            let ct = resp.headers.get("content-type").map(|s| s.to_lowercase()).unwrap_or_default();
            let body_str = if ct.contains("application/octet-stream") {
                general_purpose::STANDARD.encode(&resp.body)
            } else {
                String::from_utf8_lossy(&resp.body).into_owned()
            };
            RunnerOutput {
                response: Some(SdkResponse { status_code: resp.status_code, headers: resp.headers, body: body_str }),
                error: None,
            }
        }
        Err(e) => RunnerOutput {
            response: None,
            error: Some(SdkError { code: e.error_code().to_string(), message: e.to_string() }),
        },
    };

    write_output(&actual_file, &output);
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
}

// ── Then (delegate ALL assertions to the shared judge) ──────────

#[then(expr = "the response status should be {int}")]
async fn check_status(w: &mut SanityWorld, _expected: i32) {
    if w.skipped { return; }
}

#[then(expr = "the response body should be {string}")]
async fn check_body(w: &mut SanityWorld, _expected: String) {
    if w.skipped { return; }
}

#[then(expr = "the response header {string} should be {string}")]
async fn check_header(w: &mut SanityWorld, _name: String, _value: String) {
    if w.skipped { return; }
}

#[then(expr = "the response should have multi-value header {string} with values {string}")]
async fn check_multi_header(w: &mut SanityWorld, _name: String, _values: String) {
    if w.skipped { return; }
}

#[then(expr = "the SDK should return error {string}")]
async fn check_error(w: &mut SanityWorld, _code: String) {
    if w.skipped { return; }
    run_judge(w);
}

#[then("the server should have received the correct request")]
async fn check_capture(w: &mut SanityWorld) {
    if w.skipped { return; }
    run_judge(w);
}

// ── Helpers ─────────────────────────────────────────────────────

fn write_output(path: &PathBuf, output: &RunnerOutput) {
    if let Ok(json) = serde_json::to_string_pretty(output) {
        let _ = fs::write(path, json);
    }
}

fn run_judge(w: &mut SanityWorld) {
    if w.judged { return; }
    w.judged = true;
    let result = Command::new("node")
        .arg(judge_script())
        .arg(LANG)
        .arg(&w.scenario_id)
        .output()
        .expect("Failed to run judge_scenario.js");
    if !result.status.success() {
        let stdout = String::from_utf8_lossy(&result.stdout);
        let msg = serde_json::from_str::<serde_json::Value>(&stdout)
            .ok()
            .and_then(|v| v["message"].as_str().map(String::from))
            .unwrap_or_else(|| format!("Judge FAILED for {}", w.scenario_id));
        panic!("{}", msg);
    }
}

#[tokio::main]
async fn main() {
    let features_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("tests")
        .join("client_sanity")
        .join("features");

    SanityWorld::cucumber()
        .with_default_cli()
        .before(move |_feature, _rule, scenario, world| {
            Box::pin(async move {
                let title = &scenario.name;
                if let Some(id) = TITLE_TO_ID.get(title.as_str()) {
                    world.scenario_id = id.clone();
                    world.source_id = format!("{}_{}", LANG, id);
                }
            })
        })
        .run_and_exit(features_path)
        .await;
}
