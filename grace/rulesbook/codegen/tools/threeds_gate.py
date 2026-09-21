#!/usr/bin/env python3
"""3DS dispatch gate for UCS connectors.

Three static checks over a connector's source, plus an optional cross-check against
the run's plan.json:

  TDS-01  The PostAuthenticate leg must not move money. The composite loop
          (crates/internal/composite-service/src/payments.rs, PostAuthenticate arm)
          has no break condition, so control always returns to next_authentication_step
          and the only non-looping successor is Authorize -- which charges. A
          PostAuthenticate that already charged therefore produces a second,
          independent, full-amount charge with no CAVV/ECI.
  TDS-02  Every authentication marker the connector declares must be reachable:
          next_authentication_step has to be able to return it, and it must agree
          with plan.json .three_ds.legs_used.
  TDS-03  next_authentication_step must terminate from every reachable state.

Exit codes: 0 pass, 1 fail, 2 could not parse (treated as fail by the caller).
Stdlib only.
"""

import argparse
import json
import os
import re
import sys

MARKERS = ("PreAuthenticate", "Authenticate", "PostAuthenticate")
STEPS = ("PreAuthenticate", "Authenticate", "PostAuthenticate", "Authorize")
REDIRECT_STATES = ("InitialRequest", "RedirectWithParams", "RedirectWithoutParams")
# completed_step domain: None plus Some(<marker>). Authorize is never a completed_step
# (the composite loop breaks unconditionally on that arm) so it is not in this list.
COMPLETED = (None,) + MARKERS


# --------------------------------------------------------------------------- utils

def _strip_comments(src):
    """Blank out // and /* */ comments, preserving offsets and newlines."""
    out = []
    i, n = 0, len(src)
    while i < n:
        two = src[i:i + 2]
        if two == "//":
            j = src.find("\n", i)
            j = n if j < 0 else j
            out.append(" " * (j - i))
            i = j
        elif two == "/*":
            j = src.find("*/", i + 2)
            j = n if j < 0 else j + 2
            out.append("".join(c if c == "\n" else " " for c in src[i:j]))
            i = j
        elif src[i] == '"':
            j = i + 1
            while j < n and src[j] != '"':
                j += 2 if src[j] == "\\" else 1
            j = min(j + 1, n)
            out.append(src[i:j])
            i = j
        else:
            out.append(src[i])
            i += 1
    return "".join(out)


def _match_delim(src, start, open_ch, close_ch):
    """Index just past the delimiter that opens at or after `start`. -1 if unbalanced."""
    i = src.find(open_ch, start)
    if i < 0:
        return -1
    depth = 0
    while i < len(src):
        if src[i] == open_ch:
            depth += 1
        elif src[i] == close_ch:
            depth -= 1
            if depth == 0:
                return i + 1
        i += 1
    return -1


def _line_of(src, idx):
    return src.count("\n", 0, idx) + 1


def _ev(path, src, idx, text, signal):
    return {
        "file": path,
        "line": _line_of(src, idx),
        "text": text.strip()[:200],
        "signal": signal,
    }


# ------------------------------------------------------------------- source loading

def load_sources(connector, src_root):
    """{path: (raw, decommented)} for <root>/<c>.rs and <root>/<c>/*.rs.

    macros.rs is never included: it holds the macro definitions and would match
    every construct this gate greps for.
    """
    files = {}
    top = os.path.join(src_root, connector + ".rs")
    if os.path.isfile(top):
        files[top] = None
    sub = os.path.join(src_root, connector)
    if os.path.isdir(sub):
        for name in sorted(os.listdir(sub)):
            if name.endswith(".rs") and name != "macros.rs":
                files[os.path.join(sub, name)] = None
    out = {}
    for path in files:
        with open(path, "r", encoding="utf-8", errors="replace") as fh:
            raw = fh.read()
        out[path] = (raw, _strip_comments(raw))
    return out


# --------------------------------------------------------- macro / impl block finders

def macro_impl_blocks(text):
    """[(flow_name, body, start_idx)] for each macros::macro_connector_implementation! call."""
    blocks = []
    for m in re.finditer(r"macro_connector_implementation!\s*\(", text):
        end = _match_delim(text, m.start(), "(", ")")
        if end < 0:
            continue
        body = text[m.start():end]
        fm = re.search(r"\bflow_name\s*:\s*(\w+)", body)
        if fm:
            blocks.append((fm.group(1), body, m.start()))
    return blocks


