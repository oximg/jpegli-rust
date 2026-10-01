#!/usr/bin/env python3
"""Add benchmark-only scan choices after prepare-comparison.py."""
from pathlib import Path
import sys
PATCH='                static FOUR: [ffi::jpegli_scan_info; 4] = [scan(&[0,1,2],0,0),scan(&[0],1,63),scan(&[1],1,63),scan(&[2],1,63)];\n                static FIVE: [ffi::jpegli_scan_info; 5] = [scan(&[0,1,2],0,0),scan(&[0],1,2),scan(&[0],3,63),scan(&[1],1,63),scan(&[2],1,63)];\n                static SIX: [ffi::jpegli_scan_info; 6] = [scan(&[0,1,2],0,0),scan(&[0],1,2),scan(&[0],3,10),scan(&[0],11,63),scan(&[1],1,63),scan(&[2],1,63)];\n                let selected: &[ffi::jpegli_scan_info] = match scans {4=>&FOUR,5=>&FIVE,6=>&SIX,_=>&SCAN_SCRIPT};\n                enc.cinfo.scan_info = selected.as_ptr();\n                enc.cinfo.num_scans = selected.len() as c_int;'
root=Path(__file__).resolve().parents[1]
checkout=root/'target/comparison'/sys.argv[1]
source=checkout/'src/pipeline/jpegli_reference.rs'
if not source.exists(): source=checkout/'src/pipeline/jpegli_enc.rs'
s=source.read_text()
if 'new_with_scan' not in s:
    needle='        // SAFETY: zeroed structs are the libjpeg idiom before'
    s=s.replace(needle, '        Self::new_with_scan(w, h, quality, progressive, 8)\n    }\n\n    pub(super) fn new_with_scan(w: usize, h: usize, quality: f32, progressive: bool, scans: usize) -> JpegliEncoder {\n'+needle, 1)
    needle='                enc.cinfo.num_scans = SCAN_SCRIPT.len() as c_int;'
    s=s.replace(needle, needle+'\n'+PATCH, 1)
# Keep the investigation module out of Cargo's auto-discovered examples.
folder=checkout/'examples/research'
folder.mkdir(exist_ok=True)
(folder/'reference.rs').write_text(s)
harness=(root/'bench/scan_search.rs').read_text().replace('../src/pipeline/jpegli_reference.rs','research/reference.rs').replace('use jpegli_rust as _;', '')
(checkout/'examples/scan_search.rs').write_text(harness)

if '--pipeline' in sys.argv:
    assert sys.argv[1]=='oximg', 'pipeline experiment targets the stock core'
    s=s.replace('Self::new_with_scan(w, h, quality, progressive, 8)', 'static SCANS: std::sync::OnceLock<usize> = std::sync::OnceLock::new();\n        let scans = *SCANS.get_or_init(|| std::env::var("JPEGLI_BENCH_SCANS").ok().and_then(|s| s.parse().ok()).unwrap_or(8));\n        Self::new_with_scan(w, h, quality, progressive, scans)')
    source.write_text(s)
