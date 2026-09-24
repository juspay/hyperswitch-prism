# Run token report — what a GRACE run cost, and where

A run's cost is `calls x context`. Every call re-sends everything the spawn has accumulated, so a lookup costs the
whole pile, not the few KB it returned. In one measured run: **734M tokens, 96.6% of them context re-sent**, and
**80% of each long spawn's context was material added while it worked** — every spawn started at ~28K, and the ones
past 100 calls ended at ~358K. Context only shrinks when someone can see it. This procedure is how it gets seen.

**Advisory, never a gate.** It cannot fail a run, block a PR, or justify a retry. If anything below does not hold,
say "not available" and move on — see "When to stop", which is the most important section here.

**Cost yourself what you are measuring.** The whole procedure is about five tool calls. If you find yourself
exploring, stop: this is a fixed set of commands, not an investigation.

---

## 1. Find the transcripts

The harness writes one `*.output` JSONL per subagent spawn. **Its location is not a contract** — it is a scratch
path that a Claude Code version may move, clean, or restructure at any time.

```bash
D="${CLAUDE_TASK_OUTPUT_DIR:-}"
[ -d "$D" ] || D=$(ls -dt /tmp/claude-*/*/*/tasks 2>/dev/null | head -1)
[ -d "$D" ] && ls "$D"/*.output 2>/dev/null | wc -l
```

An operator-supplied path always wins over discovery. Use the session's own transcript too when you can identify
it (`~/.claude/projects/<project>/<session>.jsonl`) — it is the orchestrator's own cost, which is usually a
quarter of the run and invisible otherwise. Say in the provenance line whether it was included.

## 2. When to stop (read this before computing anything)

Write **nothing** and report `not available` when any of these holds:

- no directory found, or it holds no `*.output` files;
- the files hold no `.message.usage` blocks — the schema moved;
- **zero spawns match this run** (step 3), or the matched spawns total zero tokens.

The last two are the dangerous ones, because they produce a plausible-looking empty report rather than an obvious
failure. **A wrong number is worse than no number**: nobody acts on a missing report, and everybody acts on a
confident one. If the shape of the data is not what this guide describes, say exactly that — "transcript schema
differs from `guides/run_token_report.md`; no report written" — and leave the arithmetic alone.

## 3. Scope to this run

A session may have done work that has nothing to do with this run — another connector, an investigation, a
different PR. Count only spawns whose **opening user message names this `{RUN_DIR}`** (or the run id):

```bash
R=grace/runs/<run_id>/
grep -l -- "$R" "$D"/*.output
```

Without this filter the report silently attributes foreign spawns to the run. That was the defect that retired the
script this guide replaces.

## 4. The arithmetic, pinned

Run it as one command over the matched files. The numbers must be computed this way, or two runs are not
comparable:

```bash
python3 - "$D" "$R" <<'EOF'
import json,sys,glob,os,re,collections
D,R=sys.argv[1],sys.argv[2]
def spawn(p):
    u={}; order=[]; first=None
    for line in open(p,errors="replace"):
        try: e=json.loads(line)
        except ValueError: continue
        if not isinstance(e,dict): continue
        if e.get("type")=="assistant":
            m=e.get("message") or {}; g=m.get("usage"); i=m.get("id")
            if g and i and i not in u:                      # dedupe by message id
                u[i]=((g.get("input_tokens") or 0)+(g.get("cache_read_input_tokens") or 0)
                      +(g.get("cache_creation_input_tokens") or 0), g.get("output_tokens") or 0,
                      g.get("cache_read_input_tokens") or 0)
                order.append(i)
        elif e.get("type")=="user" and first is None:
            c=(e.get("message") or {}).get("content")
            first=c if isinstance(c,str) else " ".join(b.get("text","") for b in c or [] if isinstance(b,dict))
    if not u: return None
    ctx=[u[i][0] for i in order]
    return dict(calls=len(ctx), ctx=sum(ctx), out=sum(u[i][1] for i in order),
                cread=sum(u[i][2] for i in order), first_ctx=ctx[0], last_ctx=ctx[-1],
                floor=ctx[0]*len(ctx), prompt=first or "")
def stage(p):
    m=re.search(r"(?:^|/)(1_orchestrator|2\.\d[a-z]?_[a-z0-9_]+)\.md",p or "")  # WORKFLOW_DIR copy too; digits matter (2.5_e2e)
    s=m.group(1) if m else "other"
    unit=re.search(r"UNIT:\s*(\S+)",p or "")
    if s=="2.3b_codegen_unit" and unit:
        w=unit.group(1); s+="/"+("finalize" if w=="__finalize__" else "hs" if w=="__hs__" else "unit")
    return s
rows=[]
for f in sorted(glob.glob(os.path.join(D,"*.output"))):
    if R not in open(f,errors="replace").read(200000): continue     # step 3 scope
    s=spawn(f)
    if s: s["file"]=os.path.basename(f); s["stage"]=stage(s["prompt"]); rows.append(s)
if not rows: print("not available (no spawn matched %s)"%R); raise SystemExit
tot=sum(r["ctx"]+r["out"] for r in rows); cr=sum(r["cread"] for r in rows)
calls=sum(r["calls"] for r in rows); floor=sum(r["floor"] for r in rows)
acc=sum(max(r["ctx"]-r["floor"],0) for r in rows)
print(f"TOTAL {tot:,} tokens | {calls:,} calls | avg ctx {sum(r['ctx'] for r in rows)/calls:,.0f}"
      f" | cache read {100*cr/tot:.1f}% | accumulation {100*acc/max(acc+floor,1):.0f}%")
by=collections.defaultdict(lambda:[0,0,0])
for r in rows:
    b=by[r["stage"]]; b[0]+=1; b[1]+=r["calls"]; b[2]+=r["ctx"]+r["out"]
print("\nstage | spawns | calls | tokens | avg ctx")
for s,(n,c,t) in sorted(by.items(), key=lambda x:-x[1][2]):
    print(f"{s} | {n} | {c} | {t:,} | {t//max(c,1):,}")
print("\nlongest spawns | calls | tokens | first ctx | last ctx | accumulation")
for r in sorted(rows,key=lambda x:-x["calls"])[:5]:
    a=100*max(r["ctx"]-r["floor"],0)/max(r["ctx"],1)
    print(f"{r['stage']} | {r['calls']} | {r['ctx']+r['out']:,} | {r['first_ctx']:,} | {r['last_ctx']:,} | {a:.0f}%")
EOF
```

What each number means:

| Term | Definition |
|---|---|
| tokens per call | `input_tokens + cache_read_input_tokens + cache_creation_input_tokens`; output counted separately |
| floor | `first-call context x calls` — what the spawn would cost if it never accumulated |
| accumulation | `total context - floor` — what it added while working, then re-sent on every later call |
| cache read share | the fraction that is context re-sent rather than seen for the first time |

## 5. Write the report

`{RUN_DIR}report/tokens.md`, in this fixed shape so runs are comparable:

```markdown
# Run cost — <run_id>
TOTAL <tokens> over <calls> calls | avg context <n> | cache read <pct>% | accumulation <pct>%

| stage | spawns | calls | tokens | avg context |
|---|---|---|---|---|

| longest spawn | calls | tokens | first ctx | last ctx | accumulation |
|---|---|---|---|---|---|

Provenance: <transcript dir> · <n> transcripts found · <n> matched this run · session log <included|not found>
```

The provenance line is not decoration. It is how a reader tells "this run was cheap" from "this report measured
almost nothing".

## 6. Pitfalls that have already cost someone a day

- **Not deduplicating by `message.id`.** A streamed message repeats its id across lines; counting each line
  doubles every total. The report then looks precise and is 2x wrong.
- **Amplifying a read by the call index at the time instead of the spawn's final call count.** A file read at step
  5 of a 100-call spawn rides along for 95 more calls; using the index understates its cost by roughly 40x.
- **Treating characters as tokens.** Mixed code and JSON run ~2.6–3.6 chars per token; a chars/4 estimate
  understates by a third. Use the `usage` numbers for anything you report, and estimates only for attribution.
- **A spawn still running** when the report is taken undercounts it. Note it rather than waiting.