def get_url_body(macro_body):
    """Normalised text of the macro block's get_url fn, or None."""
    m = re.search(r"fn\s+get_url\s*\(", macro_body)
    if not m:
        return None
    sig_end = _match_delim(macro_body, m.start(), "(", ")")
    if sig_end < 0:
        return None
    body_end = _match_delim(macro_body, sig_end, "{", "}")
    if body_end < 0:
        return None
    body = macro_body[macro_body.index("{", sig_end):body_end]
    body = re.sub(r"\bself\.", "", body)
    body = re.sub(r"\breq\b", "", body)
    return re.sub(r"\s+", "", body)


def impl_header_and_body(text, idx):
    """(header, body, body_start) for the `impl` token at idx."""
    brace = text.find("{", idx)
    if brace < 0:
        return None
    # Skip past generics/where-clause angle brackets to find the real body brace.
    i, depth = idx, 0
    while i < len(text):
        c = text[i]
        if c == "<":
            depth += 1
        elif c == ">":
            depth -= 1
        elif c == "{" and depth <= 0:
            brace = i
            break
        i += 1
    end = _match_delim(text, brace, "{", "}")
    if end < 0:
        return None
    return text[idx:brace], text[brace:end], brace


def postauth_request_impls(text):
    """impl blocks that BUILD a PostAuthenticate request (not parse its response)."""
    found = []
    for m in re.finditer(r"\bimpl\b", text):
        hb = impl_header_and_body(text, m.start())
        if not hb:
            continue
        header, body, body_start = hb
        if "PaymentsPostAuthenticateData" not in header and "PaymentsPostAuthenticateData" not in body:
            continue
        # The response direction is `... for RouterDataV2<...>`; skip it.
        tail = header
        depth, cut = 0, None
        for i, c in enumerate(header):
            if c == "<":
                depth += 1
            elif c == ">":
                depth -= 1
            elif depth == 0 and header.startswith(" for ", i):
                cut = i + 5
        if cut is not None:
            tail = header[cut:]
        if re.match(r"\s*RouterDataV2\b", tail):
            continue
        found.append((header, body, body_start))
    return found


# ------------------------------------------------------------------ declared markers

def declared_markers(sources):
    """Markers wired up in create_all_prerequisites! minus those parked as
    not_implemented/not_supported."""
    declared, parked = set(), set()
    for path, (_raw, text) in sources.items():
        for m in re.finditer(r"create_all_prerequisites!\s*\(", text):
            end = _match_delim(text, m.start(), "(", ")")
            if end < 0:
                continue
            for fm in re.finditer(r"\bflow\s*:\s*(\w+)\s*,", text[m.start():end]):
                if fm.group(1) in MARKERS:
                    declared.add(fm.group(1))
        for m in re.finditer(r"macro_connector_flow_status_impls!\s*\(", text):
            end = _match_delim(text, m.start(), "(", ")")
            if end < 0:
                continue
            body = text[m.start():end]
            for key in ("not_implemented", "not_supported"):
                km = re.search(r"\b%s\s*:\s*\[" % key, body)
                if not km:
                    continue
                lend = _match_delim(body, km.start(), "[", "]")
                if lend < 0:
                    continue
                for name in re.findall(r"\w+", body[body.index("[", km.start()):lend]):
                    if name in MARKERS:
                        parked.add(name)
    return declared - parked, parked


# ------------------------------------------------- next_authentication_step extraction

class HookParseError(Exception):
    pass


def _norm_state_pat(pat):
    pat = pat.strip()
    if pat == "_":
        return list(REDIRECT_STATES)
    names = re.findall(r"RedirectState::(\w+)", pat)
    if not names:
        raise HookParseError("unrecognised redirect_state pattern: %r" % pat[:60])
    for n in names:
        if n not in REDIRECT_STATES:
            raise HookParseError("unknown RedirectState::%s" % n)
    return names


def _norm_completed_pat(pat):
    pat = pat.strip()
    if pat == "_":
        return list(COMPLETED)
    out = []
    if re.search(r"\bNone\b", pat):
        out.append(None)
    for n in re.findall(r"AuthenticationStep::(\w+)", pat):
        if n not in STEPS:
            raise HookParseError("unknown AuthenticationStep::%s" % n)
        out.append(n)
    if not out:
        raise HookParseError("unrecognised completed_step pattern: %r" % pat[:60])
    return out


def _split_top(text, sep):
    """Split on `sep` at nesting depth 0 (parens/brackets/angles)."""
    parts, depth, cur = [], 0, []
    for ch in text:
        if ch in "(<[":
            depth += 1
        elif ch in ")>]":
            depth -= 1
        if ch == sep and depth == 0:
            parts.append("".join(cur))
            cur = []
        else:
            cur.append(ch)
    parts.append("".join(cur))
    return parts


