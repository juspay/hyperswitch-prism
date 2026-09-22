#!/usr/bin/env python3
"""A mappings-driven HTTP stand-in for a connector's sandbox, for alpha runs.

An alpha connector has no credentials, so no request in the run ever reaches the real API. This
server answers in its place, from `mappings.json` — a file whose every response body is copied from
an example in the connector's own documentation, with the citation carried alongside it.

WHAT A RUN AGAINST THIS SERVER PROVES
  the request the connector module builds (method, path, headers, body) is the shape the docs
  describe; the response parser, status map, error map and guards execute on a documented body;
  the Hyperswitch -> UCS -> connector plumbing carries it end to end.

WHAT IT CANNOT PROVE, EVER
  that the real API accepts that request, that its live statuses and error codes are these, or that
  any of it moves money. A green run here is evidence of wiring, not of behaviour. That is why an
  alpha run reports FLOW_STATUS DELIVERED_MOCK_ONLY, registers the connector in
  `alpha_connectors.json`, and says so on its pull request.

THE TRAP THIS FILE EXISTS TO AVOID
  a fixture invented by the same run that writes the assertion, so the assertion passes against a
  body nobody ever received from the connector. Every route therefore carries `cite`, and
  `--require-cite` (the default) refuses to serve a route whose citation is missing or empty.

Mappings (`mock/mappings.json`):

  {"connector": "<name>",
   "spec_ref": "<path to the technical specification the bodies were copied from>",
   "routes": [
     {"id": "authorize.approved",
      "match": {"method": "POST", "path": "/payment.do",
                "body_contains": {"transactionType": "Sale"}},
      "response": {"status": 200, "body": {"status": "APPROVED",
                                           "transactionId": "{{uuid}}",
                                           "clientRequestId": "{{req:clientRequestId}}"}},
      "cite": "spec:## Authorize -> Response example",
      "times": null}
   ],
   "default_response": {"status": 501, "body": {"error": "no route matched"}}}

  match      method (exact), path (exact, or a /regex/ when wrapped in slashes), query (subset),
             body_contains (recursive subset of the parsed JSON body; scalars compare equal)
  response   status, headers, body (object, or a string sent verbatim)
  times      how many times the route may serve before it is exhausted and matching falls through
             to the next route of the same shape. This is how a sequence is expressed: a PSync that
             is PENDING once and then CHARGED is two routes, the first with times 1.
  templates  inside any response string: {{req:<dot.path>}} echoes a value from the request body,
             {{uuid}} a fresh uuid4, {{now}} an ISO-8601 UTC timestamp, {{query:<k>}} a query param.

Every request is appended to the log as one JSON line — method, path, headers (authorization and
anything secret-shaped redacted), body, the route that matched and the status served. That log is
the run's evidence of what the connector module actually put on the wire.

Stdlib only. Usage:
  mock_connector.py --mappings mock/mappings.json --port 0 --log mock/requests.jsonl [--pid-file F]
Prints one line to stdout once listening: MOCK_LISTENING <host> <port>
"""

import argparse
import json
import os
import re
import sys
import threading
import uuid
from datetime import datetime, timezone
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlparse

SECRET_HEADERS = {"authorization", "x-api-key", "api-key", "x-auth-token", "cookie", "proxy-authorization"}
REDACTED = "[REDACTED]"


def now_iso():
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def dig(obj, dotted):
    """Value at a dotted path inside a parsed body, or None."""
    cur = obj
    for part in dotted.split("."):
        if isinstance(cur, list):
            try:
                cur = cur[int(part)]
                continue
            except (ValueError, IndexError):
                return None
        if not isinstance(cur, dict) or part not in cur:
            return None
        cur = cur[part]
    return cur


def subset_matches(expected, actual):
    """True when every key/value of `expected` appears in `actual`, recursively."""
    if isinstance(expected, dict):
        if not isinstance(actual, dict):
            return False
        return all(k in actual and subset_matches(v, actual[k]) for k, v in expected.items())
    if isinstance(expected, list):
        if not isinstance(actual, list) or len(expected) > len(actual):
            return False
        return all(any(subset_matches(e, a) for a in actual) for e in expected)
    return expected == actual


def render(value, body, query):
    """Substitute {{req:…}} / {{query:…}} / {{uuid}} / {{now}} inside strings, recursively."""
    if isinstance(value, dict):
        return {k: render(v, body, query) for k, v in value.items()}
    if isinstance(value, list):
        return [render(v, body, query) for v in value]
    if not isinstance(value, str):
        return value

    def one(m):
        token = m.group(1).strip()
        if token == "uuid":
            return str(uuid.uuid4())
        if token == "now":
            return now_iso()
        if token.startswith("req:"):
            got = dig(body if isinstance(body, dict) else {}, token[4:])
            return "" if got is None else str(got)
        if token.startswith("query:"):
            got = query.get(token[6:])
            return got[0] if got else ""
        return m.group(0)

    rendered = re.sub(r"\{\{([^}]+)\}\}", one, value)
    return rendered


def path_matches(pattern, path):
    if pattern is None:
        return True
    if len(pattern) > 1 and pattern.startswith("/") and pattern.endswith("/"):
        return re.search(pattern[1:-1], path) is not None
    return pattern == path


