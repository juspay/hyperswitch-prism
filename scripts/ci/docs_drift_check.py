#!/usr/bin/env python3
"""
Reports which documentation pages a proto change leaves behind.

What this accomplishes:
    In the repo, when a proto changes the documentation reflect this change. 
    Right now, when a proto is updated, the connector docs regenerate, but the api-ref
    does not. 

    Given a base ref, it finds the proto messages and services a PR touched,
    then checks only the documentation for those. Three checks:

      1. Every RPC in a touched service has a page
      2. Every page's field table lists every field of its request and
         response message
      3. Every row in those tables names a field the message actually has

    It reports that diff. This script basically says this thing change, but these
    pages no longer describes it. 

EXIT CODE
    Always 0, so this never blocks a PR. It prints what it found and lets the
    build pass.

    Pass --fail-on-findings to make it exit 1 instead. Do not turn that on
    yet. Right now there is no generator that can fix the pages it reports,
    so failing the build would block proto changes on a docs gap nobody can
    close.

USAGE
    Run from the repo root, with the base branch fetched.

    python3 scripts/ci/docs_drift_check.py --base-ref origin/main

        Prints findings as text. Use this locally before opening a PR.

    python3 scripts/ci/docs_drift_check.py --base-ref origin/main --format github

        Prints the same findings as ::warning:: lines. GitHub Actions turns
        those into annotations on the changed files, so reviewers see them in
        the PR instead of the build log. This is what CI runs.

    --repo PATH           run against a clone elsewhere, defaults to .
    --fail-on-findings    exit 1 instead of 0, see EXIT CODE


   TESTING IT
    Add a field to any message in the proto, commit, and run the check. It
    should name the pages that document that message and list the field as
    undocumented.

        # add `optional string merchant_reference = 10;` to
        # PaymentServiceCreateOrderRequest, then
        git commit -am "test: temporary proto change"
        python3 scripts/ci/docs_drift_check.py --base-ref origin/main
        git reset --hard HEAD~1

    On a branch with no proto changes it prints that and exits, which is the
    other case worth checking.
"""

import argparse
import os
import re
import subprocess
import sys

PROTO_DIR = "crates/types-traits/grpc-api-types/proto"
DOC_TREES = [
    ("api-reference", "docs-generated/api-reference/services")
    # ("sdk/java", "docs-generated/sdks/java"),
    # ("sdk/node", "docs-generated/sdks/node"),
    # ("sdk/python", "docs-generated/sdks/python"),
]
# ----------------------------------------------------------------- git

def changed_protos(repo, base_ref):
    merge_base = _git(repo, "merge-base", base_ref, "HEAD")
    out = _git(repo, "diff", "--name-only", f"{merge_base}...HEAD")
    return [
        p for p in out.split("\n")
        if p.endswith(".proto") and p.startswith(PROTO_DIR)
    ]


def changed_symbols(repo, base_ref, proto_files):
    merge_base = _git(repo, "merge-base", base_ref, "HEAD")
    messages, services = set(), set()

    for path in proto_files:
        diff = _git(repo, "diff", "-U0", f"{merge_base}...HEAD", "--", path)
        for line in diff.split("\n"):
            if not line.startswith("@@"):
                continue
            m = re.search(r"@@.*@@\s*(message|service)\s+(\w+)", line)
            if m:
                (messages if m.group(1) == "message" else services).add(m.group(2))

    return messages, services


def _git(repo, *args):
    return subprocess.run(
        ["git", "-C", repo, *args],
        capture_output=True, text=True, check=True,
    ).stdout.strip()


# --------------------------------------------------------------- proto

def load_proto(repo):
    from grpc_tools import protoc
    from google.protobuf.descriptor_pb2 import FileDescriptorSet
    import tempfile

    proto_dir = os.path.join(repo, PROTO_DIR)
    files = sorted(f for f in os.listdir(proto_dir) if f.endswith(".proto"))
    include = os.path.join(os.path.dirname(protoc.__file__), "_proto")

    with tempfile.NamedTemporaryFile(suffix=".desc", delete=False) as fh:
        out = fh.name
    try:
        rc = protoc.main([
            "protoc",
            f"--proto_path={proto_dir}",
            f"--proto_path={include}",
            f"--descriptor_set_out={out}",
        ] + [os.path.join(proto_dir, f) for f in files])
        if rc != 0:
            sys.exit(f"protoc exited {rc}")
        with open(out, "rb") as fh:
            desc = FileDescriptorSet.FromString(fh.read())
    finally:
        os.unlink(out)

    services = {}
    for f in desc.file:
        for s in f.service:
            services[s.name] = [
                {
                    "name": m.name,
                    "request": m.input_type.split(".")[-1],
                    "response": m.output_type.split(".")[-1],
                }
                for m in s.method
            ]

    messages = {}
    for f in desc.file:
        for m in f.message_type:
            _walk(m, messages)

    return services, messages


def _walk(message, out):
    out[message.name] = [f.name for f in message.field]
    for nested in message.nested_type:
        if not nested.options.map_entry:
            _walk(nested, out)


# ---------------------------------------------------------------- docs

def snake_to_camel(name):
    head, *rest = name.split("_")
    return head + "".join(w.capitalize() for w in rest)


