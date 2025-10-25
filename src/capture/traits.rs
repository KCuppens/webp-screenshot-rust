//! Traits for screen capture functionality

use crate::error::CaptureResult;
use crate::types::{CaptureRegion, DisplayInfo, RawImage};

/// Main trait for screen capture implementations
pub trait ScreenCapture: Send + Sync {
    /// Get list of available displays
    fn get_displays(&self) -> CaptureResult<Vec<DisplayInfo>>;

    /// Capture a screenshot from a specific display
    fn capture_display(&self, display_index: usize) -> CaptureResult<RawImage>;

    /// Capture a specific region of the screen
    fn capture_region(&self, region: CaptureRegion) -> CaptureResult<RawImage>;

    /// Get the implementation name
    fn implementation_name(&self) -> String;

    /// Check if hardware acceleration is available
    fn is_hardware_accelerated(&self) -> bool {
        false
    }

    /// Check if the implementation is available on the current system
    fn is_available(&self) -> bool {
        true
    }

    /// Get capabilities of the implementation
    fn capabilities(&self) -> CaptureCapabilities {
        CaptureCapabilities::default()
    }
}

/// Capabilities of a capture implementation
#[derive(Debug, Clone, Default)]
pub struct CaptureCapabilities {
    /// Supports capturing cursor
    pub supports_cursor: bool,
    /// Supports capturing specific windows
    pub supports_window_capture: bool,
    /// Supports HDR capture
    pub supports_hdr: bool,
    /// Maximum resolution supported (0 = unlimited)
    pub max_resolution: (u32, u32),
    /// Supports multi-display capture
    pub supports_multi_display: bool,
    /// Supports GPU acceleration
    pub supports_gpu_acceleration: bool,
    /// Estimated capture latency in milliseconds
    pub estimated_latency_ms: u32,
}

/// Helper trait for pixel format conversion
pub trait PixelFormatConverter {
    /// Convert BGRA to RGBA
    fn convert_bgra_to_rgba(&self, data: &mut [u8]);

    /// Convert BGR to RGB
    fn convert_bgr_to_rgb(&self, data: &mut [u8]);

    /// Convert RGBA to RGB (remove alpha channel)
    fn convert_rgba_to_rgb(&self, src: &[u8], dst: &mut [u8]);

    /// Flip image vertically
    fn flip_vertical(&self, data: &mut [u8], width: u32, height: u32, bytes_per_pixel: u32);
}

/// Default implementation for pixel format conversion
pub struct DefaultPixelConverter;

impl PixelFormatConverter for DefaultPixelConverter {
    fn convert_bgra_to_rgba(&self, data: &mut [u8]) {
        // Process 4 bytes at a time (BGRA -> RGBA)
        for chunk in data.chunks_exact_mut(4) {
            chunk.swap(0, 2); // Swap B and R
        }
    }

    fn convert_bgr_to_rgb(&self, data: &mut [u8]) {
        // Process 3 bytes at a time (BGR -> RGB)
        for chunk in data.chunks_exact_mut(3) {
            chunk.swap(0, 2); // Swap B and R
        }
    }

    fn convert_rgba_to_rgb(&self, src: &[u8], dst: &mut [u8]) {
        let mut dst_idx = 0;
        for chunk in src.chunks_exact(4) {
            dst[dst_idx] = chunk[0];     // R
            dst[dst_idx + 1] = chunk[1]; // G
            dst[dst_idx + 2] = chunk[2]; // B
            dst_idx += 3;
        }
    }

    fn flip_vertical(&self, data: &mut [u8], width: u32, height: u32, bytes_per_pixel: u32) {
        let row_size = (width * bytes_per_pixel) as usize;
        let mut temp_row = vec![0u8; row_size];

        for y in 0..height / 2 {
            let top_offset = (y * width * bytes_per_pixel) as usize;
            let bottom_offset = ((height - 1 - y) * width * bytes_per_pixel) as usize;

            // Copy top row to temp
            temp_row.copy_from_slice(&data[top_offset..top_offset + row_size]);
            // Copy bottom to top
            data.copy_within(bottom_offset..bottom_offset + row_size, top_offset);
            // Copy temp to bottom
            data[bottom_offset..bottom_offset + row_size].copy_from_slice(&temp_row);
        }
    }
}

