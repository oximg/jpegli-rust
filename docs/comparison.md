# oximg comparison — 2026-09-30

Version **0.1.1**, with sampling configured before quality. This supersedes the
0.1.0 measurements: that release built quantization tables using upstream's
4:2:0 default before switching the output to 4:4:4. The resulting size and speed
differences were not a fair native-core comparison.

With the correction, all 62 cases produce byte-identical JPEGs across stock,
reference and POC in all five rounds. The wrapper still does **not** demonstrate
an oximg pipeline speedup. Whole-frame RGB encoding is effectively equal to
stock; one-row calls are slower in this run, especially the synthetic 4K case.
See [performance investigation](performance-investigation.md) for scan-policy
experiments and measured optimization priorities.

## Results

Ratios below 1 favor the POC. Each ratio is the geometric mean of paired
per-process median latency or peak RSS ratios across cases and five rounds.
These are descriptive measurements, not confidence intervals.

| Workload | Cases | POC / stock time | POC / same-core time | POC / stock peak RSS | POC / same-core peak RSS |
|---|---:|---:|---:|---:|---:|
| 4K encode | 1 | 1.005x | 0.963x | 1.002x | 0.986x |
| 4K rows | 1 | 1.067x | 1.075x | 1.010x | 1.010x |
| RGB encode | 18 | 1.001x | 0.972x | 0.932x | 0.939x |
| RGB rows | 18 | 1.031x | 1.018x | 0.976x | 0.962x |
| pipeline 128 | 12 | 1.001x | 0.994x | 0.993x | 0.999x |
| pipeline 512 | 12 | 1.006x | 1.003x | 0.978x | 0.986x |

Output was byte-identical for all three variants in all 62 cases in all five
rounds. This controls quality and file size for this corpus; it is not a promise
of cross-version identity for every possible image. Lower RSS on small encoding cases is not evidence
of lower allocator traffic or bounded codec memory. Whole-process peak RSS
includes input and libraries; macOS accounting and small baselines make these
ratios sensitive. Pipeline RSS is effectively unchanged at this granularity.

[Raw samples and hashes](comparison-raw.json),
[per-case p50, p95, size and RSS](comparison-summary.txt).

## Design

- macOS 26.6.2 arm64, Rust 1.98.1, Apple Clang 21, CMake 4.4.3.
- Stock: oximg `73a68e0d77b210e5468d0b385c0bfc2937c8e706`, bundled
  `jpegli-sys 0.1.0+0.10.2` native core.
- Reference: original oximg wrapper linked to this crate's new jpegli core.
- POC: new wrapper linked to the **same native archive in the same binary** as
  reference. An environment switch selects the wrapper once at runtime.
- Both controlled wrappers use ABI 8 for the legacy bindings, C++ exceptions,
  and identical compiler flags. Normal crate builds use ABI 62. The stock/core
  comparison also includes native build/dependency differences.
- All cases use quality 80, explicit 4:4:4, and oximg's eight-scan progression.
  The new core defaults to 4:2:0, so the reference explicitly forces 4:4:4
  **before setting quality** to preserve both sampling and quantization.
- Six [Kodak images](https://r0k.us/graphics/kodak/) (01, 03, 05, 08, 13, 23),
  RGB resized to longest edge 128, 512, 768, plus deterministic 3840×2160 noise
  and gradient. The pipeline uses JPEG inputs generated from the six PNGs,
  target boxes 128 and 512, with oximg `parallel` set to 1 and 2. This is not
  simultaneous request concurrency.
- Five rounds shuffle cases and rotate the adjacent stock/reference/POC order.
  Each child warms up three times, then measures 12 iterations (4 for 4K).
  There were 930 child processes. No benchmark overlapped our compilation.
- Encoding times include encoder creation, allocation, finish, and POC's
  explicit copy to Vec required by the existing oximg contract. Input loading
  and destruction of the previous output are outside timings. `rows` passes
  one row at a time; `encode` passes the complete image.
- Pipeline timings call the real oximg decode/resize/encode function. They do
  not include HTTP, queueing, caching or network overhead. Reported p95 values
  are in-process samples, not server latency under load.
- `wait4` captures each child's peak RSS and total user/system CPU separately.
  CPU totals include initialization and warmups. CPU affinity was not pinned;
  frequency, thermals and unrelated system activity were not controlled.
- No perceptual metric, x86 benchmark, confidence interval, fuzz campaign or
  production concurrency test was performed. The current adapter uses `expect`
  for controlled valid inputs and needs Result propagation before adoption.

## Actual server smoke test

The compiled controlled oximg server was also exercised with:

```sh
target/release/oximg-ctl --env JPEGLI_BENCH_WRAPPER=poc \
  get /resize/100/100/photo.jpg --expect 200
```

Observed JSON fields: `ok=true`, `status=200`, `content_type=image/jpeg`,
`probe.width=100`, `probe.height=75`, `bytes=2004`,
`sha256=612a73e1b296cd7053ef4125ddcf8ab7e3f7bbb61be90253fabced3381937d3d`.
This validates a real HTTP path, not HTTP throughput.

## Reproduction

Bootstrap this crate first. Download the pinned oximg source archive from
`https://codeload.github.com/oximg/oximg/tar.gz/73a68e0d77b210e5468d0b385c0bfc2937c8e706`
and extract with one leading component removed into `target/comparison/oximg`.
Download `https://static.crates.io/crates/jpegli-sys/jpegli-sys-0.1.0%2B0.10.2.crate`
and extract similarly into `target/comparison/jpegli-sys`. Save Kodak PNGs from
`https://r0k.us/graphics/kodak/kodak/kodimNN.png` under
`target/comparison/corpus` for the six IDs above. Corpus and binary SHA-256
hashes are recorded in the raw JSON.

```sh
python3 scripts/prepare-comparison.py
cargo run --release --example prepare_corpus -- target/comparison/corpus
(cd target/comparison/oximg && cargo build --release --no-default-features --example compare)
(cd target/comparison/controlled && JPEGLI_BENCH_ABI=8 cargo build --release --no-default-features --example compare)
python3 scripts/run-comparison.py 5
python3 scripts/summarize-comparison.py > docs/comparison-summary.txt
```

Builds must finish before running the measurement. Do not run variants in
parallel. For the server smoke test, build the controlled checkout with
`JPEGLI_BENCH_ABI=8 cargo build --release --bins`.
