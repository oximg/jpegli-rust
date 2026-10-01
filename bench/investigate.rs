use jpegli_rust::{Encoder, Options, ScanMode};
use std::{hint::black_box, time::Instant};
fn main() -> anyhow::Result<()> {
    let a: Vec<_> = std::env::args().collect();
    let rgb = std::fs::read(&a[1])?;
    let w: usize = a[2].parse()?;
    let h: usize = a[3].parse()?;
    let n: usize = a[4].parse()?;
    let mut records = Vec::new();
    let mut reference_pixels = None;
    let mut reference_bytes = None;
    for round in 0..n+3 {
        for offset in 0..4 {
            let v = (round + offset) % 4;
            let mode = if v == 2 { ScanMode::Sequential } else if v == 3 { ScanMode::JpegliDefault } else { ScanMode::Oximg };
            let t = Instant::now();
            let mut enc = Encoder::new(w,h,Options {quality:80, scan_mode:mode, max_pixels:128_000_000, ..Options::default()})?;
            let t1 = Instant::now();
            enc.write_rows(black_box(&rgb),h,w*3)?;
            let t2 = Instant::now();
            let jpeg = enc.finish()?;
            let t3 = Instant::now();
            let copy = if v == 1 { Some(black_box(jpeg.as_ref()).to_vec()) } else { None };
            black_box(&copy);
            let t4 = Instant::now();
            let bytes = jpeg.len();
            // Validate after timing; only during warmups to avoid affecting timed rounds.
            if round < 3 {
                let mut decoder = mozjpeg::Decompress::new_mem(&jpeg)?.rgb()?;
                let pixels: Vec<u8> = decoder.read_scanlines()?;
                decoder.finish()?;
                if let Some(ref reference) = reference_pixels { anyhow::ensure!(*reference == pixels, "decoded pixels differ"); }
                else { reference_pixels = Some(pixels); }
                if v == 0 { reference_bytes = Some(jpeg.as_ref().to_vec()); }
                if v == 1 { if let Some(ref reference) = reference_bytes { anyhow::ensure!(*reference == jpeg.as_ref(), "copy byte mismatch"); } }
            }
            drop(copy); drop(jpeg);
            let t5 = Instant::now();
            if round >= 3 { records.push(serde_json::json!({"variant":(["native","vec","sequential","default"][v]),"round":round-3,"bytes":bytes,
                "new_ms":(t1-t).as_secs_f64()*1000.,"write_ms":(t2-t1).as_secs_f64()*1000.,"finish_ms":(t3-t2).as_secs_f64()*1000.,"copy_ms":(t4-t3).as_secs_f64()*1000.,"total_ms":(t4-t).as_secs_f64()*1000.,"with_drop_ms":(t5-t).as_secs_f64()*1000.})); }
        }
    }
    println!("{}",serde_json::json!({"input":a[1],"width":w,"height":h,"records":records})); Ok(())
}
