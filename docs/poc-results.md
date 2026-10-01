# POC validation — 2026-09-30

Local platform: macOS 26.6.2, arm64 (Apple Silicon); Rust 1.98.1, Apple Clang
21.0.0, CMake 4.4.3. The specific CPU model was not accessible in the sandbox.
jpegli revision: `031a0077f5799a6041004267fc12b956c1f52a20`.
MSRV has not been tested locally. Actual oximg integration and comparative
measurements are documented in [comparison.md](comparison.md); GitHub CI also
tests Linux and macOS.

Passed:

- `cargo test --offline`: 1 native error unit test, 8 integration tests and 1
  compile-only documentation test. Integration includes 81 combinations of
  dimensions, sampling, quality and scan mode, decoded with image/zune-jpeg.
- `cargo clippy --all-targets --offline -- -D warnings` and `cargo fmt --check`.
- Native ASan + UBSan: all 9 unit/integration tests passed without diagnostics.
  Instrumentation covers the shim, jpegli and Highway, not Rust. This is not a
  leak-sanitizer result or a full security audit.
- `panic=abort` example: a real native output-limit failure returned an error,
  then another encode succeeded (312-byte JPEG) in the same process.
- Release CLI: generated 64×48 RGB PNG → 633-byte eight-scan JPEG, zero warnings.
- Checksum-pinned source provisioning and idempotent second bootstrap.

## Synthetic encoding benchmark

Command after compiling the release examples:

```sh
python3 scripts/measure.py --synthetic 30
```

1024×768 gradient + deterministic noise, quality 85, 4:4:4. Each mode warms up
three times, then has 30 timed samples with rotating mode order. Input generation
and decoding are outside the timings. Native creation, encoding, allocation and
finish are inside; destruction of the detached output is outside. Native code
is built at Release/O3 even for Cargo debug tests; Rust benchmark code is release.

| Scan mode | JPEG bytes | p50 ms | p95 ms | MPix/s at p50 |
|---|---:|---:|---:|---:|
| Sequential | 414,255 | 6.111 | 6.324 | 128.688 |
| jpegli level 2 | 336,186 | 20.076 | 20.333 | 39.172 |
| oximg eight-scan | 332,808 | 10.624 | 10.990 | 74.024 |

The whole benchmark child process peaked at **19,775,488 bytes RSS** (~18.9 MiB),
including its RGB input and all three modes run sequentially. This is not
per-mode memory, allocation count, a server-concurrency measurement, or a
guaranteed bound. The final recorded run did not overlap our compilation jobs;
the machine was not CPU-pinned and unrelated system activity was not controlled.

Raw output: [benchmark-synthetic.json](benchmark-synthetic.json).

The oximg scan script is faster than upstream level 2 on this synthetic input;
oximg already uses that script. **This is not evidence of a speedup over the
existing oximg encoder**, MozJPEG, or a matched-perceptual-quality comparison.
No real-image corpus quality evaluation or world-leading performance claim has
been established. The POC primarily validates the API, error boundary, row
layout handling, native ownership, and reproducible build approach.

The subsequent [oximg comparison](comparison.md) isolates wrapper overhead from
native-version changes. Perceptual quality, HTTP load and concurrent server RSS
remain future work.
