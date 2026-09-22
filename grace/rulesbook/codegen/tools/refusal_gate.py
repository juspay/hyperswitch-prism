#!/usr/bin/env python3
"""Refusal and capability-parity gate, backed by data/field_probe/<connector>.json.

The connector integration-test harness can assert *what error* a refusal returns
(`IntegrationError.error_code` arrives via the grpc-status-details-bin trailer and is
readable as `error.error_code`), but it cannot assert that **no HTTP request was made**.
`field_probe` can: it calls the request transformer directly and never performs a network
call, recording per flow and per payment-method arm whether the connector builds a request
at all.

  REF-01  a plan negative that claims a payment-method arm is refused must be backed by a
          field_probe status of not_implemented / not_supported for that arm
  CAP-01  the flows field_probe says are supported and the suites specs.json declares must
          agree in both directions
  CAP-02  a wire field the plan says this run added must appear in the probed request body

REF-01 and CAP-02 answer only for an arm the probe actually built. When field-probe has no entry
for the arm the plan names, the check is *inconclusive*, not failing: it is recorded in the check's
`inconclusive[]`, counted in `summary`, and explained in `needs_human` (which arm, which flow key,
which arms the probe does have), and the gate stays green on it. Reading another arm's entry --
notably the `default` one -- in its place is what made this gate permanently red in the nuvei run.

Exit codes: 0 pass, 1 fail, 2 could not evaluate (treated as fail by the caller).
Stdlib only.
"""

import argparse
import json
import os
import sys

PROBE_DIR = "data/field_probe"
SPECS_DIR = "crates/internal/integration-tests/src/connector_specs"

# Flow marker -> the key field_probe files use.
#
# UNVERIFIED. field_probe's flow keys are generated from the connector's `*_req_transformer`
# function names (crates/internal/field-probe/build.rs), not from the flow markers, so this map
# is a best-effort correspondence and is known to be wrong for at least PaymentMethodToken.
# Consequently: a lookup that misses is a note, never a failure, and CAP-01 is advisory. Only
# REF-01 and CAP-02 can fail, and both fail solely on a POSITIVE probe fact (an arm the probe
# reports `supported`), which a wrong key cannot manufacture.
MARKER_TO_PROBE = {
    "Authorize": "authorize", "PSync": "get", "Capture": "capture", "Void": "void",
    "Refund": "refund", "RSync": "refund_get",
    "SetupMandate": "setup_recurring", "RepeatPayment": "recurring_charge",
    "MandateRevoke": "recurring_revoke",
    "CreateConnectorCustomer": "customer_create", "GetConnectorCustomer": "customer_get",
    "PaymentMethodToken": "token_authorize",
    "PaymentMethodEligibility": "payment_method_eligibility",
    "ServerAuthenticationToken": "create_server_authentication_token",
    "ClientAuthenticationToken": "create_client_authentication_token",
    "ServerSessionAuthenticationToken": "create_server_session_authentication_token",
    "PreAuthenticate": "pre_authenticate", "Authenticate": "authenticate",
    "PostAuthenticate": "post_authenticate",
    "CreateOrder": "create_order",
    "IncrementalAuthorization": "incremental_authorization",
}

# Mirrors flow_to_suites() in crates/internal/integration-tests/src/bin/check_connector_specs.rs.
# A marker absent here has no harness suite -- IncomingWebhook and the dispute/payout flows --
# and is provable only end to end.
MARKER_TO_SUITES = {
    "Authorize": ["PaymentService/Authorize"], "PSync": ["PaymentService/Get"],
    "Capture": ["PaymentService/Capture"], "Void": ["PaymentService/Void"],
    "Refund": ["PaymentService/Refund"], "RSync": ["RefundService/Get"],
    "SetupMandate": ["PaymentService/SetupRecurring"],
    "RepeatPayment": ["RecurringPaymentService/Charge"],
    "MandateRevoke": ["RecurringPaymentService/Revoke"],
    "CreateConnectorCustomer": ["CustomerService/Create"],
    "GetConnectorCustomer": ["CustomerService/Get"],
    "PaymentMethodToken": ["PaymentMethodService/Tokenize"],
    "PaymentMethodEligibility": ["PaymentMethodService/Eligibility"],
    "ServerAuthenticationToken": ["MerchantAuthenticationService/CreateServerAuthenticationToken"],
    "ClientAuthenticationToken": ["MerchantAuthenticationService/CreateClientAuthenticationToken"],
    "ServerSessionAuthenticationToken":
        ["MerchantAuthenticationService/CreateServerSessionAuthenticationToken"],
    "PreAuthenticate": ["PaymentMethodAuthenticationService/PreAuthenticate"],
    "Authenticate": ["PaymentMethodAuthenticationService/Authenticate"],
    "PostAuthenticate": ["PaymentMethodAuthenticationService/PostAuthenticate"],
    "CreateOrder": ["PaymentService/CreateOrder"],
    "IncrementalAuthorization": ["PaymentService/IncrementalAuthorization"],
}

REFUSED = ("not_implemented", "not_supported")


