//! Experimental RGB8 jpegli encoder for oximg.
//!
//! Native jpegli failures become [`Error`] without unwinding through Rust.
//! Input is borrowed per call; [`Jpeg`] owns the native output allocation.
//! Row streaming does not bound jpegli's internal image/coefficient storage.
//!
//! ```no_run
//! use jpegli_rust::{Encoder, Options};
//! let rgb = vec![128; 64 * 48 * 3];
//! let mut encoder = Encoder::new(64, 48, Options::default())?;
//! encoder.write_rows(&rgb, 48, 64 * 3)?;
//! let jpeg = encoder.finish()?;
//! std::fs::write("out.jpg", jpeg.as_ref())?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use std::{
    ffi::{CStr, c_char, c_int, c_void},
    fmt,
    marker::PhantomData,
    ptr::NonNull,
    rc::Rc,
};

/// Pinned native jpegli revision (also recorded in native/sources.json).
pub const JPEGLI_REVISION: &str = "031a0077f5799a6041004267fc12b956c1f52a20";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanMode {
    /// One sequential scan. May use SOF1; this is not a strict SOF0 guarantee.
    Sequential,
    /// Upstream level 2 progression.
    JpegliDefault,
    /// oximg's eight spectrum-only scans, without successive approximation.
    Oximg,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Subsampling {
    S444,
    S422,
    S420,
}

#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// jpegli quality, 1..=100. Not perceptually equivalent to MozJPEG quality.
    pub quality: u8,
    pub scan_mode: ScanMode,
    pub subsampling: Subsampling,
    /// Admission limit checked before native initialization.
    pub max_pixels: usize,
    /// Maximum destination allocation and output size; NOT a total memory cap.
    pub max_output_bytes: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            quality: 85,
            scan_mode: ScanMode::Oximg,
            subsampling: Subsampling::S444,
            max_pixels: 64_000_000,
            max_output_bytes: 64 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    InvalidInput(&'static str),
    Codec(String),
    OutputLimit,
    Allocation,
    Poisoned,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(s) => write!(f, "invalid JPEG input: {s}"),
            Self::Codec(s) => write!(f, "jpegli: {s}"),
            Self::OutputLimit => f.write_str("JPEG output exceeds max_output_bytes"),
            Self::Allocation => f.write_str("native allocation failed"),
            Self::Poisoned => f.write_str("encoder is unusable after a native failure"),
        }
    }
}
impl std::error::Error for Error {}

unsafe extern "C" {
    fn oxj_alloc() -> *mut c_void;
    fn oxj_start(
        e: *mut c_void,
        width: u32,
        height: u32,
        quality: c_int,
        scans: c_int,
        sampling: c_int,
        max_output: usize,
    ) -> c_int;
    fn oxj_write(e: *mut c_void, pixels: *const u8, stride: usize, rows: u32) -> c_int;
    fn oxj_icc(e: *mut c_void, data: *const u8, len: u32) -> c_int;
    fn oxj_marker(e: *mut c_void, marker: c_int, data: *const u8, len: u32) -> c_int;
    fn oxj_finish(e: *mut c_void) -> c_int;
    fn oxj_message(e: *const c_void) -> *const c_char;
    fn oxj_warnings(e: *const c_void) -> u64;
    fn oxj_take_output(e: *mut c_void, len: *mut usize) -> *mut u8;
    fn oxj_destroy(e: *mut c_void);
    fn oxj_free_output(data: *mut u8);
}

/// Owns a stable native context. Intentionally !Send and !Sync in this POC;
/// construct it inside the worker that performs encoding.
pub struct Encoder {
    native: NonNull<c_void>,
    width: usize,
    height: usize,
    rows_written: usize,
    poisoned: bool,
    has_icc: bool,
    _thread: PhantomData<Rc<()>>,
}