def _find_fat_arrow(text, start):
    """Index of the next `=>` at nesting depth 0. Angle brackets are deliberately not
    counted: `>` is part of the arrow itself."""
    depth = 0
    i = start
    while i < len(text) - 1:
        c = text[i]
        if c in "([{":
            depth += 1
        elif c in ")]}":
            depth -= 1
        elif c == "=" and text[i + 1] == ">" and depth == 0:
            return i
        i += 1
    return -1


def parse_hook(sources):
    """[(state_list, completed_list, step, line)] in source order, or None if the fn
    is absent. Raises HookParseError on a shape this gate cannot reason about."""
    for path, (_raw, text) in sources.items():
        m = re.search(r"fn\s+next_authentication_step\s*\(", text)
        if not m:
            continue
        sig_end = _match_delim(text, m.start(), "(", ")")
        if sig_end < 0:
            raise HookParseError("unbalanced signature")
        body_start = text.index("{", sig_end)
        body_end = _match_delim(text, sig_end, "{", "}")
        if body_end < 0:
            raise HookParseError("unbalanced body")
        body = text[body_start:body_end]

        mm = re.search(r"\bmatch\s+(.+?)\s*\{", body, re.S)
        if not mm:
            bare = re.search(r"AuthenticationStep::(\w+)", body)
            if bare:
                return [(list(REDIRECT_STATES), list(COMPLETED), bare.group(1),
                         _line_of(text, body_start + bare.start()))], path
            raise HookParseError("no match expression and no AuthenticationStep returned")

        scrutinee = mm.group(1).strip()
        tuple_form = scrutinee.startswith("(")
        if not tuple_form and "redirect_state" not in scrutinee:
            raise HookParseError("unsupported match scrutinee: %r" % scrutinee[:60])

        arms_end = _match_delim(body, mm.start(), "{", "}")
        if arms_end < 0:
            raise HookParseError("unbalanced match arms")
        arms_src = body[body.index("{", mm.start()) + 1:arms_end - 1]

        arms, pos = [], 0
        while pos < len(arms_src):
            fat = _find_fat_arrow(arms_src, pos)
            if fat < 0:
                break
            pat = arms_src[pos:fat].strip().strip(",").strip()
            if re.search(r"\bif\b", pat):
                raise HookParseError("guarded match arm: %r" % pat[:60])

            # Advance past the arm body: either a braced block or everything up to the
            # next top-level comma. Getting this wrong walks into the next arm's pattern.
            k = fat + 2
            while k < len(arms_src) and arms_src[k].isspace():
                k += 1
            if k < len(arms_src) and arms_src[k] == "{":
                end = _match_delim(arms_src, k, "{", "}")
                if end < 0:
                    raise HookParseError("unbalanced arm block")
                rhs, pos = arms_src[k:end], end
                while pos < len(arms_src) and (arms_src[pos].isspace() or arms_src[pos] == ","):
                    pos += 1
            else:
                depth, e = 0, k
                while e < len(arms_src):
                    c = arms_src[e]
                    if c in "([{":
                        depth += 1
                    elif c in ")]}":
                        depth -= 1
                    elif c == "," and depth == 0:
                        break
                    e += 1
                rhs, pos = arms_src[k:e], e + 1

            rm = re.search(r"AuthenticationStep::(\w+)", rhs)
            if not rm:
                raise HookParseError("arm does not return an AuthenticationStep: %r" % pat[:60])
            step = rm.group(1)
            if step not in STEPS:
                raise HookParseError("unknown AuthenticationStep::%s" % step)

            if pat.startswith("(") and pat.endswith(")"):
                halves = _split_top(pat[1:-1], ",")
                halves = [h for h in halves if h.strip()]
                if len(halves) != 2:
                    raise HookParseError("tuple arm is not a pair: %r" % pat[:60])
                states = _norm_state_pat(halves[0])
                completed = _norm_completed_pat(halves[1])
            elif tuple_form:
                if pat == "_":
                    states, completed = list(REDIRECT_STATES), list(COMPLETED)
                else:
                    raise HookParseError("non-tuple arm in a tuple match: %r" % pat[:60])
            else:
                states = _norm_state_pat(pat) if pat != "_" else list(REDIRECT_STATES)
                completed = list(COMPLETED)
            arms.append((states, completed, step,
                         _line_of(text, body_start + mm.start() + fat)))
        if not arms:
            raise HookParseError("no arms parsed")
        return arms, path
    return None, None


def resolve(arms, state, completed):
    """First-match-wins, like rustc. Falls back to the trait default (Authorize)."""
    for states, completes, step, _line in arms:
        if state in states and completed in completes:
            return step
    return "Authorize"


