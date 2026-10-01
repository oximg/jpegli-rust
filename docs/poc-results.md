# Validation — jpegli-rust 0.1.1

Local platform: macOS 26.6.2, arm64; Rust 1.98.1, Apple Clang 21, CMake 4.4.3.

- 1 native-error unit test, 9 integration tests, 1 compile-only documentation
  test. The integration suite includes 81 dimension/quality/sampling/scan
  combinations decoded independently with image/zune-jpeg.
- Regression coverage checks sampling-dependent DQT tables, including a Q80
  luma table prefix independently captured from stock oximg.
- Formatting and all-target Clippy pass. GitHub CI tests macOS and GNU/Linux,
  verifies the packaged source tree and checks native errors under panic=abort.
- Native ASan/UBSan covers the shim, jpegli and Highway; Rust is not instrumented.
  See the README for the command. This is not a security audit or leak check.
- [Controlled oximg comparison](comparison.md): 62 cases, five interleaved
  rounds; all three backends emit identical bytes after the quantization fix.
- [Performance investigation](performance-investigation.md): stage timings,
  output-copy ablation, sequential and progressive scan experiments.

The original [0.1.0 synthetic report](https://github.com/oximg/jpegli-rust/blob/v0.1.0/docs/poc-results.md)
and `benchmark-synthetic.json` are historical. They used quantization tables
computed before explicit sampling and should not be presented as 0.1.1 results.
MSRV, cross-compilation, Windows, production HTTP load and perceptual-quality
comparisons against other codecs have not been validated.