class Mappings:
    def __init__(self, blob, require_cite=True):
        self.connector = blob.get("connector")
        self.spec_ref = blob.get("spec_ref")
        self.routes = blob.get("routes") or []
        self.default = blob.get("default_response") or {
            "status": 501,
            "body": {"error": "no route matched; add one to mappings.json, cited from the docs"},
        }
        self.served = {}
        self.lock = threading.Lock()
        if require_cite:
            uncited = [r.get("id") or "<no id>" for r in self.routes if not (r.get("cite") or "").strip()]
            if uncited:
                raise SystemExit(
                    "mock_connector: these routes carry no `cite`: %s\n"
                    "Every response body must name the documented example it was copied from; an\n"
                    "uncited body is a value this run invented, and a test that passes against it\n"
                    "has graded the run against itself. Cite it, or drop the route."
                    % ", ".join(uncited))

    def pick(self, method, path, query, body):
        with self.lock:
            for idx, route in enumerate(self.routes):
                m = route.get("match") or {}
                if m.get("method") and m["method"].upper() != method:
                    continue
                if not path_matches(m.get("path"), path):
                    continue
                if m.get("query") and not subset_matches(m["query"], {k: v[0] for k, v in query.items()}):
                    continue
                if m.get("body_contains") and not subset_matches(m["body_contains"], body):
                    continue
                cap = route.get("times")
                used = self.served.get(idx, 0)
                if cap is not None and used >= cap:
                    continue
                self.served[idx] = used + 1
                return route
        return None


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    mappings = None
    log_path = None
    log_lock = threading.Lock()

    def log_message(self, *_args):  # the access log is the JSONL file, not stderr
        pass

    def _record(self, entry):
        if not self.log_path:
            return
        line = json.dumps(entry, sort_keys=True)
        with self.log_lock:
            with open(self.log_path, "a", encoding="utf-8") as fh:
                fh.write(line + "\n")

    def _handle(self, method):
        parsed = urlparse(self.path)
        query = parse_qs(parsed.query)
        length = int(self.headers.get("content-length") or 0)
        raw = self.rfile.read(length).decode("utf-8", "replace") if length else ""
        try:
            body = json.loads(raw) if raw else {}
        except ValueError:
            body = {}

        route = self.mappings.pick(method, parsed.path, query, body)
        spec = (route or {}).get("response") or self.mappings.default
        status = int(spec.get("status", 200))
        payload = render(spec.get("body", {}), body, query)
        blob = payload if isinstance(payload, str) else json.dumps(payload)
        blob = blob.encode("utf-8")

        self.send_response(status)
        for key, value in (spec.get("headers") or {"content-type": "application/json"}).items():
            self.send_header(key, value)
        self.send_header("content-length", str(len(blob)))
        self.end_headers()
        self.wfile.write(blob)

        headers = {k.lower(): (REDACTED if k.lower() in SECRET_HEADERS else v)
                   for k, v in self.headers.items()}
        self._record({
            "ts": now_iso(), "method": method, "path": parsed.path,
            "query": {k: v for k, v in query.items()}, "headers": headers,
            "request_body": body if body else (raw if raw else None),
            "matched_route": (route or {}).get("id"),
            "cite": (route or {}).get("cite"),
            "response_status": status,
            "response_body": payload,
            "unmatched": route is None,
        })

    def do_GET(self):
        self._handle("GET")

    def do_POST(self):
        self._handle("POST")

    def do_PUT(self):
        self._handle("PUT")

    def do_PATCH(self):
        self._handle("PATCH")

    def do_DELETE(self):
        self._handle("DELETE")


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--mappings", required=True)
    ap.add_argument("--port", type=int, default=0, help="0 asks the OS for a free port")
    ap.add_argument("--host", default="127.0.0.1")
    ap.add_argument("--log", help="JSONL request log; the run's evidence of what went on the wire")
    ap.add_argument("--pid-file")
    ap.add_argument("--port-file", help="write the bound port here, for a caller that passed 0")
    ap.add_argument("--allow-uncited", action="store_true",
                    help="serve routes with no citation (never in a run that will raise a PR)")
    ap.add_argument("--check", action="store_true",
                    help="validate the mappings and exit — 0 usable, 1 not. Serves nothing, binds no port")
    args = ap.parse_args()

    try:
        with open(args.mappings, "r", encoding="utf-8") as fh:
            blob = json.load(fh)
    except (OSError, ValueError) as exc:
        raise SystemExit("mock_connector: cannot read %s: %s" % (args.mappings, exc))

    mappings = Mappings(blob, require_cite=not args.allow_uncited)

    if args.check:
        ids = [r.get("id") for r in mappings.routes]
        dupes = sorted({i for i in ids if i and ids.count(i) > 1})
        nameless = sum(1 for i in ids if not i)
        problems = []
        if dupes:
            problems.append("duplicate route ids: %s" % ", ".join(dupes))
        if nameless:
            problems.append("%d route(s) with no id" % nameless)
        if not mappings.routes:
            problems.append("no routes: every request would get the 501 default")
        for p in problems:
            print("mock_connector: %s" % p, file=sys.stderr)
        if problems:
            return 1
        print("mock_connector: %d route(s), all cited, for connector %s"
              % (len(mappings.routes), mappings.connector))
        return 0

    Handler.mappings = mappings
    Handler.log_path = args.log
    if args.log:
        os.makedirs(os.path.dirname(os.path.abspath(args.log)), exist_ok=True)

    server = ThreadingHTTPServer((args.host, args.port), Handler)
    host, port = server.server_address[0], server.server_address[1]
    if args.pid_file:
        with open(args.pid_file, "w", encoding="utf-8") as fh:
            fh.write(str(os.getpid()))
    if args.port_file:
        with open(args.port_file, "w", encoding="utf-8") as fh:
            fh.write(str(port))
    print("MOCK_LISTENING %s %d" % (host, port), flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()
    return 0


if __name__ == "__main__":
    sys.exit(main())
