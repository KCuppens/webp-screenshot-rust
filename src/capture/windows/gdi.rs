//! Windows GDI-based screen capture implementation

use crate::{
    capture::traits::DefaultPixelConverter,
    encoder::global_simd_converter,
    error::{CaptureError, CaptureResult},
    memory_pool::global_pool,
    types::{CaptureRegion, PixelFormat, RawImage},
};

use windows::{
    Win32::{
        Foundation::HWND,
        Graphics::Gdi::*,
        UI::WindowsAndMessaging::*,
    },
};

/// GDI-based screen capture
pub struct GdiCapture {
    #[allow(dead_code)]
    pixel_converter: DefaultPixelConverter,
}

impl GdiCapture {
    /// Create a new GDI capturer
    pub fn new() -> CaptureResult<Self> {
        Ok(Self {
            pixel_converter: DefaultPixelConverter,
        })
    }

    /// Capture a display using GDI
    pub fn capture_display(&self, display_index: usize) -> CaptureResult<RawImage> {
            // Get display info
            let displays = super::WindowsCapture::enumerate_displays()?;
            let display = displays
                .get(display_index)
                .ok_or_else(|| CaptureError::DisplayNotFound(display_index))?;

        // Capture the display region
        self.capture_region(CaptureRegion {
            x: display.x,
            y: display.y,
            width: display.width,
            height: display.height,
        })
    }

