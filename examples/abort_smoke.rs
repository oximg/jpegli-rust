//! Run with RUSTFLAGS=-Cpanic=abort to check a real native error under abort.
use jpegli_rust::{Error, Options, encode_rgb};
fn main() {
    let pixels = vec![128; 64 * 64 * 3];
    let result = encode_rgb(
        &pixels,
        64,
        64,
        192,
        Options {
            max_output_bytes: 100,
            ..Options::default()
        },
    );
    assert!(matches!(result, Err(Error::OutputLimit)));
    let jpeg = encode_rgb(&pixels, 64, 64, 192, Options::default()).unwrap();
    assert_eq!(&jpeg[..2], &[255, 216]);
    println!(
        "native error returned; next encode succeeded ({} bytes)",
        jpeg.len()
    );
}
