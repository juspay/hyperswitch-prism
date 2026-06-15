# Command Cookbook — coverage-report

All commands are **read-only** and **print inline**. Run from the repo root
(`/Users/tushar.shukla/Downloads/Work/UCS-dup/hyperswitch-prism`). They reuse
`scripts/generators/docs/coverage_summary.py` (`compute` / `load_probe_files` / `normalize_status`) so the
same aggregation rules apply to every snapshot (`not_supported` excluded, `error`→`not_implemented`).

**"Latest main" = `prism/main` (juspay/hyperswitch-prism)** — the remote your PRs merge into. Coverage is read
**straight from git**; nothing stashes, checks out, or pulls, so your working tree / branch / stash are never
touched. Set the knobs once:
```bash
SINCE=2026-05-01      # baseline: date YYYY-MM-DD | CalVer tag | Nd (e.g. 30d) | commit SHA
BASE=prism/main       # the "latest main" ref (default). PR activity (§2) MUST use the same remote.
SOURCE=base           # "base" = compare baseline vs latest main (default). "worktree" = baseline vs your checkout.
```

---

## §0 Sync to latest main (run first; offline-tolerant)

```bash
git fetch prism main 2>/dev/null && echo "✓ fetched prism/main" \
  || echo "⚠ fetch failed/offline — using cached prism/main (may be stale)"
git log -1 --format='latest main → %h  %ci  %s' prism/main
ls -l .git/FETCH_HEAD 2>/dev/null | awk '{print "   last fetch:", $6, $7, $8}'
```
This is the **only** network step. It updates the `prism/main` remote-tracking ref; it does **not** modify
your working tree, branch, or stash.

---

## §1 Diff (flagship) — supported deltas + PM/API drift + caveats

Compares the baseline to **latest main** (`$BASE`), both read from git. Prints (a) supported COUNT deltas
(trustworthy), (b) **total drift for PM and API** (measurement churn), and (c) shares gated by the drift check.

```bash
python3 - "$SINCE" "$BASE" "$SOURCE" <<'PY'
import sys, json, subprocess
from collections import Counter
sys.path.insert(0, "scripts/generators/docs")
import coverage_summary as cs

since, base_ref, source = sys.argv[1], sys.argv[2], sys.argv[3]
def sh(*a): return subprocess.run(a, capture_output=True, text=True).stdout.strip()

def resolve(rev, base):
    if rev.endswith("d") and rev[:-1].isdigit():
        return sh("git","rev-list","-1",f"--before={rev[:-1]} days ago",base)
    if len(rev)==10 and rev[4]=="-" and rev[7]=="-":
        return sh("git","rev-list","-1",f"--before={rev}",base)
    return sh("git","rev-parse","--verify",f"{rev}^{{commit}}")

def load_git(rev):
    conns={}
    for p in sh("git","ls-tree","-r","--name-only",rev,"data/field_probe").split():
        if not p.endswith(".json"): continue
        try: d=json.loads(subprocess.run(["git","show",f"{rev}:{p}"],capture_output=True,text=True).stdout)
        except Exception: continue
        if d.get("connector") and isinstance(d.get("flows"),dict): conns[d["connector"]]=d["flows"]
    return conns

baseline = resolve(since, base_ref)
assert baseline, f"could not resolve baseline {since!r} on {base_ref}"
bc = load_git(baseline)
nc = cs.load_probe_files(cs.DEFAULT_PROBE_DIR) if source=="worktree" else load_git(base_ref)
now_label = "your working tree" if source=="worktree" else base_ref

# --- supported counts (trustworthy) ---
def summ(conns):
    agg=cs.compute(conns)
    pm=Counter()
    for c in agg["pm_status"].values(): pm.update(c)
    api=Counter()
    for b in agg["flow_status_conns"].values():
        for st,names in b.items(): api[st]+=len(names)
    return dict(conns=agg["n_connectors"], pm_sup=pm.get("supported",0), pm_tot=sum(pm.values()),
                api_sup=api.get("supported",0), api_tot=sum(api.values()))
b, n = summ(bc), summ(nc)
rel = lambda o,c: f"{(100.0*(c-o)/o):+.1f}%" if o else "n/a"

# --- Coverage Growth Summary (markdown) ---
def frac(s, t):    # "supported/total (pct%)"  — total = supported + not_implemented (not_supported excluded)
    return f"{s}/{t} ({(100.0*s/t if t else 0):.1f}%)"

rows = [
    ("Connectors",             str(b['conns']),                 str(n['conns']),                 n['conns']-b['conns'],     rel(b['conns'],n['conns'])),
    ("API Supported (Flows)",  frac(b['api_sup'],b['api_tot']), frac(n['api_sup'],n['api_tot']), n['api_sup']-b['api_sup'], rel(b['api_sup'],n['api_sup'])),
    ("PM Supported (Methods)", frac(b['pm_sup'],b['pm_tot']),   frac(n['pm_sup'],n['pm_tot']),   n['pm_sup']-b['pm_sup'],   rel(b['pm_sup'],n['pm_sup'])),
]
print(f"### Coverage Growth Summary  (baseline {baseline[:9]} @ {since} → {now_label})\n")
print("| Metric | Base | Current | Change | Growth |")
print("| --- | ---: | ---: | ---: | ---: |")
for m, bb, cc, ch, gr in rows:
    print(f"| {m} | {bb} | {cc} | {ch:+d} | {gr} |")

print("\n**Highlights**\n")
print(f"* Added **{n['conns']-b['conns']} new connectors**, total coverage **{b['conns']} → {n['conns']}** (**{rel(b['conns'],n['conns'])}**).")
print(f"* API-supported flows **{b['api_sup']} → {n['api_sup']}**, adding **{n['api_sup']-b['api_sup']} flows** (**{rel(b['api_sup'],n['api_sup'])}**).")
print(f"* PM-supported methods **{b['pm_sup']} → {n['pm_sup']}**, adding **{n['pm_sup']-b['pm_sup']} methods** (**{rel(b['pm_sup'],n['pm_sup'])}**).")
_d = {"API-supported flows": n['api_sup']-b['api_sup'], "PM-supported methods": n['pm_sup']-b['pm_sup'], "connectors": n['conns']-b['conns']}
_s = max(_d, key=_d.get)
print(f"\n**Overall:** Consistent growth across all coverage dimensions, with the strongest increase in {_s} (+{_d[_s]}).")
print("\n> Note: the API/PM `%` is supported ÷ (supported + not_implemented). Its denominator shrinks as flows")
print("> are reclassified to not_supported, so quote the **counts / Change** as the real growth — not the %.")

P=lambda s,t:(100.0*s/t if t else 0)
print("\nShares:")
print(f"  API supported share: {P(b['api_sup'],b['api_tot']):.1f}% → {P(n['api_sup'],n['api_tot']):.1f}%")
print(f"  PM  supported share: {P(b['pm_sup'],b['pm_tot']):.1f}% → {P(n['pm_sup'],n['pm_tot']):.1f}%")
shift = total_drift >= 50
print("\n" + ("⚠ DRIFT DETECTED — do NOT quote shares as growth; quote supported COUNTS above."
              if shift else "✓ Low drift — shares are comparable this window."))
PY
```