    /// Capture a specific region using GDI
    pub fn capture_region(&self, region: CaptureRegion) -> CaptureResult<RawImage> {
        unsafe {
            // Get a DC for the entire virtual desktop (all monitors)
            // GetDC(NULL) returns a DC for the entire virtual screen across all monitors
            // Research confirms this works correctly with negative coordinates for multi-monitor setups
            let desktop_dc = GetDC(HWND(std::ptr::null_mut()));
            if desktop_dc.is_invalid() {
                return Err(CaptureError::CaptureFailed(
                    "Failed to get screen DC for virtual desktop".to_string(),
                ));
            }

            eprintln!("[GDI] 🖥️  Created DC for virtual desktop (all monitors)");
            eprintln!("[GDI] Capturing region: x={}, y={}, width={}, height={}",
                     region.x, region.y, region.width, region.height);

            // Create a compatible DC
            let mem_dc = CreateCompatibleDC(desktop_dc);
            if mem_dc.is_invalid() {
                ReleaseDC(HWND(std::ptr::null_mut()), desktop_dc);
                return Err(CaptureError::CaptureFailed(
                    "Failed to create compatible DC".to_string(),
                ));
            }

            // Create a compatible bitmap
            let hbitmap = CreateCompatibleBitmap(desktop_dc, region.width as i32, region.height as i32);
            if hbitmap.is_invalid() {
                let _ = DeleteDC(mem_dc);
                ReleaseDC(HWND(std::ptr::null_mut()), desktop_dc);
                return Err(CaptureError::CaptureFailed(
                    "Failed to create compatible bitmap".to_string(),
                ));
            }

            eprintln!("[GDI] ✅ Created compatible bitmap: {}x{}", region.width, region.height);

            // Select the bitmap into the DC
            let old_bitmap = SelectObject(mem_dc, hbitmap);

            // Perform the bit-block transfer from desktop DC to memory DC
            // This captures from the virtual desktop using absolute screen coordinates
            // Negative coordinates are valid for monitors positioned left/above the primary
            eprintln!("[GDI] 📸 Calling BitBlt:");
            eprintln!("[GDI]   Source: desktop_dc at ({}, {})", region.x, region.y);
            eprintln!("[GDI]   Destination: mem_dc at (0, 0)");
            eprintln!("[GDI]   Size: {}x{}", region.width, region.height);

            let result = BitBlt(
                mem_dc,
                0,
                0,
                region.width as i32,
                region.height as i32,
                desktop_dc,
                region.x,
                region.y,
                SRCCOPY,
            );

            if result.is_err() {
                use windows::Win32::Foundation::GetLastError;
                let error_code = GetLastError();
                eprintln!("[GDI] ❌ BitBlt FAILED! Error code: {:?}", error_code);

                SelectObject(mem_dc, old_bitmap);
                let _ = DeleteObject(hbitmap);
                let _ = DeleteDC(mem_dc);
                ReleaseDC(HWND(std::ptr::null_mut()), desktop_dc);
                return Err(CaptureError::CaptureFailed(
                    format!("BitBlt failed with error code: {:?}", error_code)
                ));
            }

            eprintln!("[GDI] ✅ BitBlt succeeded!");

            // Get bitmap info
            let mut bmp_info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: region.width as i32,
                    biHeight: -(region.height as i32), // Negative for top-down bitmap
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    biSizeImage: 0,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                ..Default::default()
            };

            // Calculate buffer size
            let pixel_count = (region.width * region.height) as usize;
            let buffer_size = pixel_count * 4; // 4 bytes per pixel (BGRA)

            // Get buffer from pool
            let pool = global_pool();
            let mut pooled_buffer = pool
                .acquire(buffer_size)
                .map_err(|_| CaptureError::MemoryAllocationFailed { size: buffer_size })?;

            // Get the bitmap bits
            let scan_lines = GetDIBits(
                mem_dc,
                hbitmap,
                0,
                region.height,
                Some(pooled_buffer.data_mut().as_mut_ptr() as *mut _),
                &mut bmp_info,
                DIB_RGB_COLORS,
            );

            if scan_lines == 0 || scan_lines == -1 {
                SelectObject(mem_dc, old_bitmap);
                let _ = DeleteObject(hbitmap);
                let _ = DeleteDC(mem_dc);
                ReleaseDC(HWND(std::ptr::null_mut()), desktop_dc);
                return Err(CaptureError::CaptureFailed(
                    "Failed to get bitmap bits".to_string(),
                ));
            }

            // Clean up GDI objects
            SelectObject(mem_dc, old_bitmap);
            let _ = DeleteObject(hbitmap);
            let _ = DeleteDC(mem_dc);
            let _ = DeleteDC(desktop_dc);

            // Convert BGRA to RGBA using optimized SIMD implementation
            global_simd_converter().convert_bgra_to_rgba(pooled_buffer.data_mut());

            // Create RawImage from pooled buffer
            let data = pooled_buffer.into_vec();
            Ok(RawImage::new(
                data,
                region.width,
                region.height,
                PixelFormat::RGBA8,
            ))
        }
    }

    /// Capture with cursor overlay
    #[allow(dead_code)]
    pub fn capture_with_cursor(&self, region: CaptureRegion) -> CaptureResult<RawImage> {
        let image = self.capture_region(region)?;

        unsafe {
            // Get cursor info
            let mut cursor_info = CURSORINFO {
                cbSize: std::mem::size_of::<CURSORINFO>() as u32,
                ..Default::default()
            };

            if GetCursorInfo(&mut cursor_info).is_ok() {
                if cursor_info.flags.0 & CURSOR_SHOWING.0 != 0 {
                    // Draw cursor onto the image
                    let cursor_x = cursor_info.ptScreenPos.x - region.x;
                    let cursor_y = cursor_info.ptScreenPos.y - region.y;

                    if cursor_x >= 0
                        && cursor_y >= 0
                        && cursor_x < region.width as i32
                        && cursor_y < region.height as i32
                    {
                        // Here you would draw the cursor onto the image
                        // This is simplified - actual implementation would need to:
                        // 1. Get cursor icon data
                        // 2. Blend it with the captured image
                        log::debug!("Cursor at ({}, {})", cursor_x, cursor_y);
                    }
                }
            }
        }

        Ok(image)
    }

    /// Capture primary display
    #[allow(dead_code)]
    pub fn capture_primary(&self) -> CaptureResult<RawImage> {
        unsafe {
            let width = GetSystemMetrics(SM_CXSCREEN);
            let height = GetSystemMetrics(SM_CYSCREEN);

            self.capture_region(CaptureRegion {
                x: 0,
                y: 0,
                width: width as u32,
                height: height as u32,
            })
        }
    }

    /// Get virtual screen bounds (all monitors)
    #[allow(dead_code)]
    pub fn get_virtual_screen_bounds(&self) -> CaptureRegion {
        unsafe {
            let x = GetSystemMetrics(SM_XVIRTUALSCREEN);
            let y = GetSystemMetrics(SM_YVIRTUALSCREEN);
            let width = GetSystemMetrics(SM_CXVIRTUALSCREEN);
            let height = GetSystemMetrics(SM_CYVIRTUALSCREEN);

            CaptureRegion {
                x,
                y,
                width: width as u32,
                height: height as u32,
            }
        }
    }
}