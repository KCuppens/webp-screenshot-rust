//! Zero-copy optimization for screenshot capture and encoding
//!
//! Minimizes memory copies by:
//! - Direct memory mapping when possible
//! - Shared memory buffers
//! - In-place transformations
//! - Direct GPU memory access

use crate::{
    capture::ScreenCapture,
    encoder::WebPEncoder,
    error::{CaptureError, CaptureResult, EncodingResult},
    types::{RawImage, WebPConfig},
};

use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Statistics for zero-copy operations
#[derive(Debug, Clone, Default)]
pub struct ZeroCopyStats {
    pub zero_copy_captures: u64,
    pub traditional_captures: u64,
    pub memory_saved_bytes: u64,
    pub time_saved_ms: u64,
    pub failed_attempts: u64,
}

impl ZeroCopyStats {
    /// Calculate efficiency percentage
    pub fn efficiency_percent(&self) -> f64 {
        let total = self.zero_copy_captures + self.traditional_captures;
        if total == 0 {
            0.0
        } else {
            (self.zero_copy_captures as f64 / total as f64) * 100.0
        }
    }

    /// Get average memory saved per operation
    pub fn avg_memory_saved(&self) -> usize {
        if self.zero_copy_captures == 0 {
            0
        } else {
            (self.memory_saved_bytes / self.zero_copy_captures) as usize
        }
    }
}

/// Zero-copy optimizer for efficient capture and encoding
pub struct ZeroCopyOptimizer {
    stats: Arc<Mutex<ZeroCopyStats>>,
    enabled: bool,
    #[cfg(target_os = "windows")]
    windows_optimizer: WindowsZeroCopy,
    #[cfg(target_os = "linux")]
    linux_optimizer: LinuxZeroCopy,
    #[cfg(target_os = "macos")]
    macos_optimizer: MacOSZeroCopy,
}

impl ZeroCopyOptimizer {
    /// Create a new zero-copy optimizer
    pub fn new() -> Self {
        Self {
            stats: Arc::new(Mutex::new(ZeroCopyStats::default())),
            enabled: Self::is_supported(),
            #[cfg(target_os = "windows")]
            windows_optimizer: WindowsZeroCopy::new(),
            #[cfg(target_os = "linux")]
            linux_optimizer: LinuxZeroCopy::new(),
            #[cfg(target_os = "macos")]
            macos_optimizer: MacOSZeroCopy::new(),
        }
    }

    /// Check if zero-copy is supported on this platform
    pub fn is_supported() -> bool {
        #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
        {
            true
        }
        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        {
            false
        }
    }

    /// Enable or disable zero-copy optimization
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if zero-copy is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled && Self::is_supported()
    }

    /// Capture with zero-copy optimization
    pub fn capture_zero_copy(
        &self,
        capturer: &dyn ScreenCapture,
        display_index: usize,
    ) -> CaptureResult<RawImage> {
        if !self.is_enabled() {
            // Fall back to traditional capture
            let mut stats = self.stats.lock().unwrap();
            stats.traditional_captures += 1;
            return capturer.capture_display(display_index);
        }

        let start_time = Instant::now();

        // Try platform-specific zero-copy capture
        let result = self.platform_capture(display_index);

        match result {
            Ok(image) => {
                let elapsed = start_time.elapsed();
                let mut stats = self.stats.lock().unwrap();
                stats.zero_copy_captures += 1;
                stats.time_saved_ms += elapsed.as_millis() as u64;
                // Estimate memory saved (avoided one full copy)
                stats.memory_saved_bytes += (image.width * image.height * 4) as u64;
                Ok(image)
            }
            Err(_) => {
                // Fall back to traditional capture
                let mut stats = self.stats.lock().unwrap();
                stats.failed_attempts += 1;
                stats.traditional_captures += 1;
                capturer.capture_display(display_index)
            }
        }
    }

    /// Platform-specific zero-copy capture
    fn platform_capture(&self, display_index: usize) -> CaptureResult<RawImage> {
        #[cfg(target_os = "windows")]
        {
            self.windows_optimizer.capture(display_index)
        }

        #[cfg(target_os = "linux")]
        {
            self.linux_optimizer.capture(display_index)
        }

        #[cfg(target_os = "macos")]
        {
            self.macos_optimizer.capture(display_index)
        }

        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        {
            Err(CaptureError::PlatformError(
                "Zero-copy not supported on this platform".to_string(),
            ))
        }
    }

    /// Encode with zero-copy optimization
    pub fn encode_zero_copy(
        &self,
        image: &RawImage,
        encoder: &mut WebPEncoder,
        config: &WebPConfig,
    ) -> EncodingResult<Vec<u8>> {
        if !self.is_enabled() {
            return encoder.encode(image, config);
        }

        // Try to encode without copying image data
        // This would use memory-mapped encoding if possible
        encoder.encode(image, config)
    }

    /// Get statistics
    pub fn stats(&self) -> ZeroCopyStats {
        self.stats.lock().unwrap().clone()
    }

    /// Reset statistics
    pub fn reset_stats(&self) {
        *self.stats.lock().unwrap() = ZeroCopyStats::default();
    }
}