---

## §2 PR activity — what merged in the window (same remote as $BASE)

```bash
gh pr list --repo juspay/hyperswitch-prism --state merged \
  --search "merged:>=$SINCE" --limit 300 --json number,title,labels \
| python3 -c '
import sys, json, re, collections
prs = json.load(sys.stdin)
real = [p for p in prs if not p["title"].lower().startswith("chore(version)")]
pref, labels = collections.Counter(), collections.Counter()
for p in real:
    m = re.match(r"^([A-Za-z]+(?:\([^)]+\))?)", p["title"].strip())
    pref[(m.group(1).lower() if m else "(other)")] += 1
    for l in p.get("labels", []): labels[l["name"]] += 1
conn = sum(1 for p in real if p["title"].lower().startswith(("feat(connector","fix(connector")))
print(f"Merged PRs (excl. chore(version)): {len(real)}  [{len(prs)} incl. release bumps]")
print(f"Connector-coverage PRs (feat/fix(connector*)): {conn}")
print("Top title prefixes:", pref.most_common(10))
print("Labels:", dict(labels.most_common()))
'
```
Coverage-relevant labels: `integration` / `PMT` / `API-FLOW`. The `feat(connector)`/`fix(connector)` title
prefix is the strongest signal (labels are ~50% sparse). **`--repo` must match `$BASE`'s remote** (`prism`).

---

## §3 Snapshot — latest-main state (in-memory, no files)

```bash
python3 - "$BASE" "$SOURCE" <<'PY'
import sys, json, subprocess; from collections import Counter
sys.path.insert(0, "scripts/generators/docs")
import coverage_summary as cs
base_ref, source = sys.argv[1], sys.argv[2]
def sh(*a): return subprocess.run(a,capture_output=True,text=True).stdout.strip()
def load_git(rev):
    conns={}
    for p in sh("git","ls-tree","-r","--name-only",rev,"data/field_probe").split():
        if not p.endswith(".json"): continue
        try: d=json.loads(subprocess.run(["git","show",f"{rev}:{p}"],capture_output=True,text=True).stdout)
        except Exception: continue
        if d.get("connector") and isinstance(d.get("flows"),dict): conns[d["connector"]]=d["flows"]
    return conns
conns = cs.load_probe_files(cs.DEFAULT_PROBE_DIR) if source=="worktree" else load_git(base_ref)
agg = cs.compute(conns)
pm = Counter()
for c in agg["pm_status"].values(): pm.update(c)
api = Counter()
for b in agg["flow_status_conns"].values():
    for st,names in b.items(): api[st]+=len(names)
P=lambda s,t:(100.0*s/t if t else 0)
print(f"Source: {'working tree' if source=='worktree' else base_ref}")
print(f"Connectors: {agg['n_connectors']}")
print(f"API  supported flows  : {api['supported']:>5} / {sum(api.values()):>5}  ({P(api['supported'],sum(api.values())):.1f}%)")
print(f"PM   supported methods: {pm['supported']:>5} / {sum(pm.values()):>5}  ({P(pm['supported'],sum(pm.values())):.1f}%)")
PY
```
The canonical written reports come from `python3 scripts/generators/docs/coverage_summary.py` (writes
`docs-generated/coverage_pm.md` + `coverage_api.md`) — only run that if the user wants the files, and note it
uses the working tree, not `$BASE`.