#[cfg(feature = "simd")]
pub mod simd_converter {
    use super::PixelFormatConverter;

    /// SIMD-optimized pixel format converter
    pub struct SimdPixelConverter;

    impl PixelFormatConverter for SimdPixelConverter {
        #[cfg(target_arch = "x86_64")]
        fn convert_bgra_to_rgba(&self, data: &mut [u8]) {
            use std::arch::x86_64::*;

            unsafe {
                // Check for AVX2 support
                if is_x86_feature_detected!("avx2") {
                    convert_bgra_to_rgba_avx2(data);
                } else if is_x86_feature_detected!("ssse3") {
                    convert_bgra_to_rgba_ssse3(data);
                } else {
                    // Fallback to default implementation
                    DefaultPixelConverter.convert_bgra_to_rgba(data);
                }
            }
        }

        #[cfg(not(target_arch = "x86_64"))]
        fn convert_bgra_to_rgba(&self, data: &mut [u8]) {
            DefaultPixelConverter.convert_bgra_to_rgba(data);
        }

        fn convert_bgr_to_rgb(&self, data: &mut [u8]) {
            // For now, use default implementation
            DefaultPixelConverter.convert_bgr_to_rgb(data);
        }

        fn convert_rgba_to_rgb(&self, src: &[u8], dst: &mut [u8]) {
            DefaultPixelConverter.convert_rgba_to_rgb(src, dst);
        }

        fn flip_vertical(&self, data: &mut [u8], width: u32, height: u32, bytes_per_pixel: u32) {
            DefaultPixelConverter.flip_vertical(data, width, height, bytes_per_pixel);
        }
    }

    #[cfg(target_arch = "x86_64")]
    unsafe fn convert_bgra_to_rgba_avx2(data: &mut [u8]) {
        use std::arch::x86_64::*;

        let shuffle_mask = _mm256_setr_epi8(
            2, 1, 0, 3, 6, 5, 4, 7, 10, 9, 8, 11, 14, 13, 12, 15,
            2, 1, 0, 3, 6, 5, 4, 7, 10, 9, 8, 11, 14, 13, 12, 15,
        );

        let len = data.len();
        let simd_len = len & !31; // Process 32 bytes at a time

        for i in (0..simd_len).step_by(32) {
            let ptr = data.as_mut_ptr().add(i);
            let pixels = _mm256_loadu_si256(ptr as *const __m256i);
            let shuffled = _mm256_shuffle_epi8(pixels, shuffle_mask);
            _mm256_storeu_si256(ptr as *mut __m256i, shuffled);
        }

        // Handle remaining bytes
        for i in (simd_len..len).step_by(4) {
            data.swap(i, i + 2);
        }
    }

    #[cfg(target_arch = "x86_64")]
    unsafe fn convert_bgra_to_rgba_ssse3(data: &mut [u8]) {
        use std::arch::x86_64::*;

        let shuffle_mask = _mm_setr_epi8(
            2, 1, 0, 3, 6, 5, 4, 7, 10, 9, 8, 11, 14, 13, 12, 15,
        );

        let len = data.len();
        let simd_len = len & !15; // Process 16 bytes at a time

        for i in (0..simd_len).step_by(16) {
            let ptr = data.as_mut_ptr().add(i);
            let pixels = _mm_loadu_si128(ptr as *const __m128i);
            let shuffled = _mm_shuffle_epi8(pixels, shuffle_mask);
            _mm_storeu_si128(ptr as *mut __m128i, shuffled);
        }

        // Handle remaining bytes
        for i in (simd_len..len).step_by(4) {
            data.swap(i, i + 2);
        }
    }
}