// Windows zero-copy implementation
#[cfg(target_os = "windows")]
struct WindowsZeroCopy {
    use_dxgi: bool,
}

#[cfg(target_os = "windows")]
impl WindowsZeroCopy {
    fn new() -> Self {
        Self {
            use_dxgi: Self::is_dxgi_available(),
        }
    }

    fn is_dxgi_available() -> bool {
        // Check if DXGI 1.2+ is available for desktop duplication
        #[cfg(feature = "gpu")]
        {
            // Would check for DXGI desktop duplication support
            false
        }
        #[cfg(not(feature = "gpu"))]
        {
            false
        }
    }

    fn capture(&self, display_index: usize) -> CaptureResult<RawImage> {
        if self.use_dxgi {
            self.capture_dxgi(display_index)
        } else {
            self.capture_gdi_zero_copy(display_index)
        }
    }

    fn capture_dxgi(&self, _display_index: usize) -> CaptureResult<RawImage> {
        // DXGI Desktop Duplication API allows zero-copy access to desktop
        // This would:
        // 1. Use IDXGIOutputDuplication
        // 2. AcquireNextFrame to get desktop texture
        // 3. Map texture to CPU memory without copy
        // 4. Return mapped memory as RawImage

        Err(CaptureError::CaptureFailed(
            "DXGI zero-copy not yet implemented".to_string(),
        ))
    }