---

## §4 Drilldown — movers and new connectors (baseline vs latest main)

```bash
python3 - "$SINCE" "$BASE" <<'PY'
import sys, json, subprocess
sys.path.insert(0, "scripts/generators/docs")
import coverage_summary as cs
since, base_ref = sys.argv[1], sys.argv[2]
def sh(*a): return subprocess.run(a,capture_output=True,text=True).stdout.strip()
def resolve(rev, base):
    if rev.endswith("d") and rev[:-1].isdigit(): return sh("git","rev-list","-1",f"--before={rev[:-1]} days ago",base)
    if len(rev)==10 and rev[4]=="-": return sh("git","rev-list","-1",f"--before={rev}",base)
    return sh("git","rev-parse","--verify",f"{rev}^{{commit}}")
def load_git(rev):
    conns={}
    for p in sh("git","ls-tree","-r","--name-only",rev,"data/field_probe").split():
        if not p.endswith(".json"): continue
        try: d=json.loads(subprocess.run(["git","show",f"{rev}:{p}"],capture_output=True,text=True).stdout)
        except Exception: continue
        if d.get("connector") and isinstance(d.get("flows"),dict): conns[d["connector"]]=d["flows"]
    return conns
bc=cs.compute(load_git(resolve(since,base_ref)))["per_connector"]
nc=cs.compute(load_git(base_ref))["per_connector"]
new=sorted(set(nc)-set(bc))
print(f"New connectors since {since} (on {base_ref}): {', '.join(new) or '(none)'}")
movers=[(nc[k]['flows_supported']-bc.get(k,{}).get('flows_supported',0),
         nc[k]['methods_supported']-bc.get(k,{}).get('methods_supported',0), k) for k in nc]
print("\nTop movers (Δflows, Δmethods):")
for f,m,k in sorted([x for x in movers if x[0] or x[1]], reverse=True)[:15]:
    print(f"  {k:<22} flows {f:+d}  methods {m:+d}")
PY
```

---

## §5 Test coverage — pass-based growth from two grpc CI reports

Unlike §1–§4 (capability, from `data/field_probe`), this reports what actually **PASSES** end-to-end in the
grpc integration reports. It takes a **baseline** and a **current** report (local path or http(s) URL) and
prints a markdown growth table.

The reports base is read from the `REPORT_BASE_URL` environment variable; the commands construct the report
URLs from it. `report_latest.json` is the stable alias for the most recent run; dated snapshots are
`report_YYYYMMDD_HHMM.json`.

```bash
# Set REPORT_BASE_URL to your grpc reports base directory.
: "${REPORT_BASE_URL:?set REPORT_BASE_URL to your grpc reports base, e.g. export REPORT_BASE_URL=https://<host>/<path>/reports/grpc}"

BASE_REPORT="$REPORT_BASE_URL/report_20260506_1458.json"   # dated baseline snapshot
CURR_REPORT="$REPORT_BASE_URL/report_latest.json"          # stable alias for the most recent run

python3 scripts/generators/docs/test_coverage.py "$BASE_REPORT" "$CURR_REPORT" \
  --base-label "May 6" --current-label "latest"
```

Or pass **local file paths** instead — the script treats any non-`http(s)` argument as a file:
```bash
python3 scripts/generators/docs/test_coverage.py ./report_baseline.json ./report_latest.json
```

Prints rows: Passing tests · Connectors with ≥1 pass · API coverage (`connector × flow`) · PMT coverage
(`connector × method`) · Production API coverage · Production PMT coverage · Overall run pass rate — each as
`Base | Current | Change | Growth`. Override the production roster with `--prod adyen,stripe,...` if it changes
(default is the 14 live connectors baked into the script).

**Definitions (match the capability unit):** a coverage cell counts when it has ≥1 **PASS**; the denominator is
every `connector × flow` (or `connector × method`) cell **attempted** in that report — the full cross-product,
not a count of distinct flows/methods. `SKIP` and dependency setup runs are excluded. Quote the **counts /
Change** as growth; the `%` base drifts as the test matrix grows.

---

## Verification (run to confirm the cookbook works)
- §0 → prints `prism/main` SHA/date (e49dc144… or newer) without changing your branch.
- §1 with `SINCE=2026-05-01 BASE=prism/main`: prints supported deltas, **PM drift / API drift / TOTAL drift**,
  and a ⚠ DRIFT DETECTED line. (Numbers reflect prism, which is ahead of local main / origin.)
- **Non-mutation proof:** `git rev-parse HEAD`, `git status --porcelain`, `git stash list` identical before & after.
- §2 → merged PRs for the window against `juspay/hyperswitch-prism`.
- `SOURCE=worktree` → §1/§3 compare your local checkout instead of `prism/main` ("my branch vs main").
