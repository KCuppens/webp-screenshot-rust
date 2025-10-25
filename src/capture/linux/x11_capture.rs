//! X11-based screen capture for Linux

use crate::{
    capture::traits::{DefaultPixelConverter, PixelFormatConverter},
    encoder::simd::global_simd_converter,
    error::{CaptureError, CaptureResult},
    memory_pool::global_pool,
    types::{CaptureRegion, DisplayInfo, PixelFormat, RawImage},
};

use x11rb::{
    connection::Connection,
    protocol::{
        randr::{self, ConnectionExt as RandrConnectionExt},
        xfixes::{self, ConnectionExt as XfixesConnectionExt},
        xproto::{self, ConnectionExt as XprotoConnectionExt, ImageFormat},
    },
    rust_connection::RustConnection,
};

/// X11 capture implementation
pub struct X11Capture {
    connection: RustConnection,
    screen_num: usize,
    root_window: xproto::Window,
    pixel_converter: DefaultPixelConverter,
}

impl X11Capture {
    /// Create a new X11 capturer
    pub fn new() -> CaptureResult<Self> {
        // Connect to X11 server
        let (connection, screen_num) = RustConnection::connect(None).map_err(|e| {
            CaptureError::PlatformError(format!("Failed to connect to X11: {}", e))
        })?;

        // Get root window
        let setup = connection.setup();
        let screen = &setup.roots[screen_num];
        let root_window = screen.root;

        Ok(Self {
            connection,
            screen_num,
            root_window,
            pixel_converter: DefaultPixelConverter,
        })
    }

    /// Enumerate displays using XRandR
    pub fn get_displays(&self) -> CaptureResult<Vec<DisplayInfo>> {
        let mut displays = Vec::new();

        // Get screen resources using XRandR
        let resources = randr::get_screen_resources(&self.connection, self.root_window)
            .map_err(|e| CaptureError::DisplayEnumerationFailed(format!("XRandR error: {}", e)))?
            .reply()
            .map_err(|e| CaptureError::DisplayEnumerationFailed(format!("XRandR reply error: {}", e)))?;

        // Get information about each CRTC (display controller)
        for (index, &crtc) in resources.crtcs.iter().enumerate() {
            let crtc_info = randr::get_crtc_info(
                &self.connection,
                crtc,
                resources.config_timestamp,
            )
            .map_err(|e| CaptureError::DisplayEnumerationFailed(format!("CRTC info error: {}", e)))?
            .reply()
            .map_err(|e| CaptureError::DisplayEnumerationFailed(format!("CRTC reply error: {}", e)))?;

            // Skip disabled CRTCs
            if crtc_info.mode == 0 || crtc_info.num_outputs == 0 {
                continue;
            }

            // Find the mode info
            let mode = resources
                .modes
                .iter()
                .find(|m| m.id == crtc_info.mode)
                .ok_or_else(|| {
                    CaptureError::DisplayEnumerationFailed("Mode not found".to_string())
                })?;

            // Calculate refresh rate
            let refresh_rate = if mode.htotal != 0 && mode.vtotal != 0 {
                (mode.dot_clock as u64 * 1000 / (mode.htotal as u64 * mode.vtotal as u64)) as u32
            } else {
                60 // Default
            };

            displays.push(DisplayInfo {
                index,
                name: format!("Display {}", index + 1),
                width: crtc_info.width as u32,
                height: crtc_info.height as u32,
                x: crtc_info.x as i32,
                y: crtc_info.y as i32,
                scale_factor: 1.0, // X11 doesn't directly expose scale factor
                is_primary: index == 0, // First CRTC is typically primary
                refresh_rate,
                color_depth: 24, // Common default, could query actual depth
            });
        }

        if displays.is_empty() {
            // Fallback to root window dimensions if XRandR fails
            let setup = self.connection.setup();
            let screen = &setup.roots[self.screen_num];

            displays.push(DisplayInfo {
                index: 0,
                name: "Primary Display".to_string(),
                width: screen.width_in_pixels as u32,
                height: screen.height_in_pixels as u32,
                x: 0,
                y: 0,
                scale_factor: 1.0,
                is_primary: true,
                refresh_rate: 60,
                color_depth: screen.root_depth,
            });
        }

        Ok(displays)
    }

