# jpegli-rust

An experimental Rust API over Google's native **jpegli** encoder, maintained by
[oximg](https://github.com/oximg). The crate is usable independently of oximg;
the codec itself is C++, not a pure-Rust reimplementation.

The first milestone is a usable encoding boundary: checked borrowed RGB8 rows,
explicit configuration, native failures returned as `Result`, and output
ownership without a mandatory final copy. It makes no performance leadership
claim. See the [controlled oximg benchmark](docs/comparison.md): the current
wrapper does not demonstrate an end-to-end speed improvement.

## Build and run

Requires Rust 1.90+, CMake 3.16+, and a C++17 compiler. A Git checkout also
requires Python 3.12+ and curl for the one-time bootstrap.
Native macOS and GNU/Linux builds are supported by the build script; local
validation was on Apple Silicon. Cross-compilation and Windows are not supported
by this POC. The crates.io package includes the pinned native sources;
`build.rs` has no network step.

```toml
[dependencies]
jpegli-rust = "0.1.1"
```

```sh
python3 scripts/fetch-native.py
cargo test --locked
cargo run --release --example encode -- input.png output.jpg 85 oximg
cargo run --release --example bench -- --synthetic 30
cargo run --release --example bench -- input.png 30
python3 scripts/measure.py --synthetic 30
```

`fetch-native.py` verifies SHA-256 hashes for jpegli and its pinned dependencies.
Downloaded sources and archives are ignored by git. Keep `Cargo.lock` and
`native/sources.json` when sharing the POC. The initial 0.1 release is
experimental; the API and supported-target list may evolve.

The `encode` example is deliberately small: opaque, already oriented, sRGB
PNG/JPEG input only. It rejects alpha, does not perform ICC color conversion,
does not apply EXIF orientation, and does not preserve input metadata. The
library's ICC method is available for oximg's existing metadata policy.

## API

```rust
use jpegli_rust::{Encoder, Options, ScanMode, Subsampling};

fn encode(rgb: &[u8], width: usize, height: usize)
    -> Result<jpegli_rust::Jpeg, jpegli_rust::Error>
{
    let options = Options {
        quality: 85,
        scan_mode: ScanMode::Oximg,
        subsampling: Subsampling::S444,
        ..Options::default()
    };
    let mut encoder = Encoder::new(width, height, options)?;
    // Optional: encoder.write_icc_profile(profile)?; before the first row.
    encoder.write_rows(rgb, height, width * 3)?;
    encoder.finish()
}
```

Rows can arrive one at a time or in batches. Stride can exceed `width * 3`;
the final row does not need trailing padding. Input pointers are borrowed only
for the duration of `write_rows`. A convenience `encode_rgb` function accepts a
complete strided image.

Scan modes are sequential, upstream jpegli level 2, and oximg's eight-scan
spectrum-only progression. Sequential can produce SOF1 and is not a strict
baseline/SOF0 promise. Sampling is explicitly 4:4:4 (default), 4:2:2 or 4:2:0.
Quality is jpegli's integer 1–100 scale, not a cross-codec perceptual scale.

`Jpeg` owns a native allocation and implements `AsRef<[u8]>` and `Deref<[u8]>`.
It can cross threads. `jpeg.as_ref().to_vec()` makes an explicit copy if a
consumer requires a `Vec`; never reconstruct a Rust Vec from the native pointer.
`Encoder` itself is intentionally neither `Send` nor `Sync` in the POC. Create
it inside the encoding worker.

## Error and memory boundaries

- Width, height, row spans, stride overflow, quality and pixel limits are
  validated before native pixel reads. Configuration is applied once.
- Every retained native pointer targets a stable heap context. No Rust structs
  mirror libjpeg ABI layouts and no Rust callbacks run inside jpegli.
- jpegli is built with C++ exceptions enabled. Error callbacks throw an internal
  exception caught inside the C++ shim, which returns a status to Rust. No
  exception crosses into Rust, and no `longjmp` skips C++ destructors. This
  choice differs from upstream's usual no-exceptions build.
- Native failures poison the encoder. Drop releases incomplete and failed
  contexts. Recoverable warnings are counted, not printed.
- The output grows geometrically with an explicit limit and transfers ownership
  at finish. There is no required final output memcpy. Reallocation can still
  copy bytes and temporarily require both old and new allocations.
- `max_pixels` is an admission check; `max_output_bytes` limits the destination
  allocation. **Neither is a total-memory limit.** jpegli retains internal
  buffers, especially for progressive encoding. Input row streaming does not
  imply constant total memory.
- This does not guarantee recovery from arbitrary OOM or a bug in native code.
  Catching `bad_alloc` is not an audit of all upstream exception-safety paths.

Not yet implemented: decoding, planar YCbCr input, generic `io::Write`, workspace
reuse, user-defined scan scripts, SIMD tuning, full memory accounting, fuzzing,
or a production security audit. APP/COM marker writing is available, but callers
are responsible for the correctness of metadata payloads.

## Validation

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
RUSTFLAGS='-C panic=abort' CARGO_TARGET_DIR=target/abort \
  cargo run --locked --example abort_smoke
```

Tests use the independent `image`/zune-jpeg decoder. They cover an 81-case
dimension/quality/sampling/scan matrix, decoded-pixel equivalence across scan
modes, padded rows, batching, moving an encoder, output growth, multi-chunk ICC,
invalid lengths/overflow, native failures, interrupted encodes and transferring
the completed allocation to another thread.

Native ASan + UBSan (Rust code itself is not sanitizer-instrumented):

```sh
CC=clang CXX=clang++ OXIMG_JPEGLI_SANITIZE=1 \
  RUSTFLAGS='-C linker=clang -C default-linker-libraries=yes' \
  CARGO_TARGET_DIR=target/sanitized \
  cargo test --locked --lib --test encode
```

The system linker must be the matching Clang driver. See
[local validation results](docs/poc-results.md) and
[oximg integration notes](docs/oximg-integration.md).

## Licenses

Wrapper: Apache-2.0. The eight-scan arrangement is adapted from oximg's
Apache-2.0 encoder; see `NOTICE`. Native sources retain their own license files:
jpegli (BSD-3-Clause), Highway (Apache-2.0), skcms (BSD-3-Clause), and the
libjpeg-turbo header source (see its LICENSE.md and accompanying notices).
Only jpegli and Highway are linked into this encoder; skcms is provisioned for
upstream CMake configuration, and libjpeg-turbo supplies ABI headers.