# ------------------------------------------------------------------------- the checks

def check_tds01(sources, plan, declared):
    """PostAuthenticate must not move money."""
    evidence, warn = [], []

    # Signal A -- the request builder derives a transaction type from the capture method,
    # or otherwise reads capture_method. This is what distinguishes "settle the payment"
    # from "validate the authentication result".
    for path, (raw, text) in sources.items():
        for _header, body, body_start in postauth_request_impls(text):
            for pat, sig in (
                (r"get_from_capture_method", "get_from_capture_method"),
                (r"\brequest\.capture_method\b", "request.capture_method"),
                (r"\bcapture_method\b", "capture_method"),
            ):
                m = re.search(pat, body)
                if m:
                    idx = body_start + m.start()
                    line_text = raw.splitlines()[_line_of(text, idx) - 1] if raw else ""
                    evidence.append(_ev(path, text, idx, line_text, "A:" + sig))
                    break
            m = re.search(r"\brequest\.amount\b", body)
            if m:
                idx = body_start + m.start()
                line_text = raw.splitlines()[_line_of(text, idx) - 1] if raw else ""
                warn.append(_ev(path, text, idx, line_text, "C:request.amount"))

    # Signal B -- PostAuthenticate posts to the same URL as Authorize.
    urls = {}
    for path, (_raw, text) in sources.items():
        for flow, body, start in macro_impl_blocks(text):
            if flow in ("Authorize", "PostAuthenticate"):
                u = get_url_body(body)
                if u:
                    urls.setdefault(flow, []).append((u, path, text, start))
    if "Authorize" in urls and "PostAuthenticate" in urls:
        auth_urls = {u for u, _p, _t, _s in urls["Authorize"]}
        for u, path, text, start in urls["PostAuthenticate"]:
            if u in auth_urls:
                justified = False
                for call in (plan or {}).get("three_ds", {}).get("calls", []) or []:
                    if call.get("maps_to") == "PostAuthenticate" and call.get("same_endpoint_justified"):
                        justified = True
                if not justified:
                    evidence.append(_ev(path, text, start,
                                        "PostAuthenticate get_url == Authorize get_url",
                                        "B:same_endpoint_as_authorize"))

    # TDS-01b -- code declares the marker, the plan does not map a call to it.
    if plan is not None and "PostAuthenticate" in declared:
        td = plan.get("three_ds") or {}
        if td.get("applicable"):
            mapped = [c for c in (td.get("calls") or []) if c.get("maps_to") == "PostAuthenticate"]
            if not mapped:
                evidence.append({
                    "file": "plan.json", "line": 0,
                    "text": "three_ds.calls[] has no entry with maps_to == PostAuthenticate",
                    "signal": "TDS-01b:plan_code_divergence",
                })

    msg = ("PostAuthenticate moves money. The composite loop's PostAuthenticate arm has no "
           "break, so control returns to next_authentication_step and routes to Authorize, "
           "which charges again -- a second full-amount charge with no CAVV/ECI.")
    return {
        "id": "TDS-01", "name": "postauth_charges", "pass": not evidence,
        "evidence": evidence, "warnings": warn,
        "message": msg if evidence else "PostAuthenticate does not move money.",
    }


def check_tds02(declared, parked, arms, hook_path, plan):
    """Declared markers must be reachable, and must agree with the plan."""
    evidence = []
    returned = set()
    if arms:
        for _s, _c, step, _line in arms:
            if step in MARKERS:
                returned.add(step)

    for marker in sorted(declared - returned):
        evidence.append({
            "file": hook_path or "<no next_authentication_step>", "line": 0,
            "text": "%s is wired up but next_authentication_step never returns it" % marker,
            "signal": "TDS-02:dead_leg",
        })
    for marker in sorted(returned & parked):
        evidence.append({
            "file": hook_path or "?", "line": 0,
            "text": "next_authentication_step returns %s, which is parked as "
                    "not_implemented/not_supported (runtime error, not a compile error)" % marker,
            "signal": "TDS-02:returns_unimplemented",
        })
    if plan is not None:
        td = plan.get("three_ds") or {}
        if td.get("applicable"):
            legs = set(td.get("legs_used") or [])
            if legs != declared:
                evidence.append({
                    "file": "plan.json", "line": 0,
                    "text": "three_ds.legs_used %s != declared markers %s"
                            % (sorted(legs), sorted(declared)),
                    "signal": "TDS-02:legs_used_mismatch",
                })
    return {
        "id": "TDS-02", "name": "marker_reachability", "pass": not evidence,
        "evidence": evidence, "warnings": [],
        "message": ("A declared marker the dispatch hook never returns is dead code on the "
                    "composite path." if evidence else "Declared markers are reachable."),
    }


