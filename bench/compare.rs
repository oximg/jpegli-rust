//! Copied into a pinned oximg checkout by scripts/prepare-comparison.py.
#[allow(dead_code)]
#[path = "../src/pipeline/jpegli_enc.rs"]
mod encoder;
use std::{hint::black_box, time::Instant};

fn main() -> anyhow::Result<()> {
    let a: Vec<_> = std::env::args().collect();
    anyhow::ensure!(a.len() == 9, "compare INPUT W H MODE ITERS QUALITY OUTPUT PARALLEL");
    let input = std::fs::read(&a[1])?;
    let w: usize = a[2].parse()?;
    let h: usize = a[3].parse()?;
    let mode = &a[4];
    let iters: usize = a[5].parse()?;
    let quality: f32 = a[6].parse()?;
    let parallel: usize = a[8].parse()?;
    let p = oximg::pipeline::Params {
        max_width: w as u32, max_height: h as u32, quality, parallel,
        output: Some(oximg::pipeline::ImageFormat::Jpeg),
        ..Default::default()
    };
    let mut samples = Vec::new();
    let mut last = Vec::new();
    for iteration in 0..iters + 3 {
        let now = Instant::now();
        let out = if mode == "pipeline" {
            oximg::pipeline::process(black_box(&input), &p)?.0
        } else {
            anyhow::ensure!(input.len() == w * h * 3, "RGB length mismatch");
            let mut enc = encoder::JpegliEncoder::new(w, h, quality, true);
            if mode == "rows" {
                for row in input.chunks_exact(w * 3) { enc.write_scanlines(row)?; }
            } else { enc.write_scanlines(black_box(&input))?; }
            enc.finish()
        };
        black_box(out.len());
        let ms = now.elapsed().as_secs_f64() * 1000.0;
        if iteration >= 3 { samples.push(ms); }
        last = out;
    }
    std::fs::write(&a[7], &last)?;
    println!("{}", serde_json::json!({"samples_ms": samples, "bytes": last.len(),
        "mode":mode, "quality":quality, "parallel":parallel}));
    Ok(())
}
