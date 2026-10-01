use jpegli_rust::{Encoder, Error, Options, ScanMode, Subsampling, encode_rgb};

fn pixels(w: usize, h: usize) -> Vec<u8> {
    let mut seed = 0x12345678u32;
    (0..w * h * 3)
        .map(|i| {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            ((i % (w * 3)) * 255 / (w * 3)) as u8 ^ (seed >> 27) as u8
        })
        .collect()
}

fn decode(jpeg: &[u8]) -> image::RgbImage {
    image::load_from_memory_with_format(jpeg, image::ImageFormat::Jpeg)
        .unwrap()
        .into_rgb8()
}

// Walk JPEG markers, including entropy sections (stuffed bytes/restart markers).
fn segments(jpeg: &[u8]) -> Vec<(u8, &[u8])> {
    assert_eq!(&jpeg[..2], &[255, 216]);
    let mut out = Vec::new();
    let mut i = 2;
    while i < jpeg.len() {
        assert_eq!(jpeg[i], 255);
        while jpeg[i] == 255 {
            i += 1;
        }
        let marker = jpeg[i];
        i += 1;
        if marker == 217 {
            break;
        }
        let len = u16::from_be_bytes([jpeg[i], jpeg[i + 1]]) as usize;
        assert!(len >= 2);
        out.push((marker, &jpeg[i + 2..i + len]));
        i += len;
        if marker == 218 {
            loop {
                if jpeg[i] == 255 {
                    if jpeg[i + 1] == 0 || (208..=215).contains(&jpeg[i + 1]) {
                        i += 2;
                        continue;
                    }
                    break;
                }
                i += 1;
            }
        }
    }
    out
}

#[test]
fn independent_decoder_accepts_all_modes_and_odd_dimensions() {
    for (w, h) in [(1, 1), (17, 19), (127, 65)] {
        let src = pixels(w, h);
        for sampling in [Subsampling::S444, Subsampling::S422, Subsampling::S420] {
            for quality in [1, 85, 100] {
                let mut decoded = Vec::new();
                for mode in [
                    ScanMode::Sequential,
                    ScanMode::JpegliDefault,
                    ScanMode::Oximg,
                ] {
                    let options = Options {
                        quality,
                        scan_mode: mode,
                        subsampling: sampling,
                        ..Options::default()
                    };
                    let jpeg = encode_rgb(&src, w, h, w * 3, options).unwrap();
                    let rgb = decode(&jpeg);
                    assert_eq!(rgb.dimensions(), (w as u32, h as u32));
                    let seg = segments(&jpeg);
                    let scans = seg.iter().filter(|(m, _)| *m == 218).count();
                    match mode {
                        ScanMode::Sequential => assert_eq!(scans, 1),
                        ScanMode::Oximg => assert_eq!(scans, 8),
                        ScanMode::JpegliDefault => assert!(scans > 8),
                    }
                    let sof = seg
                        .iter()
                        .find(|(m, _)| [192, 193, 194].contains(m))
                        .unwrap()
                        .1;
                    assert_eq!(
                        sof[7],
                        match sampling {
                            Subsampling::S444 => 0x11,
                            Subsampling::S422 => 0x21,
                            Subsampling::S420 => 0x22,
                        }
                    );
                    decoded.push(rgb);
                }
                assert_eq!(decoded[0], decoded[1], "scan mode changed decoded pixels");
                assert_eq!(decoded[0], decoded[2], "oximg scans changed decoded pixels");
            }
        }
    }
}

#[test]
fn padding_batch_size_and_encoder_moves_do_not_change_output() {
    let (w, h) = (613, 471);
    let src = pixels(w, h);
    let expected = encode_rgb(&src, w, h, w * 3, Options::default()).unwrap();
    assert!(expected.len() > 65536, "exercise destination growth");
    let stride = w * 3 + 13;
    let mut padded = vec![0xA5; stride * (h - 1) + w * 3];
    for (i, row) in src.chunks_exact(w * 3).enumerate() {
        padded[i * stride..i * stride + w * 3].copy_from_slice(row);
    }
    let actual = encode_rgb(&padded, w, h, stride, Options::default()).unwrap();
    assert_eq!(&*expected, &*actual);
    let mut enc = Encoder::new(w, h, Options::default()).unwrap();
    for (i, row) in src.chunks_exact(w * 3).enumerate() {
        enc.write_rows(row, 1, w * 3).unwrap();
        if i == 10 {
            enc = *Box::new(enc);
        }
    }
    assert_eq!(&*expected, &*enc.finish().unwrap());
}

#[test]
fn icc_chunks_are_one_based_and_round_trip() {
    let profile = vec![0x72; 150_000];
    let mut enc = Encoder::new(1, 1, Options::default()).unwrap();
    enc.write_icc_profile(&profile).unwrap();
    assert!(enc.write_icc_profile(&profile).is_err());
    enc.write_rows(&[30, 40, 50], 1, 3).unwrap();
    assert!(enc.write_icc_profile(&profile).is_err());
    let jpeg = enc.finish().unwrap();
    let segments = segments(&jpeg);
    let chunks: Vec<_> = segments
        .iter()
        .filter(|(m, p)| *m == 226 && p.starts_with(b"ICC_PROFILE\0"))
        .collect();
    assert_eq!(chunks.len(), 3);
    let mut reassembled = Vec::new();
    for (i, (_, payload)) in chunks.iter().enumerate() {
        assert_eq!(payload[12], (i + 1) as u8);
        assert_eq!(payload[13], chunks.len() as u8);
        reassembled.extend_from_slice(&payload[14..]);
    }
    assert_eq!(reassembled, profile);
    assert_eq!(decode(&jpeg).dimensions(), (1, 1));
}

