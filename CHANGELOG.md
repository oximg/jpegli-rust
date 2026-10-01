# Changelog

## 0.1.1

Fix sampling-dependent quantization: apply the requested subsampling before
setting quality. Previously 4:4:4 and 4:2:2 inherited quantization tables built
for upstream's default 4:2:0. This changes JPEG bytes and quality/size behavior
for those modes. Add a DQT regression test and rerun controlled benchmarks.

## 0.1.0

Experimental first release: RGB8 scanline encoder with stride validation,
sequential/default/eight-scan progression, explicit subsampling, ICC and APP/COM
markers, contained native errors, output-size limits, and owned native JPEG
buffers. Pinned jpegli/Highway sources are bundled in the registry package.