    /// Capture a display
    pub fn capture_display(&self, display_index: usize) -> CaptureResult<RawImage> {
        let displays = self.get_displays()?;
        let display = displays
            .get(display_index)
            .ok_or_else(|| CaptureError::DisplayNotFound(display_index))?;

        self.capture_region(CaptureRegion {
            x: display.x,
            y: display.y,
            width: display.width,
            height: display.height,
        })
    }

    /// Capture a specific region
    pub fn capture_region(&self, region: CaptureRegion) -> CaptureResult<RawImage> {
        // Get image from X server
        let image_reply = xproto::get_image(
            &self.connection,
            ImageFormat::Z_PIXMAP,
            self.root_window,
            region.x as i16,
            region.y as i16,
            region.width as u16,
            region.height as u16,
            !0, // All planes
        )
        .map_err(|e| CaptureError::CaptureFailed(format!("X11 GetImage error: {}", e)))?
        .reply()
        .map_err(|e| CaptureError::CaptureFailed(format!("X11 GetImage reply error: {}", e)))?;

        // Get visual info for pixel format detection
        let setup = self.connection.setup();
        let screen = &setup.roots[self.screen_num];
        let visual = setup
            .roots
            .iter()
            .flat_map(|screen| &screen.allowed_depths)
            .flat_map(|depth| &depth.visuals)
            .find(|v| v.visual_id == screen.root_visual)
            .ok_or_else(|| CaptureError::CaptureFailed("Visual not found".to_string()))?;

        // Determine pixel format based on depth and visual
        let (pixel_format, bytes_per_pixel) = match image_reply.depth {
            24 | 32 => {
                // Check if it's BGR or RGB based on visual masks
                let is_bgr = self.is_bgr_format(visual);
                if image_reply.depth == 32 {
                    (if is_bgr { PixelFormat::BGRA8 } else { PixelFormat::RGBA8 }, 4)
                } else {
                    (if is_bgr { PixelFormat::BGR8 } else { PixelFormat::RGB8 }, 3)
                }
            }
            depth => {
                return Err(CaptureError::CaptureFailed(
                    format!("Unsupported bit depth: {}", depth),
                ))
            }
        };

        // Get buffer from pool
        let pool = global_pool();
        let buffer_size = (region.width * region.height * bytes_per_pixel as u32) as usize;
        let mut pooled_buffer = pool
            .acquire(buffer_size)
            .map_err(|_| CaptureError::MemoryAllocationFailed { size: buffer_size })?;

        // Process and copy image data
        match pixel_format {
            PixelFormat::BGRA8 => {
                // Copy data first, then convert BGRA to RGBA in place using SIMD
                pooled_buffer.data_mut()[..image_reply.data.len()].copy_from_slice(&image_reply.data);
                global_simd_converter().convert_bgra_to_rgba(pooled_buffer.data_mut());
            }
            PixelFormat::BGR8 => {
                // Copy data first, then convert BGR to RGB in place using SIMD
                pooled_buffer.data_mut()[..image_reply.data.len()].copy_from_slice(&image_reply.data);
                global_simd_converter().convert_bgr_to_rgb(pooled_buffer.data_mut());
            }
            PixelFormat::RGBA8 | PixelFormat::RGB8 => {
                // Direct copy
                pooled_buffer.data_mut()[..image_reply.data.len()].copy_from_slice(&image_reply.data);
            }
            _ => unreachable!(),
        }

        // Convert pixel format for consistency
        let final_format = if bytes_per_pixel == 4 {
            PixelFormat::RGBA8
        } else {
            PixelFormat::RGB8
        };

        let data = pooled_buffer.into_vec();
        Ok(RawImage::new(data, region.width, region.height, final_format))
    }