def load(path):
    if not os.path.isfile(path):
        return None
    try:
        with open(path, "r", encoding="utf-8") as fh:
            return json.load(fh)
    except (OSError, ValueError):
        return None


def flow_arms(probe, marker):
    """(flow_key, arms dict) for a marker; arms is None when the probe has no such flow."""
    flow_key = MARKER_TO_PROBE.get(marker)
    if not flow_key:
        return None, None
    arms = ((probe or {}).get("flows") or {}).get(flow_key)
    return flow_key, arms if isinstance(arms, dict) else None


def arm_entry(probe, marker, arm):
    """The probe entry for exactly this flow/arm, or None.

    There is deliberately **no** fallback to the `default` arm. field-probe builds `default` from
    one synthetic single-flow request (Card, for the flows that have no per-arm probes), so reading
    it for a NetworkToken / Wallet / NetworkMandateIdCard arm answers a different question than the
    one asked. That fallback produced two permanent false failures in the nuvei run. A missing arm
    is inconclusive -- see inconclusive_note() -- never evidence.
    """
    _, arms = flow_arms(probe, marker)
    if arms is None:
        return None
    entry = arms.get(arm)
    return entry if isinstance(entry, dict) else None


def arm_status(probe, marker, arm):
    """field_probe status for one flow/arm, or None when the probe has no entry for that arm."""
    entry = arm_entry(probe, marker, arm)
    return entry.get("status") if entry else None


def inconclusive_note(check, unit, marker, arm, probe, subject=""):
    """One needs_human line naming the arm, its flow key and the arms the probe actually has."""
    flow_key, arms = flow_arms(probe, marker)
    names = sorted(arms) if arms else []
    have = ("(no such flow in the probe)" if not names else
            ", ".join(names[:12]) + ("" if len(names) <= 12
                                     else ", ... (%d arms in all)" % len(names)))
    return ("%s: %s/%s arm %r%s has no field_probe entry under flow key %r -- inconclusive, not a "
            "failure. The probe has: %s. Extend crates/internal/field-probe to probe this arm."
            % (check, unit, marker, arm, subject, flow_key, have))


def inconclusive_row(check, unit, marker, arm, probe):
    flow_key, arms = flow_arms(probe, marker)
    return {"check": check, "unit": unit, "marker": marker, "arm": arm, "flow_key": flow_key,
            "probe_arms": sorted(arms) if arms else []}


def check_ref01(probe, plan):
    """A claimed payment-method refusal must be visible in the probe."""
    evidence, notes, inconclusive = [], [], []
    for hook in ((plan or {}).get("test_hooks") or []):
        unit = hook.get("unit")
        for neg in (hook.get("negatives") or []):
            arm = neg.get("arm")
            # Only payment-method arms are probe-visible; capture_method / auth_type /
            # currency cells are not a dimension field_probe varies.
            if not arm or neg.get("capture_method") or neg.get("auth_type"):
                continue
            markers = hook.get("markers") or ([unit] if unit in MARKER_TO_PROBE else [])
            for marker in markers:
                status = arm_status(probe, marker, arm)
                if status is None:
                    notes.append(inconclusive_note(
                        "REF-01", unit, marker, arm, probe,
                        " (guard %s)" % neg.get("guard_id")))
                    inconclusive.append(inconclusive_row("REF-01", unit, marker, arm, probe))
                elif status not in REFUSED:
                    evidence.append({
                        "unit": unit, "guard": neg.get("guard_id"), "arm": arm,
                        "detail": "plan says this arm is refused (%s), but field_probe reports "
                                  "status %r -- the connector builds a request for it, so the "
                                  "refusal does not precede request construction"
                                  % (neg.get("expect_error") or "refused", status)})
    return evidence, notes, inconclusive


def check_cap01(probe, specs, plan):
    """Flow/suite parity in both directions."""
    evidence, notes = [], []
    declared = set((specs or {}).get("supported_suites") or [])
    planned = {u.get("unit") for u in ((plan or {}).get("units") or [])}

    for marker, suites in MARKER_TO_SUITES.items():
        flow_key = MARKER_TO_PROBE.get(marker)
        arms = ((probe or {}).get("flows") or {}).get(flow_key) or {}
        supported = any(isinstance(v, dict) and v.get("status") == "supported"
                        for v in arms.values())
        has_suite = any(s in declared for s in suites)

        if supported and not has_suite and (not planned or marker in planned):
            notes.append(
                "CAP-01: field_probe reports a supported arm for %s but specs.json declares none "
                "of %s. Advisory only -- see the note on MARKER_TO_PROBE." % (marker, suites))
        if has_suite and arms and not supported:
            # `error` means the probe could not build a complete request from its synthetic
            # data (a missing required field), not that the connector refuses the flow. Only
            # an explicit not_implemented / not_supported is evidence of absence.
            statuses = {v.get("status") for v in arms.values() if isinstance(v, dict)}
            if statuses <= set(REFUSED):
                notes.append(
                    "CAP-01: specs.json declares %s but every field_probe arm for %s is %s. "
                    "Advisory only -- see the note on MARKER_TO_PROBE."
                    % (suites, marker, "/".join(sorted(statuses))))
            else:
                notes.append(
                    "CAP-01: %s declares %s and field_probe shows no supported arm, but the arms "
                    "are %s (the probe could not build a request, which is not the same as the "
                    "connector refusing). Not treated as a failure."
                    % (marker, suites, "/".join(sorted(statuses))))
    return evidence, notes