def camel_to_snake(name):
    return re.sub(r"(?<!^)(?=[A-Z])", "_", name).lower()


def slug(name):
    """CreateLink -> create-link, and PayoutService -> payout-service."""
    return re.sub(r"(?<!^)(?=[A-Z])", "-", name).lower()


def table_rows(text, heading):
    m = re.search(
        r"^#+\s*%s\s*$(.*?)(?=^#+\s|\Z)" % re.escape(heading),
        text, re.S | re.M,
    )
    if not m:
        return None
    return re.findall(r"^\|\s*`(\w+)`\s*\|", m.group(1), re.M)


# --------------------------------------------------------------- check

def check(repo, services, messages, touched_messages, touched_services):
    findings = []

    in_scope = set(touched_services)
    for service, rpcs in services.items():
        for rpc in rpcs:
            if rpc["request"] in touched_messages or rpc["response"] in touched_messages:
                in_scope.add(service)

    for service in sorted(in_scope):
        rpcs = services.get(service, [])
        if service in touched_services:
            relevant = rpcs
        else:
            relevant = [
                r for r in rpcs
                if r["request"] in touched_messages
                or r["response"] in touched_messages
            ]
        if not relevant:
            continue

        for tree, root in DOC_TREES:
            directory = os.path.join(repo, root, slug(service))
            if not os.path.isdir(directory):
                findings.append({
                    "kind": "service undocumented",
                    "where": f"{tree}/{slug(service)}",
                    "detail": "no directory for this service",
                })
                continue

            for rpc in relevant:
                page = os.path.join(directory, slug(rpc["name"]) + ".md")
                rel = os.path.relpath(page, repo)

                if not os.path.isfile(page):
                    findings.append({
                        "kind": "no page",
                        "where": rel,
                        "detail": f"{service}.{rpc['name']} has no page",
                    })
                    continue

                with open(page, encoding="utf-8") as fh:
                    text = fh.read()

                for heading, msg_name in (
                    ("Request Fields", rpc["request"]),
                    ("Response Fields", rpc["response"]),
                ):
                    # Only check tables for messages this PR actually touched.
                    # A page can be in scope because its request changed while
                    # its response did not.
                    if msg_name not in touched_messages:
                        continue

                    expected = messages.get(msg_name)
                    if expected is None:
                        continue

                    documented = table_rows(text, heading)
                    if documented is None:
                        findings.append({
                            "kind": "table missing",
                            "where": rel,
                            "detail": f"no {heading} table",
                        })
                        continue

                    present = set(documented)
                    absent = [
                        f for f in expected
                        if f not in present and snake_to_camel(f) not in present
                    ]
                    if absent:
                        findings.append({
                            "kind": "field missing from table",
                            "where": rel,
                            "detail": (
                                f"{msg_name} has {len(expected)} fields, "
                                f"{heading} has {len(documented)} rows. "
                                f"Not documented: {', '.join(absent)}"
                            ),
                        })

                    proto_names = set(expected)
                    unknown = [
                        r for r in documented
                        if r not in proto_names and camel_to_snake(r) not in proto_names
                    ]
                    if unknown:
                        findings.append({
                            "kind": "field not in proto",
                            "where": rel,
                            "detail": (
                                f"{heading} documents fields {msg_name} does not "
                                f"have: {', '.join(unknown)}"
                            ),
                        })

    return findings


# -------------------------------------------------------------- report

def report(findings, touched_messages, touched_services, fmt):
    if not findings:
        print("Docs drift check: no findings for the protos this PR touched.")
        return

    print("Docs drift check")
    print("")
    print(f"Touched services: {', '.join(sorted(touched_services)) or 'none'}")
    print(f"Touched messages: {', '.join(sorted(touched_messages)) or 'none'}")
    print("")
    print(f"{len(findings)} finding(s). These pages no longer match the proto.")
    print("")

    for f in findings:
        if fmt == "github":
            # Renders as an annotation in the Actions log rather than plain text.
            print(f"::warning file={f['where']}::{f['kind']}: {f['detail']}")
        else:
            print(f"  {f['kind']}")
            print(f"    {f['where']}")
            print(f"    {f['detail']}")
            print("")


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--repo", default=".")
    ap.add_argument("--base-ref", default="origin/main")
    ap.add_argument("--format", choices=["text", "github"], default="text")
    ap.add_argument(
        "--fail-on-findings",
        action="store_true",
        help="exit 1 when findings exist. Off by default: until a generator "
             "exists to close them, failing a proto PR on a docs gap blocks "
             "work nobody can act on.",
    )
    args = ap.parse_args()

    protos = changed_protos(args.repo, args.base_ref)
    if not protos:
        print("Docs drift check: this PR touches no proto files.")
        return

    touched_messages, touched_services = changed_symbols(
        args.repo, args.base_ref, protos
    )
    if not touched_messages and not touched_services:
        print("Docs drift check: proto files changed, but no message or "
              "service bodies were touched.")
        return

    services, messages = load_proto(args.repo)
    findings = check(
        args.repo, services, messages, touched_messages, touched_services
    )
    report(findings, touched_messages, touched_services, args.format)

    if findings and args.fail_on_findings:
        sys.exit(1)


if __name__ == "__main__":
    main()