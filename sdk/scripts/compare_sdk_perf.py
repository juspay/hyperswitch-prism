#!/usr/bin/env python3
"""Read /tmp/sdk-perf/*.json and print a cross-SDK FFI overhead comparison table."""
import json, glob, sys

files = sorted(glob.glob("/tmp/sdk-perf/*.json"))
if not files:
    sys.exit(0)

sdks = []
for f in files:
    d = json.load(open(f))
    flows = d["flows"]
    if not flows:
        continue
    n = len(flows)
    req = sum(e["req_ffi_ms"] for e in flows) / n
    http = sum(e["http_ms"] for e in flows) / n
    res = sum(e["res_ffi_ms"] for e in flows) / n
    oh = req + res
    tot = req + http + res
    pct = oh / tot * 100 if tot > 0 else 0
    sdks.append((d["sdk"], n, req, http, res, oh, tot, pct))

if not sdks:
    sys.exit(0)

# Sort: Rust first, then alphabetical
order = {"Rust": 0}
sdks.sort(key=lambda s: (order.get(s[0], 1), s[0]))

W = 14  # column width
print()
print("═" * (16 + (W + 2) * len(sdks)))
print("\033[1mCROSS-SDK FFI OVERHEAD COMPARISON\033[0m")
print("═" * (16 + (W + 2) * len(sdks)))
print()

hdr = f"  {'Metric':<{W}}"
for s in sdks:
    hdr += f"  {s[0]:>{W}}"
print(hdr)

sep = f"  {'─' * W}"
for _ in sdks:
    sep += f"  {'─' * W}"
print(sep)

for label, idx in [
    ("Avg req_ffi", 2),
    ("Avg res_ffi", 4),
    ("Avg overhead", 5),
    ("Avg HTTP", 3),
    ("Avg total", 6),
]:
    row = f"  {label:<{W}}"
    for s in sdks:
        row += f"  {s[idx]:>{W - 2}.2f}ms"
    print(row)

row = f"  {'Overhead %':<{W}}"
for s in sdks:
    row += f"  {s[7]:>{W - 1}.1f}%"
print(row)

row = f"  {'Flows tested':<{W}}"
for s in sdks:
    row += f"  {s[1]:>{W}}"
print(row)

print()
