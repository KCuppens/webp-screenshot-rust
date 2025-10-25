//! CoreGraphics-based screen capture for macOS

use crate::{
    capture::traits::{DefaultPixelConverter, PixelFormatConverter},
    encoder::simd::global_simd_converter,
    error::{CaptureError, CaptureResult},
    memory_pool::global_pool,
    types::{CaptureRegion, DisplayInfo, PixelFormat, RawImage},
};

use core_foundation::base::{CFRelease, TCFType};
use core_graphics::{
    display::{CGDisplay, CGDisplayCreateImage, CGRect},
    image::CGImage,
};
use std::slice;

/// CoreGraphics capture implementation
pub struct CoreGraphicsCapture {
    pixel_converter: DefaultPixelConverter,
}

impl CoreGraphicsCapture {
    /// Create a new CoreGraphics capturer
    pub fn new() -> CaptureResult<Self> {
        Ok(Self {
            pixel_converter: DefaultPixelConverter,
        })
    }

    /// Capture a display using CoreGraphics
    pub fn capture_display(&self, display_index: usize) -> CaptureResult<RawImage> {
        let displays = super::MacOSCapture::enumerate_displays()?;

        let display = displays
            .get(display_index)
            .ok_or_else(|| CaptureError::DisplayNotFound(display_index))?;

        // Create CGRect for the display
        let rect = CGRect::new(
            &core_graphics::geometry::CGPoint::new(display.x as f64, display.y as f64),
            &core_graphics::geometry::CGSize::new(display.width as f64, display.height as f64),
        );

        self.capture_rect(rect, display.scale_factor)
    }

    /// Capture a specific region using CoreGraphics
    pub fn capture_region(&self, region: CaptureRegion) -> CaptureResult<RawImage> {
        let rect = CGRect::new(
            &core_graphics::geometry::CGPoint::new(region.x as f64, region.y as f64),
            &core_graphics::geometry::CGSize::new(region.width as f64, region.height as f64),
        );

        self.capture_rect(rect, 1.0)
    }

    /// Internal method to capture a CGRect
    fn capture_rect(&self, rect: CGRect, scale_factor: f32) -> CaptureResult<RawImage> {
        unsafe {
            // Capture the screen region
            let image = CGDisplayCreateImage(CGDisplay::main().id, Some(rect));

            if image.is_null() {
                return Err(CaptureError::CaptureFailed(
                    "CGDisplayCreateImage failed".to_string(),
                ));
            }

            // Get image properties
            let width = CGImageGetWidth(image) as u32;
            let height = CGImageGetHeight(image) as u32;
            let bytes_per_row = CGImageGetBytesPerRow(image) as usize;
            let bits_per_pixel = CGImageGetBitsPerPixel(image);
            let bits_per_component = CGImageGetBitsPerComponent(image);

            // Get data provider and data
            let data_provider = CGImageGetDataProvider(image);
            if data_provider.is_null() {
                CFRelease(image as _);
                return Err(CaptureError::CaptureFailed(
                    "Failed to get data provider".to_string(),
                ));
            }

            let data_ref = CGDataProviderCopyData(data_provider);
            if data_ref.is_null() {
                CFRelease(image as _);
                return Err(CaptureError::CaptureFailed(
                    "Failed to copy image data".to_string(),
                ));
            }

            let data_ptr = CFDataGetBytePtr(data_ref);
            let data_len = CFDataGetLength(data_ref) as usize;

            // Validate data size
            let expected_size = (height as usize) * bytes_per_row;
            if data_len < expected_size {
                CFRelease(data_ref as _);
                CFRelease(image as _);
                return Err(CaptureError::CaptureFailed(
                    format!("Invalid data size: got {}, expected {}", data_len, expected_size),
                ));
            }

            // Get buffer from pool
            let pool = global_pool();
            let buffer_size = (width * height * 4) as usize; // RGBA
            let mut pooled_buffer = pool
                .acquire(buffer_size)
                .map_err(|_| CaptureError::MemoryAllocationFailed { size: buffer_size })?;

            // Copy and convert data
            let src_slice = slice::from_raw_parts(data_ptr, data_len);

            // Determine pixel format and convert if necessary
            let pixel_format = if bits_per_pixel == 32 && bits_per_component == 8 {
                // Check if it's BGRA or RGBA
                let color_space = CGImageGetColorSpace(image);
                let is_bgra = self.is_bgra_format(color_space);

                if is_bgra {
                    // Convert BGRA to RGBA
                    self.convert_bgra_to_rgba(
                        src_slice,
                        pooled_buffer.data_mut(),
                        width,
                        height,
                        bytes_per_row,
                    );
                } else {
                    // Already RGBA, just copy
                    self.copy_rgba_data(
                        src_slice,
                        pooled_buffer.data_mut(),
                        width,
                        height,
                        bytes_per_row,
                    );
                }

                PixelFormat::RGBA8
            } else if bits_per_pixel == 24 && bits_per_component == 8 {
                // RGB format
                self.copy_rgb_data(
                    src_slice,
                    pooled_buffer.data_mut(),
                    width,
                    height,
                    bytes_per_row,
                );
                PixelFormat::RGB8
            } else {
                CFRelease(data_ref as _);
                CFRelease(image as _);
                return Err(CaptureError::CaptureFailed(
                    format!("Unsupported pixel format: {}bpp, {}bpc", bits_per_pixel, bits_per_component),
                ));
            };

            // Clean up CoreGraphics objects
            CFRelease(data_ref as _);
            CFRelease(image as _);

            // Create RawImage from pooled buffer
            let data = pooled_buffer.into_vec();
            Ok(RawImage::new(data, width, height, pixel_format))
        }
    }

