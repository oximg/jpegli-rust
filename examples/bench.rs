//! Encoding-only microbenchmark. Same jpegli revision for all scan modes;
//! does NOT compare perceptual quality with other encoders or measure RSS.
use jpegli_rust::{Options, ScanMode, encode_rgb};
use std::{hint::black_box, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let (pixels, w, h) = if let Some(path) = args.get(1).filter(|p| p.as_str() != "--synthetic") {
        let image = image::open(path)?.into_rgb8();
        let (w, h) = image.dimensions();
        (image.into_raw(), w as usize, h as usize)
    } else {
        let (w, h) = (1024, 768);
        let mut seed = 7u32;
        let pixels = (0..w * h * 3)
            .map(|i| {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                ((i / 3 % w) * 255 / w) as u8 ^ (seed >> 27) as u8
            })
            .collect();
        (pixels, w, h)
    };
    let iterations: usize = args.get(2).map(|s| s.parse()).transpose()?.unwrap_or(20);
    if iterations == 0 {
        return Err("iterations must be positive".into());
    }
    eprintln!(
        "jpegli {}, {}x{}, {} iterations; input decode excluded",
        jpegli_rust::JPEGLI_REVISION,
        w,
        h,
        iterations
    );
    println!("mode,quality,subsampling,bytes,p50_ms,p95_ms,mpix_per_s");
    let modes = [
        ScanMode::Sequential,
        ScanMode::JpegliDefault,
        ScanMode::Oximg,
    ];
    let mut samples: [Vec<f64>; 3] = std::array::from_fn(|_| Vec::new());
    let mut sizes = [0; 3];
    // Warm up every mode, then rotate order to reduce systematic thermal bias.
    for round in 0..iterations + 3 {
        for offset in 0..3 {
            let i = (round + offset) % 3;
            let t = Instant::now();
            let jpeg = encode_rgb(
                black_box(&pixels),
                w,
                h,
                w * 3,
                Options {
                    scan_mode: modes[i],
                    ..Options::default()
                },
            )?;
            let elapsed = t.elapsed().as_secs_f64();
            sizes[i] = black_box(jpeg.len());
            if round >= 3 {
                samples[i].push(elapsed);
            }
        }
    }
    for i in 0..3 {
        samples[i].sort_by(f64::total_cmp);
        let p50 = samples[i][iterations / 2];
        let p95 = samples[i][(iterations * 95).div_ceil(100).saturating_sub(1)];
        println!(
            "{:?},85,444,{}, {:.3},{:.3},{:.3}",
            modes[i],
            sizes[i],
            p50 * 1000.0,
            p95 * 1000.0,
            (w * h) as f64 / p50 / 1e6
        );
    }
    Ok(())
}