#[test]
fn invalid_inputs_return_errors_before_native_reads() {
    for (w, h) in [(0, 1), (1, 0), (65501, 1), (usize::MAX, 2)] {
        assert!(matches!(
            Encoder::new(w, h, Options::default()),
            Err(Error::InvalidInput(_))
        ));
    }
    for options in [
        Options {
            quality: 0,
            ..Options::default()
        },
        Options {
            quality: 101,
            ..Options::default()
        },
        Options {
            max_pixels: 1,
            ..Options::default()
        },
        Options {
            max_output_bytes: 0,
            ..Options::default()
        },
        Options {
            max_output_bytes: usize::MAX,
            ..Options::default()
        },
    ] {
        assert!(Encoder::new(2, 2, options).is_err());
    }
    let mut enc = Encoder::new(2, 2, Options::default()).unwrap();
    assert!(enc.write_rows(&[0; 12], 2, usize::MAX).is_err());
    assert!(enc.write_rows(&[0; 12], 2, 5).is_err());
    assert!(enc.write_rows(&[0; 11], 2, 6).is_err());
    assert!(enc.write_rows(&[0; 18], 3, 6).is_err());
    // Rust validation failures do not poison a usable encoder.
    enc.write_rows(&[0; 12], 2, 6).unwrap();
    assert!(enc.write_rows(&[0; 6], 1, 6).is_err());
    assert!(enc.finish().is_ok());
    assert!(matches!(
        Encoder::new(2, 2, Options::default()).unwrap().finish(),
        Err(Error::InvalidInput(_))
    ));
}

#[test]
fn native_output_limit_is_recoverable_and_next_encode_works() {
    let src = pixels(31, 19);
    for _ in 0..20 {
        let result = encode_rgb(
            &src,
            31,
            19,
            93,
            Options {
                max_output_bytes: 100,
                ..Options::default()
            },
        );
        assert!(matches!(result, Err(Error::OutputLimit)));
        // Drop an unfinished encoder as well.
        let mut enc = Encoder::new(31, 19, Options::default()).unwrap();
        enc.write_rows(&src[..93], 1, 93).unwrap();
    }
    let jpeg = encode_rgb(&src, 31, 19, 93, Options::default()).unwrap();
    assert_eq!(jpeg.warning_count(), 0);
    assert_eq!(decode(&jpeg).dimensions(), (31, 19));
    let len = jpeg.len();
    let exact = encode_rgb(
        &src,
        31,
        19,
        93,
        Options {
            max_output_bytes: len,
            ..Options::default()
        },
    )
    .unwrap();
    assert_eq!(&*exact, &*jpeg);
}

#[test]
fn detached_output_survives_thread_transfer() {
    let jpeg = encode_rgb(&[10; 12], 2, 2, 6, Options::default()).unwrap();
    let dimensions = std::thread::spawn(move || decode(&jpeg).dimensions())
        .join()
        .unwrap();
    assert_eq!(dimensions, (2, 2));
}

#[test]
fn encoded_pixels_preserve_rgb_channels() {
    let src: Vec<u8> = [200, 80, 20]
        .into_iter()
        .cycle()
        .take(17 * 19 * 3)
        .collect();
    let jpeg = encode_rgb(
        &src,
        17,
        19,
        51,
        Options {
            quality: 95,
            ..Options::default()
        },
    )
    .unwrap();
    let decoded = decode(&jpeg);
    for (expected, actual) in src.iter().zip(decoded.as_raw()) {
        assert!(expected.abs_diff(*actual) <= 3, "RGB channels/data changed");
    }
    let mut enc = Encoder::new(
        1,
        1,
        Options {
            max_output_bytes: 100,
            ..Options::default()
        },
    )
    .unwrap();
    // Fail while native marker-writing has a live C++ vector. Exception cleanup
    // must work here too, not only on the finish/entropy path.
    assert_eq!(enc.write_icc_profile(&[0; 1000]), Err(Error::OutputLimit));
    assert_eq!(enc.write_rows(&[0; 3], 1, 3), Err(Error::Poisoned));
}

#[test]
fn raw_markers_validate_and_preserve_payloads() {
    let mut enc = Encoder::new(1, 1, Options::default()).unwrap();
    assert!(enc.write_marker(0xD8, b"invalid SOI").is_err());
    assert!(enc.write_marker(0xFE, &vec![0; 65534]).is_err());
    enc.write_marker(0xFE, b"jpegli-rust").unwrap();
    enc.write_marker(0xEF, b"application payload").unwrap();
    enc.write_rows(&[10, 20, 30], 1, 3).unwrap();
    assert!(enc.write_marker(0xFE, b"too late").is_err());
    let out = enc.finish().unwrap();
    let segments = segments(&out);
    assert!(segments.contains(&(0xFE, b"jpegli-rust".as_slice())));
    assert!(segments.contains(&(0xEF, b"application payload".as_slice())));
    assert_eq!(decode(&out).dimensions(), (1, 1));
}
