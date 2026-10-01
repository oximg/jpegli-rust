# Can jpegli-rust outperform oximg?

**There is optimization headroom, but replacing the wrapper is not a demonstrated
pipeline improvement.** The strongest measured opportunity here is a small-image
progressive scan policy, which existing oximg can also adopt. The practical
investment should target native codec work and end-to-end integration, with this
crate serving as the maintained boundary and benchmark vehicle.

All current new-core results below use the 0.1.1 sampling-before-quality fix.
The 0.1.0 core comparison was confounded by incorrect quantization setup and is
superseded. The corrected 62-case comparison emits identical JPEGs from old and
new cores and both wrappers. See [comparison.md](comparison.md).

## Where time goes

Across the six photographs, native initialization takes about 3% at 128px and
0.2–0.3% at 512/768px. Row processing takes 35–49%, and finish takes 51–61%.
The explicit output copy measures below 0.1% of total encoding time here.
Removing the copy does not produce a consistent overall latency improvement:
the native-owner and Vec paths are within roughly 1% in grouped measurements.
Fine-grained copy timings approach timer/allocator noise and are not general
memory-bandwidth claims.

`finish` includes tokenization, Huffman optimization, bitstream output and
cleanup, not just returning a buffer. The pinned native source implements these
in `lib/jpegli/encode.cc::jpegli_finish_compress`. Improving finish by 20% would
save roughly 10–12% of encoding time on these photos, before pipeline dilution.
This is an Amdahl-law illustration, not a measured optimization.

Stock oximg already returns its destination Vec without a final copy. Our
adapter's `.to_vec()` adds an integration cost; native output ownership removes
that cost, rather than establishing an advantage over stock. Stock also already
batches up to 16 scanlines and has a streaming decode/resize/encode path.

The corrected wrapper comparison finds whole-frame RGB encoding effectively
equal to stock, while one-row-at-a-time input is about 3% slower over the photo
cases and 6.7% slower on the single 4K synthetic case. Investigate the extra
checked Rust-to-shim call path and row batching; do not remove bounds checks to
claim a win. These are observations, not a proved attribution to one function.

## Progressive scan experiments

All candidates retain progressive JPEG and the same quantized coefficients.
Independent MozJPEG decoding verified identical final RGB pixels for every
candidate and every corpus image. Files differ in scan structure and size;
progressive preview behavior also changes.

Ratios relative to the existing eight scans, old oximg core:

| Longest edge | Four-scan time | Four-scan size | Five-scan time | Five-scan size |
|---|---:|---:|---:|---:|
| 128 | 0.890x | 1.000x | 0.924x | 0.993x |
| 512 | 0.975x | 1.018x | 0.979x | 1.004x |
| 768 | 0.981x | 1.018x | 0.983x | 1.005x |

The corrected new core gives similar results: four scans at 128px take 0.884x
the eight-scan time with the same size ratio. This opportunity belongs to scan
selection, not to the Rust wrapper. Existing oximg's source records a prior
17-candidate scan search on DIV2K/Zen 4; these six Kodak images on arm64 do not
justify replacing its globally selected script with four scans everywhere.

Four scans mean one interleaved DC scan and one full AC scan per component.
Five splits luma AC at 2; six also splits luma at 10; eight is oximg's existing
script. Candidates were measured in one binary per core, rotating order for
30 timed rounds after three warmups. They are exploratory measurements, not
independent-host replications or confidence intervals.

## Does it improve the actual pipeline?

Four vs eight scans in **the same stock-core oximg binary**, five interleaved
rounds, six JPEG inputs, 30 timed iterations per child:

| Target box | parallel | Time ratio | Per-round ratio range | JPEG size ratio |
|---|---:|---:|---:|---:|
| 128 | 1 | 0.9858x | 0.9833–0.9872x | 0.9974x |
| 128 | 2 | 0.9695x | 0.8966–0.9952x | 0.9969x |
| 512 | 1 | 0.9820x | 0.9784–0.9846x | 1.0168x |
| 512 | 2 | 0.9925x | 0.9828–1.0168x | 1.0170x |

