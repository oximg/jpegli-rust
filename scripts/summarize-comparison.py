#!/usr/bin/env python3
"""Summarize paired, interleaved process measurements (descriptive, not a CI)."""
import json, math, statistics
from pathlib import Path
root = Path(__file__).resolve().parents[1]
data = json.loads((root / "docs/comparison-raw.json").read_text())
groups = {}
for r in data["records"]:
    case = r["case"]
    group = ("pipeline " + case.split(":pipeline:")[1].split(":")[0]) if r["mode"] == "pipeline" else ("4K " if case.startswith("synthetic") else "RGB ") + r["mode"]
    groups.setdefault(group, {}).setdefault((case, r["round"]), {})[r["variant"]] = r
geomean = lambda xs: math.exp(statistics.mean(map(math.log, xs)))
print("| Workload | Cases | POC / stock time | POC / same-core time | POC / stock peak RSS | POC / same-core peak RSS |")
print("|---|---:|---:|---:|---:|---:|")
for group, pairs in sorted(groups.items()):
    ratios = []
    for metric in ["samples_ms", "peak_rss_bytes"]:
        for base in ["stock", "reference"]:
            vals = []
            for rs in pairs.values():
                get = lambda r: statistics.median(r[metric]) if metric == "samples_ms" else r[metric]
                vals.append(get(rs["poc"]) / get(rs[base]))
            ratios.append(geomean(vals))
    print(f"| {group} | {len(set(c for c,r in pairs))} | " + " | ".join(f"{x:.3f}x" for x in ratios) + " |")
print("\nPer-case results (pooled samples; p95 is in-process latency, not HTTP latency):")
for case in sorted(set(r["case"] for r in data["records"])):
    print(case)
    for v in ["stock", "reference", "poc"]:
        rs = [r for r in data["records"] if r["case"] == case and r["variant"] == v]
        ts = sorted(t for r in rs for t in r["samples_ms"])
        print(f"  {v}: p50={statistics.median(ts):.4f}ms p95={ts[math.ceil(len(ts)*.95)-1]:.4f}ms bytes={rs[0]['bytes']} rss={statistics.median(r['peak_rss_bytes'] for r in rs)/1048576:.2f}MiB")
