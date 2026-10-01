#!/usr/bin/env python3
"""Record benchmark CSV and whole-child resource use (not per-mode RSS)."""
import json
import platform
import resource
import subprocess
import sys

command = ["target/release/examples/bench", *(sys.argv[1:] or ["--synthetic", "30"])]
result = subprocess.run(command, capture_output=True, text=True, check=True)
usage = resource.getrusage(resource.RUSAGE_CHILDREN)
print(json.dumps({
    "command": command,
    "platform": platform.platform(),
    "machine": platform.machine(),
    "stderr": result.stderr.strip(),
    "csv": result.stdout,
    "peak_rss_bytes": usage.ru_maxrss * (1 if sys.platform == "darwin" else 1024),
    "user_cpu_seconds": usage.ru_utime,
    "system_cpu_seconds": usage.ru_stime,
    "memory_scope": "entire child process: input plus all scan modes, not codec-only or per-mode",
}, indent=2))
