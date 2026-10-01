//! Benchmark-only adapter preserving oximg's original internal signatures.
//! Valid-input timing only; a production adapter must propagate Result instead
//! of the expect calls required by the old infallible new/finish signatures.
#[path = "jpegli_reference.rs"]
mod reference;
use jpegli_rust::{Encoder, Options, ScanMode};
use std::sync::OnceLock;

pub(super) const JPEG_APP2: i32 = 0xE2;
enum Backend {
    Reference(reference::JpegliEncoder),
    Poc(Encoder),
}
pub(super) struct JpegliEncoder { backend: Backend, width: usize }
impl JpegliEncoder {
    pub(super) fn new(w: usize, h: usize, quality: f32, progressive: bool) -> Self {
        static POC: OnceLock<bool> = OnceLock::new();
        let poc = *POC.get_or_init(|| std::env::var("JPEGLI_BENCH_WRAPPER").as_deref() == Ok("poc"));
        let backend = if poc {
            Backend::Poc(Encoder::new(w, h, Options {
                quality: quality as u8,
                scan_mode: if progressive { ScanMode::Oximg } else { ScanMode::Sequential },
                max_pixels: 128_000_000,
                max_output_bytes: 256 * 1024 * 1024,
                ..Options::default()
            }).expect("benchmark valid image"))
        } else { Backend::Reference(reference::JpegliEncoder::new(w, h, quality, progressive)) };
        Self { backend, width: w }
    }
    pub(super) fn write_marker(&mut self, marker: i32, data: &[u8]) {
        match &mut self.backend {
            Backend::Reference(enc) => enc.write_marker(marker, data),
            Backend::Poc(enc) => enc.write_marker(marker as u8, data).expect("benchmark valid marker"),
        }
    }
    pub(super) fn write_scanlines(&mut self, data: &[u8]) -> std::io::Result<()> {
        match &mut self.backend {
            Backend::Reference(enc) => enc.write_scanlines(data),
            Backend::Poc(enc) => enc.write_rows(data, data.len() / (self.width * 3), self.width * 3)
                .map_err(std::io::Error::other),
        }
    }
    pub(super) fn finish(self) -> Vec<u8> {
        match self.backend {
            Backend::Reference(enc) => enc.finish(),
            // Include the copy actually needed by oximg's current Vec API.
            Backend::Poc(enc) => enc.finish().expect("benchmark complete image").as_ref().to_vec(),
        }
    }
}