    /// Check if the color space indicates BGRA format
    fn is_bgra_format(&self, color_space: CGColorSpaceRef) -> bool {
        // macOS typically uses BGRA format for screen captures
        // This is a simplified check - in production, you'd check the actual color space
        true
    }

    /// Convert BGRA to RGBA
    fn convert_bgra_to_rgba(
        &self,
        src: &[u8],
        dst: &mut [u8],
        width: u32,
        height: u32,
        src_stride: usize,
    ) {
        let dst_stride = (width * 4) as usize;

        for y in 0..height as usize {
            let src_row = &src[y * src_stride..];
            let dst_row = &mut dst[y * dst_stride..];

            for x in 0..width as usize {
                let src_offset = x * 4;
                let dst_offset = x * 4;

                // BGRA -> RGBA
                dst_row[dst_offset] = src_row[src_offset + 2];     // R
                dst_row[dst_offset + 1] = src_row[src_offset + 1]; // G
                dst_row[dst_offset + 2] = src_row[src_offset];     // B
                dst_row[dst_offset + 3] = src_row[src_offset + 3]; // A
            }
        }
    }

    /// Copy RGBA data
    fn copy_rgba_data(
        &self,
        src: &[u8],
        dst: &mut [u8],
        width: u32,
        height: u32,
        src_stride: usize,
    ) {
        let dst_stride = (width * 4) as usize;

        for y in 0..height as usize {
            let src_row = &src[y * src_stride..y * src_stride + dst_stride];
            let dst_row = &mut dst[y * dst_stride..y * dst_stride + dst_stride];
            dst_row.copy_from_slice(src_row);
        }
    }

    /// Copy RGB data
    fn copy_rgb_data(
        &self,
        src: &[u8],
        dst: &mut [u8],
        width: u32,
        height: u32,
        src_stride: usize,
    ) {
        let dst_stride = (width * 3) as usize;

        for y in 0..height as usize {
            let src_row = &src[y * src_stride..y * src_stride + dst_stride];
            let dst_row = &mut dst[y * dst_stride..y * dst_stride + dst_stride];
            dst_row.copy_from_slice(src_row);
        }
    }

    /// Capture with cursor overlay
    pub fn capture_with_cursor(&self, region: CaptureRegion) -> CaptureResult<RawImage> {
        // First capture without cursor
        let mut image = self.capture_region(region)?;

        // Get cursor position and draw it
        unsafe {
            use core_graphics::event::{CGEvent, CGEventType};

            // Get current cursor position
            if let Ok(event) = CGEvent::new(CGEventType::MouseMoved) {
                let cursor_pos = event.location();

                // Check if cursor is within capture region
                if cursor_pos.x >= region.x as f64
                    && cursor_pos.y >= region.y as f64
                    && cursor_pos.x < (region.x + region.width as i32) as f64
                    && cursor_pos.y < (region.y + region.height as i32) as f64
                {
                    // Draw cursor at relative position
                    // This is simplified - actual implementation would need cursor image data
                    let rel_x = (cursor_pos.x - region.x as f64) as u32;
                    let rel_y = (cursor_pos.y - region.y as f64) as u32;

                    log::debug!("Cursor at ({}, {}) in captured region", rel_x, rel_y);
                }
            }
        }

        Ok(image)
    }
}

// CoreGraphics FFI declarations
type CGColorSpaceRef = *mut std::ffi::c_void;
type CGImageRef = *mut std::ffi::c_void;
type CGDataProviderRef = *mut std::ffi::c_void;
type CFDataRef = *mut std::ffi::c_void;

extern "C" {
    fn CGImageGetWidth(image: CGImageRef) -> usize;
    fn CGImageGetHeight(image: CGImageRef) -> usize;
    fn CGImageGetBytesPerRow(image: CGImageRef) -> usize;
    fn CGImageGetBitsPerPixel(image: CGImageRef) -> usize;
    fn CGImageGetBitsPerComponent(image: CGImageRef) -> usize;
    fn CGImageGetColorSpace(image: CGImageRef) -> CGColorSpaceRef;
    fn CGImageGetDataProvider(image: CGImageRef) -> CGDataProviderRef;
    fn CGDataProviderCopyData(provider: CGDataProviderRef) -> CFDataRef;
    fn CFDataGetBytePtr(data: CFDataRef) -> *const u8;
    fn CFDataGetLength(data: CFDataRef) -> isize;
}