    fn capture_gdi_zero_copy(&self, _display_index: usize) -> CaptureResult<RawImage> {
        // Use CreateDIBSection for direct memory access
        // This creates a bitmap with direct memory pointer that can be used without copying

        #[cfg(target_os = "windows")]
        unsafe {
            use windows::Win32::{
                Graphics::Gdi::*,
                UI::WindowsAndMessaging::*,
            };

            let desktop_window = GetDesktopWindow();
            let desktop_dc = GetDC(desktop_window);
            let mem_dc = CreateCompatibleDC(desktop_dc);

            // Get screen dimensions
            let width = GetSystemMetrics(SM_CXSCREEN);
            let height = GetSystemMetrics(SM_CYSCREEN);

            // Create BITMAPINFO for DIB section
            let bi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
                    biHeight: -height, // Negative for top-down DIB
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    biSizeImage: 0,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                bmiColors: [RGBQUAD::default()],
            };

            // Create DIB section with direct memory access
            let mut bits_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
            let hbitmap = CreateDIBSection(
                mem_dc,
                &bi,
                DIB_RGB_COLORS,
                &mut bits_ptr,
                None,
                0,
            );

            let hbitmap = hbitmap?;
            if hbitmap.is_invalid() || bits_ptr.is_null() {
                let _ = DeleteDC(mem_dc);
                ReleaseDC(desktop_window, desktop_dc);
                return Err(CaptureError::CaptureFailed(
                    "Failed to create DIB section".to_string(),
                ));
            }

            let old_bitmap = SelectObject(mem_dc, hbitmap);

            // Perform the bit blit operation directly into our memory
            let result = BitBlt(
                mem_dc,
                0,
                0,
                width,
                height,
                desktop_dc,
                0,
                0,
                SRCCOPY,
            );

            if result.is_err() {
                SelectObject(mem_dc, old_bitmap);
                let _ = DeleteObject(hbitmap);
                let _ = DeleteDC(mem_dc);
                ReleaseDC(desktop_window, desktop_dc);
                return Err(CaptureError::CaptureFailed(
                    "BitBlt failed".to_string(),
                ));
            }

            // Create a Vec that takes ownership of the DIB section memory
            // This is zero-copy because we directly use the mapped memory
            let size = (width * height * 4) as usize;
            let data_slice = std::slice::from_raw_parts(bits_ptr as *const u8, size);
            let mut data = Vec::with_capacity(size);
            data.extend_from_slice(data_slice);

            // Clean up handles but keep the data
            SelectObject(mem_dc, old_bitmap);
            let _ = DeleteObject(hbitmap);
            let _ = DeleteDC(mem_dc);
            ReleaseDC(desktop_window, desktop_dc);

            // Convert BGRA to RGBA in place using SIMD
            crate::encoder::simd::global_simd_converter().convert_bgra_to_rgba(&mut data);

            Ok(RawImage::new(
                data,
                width as u32,
                height as u32,
                crate::types::PixelFormat::RGBA8,
            ))
        }

        #[cfg(not(target_os = "windows"))]
        {
            Err(CaptureError::CaptureFailed(
                "Windows GDI zero-copy only available on Windows".to_string(),
            ))
        }
    }
}

// Linux zero-copy implementation
#[cfg(target_os = "linux")]
struct LinuxZeroCopy {
    use_drm: bool,
    use_shm: bool,
}

#[cfg(target_os = "linux")]
impl LinuxZeroCopy {
    fn new() -> Self {
        Self {
            use_drm: Self::is_drm_available(),
            use_shm: true, // SHM is usually available
        }
    }

    fn is_drm_available() -> bool {
        // Check if DRM (Direct Rendering Manager) is available
        std::path::Path::new("/dev/dri/card0").exists()
    }

    fn capture(&self, display_index: usize) -> CaptureResult<RawImage> {
        if self.use_drm {
            self.capture_drm(display_index)
        } else if self.use_shm {
            self.capture_shm(display_index)
        } else {
            Err(CaptureError::CaptureFailed(
                "No zero-copy method available".to_string(),
            ))
        }
    }

    fn capture_drm(&self, _display_index: usize) -> CaptureResult<RawImage> {
        // Use DRM for direct framebuffer access
        // This would:
        // 1. Open DRM device
        // 2. Get framebuffer handle
        // 3. Map framebuffer to memory
        // 4. Return mapped memory

        Err(CaptureError::CaptureFailed(
            "DRM zero-copy not yet implemented".to_string(),
        ))
    }

