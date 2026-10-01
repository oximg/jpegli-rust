# Integration target

Reviewed oximg main at `73a68e0d77b210e5468d0b385c0bfc2937c8e706`:
[src/pipeline/jpegli_enc.rs](https://github.com/oximg/oximg/blob/73a68e0d77b210e5468d0b385c0bfc2937c8e706/src/pipeline/jpegli_enc.rs).

oximg already has its own jpegli-sys encoder with 16-row calls, a growing Vec
destination, stable boxed native structs, ICC marker support, and the eight-scan
script. This POC retains that scan arrangement while moving ABI structs and
error handling out of Rust. A disposable adapter now builds in the actual oximg
library and server. See [controlled comparison](comparison.md). The benchmark
adapter uses `expect` for validated inputs; production integration still needs
proper error propagation.

Local dependency, in an oximg checkout next to this crate:

```toml
[dependencies]
jpegli-rust = { path = "../jpegli-rust" }
```

Suggested adapter mapping:

| Existing encoder | POC |
|---|---|
| `new(w, h, quality, progressive)` | `Encoder::new(w, h, Options { ... })?` |
| progressive true | `ScanMode::Oximg` |
| progressive false | `ScanMode::Sequential` |
| packed RGB `write_scanlines(rows)` | `write_rows(rows, rows.len() / (3*w), 3*w)?` after exact-row validation |
| manually chunked APP2 ICC markers | `write_icc_profile(profile)?` once, before rows |
| `finish() -> Vec<u8>` | `finish()? -> Jpeg` |
| fatal error caught as Rust panic | typed `Result` |

Quality is `u8`, checked 1–100; oximg currently accepts an f32 and casts to an
integer. Validate/normalize that conversion at the adapter boundary. Explicitly
select the intended subsampling: this crate defaults to 4:4:4 and does not hide
the choice behind upstream defaults. The native call keeps `force_baseline=false`
like the inspected oximg implementation.

Keep oximg's resize/color-conversion/metadata policy upstream of the encoder.
Do not tag unconverted pixels with an unrelated ICC profile. The current POC
exposes both ICC chunking and raw APP/COM markers.

For an initial integration, copying `Jpeg` to Vec is acceptable and visible in
the adapter. To retain the no-final-copy path, carry the owner through the
response body until the last consumer has finished. Construct `Encoder` inside
the encoding worker because it is !Send; the finished `Jpeg` is Send + Sync.

Do not link this crate and another copy of jpegli-sys in one binary without
symbol isolation. Both export jpegli_* (and potentially Highway C++ symbols).
Remove/disable the old jpegli-sys backend when testing this one. MozJPEG's jpeg_*
symbols are distinct, but coexistence still needs a real integration build.

Before adopting:

1. Build the actual oximg library and server with a feature-selected adapter.
2. Compare old/new backends on the same native jpegli revision and build flags;
   the current oximg jpegli-sys version may vendor an older native revision.
3. Verify ICC, decoded pixels, output sampling and scan markers on the real
   corpus; do not assume version changes are bit-identical.
4. Measure both encoding-only and complete HTTP/decode/resize/encode workloads,
   including output-owner conversion, CPU, p95 latency, RSS, and concurrent jobs.
5. Compare MozJPEG, jpegli and any Rust alternatives at matched perceptual
   quality, not matched quality numbers. The included benchmark only compares
   wrappers and native revisions without claiming matched perceptual quality.

Decoder-side mozjpeg unwind/security issues remain separate; replacing an
encoder does not fix the existing decoding path.
