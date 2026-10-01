#!/usr/bin/env python3
"""Explicit, checksum-verified native source bootstrap. Python 3.12+ required."""
import hashlib
import json
from pathlib import Path
import subprocess
import tarfile

root = Path(__file__).resolve().parents[1]
manifest = root / "native/sources.json"
vendor = root / "native/vendor"
downloads = root / "native/downloads"
fingerprint = hashlib.sha256(manifest.read_bytes()).hexdigest()
stamp = vendor / ".oximg-sources"
if stamp.exists() and stamp.read_text().strip() == fingerprint:
    print("Native sources already provisioned.")
    raise SystemExit(0)
if stamp.exists():
    raise SystemExit("Native source pins changed. Move native/vendor aside, then rerun.")
downloads.mkdir(parents=True, exist_ok=True)
for item in json.loads(manifest.read_text()):
    archive = downloads / (item["rev"] + ".tar.gz")
    if not archive.exists():
        pending = archive.with_suffix(".partial")
        subprocess.run([
            "curl", "-fsSL", "--retry", "3", "--connect-timeout", "15", "--max-time", "300",
            f"https://codeload.github.com/{item['repo']}/tar.gz/{item['rev']}",
            "-o", str(pending),
        ], check=True)
        pending.rename(archive)
    actual = hashlib.sha256(archive.read_bytes()).hexdigest()
    if actual != item["sha256"]:
        raise SystemExit(f"Checksum mismatch: {archive}; remove it and retry")
    dest = vendor / item["path"]
    dest.mkdir(parents=True, exist_ok=True)
    with tarfile.open(archive) as tar:
        members = []
        for member in tar.getmembers():
            parts = member.name.split("/", 1)
            if len(parts) == 2 and parts[1]:
                member.name = parts[1]
                members.append(member)
        tar.extractall(dest, members=members, filter="data")
    print(f"Verified {item['repo']} @ {item['rev']}")
stamp.write_text(fingerprint + "\n")
