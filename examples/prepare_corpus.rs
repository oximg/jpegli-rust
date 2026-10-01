//! Prepare metadata-free real-image RGB/JPEG inputs and one synthetic 4K case.
use image::{ImageFormat, imageops::FilterType};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).ok_or("corpus directory required")?);
    for entry in std::fs::read_dir(&dir)? {
        let path = entry?.path();
        if path.extension().and_then(|s| s.to_str()) != Some("png") {
            continue;
        }
        let image = image::open(&path)?.into_rgb8();
        let name = path.file_stem().unwrap().to_str().unwrap();
        image.save_with_format(dir.join(format!("{name}.jpg")), ImageFormat::Jpeg)?;
        for size in [128, 512, 768] {
            let resized = image::imageops::resize(
                &image,
                image.width() * size / image.width().max(image.height()),
                image.height() * size / image.width().max(image.height()),
                FilterType::Lanczos3,
            );
            let (w, h) = resized.dimensions();
            std::fs::write(dir.join(format!("{name}-{w}x{h}.rgb")), resized.as_raw())?;
        }
    }
    let (w, h) = (3840, 2160);
    let mut seed = 7u32;
    let pixels: Vec<u8> = (0..w * h * 3)
        .map(|i| {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            ((i / 3 % w) * 255 / w) as u8 ^ (seed >> 27) as u8
        })
        .collect();
    std::fs::write(dir.join("synthetic-3840x2160.rgb"), pixels)?;
    Ok(())
}
