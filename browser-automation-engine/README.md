# Browser automation worker

The connector test harness starts one disposable container for each browser job.
It sends one JSON request through stdin and reads one JSON result from stdout.
The browser and container exit after the job. There is no HTTP execution server,
listening port, screenshot capture, or arbitrary JavaScript DSL action.

## Setup and execution

Install Docker (or rootless Podman), then build the image from this directory:

```sh
npm run build:worker
```

Run a job through the isolated launcher:

```sh
npm run --silent cli < examples/example-com.json
```

`npm start` and `npm run dev` use the same container launcher. No watcher or
background service is started. File input/output arguments are unsupported;
pass the request through stdin. Local Node/TypeScript dependencies are needed
only for source checks and tests (`npm ci --ignore-scripts`, `npm run check`,
`npm test`), not for an already-built worker image.

The Rust harness uses this launcher automatically for matching connector hooks.
It requires an already-built image and never falls back to host execution.
`./scripts/setup-connector-tests.sh` builds the image during local setup; GitHub
connector certification builds and checks it before running scenarios.

## Job format

```json
{
  "url": "https://example.com/",
  "rules": [
    { "action": "extract", "selector": "h1", "as": "heading" }
  ],
  "options": {
    "defaultTimeoutMs": 12000,
    "navigationTimeoutMs": 25000
  }
}
```

Supported actions are `goto`, `click`, `fill`, `press`, `waitFor`, `assertText`,
`assertVisible`, `extract`, `extractAll`, and `initializeCashfreeSandbox`.
The Cashfree operation is restricted to its HTTPS sandbox origin. Unknown
`screenshot` and `evaluate` actions are rejected. Screenshots have also been
removed from the separate wallet token-generator utilities.

For 3DS POST redirects, `formFields` is a map of strings. The worker builds DOM
form elements in memory and submits them through the browser, preserving browser
cookies and form semantics. It never writes a temporary HTML file. `callbackUrl`
identifies a completion origin/path that is fulfilled locally without contacting
that destination. The final URL and its query parameters remain available to the
harness for result mapping.

## Isolation and limits

- The container runs as UID/GID 10001 with a read-only root filesystem, no Linux
  capabilities, a private temporary filesystem, and CPU/memory/PID limits.
- The image entrypoint sets `no_new_privs` before starting Node and Chromium.
  Chromium sandboxing is enabled and must work on the host; it is never silently
  disabled. The bundled seccomp profile permits the namespaces Chromium needs.
- No checkout, home directory, credential file, saved login profile, Docker
  socket, or host environment is mounted/passed into the container. Only the
  payment fields needed for the current job cross stdin. These fields and results
  are sensitive: callers must not log raw jobs or results.
- A per-job loopback forward proxy checks all resolved addresses and connects to
  the checked IP. It blocks local/private/link-local/reserved destinations and
  nonstandard ports, including redirect connections. Only public HTTP(S)
  destinations on ports 80/443 are supported. Its internal ephemeral port accepts
  network proxy traffic, never automation instructions, and is not published.
- Downloads and service workers are disabled. There is no persistent browser
  profile. The worker never captures screenshots, videos or traces.
- Jobs/results are limited to 1 MiB, 100 rules, bounded timeouts and a viewport of
  at most 1920 x 1080. Default total lifetime is 90 seconds. On failure, timeout,
  cancellation or normal exit, the launcher removes the entire container.

The proxy is a browser request control, not a replacement for deployment egress
firewalls or a hardened runtime against a browser/OS exploit. A process controlling
the Docker daemon is trusted and can override containers; do not give untrusted
users or jobs access to that daemon. This change does not prove historical
credential theft or automatically revoke previously accessible credentials.

## Configuration

| Variable | Default | Purpose |
| --- | --- | --- |
| `UCS_BROWSER_CONTAINER_RUNTIME` | `docker` | `docker` or `podman`; no host execution mode |
| `UCS_BROWSER_WORKER_IMAGE` | `prism-browser-worker:local` | Trusted, prebuilt worker image |
| `UCS_BROWSER_AUTOMATION_DIR` | checkout worker directory | Harness location of the launcher |
| `UCS_BROWSER_AUTOMATION_TIMEOUT_SECS` | `90` | Harness deadline, 1–300 seconds |
| `UCS_BROWSER_AUTOMATION_SLOW_MO_MS` | `0` | Optional bounded browser delay |

The HTTP variables `UCS_BROWSER_AUTOMATION_URL`, `HOST`, `PORT`, and `MANAGED`
(with their `UCS_BROWSER_AUTOMATION_` prefix) are rejected. Headed generic jobs
are unsupported; remove `UCS_BROWSER_AUTOMATION_HEADED=true` from old test setup.

The separate Google Pay/Apple Pay token generators remain trusted, local wallet
utilities. They intentionally consume wallet configuration and, for Google Pay,
a saved login session. They are not exposed through this worker or an HTTP
execution API. Their credential/session handling must be considered separately
when enabling those wallet flows in a shared runner.

## Verification

```sh
npm run check
npm test
npm run build:worker
npm run test:container
```

The container suite verifies process privileges, host file/environment isolation,
job cleanup, local-file/script/screenshot rejection, callback/form behavior, and
actual Chromium execution. It must fail if the required container sandbox cannot
start. The seccomp profile comes from Playwright v1.58.2's
`utils/docker/seccomp_profile.json` (Apache-2.0); update it deliberately alongside
runtime compatibility tests. This copy additionally allows `chroot` for Chromium's
user-namespace sandbox when the outer container has dropped all capabilities;
the outer process is still non-root and cannot chroot without a new namespace.