    /// Check if visual uses BGR format
    fn is_bgr_format(&self, visual: &xproto::Visualtype) -> bool {
        // Check red and blue masks to determine byte order
        // BGR typically has blue in the lower bits
        visual.blue_mask < visual.red_mask
    }

    /// Convert BGRA to RGBA
    fn convert_bgra_to_rgba(&self, src: &[u8], dst: &mut [u8], width: u32, height: u32) {
        let pixel_count = (width * height) as usize;
        for i in 0..pixel_count {
            let offset = i * 4;
            dst[offset] = src[offset + 2];     // R
            dst[offset + 1] = src[offset + 1]; // G
            dst[offset + 2] = src[offset];     // B
            dst[offset + 3] = src[offset + 3]; // A
        }
    }

    /// Convert BGR to RGB
    fn convert_bgr_to_rgb(&self, src: &[u8], dst: &mut [u8], width: u32, height: u32) {
        let pixel_count = (width * height) as usize;
        for i in 0..pixel_count {
            let offset = i * 3;
            dst[offset] = src[offset + 2];     // R
            dst[offset + 1] = src[offset + 1]; // G
            dst[offset + 2] = src[offset];     // B
        }
    }

    /// Capture with cursor using XFixes
    pub fn capture_with_cursor(&self, region: CaptureRegion) -> CaptureResult<RawImage> {
        // First capture without cursor
        let mut image = self.capture_region(region)?;

        // Try to get cursor image using XFixes
        match xfixes::get_cursor_image(&self.connection) {
            Ok(cursor_request) => {
                match cursor_request.reply() {
                    Ok(cursor) => {
                        // Calculate cursor position relative to capture region
                        let cursor_x = cursor.x as i32 - region.x;
                        let cursor_y = cursor.y as i32 - region.y;

                        if cursor_x >= 0
                            && cursor_y >= 0
                            && cursor_x < region.width as i32
                            && cursor_y < region.height as i32
                        {
                            // Blend cursor with captured image
                            self.blend_cursor(
                                &mut image.data,
                                &cursor.cursor_image,
                                cursor.width as u32,
                                cursor.height as u32,
                                cursor_x as u32,
                                cursor_y as u32,
                                region.width,
                            );
                        }
                    }
                    Err(e) => {
                        log::debug!("Failed to get cursor image: {}", e);
                    }
                }
            }
            Err(e) => {
                log::debug!("XFixes not available: {}", e);
            }
        }

        Ok(image)
    }

    /// Blend cursor image with screenshot
    fn blend_cursor(
        &self,
        image: &mut [u8],
        cursor_data: &[u32],
        cursor_width: u32,
        cursor_height: u32,
        cursor_x: u32,
        cursor_y: u32,
        image_width: u32,
    ) {
        for cy in 0..cursor_height {
            for cx in 0..cursor_width {
                let cursor_idx = (cy * cursor_width + cx) as usize;
                let cursor_pixel = cursor_data[cursor_idx];

                // Extract ARGB components
                let alpha = ((cursor_pixel >> 24) & 0xFF) as u8;
                if alpha == 0 {
                    continue; // Skip transparent pixels
                }

                let red = ((cursor_pixel >> 16) & 0xFF) as u8;
                let green = ((cursor_pixel >> 8) & 0xFF) as u8;
                let blue = (cursor_pixel & 0xFF) as u8;

                // Calculate image pixel position
                let img_x = cursor_x + cx;
                let img_y = cursor_y + cy;
                let img_idx = ((img_y * image_width + img_x) * 4) as usize;

                if img_idx + 3 < image.len() {
                    // Alpha blend
                    let alpha_f = alpha as f32 / 255.0;
                    let inv_alpha = 1.0 - alpha_f;

                    image[img_idx] = (red as f32 * alpha_f + image[img_idx] as f32 * inv_alpha) as u8;
                    image[img_idx + 1] = (green as f32 * alpha_f + image[img_idx + 1] as f32 * inv_alpha) as u8;
                    image[img_idx + 2] = (blue as f32 * alpha_f + image[img_idx + 2] as f32 * inv_alpha) as u8;
                    // Keep original alpha
                }
            }
        }
    }
}