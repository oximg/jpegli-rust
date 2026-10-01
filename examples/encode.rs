use jpegli_rust::{Encoder, Options, ScanMode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() < 3 || args.len() > 5 {
        return Err(
            "usage: encode INPUT.(png|jpg) OUTPUT.jpg [QUALITY=85] [oximg|jpegli|sequential]"
                .into(),
        );
    }
    let image = image::open(&args[1])?;
    if image.color().has_alpha() {
        return Err("flatten alpha explicitly before encoding JPEG".into());
    }
    let rgb = image.into_rgb8();
    let options = Options {
        quality: args.get(3).map(|s| s.parse()).transpose()?.unwrap_or(85),
        scan_mode: match args.get(4).map(String::as_str).unwrap_or("oximg") {
            "oximg" => ScanMode::Oximg,
            "jpegli" => ScanMode::JpegliDefault,
            "sequential" => ScanMode::Sequential,
            _ => return Err("unknown scan mode".into()),
        },
        ..Options::default()
    };
    let mut enc = Encoder::new(rgb.width() as usize, rgb.height() as usize, options)?;
    enc.write_rows(
        rgb.as_raw(),
        rgb.height() as usize,
        rgb.width() as usize * 3,
    )?;
    let jpeg = enc.finish()?;
    std::fs::write(&args[2], jpeg.as_ref())?;
    eprintln!(
        "{}x{}, {} bytes, {} warnings",
        rgb.width(),
        rgb.height(),
        jpeg.len(),
        jpeg.warning_count()
    );
    Ok(())
}