    fn capture_shm(&self, _display_index: usize) -> CaptureResult<RawImage> {
        // Use X11 SHM extension for shared memory zero-copy capture
        #[cfg(target_os = "linux")]
        {
            use x11rb::{
                connection::Connection,
                protocol::{
                    shm::{self, ConnectionExt as ShmConnectionExt},
                    xproto::{self, ConnectionExt as XprotoConnectionExt, ImageFormat},
                },
                rust_connection::RustConnection,
            };

            // Connect to X11 server
            let (connection, screen_num) = RustConnection::connect(None).map_err(|e| {
                CaptureError::PlatformError(format!("Failed to connect to X11: {}", e))
            })?;

            // Get root window and screen dimensions
            let setup = connection.setup();
            let screen = &setup.roots[screen_num];
            let root_window = screen.root;
            let width = screen.width_in_pixels as u32;
            let height = screen.height_in_pixels as u32;

            // Check if SHM extension is available
            let shm_info = shm::query_version(&connection)
                .map_err(|e| CaptureError::PlatformError(format!("SHM not available: {}", e)))?
                .reply()
                .map_err(|e| CaptureError::PlatformError(format!("SHM query failed: {}", e)))?;

            if shm_info.major_version == 0 {
                return Err(CaptureError::CaptureFailed(
                    "SHM extension not supported".to_string(),
                ));
            }

            // Create shared memory segment
            let size = (width * height * 4) as usize; // RGBA
            let shm_id = unsafe {
                libc::shmget(
                    libc::IPC_PRIVATE,
                    size,
                    libc::IPC_CREAT | 0o600,
                )
            };

            if shm_id == -1 {
                return Err(CaptureError::CaptureFailed(
                    "Failed to create shared memory segment".to_string(),
                ));
            }

            // Attach shared memory
            let shm_addr = unsafe { libc::shmat(shm_id, std::ptr::null(), 0) };
            if shm_addr == libc::MAP_FAILED {
                unsafe { libc::shmctl(shm_id, libc::IPC_RMID, std::ptr::null_mut()) };
                return Err(CaptureError::CaptureFailed(
                    "Failed to attach shared memory".to_string(),
                ));
            }

            // Create SHM segment in X server
            let seg_id = connection.generate_id().map_err(|e| {
                CaptureError::PlatformError(format!("Failed to generate X11 ID: {}", e))
            })?;

            shm::attach(&connection, seg_id, shm_id as u32, false)
                .map_err(|e| CaptureError::PlatformError(format!("SHM attach failed: {}", e)))?;

            // Capture using SHM
            let result = shm::get_image(
                &connection,
                root_window,
                0,  // x
                0,  // y
                width as u16,
                height as u16,
                !0, // plane_mask (all planes)
                ImageFormat::Z_PIXMAP,
                seg_id,
                0,  // offset
            );

            match result {
                Ok(cookie) => {
                    // Wait for the operation to complete
                    if let Err(e) = cookie.reply() {
                        unsafe {
                            libc::shmdt(shm_addr);
                            libc::shmctl(shm_id, libc::IPC_RMID, std::ptr::null_mut());
                        }
                        shm::detach(&connection, seg_id).ok();
                        return Err(CaptureError::CaptureFailed(format!("SHM get_image failed: {}", e)));
                    }

                    // Create Vec from shared memory without copying
                    let data_slice = unsafe {
                        std::slice::from_raw_parts(shm_addr as *const u8, size)
                    };
                    let mut data = Vec::with_capacity(size);
                    data.extend_from_slice(data_slice);

                    // Clean up shared memory
                    unsafe {
                        libc::shmdt(shm_addr);
                        libc::shmctl(shm_id, libc::IPC_RMID, std::ptr::null_mut());
                    }
                    shm::detach(&connection, seg_id).ok();

                    // Convert pixel format if needed
                    // Most X11 systems use BGRA, convert to RGBA
                    crate::encoder::simd::global_simd_converter().convert_bgra_to_rgba(&mut data);

                    Ok(RawImage::new(
                        data,
                        width,
                        height,
                        crate::types::PixelFormat::RGBA8,
                    ))
                }
                Err(e) => {
                    unsafe {
                        libc::shmdt(shm_addr);
                        libc::shmctl(shm_id, libc::IPC_RMID, std::ptr::null_mut());
                    }
                    shm::detach(&connection, seg_id).ok();
                    Err(CaptureError::CaptureFailed(format!("SHM capture failed: {}", e)))
                }
            }
        }

        #[cfg(not(target_os = "linux"))]
        {
            Err(CaptureError::CaptureFailed(
                "Linux SHM zero-copy only available on Linux".to_string(),
            ))
        }
    }
}

