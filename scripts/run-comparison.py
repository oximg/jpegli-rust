#!/usr/bin/env python3
"""Interleaved stock/core-only/new-wrapper benchmark; never runs builds."""
import hashlib
import json
import os
from pathlib import Path
import platform
import random
import subprocess
import sys
import tempfile

root = Path(__file__).resolve().parents[1]
work = root / "target/comparison"
corpus = work / "corpus"
rounds = int(sys.argv[1]) if len(sys.argv) > 1 else 5
variants = ["stock", "reference", "poc"]
bins = {v: work / ("oximg" if v == "stock" else "controlled") / "target/release/examples/compare" for v in variants}
cases = []
for rgb in sorted(corpus.glob("*.rgb")):
    w, h = map(int, rgb.stem.rsplit("-", 1)[1].split("x"))
    for mode in ["encode", "rows"]:
        cases.append(dict(id=rgb.stem + ":" + mode, input=rgb, w=w, h=h, mode=mode,
                          iters=4 if w > 1000 else 12, quality=80, parallel=1))
for jpg in sorted(corpus.glob("kodim*.jpg")):
    for size in [128, 512]:
        for parallel in [1, 2]:
            cases.append(dict(id=f"{jpg.stem}:pipeline:{size}:p{parallel}", input=jpg,
                              w=size, h=size, mode="pipeline", iters=12, quality=80, parallel=parallel))
records = []
destination = root / "docs/comparison-raw.json"
metadata = {
    "platform": platform.platform(), "rounds": rounds,
    "oximg_revision": "73a68e0d77b210e5468d0b385c0bfc2937c8e706",
    "new_jpegli_revision": "031a0077f5799a6041004267fc12b956c1f52a20",
    "old_jpegli": "jpegli-sys 0.1.0+0.10.2",
    "settings": "quality 80, progressive eight-scan, 4:4:4, output Vec copy included for POC",
    "variants": {v: {"binary_sha256": hashlib.sha256(p.read_bytes()).hexdigest()} for v, p in bins.items()},
    "corpus": {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(corpus.iterdir()) if p.is_file()},
}

def run(case, variant):
    outpath = work / f"last-{variant}.jpg"
    command = [str(bins[variant]), str(case["input"]), str(case["w"]), str(case["h"]),
               case["mode"], str(case["iters"]), str(case["quality"]), str(outpath), str(case["parallel"])]
    env = {k:v for k,v in os.environ.items() if not k.startswith("OXIMG_")}
    env["JPEGLI_BENCH_WRAPPER"] = variant
    with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
        child = subprocess.Popen(command, stdout=stdout, stderr=stderr, env=env)
        _, status, usage = os.wait4(child.pid, 0)
        child.returncode = os.waitstatus_to_exitcode(status)
        stdout.seek(0); stderr.seek(0)
        if child.returncode:
            raise RuntimeError(f"{case['id']} {variant}: {stderr.read().decode()}")
        data = json.load(stdout)
    data.update(variant=variant, case=case["id"],
                sha256=hashlib.sha256(outpath.read_bytes()).hexdigest(),
                peak_rss_bytes=usage.ru_maxrss * (1 if sys.platform == "darwin" else 1024),
                process_user_seconds=usage.ru_utime, process_system_seconds=usage.ru_stime)
    return data

for round_index in range(rounds):
    order = cases[:]
    random.Random(9231 + round_index).shuffle(order)
    for case_index, case in enumerate(order):
        group = {}
        for offset in range(3):
            variant = variants[(round_index + case_index + offset) % 3]
            result = run(case, variant)
            result["round"] = round_index
            records.append(result)
            group[variant] = result
        if group["reference"]["sha256"] != group["poc"]["sha256"]:
            raise RuntimeError(f"Same-core byte identity failed: {case['id']}")
    destination.write_text(json.dumps({**metadata, "records": records}, indent=2) + "\n")
    print(f"round {round_index+1}/{rounds}: {len(cases)} cases, identity passed", flush=True)
print(destination)
