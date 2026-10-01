#!/usr/bin/env python3
"""Prepare disposable pinned oximg snapshots. Run after downloading archives."""
from pathlib import Path
import shutil

root = Path(__file__).resolve().parents[1]
work = root / "target/comparison"
stock = work / "oximg"
controlled = work / "controlled"
if controlled.exists():
    raise SystemExit("controlled snapshot already exists; do not overwrite benchmark work")
shutil.copytree(stock, controlled, ignore=shutil.ignore_patterns("target", ".git"))
for checkout in [stock, controlled]:
    shutil.copyfile(root / "bench/compare.rs", checkout / "examples/compare.rs")

# Binding declarations ONLY: both controlled wrappers resolve to the exact same
# new jpegli archive. Its ABI must be 8 (JPEGLI_BENCH_ABI=8) for these structs.
bindings = work / "shared-sys"
shutil.copytree(work / "jpegli-sys/src", bindings / "src")
(bindings / "Cargo.toml").write_text('''[package]
name = "jpegli-sys"
version = "0.1.0+0.10.2"
edition = "2021"
license = "BSD-3-Clause"
[dependencies]
libc = "0.2"
''')
for name in ["LICENSE", "LICENSE.md"]:
    source = work / "jpegli-sys" / name
    if source.exists(): shutil.copyfile(source, bindings / name)

original = controlled / "src/pipeline/jpegli_enc.rs"
shutil.copyfile(original, original.with_name("jpegli_reference.rs"))
reference = original.with_name("jpegli_reference.rs")
text = reference.read_text()
needle = 'ffi::jpegli_set_quality(&mut enc.cinfo, quality as c_int, 0);'
assert text.count(needle) == 1
text = text.replace(needle, needle + '''
            // Benchmark control: old core defaults to 4:4:4, new core defaults
            // to 4:2:0. Force the original sampling for fair wrapper isolation.
            for i in 0..3 {
                (*enc.cinfo.comp_info.add(i)).h_samp_factor = 1;
                (*enc.cinfo.comp_info.add(i)).v_samp_factor = 1;
            }
''')
reference.write_text(text)
shutil.copyfile(root / "bench/adapter.rs", original)
cargo = controlled / "Cargo.toml"
text = cargo.read_text().replace('[dependencies]\n', f'[dependencies]\njpegli-rust = {{ path = "{root}" }}\n', 1)
text += '\n[patch.crates-io]\njpegli-sys = { path = "../shared-sys" }\n'
cargo.write_text(text)
print("Prepared stock and controlled snapshots; compile controlled with JPEGLI_BENCH_ABI=8")
