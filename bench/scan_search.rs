#[allow(dead_code)]
#[path = "../src/pipeline/jpegli_reference.rs"]
mod reference;
use jpegli_rust as _;
use std::{hint::black_box, time::Instant};
fn main() -> anyhow::Result<()> {
    let a: Vec<_> = std::env::args().collect();
    let rgb = std::fs::read(&a[1])?;
    let w: usize = a[2].parse()?;
    let h: usize = a[3].parse()?;
    let n: usize = a[4].parse()?;
    let mut records = Vec::new();
    let mut reference_pixels = None;
    
    for round in 0..n+3 {
        for offset in 0..4 {
            let v = (round + offset) % 4;
            let scans = [8, 4, 5, 6][v];
            let t = Instant::now();
            let mut enc = reference::JpegliEncoder::new_with_scan(w,h,80.,true,scans);
            let t1 = Instant::now();
            enc.write_scanlines(black_box(&rgb))?;
            let t2 = Instant::now();
            let jpeg = enc.finish();
            let t3 = Instant::now();
            let copy: Option<Vec<u8>> = None;
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

            }
            drop(copy); drop(jpeg);
            let t5 = Instant::now();
            if round >= 3 { records.push(serde_json::json!({"variant":(["8","4","5","6"][v]),"round":round-3,"bytes":bytes,
                "new_ms":(t1-t).as_secs_f64()*1000.,"write_ms":(t2-t1).as_secs_f64()*1000.,"finish_ms":(t3-t2).as_secs_f64()*1000.,"copy_ms":(t4-t3).as_secs_f64()*1000.,"total_ms":(t4-t).as_secs_f64()*1000.,"with_drop_ms":(t5-t).as_secs_f64()*1000.})); }
        }
    }
    println!("{}",serde_json::json!({"input":a[1],"width":w,"height":h,"records":records})); Ok(())
}