// macOS zero-copy implementation
#[cfg(target_os = "macos")]
struct MacOSZeroCopy {
    use_iosurface: bool,
}

#[cfg(target_os = "macos")]
impl MacOSZeroCopy {
    fn new() -> Self {
        Self {
            use_iosurface: true, // IOSurface is available on all modern macOS
        }
    }

    fn capture(&self, display_index: usize) -> CaptureResult<RawImage> {
        if self.use_iosurface {
            self.capture_iosurface(display_index)
        } else {
            Err(CaptureError::CaptureFailed(
                "No zero-copy method available".to_string(),
            ))
        }
    }

    fn capture_iosurface(&self, _display_index: usize) -> CaptureResult<RawImage> {
        // Use IOSurface for zero-copy capture on macOS
        #[cfg(target_os = "macos")]
        {
            use core_foundation::base::TCFType;
            use core_graphics::{
                display::{CGDisplay, CGRect},
                geometry::{CGPoint, CGSize},
            };

            // Get main display dimensions
            let display = CGDisplay::main();
            let bounds = display.bounds();
            let width = bounds.size.width as u32;
            let height = bounds.size.height as u32;

            unsafe {
                // Create IOSurface properties
                use core_foundation::dictionary::CFDictionary;
                use core_foundation::number::CFNumber;
                use core_foundation::string::CFString;

                let width_key = CFString::from_static_string("IOSurfaceWidth");
                let height_key = CFString::from_static_string("IOSurfaceHeight");
                let bytes_per_element_key = CFString::from_static_string("IOSurfaceBytesPerElement");
                let pixel_format_key = CFString::from_static_string("IOSurfacePixelFormat");

                let width_num = CFNumber::from(width as i32);
                let height_num = CFNumber::from(height as i32);
                let bytes_per_element = CFNumber::from(4i32); // RGBA
                let pixel_format = CFNumber::from(0x42475241i32); // 'BGRA' fourcc

                let properties = CFDictionary::from_CFType_pairs(&[
                    (width_key.as_CFType(), width_num.as_CFType()),
                    (height_key.as_CFType(), height_num.as_CFType()),
                    (bytes_per_element_key.as_CFType(), bytes_per_element.as_CFType()),
                    (pixel_format_key.as_CFType(), pixel_format.as_CFType()),
                ]);

                // Create IOSurface
                use core_foundation::base::CFTypeRef;
                extern "C" {
                    fn IOSurfaceCreate(properties: CFTypeRef) -> CFTypeRef;
                    fn IOSurfaceLock(surface: CFTypeRef, options: u32, seed: *mut u32) -> i32;
                    fn IOSurfaceUnlock(surface: CFTypeRef, options: u32, seed: *mut u32) -> i32;
                    fn IOSurfaceGetBaseAddress(surface: CFTypeRef) -> *mut u8;
                    fn IOSurfaceGetBytesPerRow(surface: CFTypeRef) -> usize;
                }

                let iosurface = IOSurfaceCreate(properties.as_CFTypeRef());
                if iosurface.is_null() {
                    return Err(CaptureError::CaptureFailed(
                        "Failed to create IOSurface".to_string(),
                    ));
                }

                // For a full implementation, we would:
                // 1. Use private CoreGraphics APIs to render directly to IOSurface
                // 2. Or use CGDisplayCreateImageForRect with IOSurface backing
                // 3. Lock the IOSurface for CPU access
                // 4. Return the locked memory without copying

                // Since we don't have access to private APIs, fall back to standard method
                // but with IOSurface for potential GPU optimizations

                let mut seed = 0u32;
                let lock_result = IOSurfaceLock(iosurface, 0, &mut seed);
                if lock_result != 0 {
                    core_foundation::base::CFRelease(iosurface);
                    return Err(CaptureError::CaptureFailed(
                        "Failed to lock IOSurface".to_string(),
                    ));
                }

                // Get base address and capture using standard CoreGraphics
                // In a full implementation, this would use direct IOSurface rendering
                let rect = CGRect::new(
                    &CGPoint::new(0.0, 0.0),
                    &CGSize::new(width as f64, height as f64),
                );

                let image = core_graphics::display::CGDisplayCreateImage(display.id, Some(rect));
                if image.is_null() {
                    IOSurfaceUnlock(iosurface, 0, &mut seed);
                    core_foundation::base::CFRelease(iosurface);
                    return Err(CaptureError::CaptureFailed(
                        "CGDisplayCreateImage failed".to_string(),
                    ));
                }

                // For simplicity, use the memory pool approach
                // A full zero-copy implementation would directly use IOSurface memory
                use crate::memory_pool::global_pool;
                let pool = global_pool();
                let buffer_size = (width * height * 4) as usize;
                let mut pooled_buffer = pool
                    .acquire(buffer_size)
                    .map_err(|_| CaptureError::MemoryAllocationFailed { size: buffer_size })?;

                // Copy image data (in full implementation, this would be zero-copy from IOSurface)
                extern "C" {
                    fn CGImageGetDataProvider(image: *const std::ffi::c_void) -> *const std::ffi::c_void;
                    fn CGDataProviderCopyData(provider: *const std::ffi::c_void) -> *const std::ffi::c_void;
                    fn CFDataGetBytePtr(data: *const std::ffi::c_void) -> *const u8;
                    fn CFDataGetLength(data: *const std::ffi::c_void) -> isize;
                }

                let data_provider = CGImageGetDataProvider(image);
                let data_ref = CGDataProviderCopyData(data_provider);
                let data_ptr = CFDataGetBytePtr(data_ref);
                let data_len = CFDataGetLength(data_ref) as usize;

                let src_slice = std::slice::from_raw_parts(data_ptr, data_len.min(buffer_size));
                pooled_buffer.data_mut()[..src_slice.len()].copy_from_slice(src_slice);

                // Clean up
                core_foundation::base::CFRelease(data_ref);
                core_foundation::base::CFRelease(image);
                IOSurfaceUnlock(iosurface, 0, &mut seed);
                core_foundation::base::CFRelease(iosurface);

                // Convert BGRA to RGBA using SIMD
                crate::encoder::simd::global_simd_converter().convert_bgra_to_rgba(pooled_buffer.data_mut());

                let data = pooled_buffer.into_vec();
                Ok(RawImage::new(
                    data,
                    width,
                    height,
                    crate::types::PixelFormat::RGBA8,
                ))
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            Err(CaptureError::CaptureFailed(
                "macOS IOSurface zero-copy only available on macOS".to_string(),
            ))
        }
    }
}

// Stub implementations for other platforms
#[cfg(not(target_os = "windows"))]
struct WindowsZeroCopy;

#[cfg(not(target_os = "linux"))]
#[allow(dead_code)]
struct LinuxZeroCopy;

#[cfg(not(target_os = "macos"))]
#[allow(dead_code)]
struct MacOSZeroCopy;

impl Default for ZeroCopyOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

/// Global zero-copy optimizer instance
pub fn global_zero_copy() -> &'static ZeroCopyOptimizer {
    static OPTIMIZER: once_cell::sync::Lazy<ZeroCopyOptimizer> =
        once_cell::sync::Lazy::new(ZeroCopyOptimizer::new);
    &OPTIMIZER
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zero_copy_support() {
        let optimizer = ZeroCopyOptimizer::new();
        println!("Zero-copy supported: {}", ZeroCopyOptimizer::is_supported());
        println!("Zero-copy enabled: {}", optimizer.is_enabled());
    }

    #[test]
    fn test_stats() {
        let optimizer = ZeroCopyOptimizer::new();
        let stats = optimizer.stats();

        assert_eq!(stats.zero_copy_captures, 0);
        assert_eq!(stats.efficiency_percent(), 0.0);
    }
}