def check_tds03(arms, hook_path, plan):
    """next_authentication_step must terminate from every reachable state."""
    if not arms:
        return {"id": "TDS-03", "name": "termination", "pass": True, "evidence": [],
                "warnings": [], "message": "No dispatch hook; trait default returns Authorize."}

    justified = {b.get("leg") for b in ((plan or {}).get("three_ds", {}) or {}).get(
        "break_justifications", []) or []}
    evidence, warnings = [], []

    for start_state in REDIRECT_STATES:
        seen, completed, guard = [], None, 0
        saw_postauth = False
        while guard < 32:
            guard += 1
            key = (start_state, completed)
            if key in seen:
                cycle = seen[seen.index(key):]
                if saw_postauth:
                    evidence.append({
                        "file": hook_path, "line": 0,
                        "text": "cycle from (%s, None): %s" % (
                            start_state,
                            " -> ".join("(%s, %s)" % (s, c) for s, c in cycle)),
                        "signal": "TDS-03:postauth_cycle",
                    })
                else:
                    legs = {c for _s, c in cycle if c}
                    if not legs & justified:
                        warnings.append({
                            "file": hook_path, "line": 0,
                            "text": "cycle from (%s, None) over %s relies on an unrecorded break"
                                    % (start_state, sorted(legs)),
                            "signal": "TDS-03:unjustified_break",
                        })
                break
            seen.append(key)
            step = resolve(arms, start_state, completed)
            if step == "Authorize":
                break
            if step == "PostAuthenticate":
                saw_postauth = True
            # Pessimistic walk: assume the leg did not produce a break condition.
            completed = step
    return {
        "id": "TDS-03", "name": "termination", "pass": not evidence,
        "evidence": evidence, "warnings": warnings,
        "message": ("next_authentication_step cannot terminate: a PostAuthenticate cycle is "
                    "unconditional, because that arm has no break."
                    if evidence else "Dispatch terminates from every reachable state."),
    }


def main():
    ap = argparse.ArgumentParser(description="3DS dispatch gate for UCS connectors")
    ap.add_argument("--connector", required=True)
    ap.add_argument("--src", default="crates/integrations/connector-integration/src/connectors")
    ap.add_argument("--plan", help="RUN_DIR/plan/plan.json (optional)")
    ap.add_argument("--out", help="write the JSON report here")
    args = ap.parse_args()

    sources = load_sources(args.connector, args.src)
    if not sources:
        print("threeds_gate: no source for connector %r under %s" % (args.connector, args.src),
              file=sys.stderr)
        return 2

    plan = None
    if args.plan and os.path.isfile(args.plan):
        with open(args.plan, "r", encoding="utf-8") as fh:
            plan = json.load(fh)

    declared, parked = declared_markers(sources)
    report = {"connector": args.connector, "pass": True, "checks": [],
              "unparsed": [], "needs_human": []}

    if not declared and not any("PaymentsPostAuthenticateData" in t for _r, t in sources.values()):
        report["checks"].append({
            "id": "TDS-00", "name": "not_applicable", "pass": True, "evidence": [],
            "warnings": [], "message": "Connector declares no authentication markers.",
        })
    else:
        try:
            arms, hook_path = parse_hook(sources)
        except HookParseError as exc:
            arms, hook_path = None, None
            report["unparsed"].append({"what": "next_authentication_step", "why": str(exc)})
            report["needs_human"].append(
                "next_authentication_step could not be parsed (%s); TDS-02 and TDS-03 were "
                "not evaluated. Review by hand." % exc)
        report["checks"].append(check_tds01(sources, plan, declared))
        if not report["unparsed"]:
            report["checks"].append(check_tds02(declared, parked, arms, hook_path, plan))
            report["checks"].append(check_tds03(arms, hook_path, plan))

    for chk in report["checks"]:
        for w in chk.get("warnings") or []:
            report["needs_human"].append("%s: %s" % (chk["id"], w["text"]))
    report["pass"] = all(c["pass"] for c in report["checks"]) and not report["unparsed"]

    blob = json.dumps(report, indent=1)
    if args.out:
        os.makedirs(os.path.dirname(os.path.abspath(args.out)), exist_ok=True)
        with open(args.out, "w", encoding="utf-8") as fh:
            fh.write(blob + "\n")
    print(blob)

    if report["unparsed"]:
        return 2
    return 0 if report["pass"] else 1


if __name__ == "__main__":
    sys.exit(main())