impl Encoder {
    pub fn new(width: usize, height: usize, options: Options) -> Result<Self, Error> {
        // JPEG_MAX_DIMENSION in pinned libjpeg headers is 65500.
        if width == 0 || height == 0 || width > 65500 || height > 65500 {
            return Err(Error::InvalidInput("dimensions must be in 1..=65500"));
        }
        let pixels = width
            .checked_mul(height)
            .ok_or(Error::InvalidInput("pixel count overflow"))?;
        if pixels > options.max_pixels {
            return Err(Error::InvalidInput("max_pixels exceeded"));
        }
        if !(1..=100).contains(&options.quality) {
            return Err(Error::InvalidInput("quality must be in 1..=100"));
        }
        if options.max_output_bytes == 0 || options.max_output_bytes > isize::MAX as usize {
            return Err(Error::InvalidInput("invalid max_output_bytes"));
        }
        // SAFETY: alloc creates an opaque zeroed context, owned solely here.
        let native = NonNull::new(unsafe { oxj_alloc() }).ok_or(Error::Allocation)?;
        let mut enc = Self {
            native,
            width,
            height,
            rows_written: 0,
            poisoned: false,
            has_icc: false,
            _thread: PhantomData,
        };
        let scans = match options.scan_mode {
            ScanMode::Sequential => 0,
            ScanMode::JpegliDefault => 1,
            ScanMode::Oximg => 2,
        };
        let sampling = match options.subsampling {
            Subsampling::S444 => 0,
            Subsampling::S422 => 1,
            Subsampling::S420 => 2,
        };
        // SAFETY: validated dimensions/options, uniquely owned stable context.
        let status = unsafe {
            oxj_start(
                enc.native.as_ptr(),
                width as u32,
                height as u32,
                options.quality.into(),
                scans,
                sampling,
                options.max_output_bytes,
            )
        };
        enc.check(status)?;
        Ok(enc)
    }

    fn check(&mut self, status: c_int) -> Result<(), Error> {
        if status == 0 {
            return Ok(());
        }
        self.poisoned = true;
        Err(match status {
            2 => Error::OutputLimit,
            3 => Error::Allocation,
            // SAFETY: native message is a live, NUL-terminated context field.
            _ => Error::Codec(
                unsafe { CStr::from_ptr(oxj_message(self.native.as_ptr())) }
                    .to_string_lossy()
                    .into_owned(),
            ),
        })
    }

    /// Embed one ICC profile before any pixels. The caller must ensure it
    /// describes the input pixels; this encoder performs no ICC conversion.
    pub fn write_icc_profile(&mut self, profile: &[u8]) -> Result<(), Error> {
        if self.poisoned {
            return Err(Error::Poisoned);
        }
        if self.rows_written != 0 || self.has_icc {
            return Err(Error::InvalidInput(
                "ICC must be written once, before pixels",
            ));
        }
        // APP2 payload: 65533 bytes minus 14-byte ICC header, max 255 chunks.
        if profile.is_empty() || profile.len() > 65519 * 255 {
            return Err(Error::InvalidInput("ICC profile length out of range"));
        }
        // SAFETY: bounded live bytes; native code copies data before returning.
        let status =
            unsafe { oxj_icc(self.native.as_ptr(), profile.as_ptr(), profile.len() as u32) };
        self.check(status)?;
        self.has_icc = true;
        Ok(())
    }

    /// Write an APP0..APP15 (0xE0..=0xEF) or COM (0xFE) payload before pixels.
    /// The payload excludes the marker/length bytes and is at most 65533 bytes.
    /// Prefer `write_icc_profile` for ICC; raw markers are not metadata-validated.
    pub fn write_marker(&mut self, marker: u8, data: &[u8]) -> Result<(), Error> {
        if self.poisoned {
            return Err(Error::Poisoned);
        }
        if self.rows_written != 0
            || data.len() > 65533
            || (!(0xE0..=0xEF).contains(&marker) && marker != 0xFE)
        {
            return Err(Error::InvalidInput(
                "invalid marker, size, or marker after pixels",
            ));
        }
        // SAFETY: valid marker and bounded live payload; native code consumes
        // it synchronously inside its own error boundary.
        let status = unsafe {
            oxj_marker(
                self.native.as_ptr(),
                marker.into(),
                data.as_ptr(),
                data.len() as u32,
            )
        };
        self.check(status)
    }

