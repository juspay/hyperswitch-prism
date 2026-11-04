//! One isolated container per browser job; JSON is exchanged only over pipes.

use std::{
    io::{Read, Write},
    path::PathBuf,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use serde_json::Value;
use uuid::Uuid;

use super::scenario_types::ScenarioError;

const MAX_BYTES: usize = 1_048_576;
const MAX_READ_BYTES: u64 = 1_048_577;

fn failure(message: &str) -> ScenarioError {
    ScenarioError::GrpcurlExecution {
        message: message.to_string(),
    }
}

fn launcher_command() -> Command {
    let mut command = Command::new("bash");
    // Only the host launcher receives Docker configuration. None of these are
    // passed into the browser container by run-worker.sh.
    command.env_clear();
    for key in [
        "PATH",
        "HOME",
        "DOCKER_HOST",
        "DOCKER_CONTEXT",
        "DOCKER_CONFIG",
        "DOCKER_TLS_VERIFY",
        "DOCKER_CERT_PATH",
        "XDG_RUNTIME_DIR",
    ] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    command
}

fn remove_container(runtime: &str, name: &str) {
    if let Ok(mut cleanup) = Command::new(runtime)
        .args(["rm", "--force", name])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if cleanup.try_wait().ok().flatten().is_some() {
                return;
            }
            thread::sleep(Duration::from_millis(20));
        }
        let _ = cleanup.kill();
        let _ = cleanup.wait();
    }
}

struct Job {
    child: Child,
    runtime: String,
    name: String,
}

impl Drop for Job {
    fn drop(&mut self) {
        // Remove the container, not just the npm/Node/Docker client parent PID.
        remove_container(&self.runtime, &self.name);
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub(super) fn execute(payload: &Value) -> Result<Value, ScenarioError> {
    for key in [
        "UCS_BROWSER_AUTOMATION_URL",
        "UCS_BROWSER_AUTOMATION_HOST",
        "UCS_BROWSER_AUTOMATION_PORT",
        "UCS_BROWSER_AUTOMATION_MANAGED",
    ] {
        if std::env::var_os(key).is_some() {
            return Err(failure("HTTP browser automation configuration is no longer supported; use the isolated worker"));
        }
    }
    let runtime =
        std::env::var("UCS_BROWSER_CONTAINER_RUNTIME").unwrap_or_else(|_| "docker".to_string());
    if !matches!(runtime.as_str(), "docker" | "podman") {
        return Err(failure(
            "UCS_BROWSER_CONTAINER_RUNTIME must be docker or podman",
        ));
    }
    let timeout_secs = std::env::var("UCS_BROWSER_AUTOMATION_TIMEOUT_SECS")
        .map_or(Ok(90), |value| value.parse::<u64>())
        .map_err(|_| failure("Invalid browser worker deadline"))?;
    if !(1..=300).contains(&timeout_secs) {
        return Err(failure("Browser worker deadline must be 1..300 seconds"));
    }
    let input = serde_json::to_vec(payload).map_err(|_| failure("Could not encode browser job"))?;
    if input.len() > MAX_BYTES {
        return Err(failure("Browser job exceeds the input limit"));
    }
    let root = std::env::var_os("UCS_BROWSER_AUTOMATION_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../browser-automation-engine")
        });
    let script = root
        .join("scripts/run-worker.sh")
        .canonicalize()
        .map_err(|_| failure("Browser worker launcher not found"))?;
    let name = format!("prism-browser-worker-{}", Uuid::new_v4().simple());
    let mut command = launcher_command();
    command
        .arg(script)
        .env("UCS_BROWSER_CONTAINER_RUNTIME", &runtime)
        .env("UCS_BROWSER_CONTAINER_NAME", &name)
        .env("WORKER_TIMEOUT_MS", (timeout_secs * 1000).to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if let Some(image) = std::env::var_os("UCS_BROWSER_WORKER_IMAGE") {
        command.env("UCS_BROWSER_WORKER_IMAGE", image);
    }
    let child = command
        .spawn()
        .map_err(|_| failure("Could not start the isolated browser worker"))?;
    let mut job = Job {
        child,
        runtime,
        name,
    };
    let mut stdin = job
        .child
        .stdin
        .take()
        .ok_or_else(|| failure("Browser input pipe is unavailable"))?;
    let stdout = job
        .child
        .stdout
        .take()
        .ok_or_else(|| failure("Browser output pipe is unavailable"))?;
    let writer = thread::spawn(move || stdin.write_all(&input));
    let reader = thread::spawn(move || {
        let mut output = Vec::new();
        stdout
            .take(MAX_READ_BYTES)
            .read_to_end(&mut output)
            .map(|_| output)
    });
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    let status = loop {
        match job.child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
            Ok(None) => {
                drop(job);
                let _ = writer.join();
                let _ = reader.join();
                return Err(failure(
                    "Browser worker exceeded its deadline; its container was removed",
                ));
            }
            Err(_) => return Err(failure("Could not monitor browser worker")),
        }
    };
    writer
        .join()
        .map_err(|_| failure("Browser input writer failed"))?
        .map_err(|_| failure("Browser worker did not accept the job"))?;
    let output = reader
        .join()
        .map_err(|_| failure("Browser output reader failed"))?
        .map_err(|_| failure("Could not read browser result"))?;
    if !status.success() || output.len() > MAX_BYTES {
        return Err(failure(
            "Browser worker failed; verify its image, sandbox support and job configuration",
        ));
    }
    let response: Value = serde_json::from_slice(&output)
        .map_err(|_| failure("Browser worker returned invalid JSON"))?;
    if response.get("success").and_then(Value::as_bool) != Some(true) {
        return Err(failure("Browser automation did not complete successfully"));
    }
    Ok(response)
}

#[cfg(test)]
mod tests {
    #[test]
    fn launcher_clears_the_environment_and_only_preserves_container_client_configuration() {
        let command = super::launcher_command();
        for (key, value) in command.get_envs() {
            assert!(value.is_some());
            assert!(matches!(
                key.to_str(),
                Some(
                    "PATH"
                        | "HOME"
                        | "DOCKER_HOST"
                        | "DOCKER_CONTEXT"
                        | "DOCKER_CONFIG"
                        | "DOCKER_TLS_VERIFY"
                        | "DOCKER_CERT_PATH"
                        | "XDG_RUNTIME_DIR"
                )
            ));
        }
        // Inspect the actual child environment as well: env_clear must be effective.
        let output = super::launcher_command()
            .args(["-c", "env"])
            .output()
            .expect("inspect launcher environment");
        let environment = String::from_utf8(output.stdout).expect("UTF-8 environment");
        for line in environment.lines() {
            let key = line.split('=').next().expect("environment key");
            assert!(matches!(
                key,
                "PATH"
                    | "HOME"
                    | "DOCKER_HOST"
                    | "DOCKER_CONTEXT"
                    | "DOCKER_CONFIG"
                    | "DOCKER_TLS_VERIFY"
                    | "DOCKER_CERT_PATH"
                    | "XDG_RUNTIME_DIR"
                    | "PWD"
                    | "SHLVL"
                    | "_"
            ));
        }
    }
}