The serial pipeline improves only about 1.4–1.8%, versus 11% encoding-only at
128px. Parallel results vary substantially: the 3.1% aggregate improvement at
128 is not a reliable universal promise. At 512 the parallel rounds include a
regression. CPU affinity, frequency and unrelated system load were uncontrolled.
These measurements exclude HTTP and queueing. `parallel=2` is pipeline scheduling,
not concurrent request load.

The eight-scan branch was checked against the original stock SHA-256 output for
every case and round. RGB identity across scan choices was independently checked
in the encoding experiment; the pipeline experiment records output hashes but
does not separately decode every output. No four-scan default is shipped.

## Other tradeoffs

Sequential encoding takes about 0.53–0.65x the eight-scan time on photos, with
identical decoded pixels, but files grow roughly 9–11% and lose progressive
loading. Stock oximg's encoder already supports sequential operation, so this
is a product-policy tradeoff available to both implementations. The synthetic
4K image grows 40% in this mode; it should not be hidden in a photo average.
Upstream's default progressive script is slower than oximg's tuned eight scans
on this corpus and does not provide a useful wrapper-speed baseline.

Creating a reusable encoder alone does not retain all working memory:
`jpegli_finish_compress` calls `jpegli_abort_compress`, which releases the image
pool. Meaningful workspace reuse requires examining native allocation ownership
and retained-RSS behavior under concurrency. Initialization percentages do not
bound all allocation costs, since allocations also occur during row processing
and finish. No allocation profiler or concurrent-RSS experiment has been run.

## Investment priorities

1. Keep a byte-identical baseline; fix the measured per-row regression before
   replacing oximg's backend. Consider a validated row writer or modest batching,
   with end-to-end latency and memory measurements, not only microbenchmarks.
2. Validate a size-dependent 4/5/8-scan policy on a held-out corpus, including
   text/screenshots, very small images and varied quality levels. Include x86-64
   and real server concurrency. Track size and progressive previews explicitly.
3. Profile native tokenization, Huffman construction and bit output, then target
   proven hot functions. Highway SIMD already exists; another wrapper or an
   unconditional `target-cpu=native` flag is not a portable optimization strategy.
4. Explore native workspace reuse only with allocator traces and concurrency
   limits. Removing the final copy or pooling the small wrapper object has low
   demonstrated CPU value in these measurements.

A useful proposed adoption gate is at least **5% end-to-end improvement** on the
actual target workload, equal decoded pixels, no meaningful p95/RSS regression,
and an explicit file-size budget (for example no more than 1% aggregate growth).
This is a suggested investment threshold, not an achieved result or an agreed
user requirement. The present measurements do not meet it.

## Reproduction and data

Start with [comparison.md](comparison.md). These experiments use disposable
oximg checkouts and do not modify the published library API.

```sh
cp bench/investigate.rs target/comparison/controlled/examples/investigate.rs
python3 scripts/prepare-scan-search.py controlled
python3 scripts/prepare-scan-search.py oximg
(cd target/comparison/controlled && JPEGLI_BENCH_ABI=8 cargo build --release --no-default-features --example investigate --example scan_search)
(cd target/comparison/oximg && cargo build --release --no-default-features --example scan_search)
python3 scripts/run-investigate.py
python3 scripts/run-scan-search.py controlled
python3 scripts/run-scan-search.py oximg
python3 scripts/prepare-scan-search.py oximg --pipeline
(cd target/comparison/oximg && cargo build --release --no-default-features --example compare)
python3 scripts/run-scan-pipeline.py
python3 scripts/summarize-investigation.py
```

Do not overlap compilation and measurements. Per-case phase samples and all
scan candidate timings are in [investigation-raw.json](investigation-raw.json),
[scan-search-raw.json](scan-search-raw.json),
[scan-search-stock-raw.json](scan-search-stock-raw.json) and
[scan-pipeline-raw.json](scan-pipeline-raw.json).
[Grouped tables](investigation-summary.txt) include all candidates and 4K.