    /// Consume `rows` RGB8 rows, with byte distance `stride` between row starts.
    /// The final row needs only `width * 3` bytes (no trailing padding).
    /// jpegli retains no pointer into `pixels` after this call.
    pub fn write_rows(&mut self, pixels: &[u8], rows: usize, stride: usize) -> Result<(), Error> {
        if self.poisoned {
            return Err(Error::Poisoned);
        }
        if rows == 0 {
            return Ok(());
        }
        if rows > self.height - self.rows_written {
            return Err(Error::InvalidInput("too many rows"));
        }
        let row_bytes = self.width * 3;
        if stride < row_bytes {
            return Err(Error::InvalidInput("stride is smaller than an RGB row"));
        }
        let needed = (rows - 1)
            .checked_mul(stride)
            .and_then(|n| n.checked_add(row_bytes))
            .ok_or(Error::InvalidInput("row layout overflow"))?;
        if pixels.len() < needed {
            return Err(Error::InvalidInput("input buffer is too short"));
        }
        // SAFETY: all row spans are checked against the borrowed slice above;
        // dimensions bound the u32 conversion. Native reads synchronously only.
        let status =
            unsafe { oxj_write(self.native.as_ptr(), pixels.as_ptr(), stride, rows as u32) };
        self.check(status)?;
        self.rows_written += rows;
        Ok(())
    }

    /// Number of recoverable native warnings observed so far.
    pub fn warning_count(&self) -> u64 {
        // SAFETY: native context remains alive and is not concurrently accessed.
        unsafe { oxj_warnings(self.native.as_ptr()) }
    }

    /// Consume the encoder, releasing it on success or failure.
    pub fn finish(mut self) -> Result<Jpeg, Error> {
        if self.poisoned {
            return Err(Error::Poisoned);
        }
        if self.rows_written != self.height {
            return Err(Error::InvalidInput("not all image rows were written"));
        }
        // SAFETY: all rows supplied to a live, unpoisoned context.
        let status = unsafe { oxj_finish(self.native.as_ptr()) };
        self.check(status)?;
        let warnings = self.warning_count();
        let mut len = 0;
        // SAFETY: successful finish initialized [0..len]; ownership transfers
        // from context to Jpeg. Native destructor will no longer free it.
        let ptr = unsafe { oxj_take_output(self.native.as_ptr(), &mut len) };
        Ok(Jpeg {
            data: NonNull::new(ptr).ok_or(Error::Allocation)?,
            len,
            warnings,
        })
    }
}

impl Drop for Encoder {
    fn drop(&mut self) {
        // SAFETY: uniquely owned stable context, destroyed exactly once.
        unsafe { oxj_destroy(self.native.as_ptr()) }
    }
}

/// Immutable native output; use `as_ref()` to avoid a final allocation/copy.
/// Do not convert this allocation with Vec::from_raw_parts: allocators differ.
pub struct Jpeg {
    data: NonNull<u8>,
    len: usize,
    warnings: u64,
}
impl Jpeg {
    pub fn warning_count(&self) -> u64 {
        self.warnings
    }
}
impl AsRef<[u8]> for Jpeg {
    fn as_ref(&self) -> &[u8] {
        // SAFETY: initialized immutable bytes, alive until self is dropped.
        unsafe { std::slice::from_raw_parts(self.data.as_ptr(), self.len) }
    }
}
impl std::ops::Deref for Jpeg {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        self.as_ref()
    }
}
// SAFETY: no aliases can mutate the detached output; free is thread-safe.
unsafe impl Send for Jpeg {}
unsafe impl Sync for Jpeg {}
impl Drop for Jpeg {
    fn drop(&mut self) {
        // SAFETY: exclusively owns the malloc allocation, freed exactly once.
        unsafe { oxj_free_output(self.data.as_ptr()) }
    }
}

/// Encode a complete borrowed RGB8 image, including padded/cropped views.
pub fn encode_rgb(
    pixels: &[u8],
    width: usize,
    height: usize,
    stride: usize,
    options: Options,
) -> Result<Jpeg, Error> {
    let mut enc = Encoder::new(width, height, options)?;
    enc.write_rows(pixels, height, stride)?;
    enc.finish()
}

#[cfg(test)]
mod native_error_tests {
    use super::*;
    #[test]
    fn actual_codec_failure_returns_without_unwinding() {
        let mut enc = Encoder::new(16, 16, Options::default()).unwrap();
        // Bypass the safe Rust precheck to exercise the real native error path.
        let status = unsafe { oxj_finish(enc.native.as_ptr()) };
        assert_ne!(status, 0);
        assert!(matches!(enc.check(status), Err(Error::Codec(_))));
        assert!(matches!(enc.write_rows(&[], 0, 48), Err(Error::Poisoned)));
    }
}