def check_cap02(probe, plan):
    """A wire field the plan says this run added must appear in the probed body."""
    evidence, notes, inconclusive = [], [], []
    checked = 0
    for hook in ((plan or {}).get("test_hooks") or []):
        unit = hook.get("unit")
        for field in (hook.get("wire_fields") or []):
            name = field.get("name") if isinstance(field, dict) else field
            arm = (field.get("arm") if isinstance(field, dict) else None) or "Card"
            marker = (field.get("marker") if isinstance(field, dict) else None) or unit
            entry = arm_entry(probe, marker, arm)
            if entry is None:
                notes.append(inconclusive_note(
                    "CAP-02", unit, marker, arm, probe, " carrying %r" % name))
                inconclusive.append(inconclusive_row("CAP-02", unit, marker, arm, probe))
                continue
            body = ((entry.get("sample") or {}).get("body")) or ""
            if entry.get("status") != "supported":
                notes.append("%s: arm %s is not supported in field_probe; %r not checked"
                             % (unit, arm, name))
                continue
            checked += 1
            if name and name not in body:
                evidence.append({
                    "unit": unit, "field": name, "arm": arm,
                    "detail": "plan §8 wire_fields names %r but it does not appear in the probed "
                              "request body for %s/%s" % (name, marker, arm)})
    if not checked:
        notes.append("CAP-02: no plan wire_fields[] to check")
    return evidence, notes, inconclusive


def main():
    ap = argparse.ArgumentParser(description="Refusal / capability-parity gate")
    ap.add_argument("--connector", required=True)
    ap.add_argument("--plan", default="none")
    ap.add_argument("--probe-dir", default=PROBE_DIR)
    ap.add_argument("--specs-dir", default=SPECS_DIR)
    ap.add_argument("--out")
    args = ap.parse_args()

    probe = load(os.path.join(args.probe_dir, "%s.json" % args.connector))
    specs = load(os.path.join(args.specs_dir, args.connector, "specs.json"))
    plan = load(args.plan) if args.plan and args.plan != "none" else None

    report = {"connector": args.connector, "pass": True, "checks": [],
              "unparsed": [], "needs_human": []}

    if probe is None:
        report["unparsed"].append({
            "what": "field_probe",
            "why": "data/field_probe/%s.json missing or unreadable; run `make generate`"
                   % args.connector})
        blob = json.dumps(report, indent=1)
        if args.out:
            os.makedirs(os.path.dirname(os.path.abspath(args.out)), exist_ok=True)
            open(args.out, "w", encoding="utf-8").write(blob + "\n")
        print(blob)
        return 2

    ref01, n1, i1 = check_ref01(probe, plan)
    cap01, n3 = check_cap01(probe, specs, plan)
    cap02, n2, i2 = check_cap02(probe, plan)
    report["needs_human"].extend(n1 + n2 + n3)

    report["checks"] = [
        {"id": "REF-01", "name": "refusal_precedes_request", "pass": not ref01,
         "evidence": ref01, "inconclusive": i1,
         "message": "A refusal the plan claims is not visible in field_probe, which builds the "
                    "request without any network call. Either the guard does not fire before "
                    "request construction, or the plan is wrong." if ref01 else "ok"},
        # Advisory until MARKER_TO_PROBE is verified against the generator; a parity
        # mismatch is reported in needs_human, never as a failure.
        {"id": "CAP-01", "name": "flow_suite_parity", "pass": True, "evidence": [],
         "inconclusive": [],
         "message": "advisory only; findings are in needs_human"},
        {"id": "CAP-02", "name": "wire_field_reaches_request", "pass": not cap02,
         "evidence": cap02, "inconclusive": i2,
         "message": "A field the plan says was added never reaches the built request."
                    if cap02 else "ok"},
    ]
    report["pass"] = all(c["pass"] for c in report["checks"])
    # An arm the probe does not build is not a pass -- it is an unanswered question. Counted here
    # so it stays visible rather than passing silently; it never changes `pass` or the exit code.
    report["summary"] = {
        "failed_checks": sorted(c["id"] for c in report["checks"] if not c["pass"]),
        "evidence": sum(len(c["evidence"]) for c in report["checks"]),
        "inconclusive": sum(len(c["inconclusive"]) for c in report["checks"]),
        "inconclusive_arms": sorted({"%s %s/%s:%s" % (r["check"], r["unit"], r["marker"], r["arm"])
                                     for r in i1 + i2}),
    }

    blob = json.dumps(report, indent=1)
    if args.out:
        os.makedirs(os.path.dirname(os.path.abspath(args.out)), exist_ok=True)
        open(args.out, "w", encoding="utf-8").write(blob + "\n")
    print(blob)
    return 0 if report["pass"] else 1


if __name__ == "__main__":
    sys.exit(main())
