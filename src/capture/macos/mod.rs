//! macOS screen capture implementation

#[cfg(target_os = "macos")]
mod core_graphics_capture;

#[cfg(target_os = "macos")]
mod screencapturekit;

#[cfg(target_os = "macos")]
use crate::{
    capture::traits::{CaptureCapabilities, ScreenCapture},
    error::{CaptureError, CaptureResult},
    types::{CaptureRegion, DisplayInfo, PixelFormat, RawImage},
};

#[cfg(target_os = "macos")]
use core_foundation::base::TCFType;
#[cfg(target_os = "macos")]
use core_foundation::string::CFString;
#[cfg(target_os = "macos")]
use core_graphics::display::*;
#[cfg(target_os = "macos")]
use std::ptr;

/// macOS screen capture implementation
#[cfg(target_os = "macos")]
pub struct MacOSCapture {
    use_screencapturekit: bool,
    core_graphics: core_graphics_capture::CoreGraphicsCapture,
    screencapturekit: Option<screencapturekit::ScreenCaptureKit>,
}

#[cfg(target_os = "macos")]
impl MacOSCapture {
    /// Create a new macOS capturer
    pub fn new() -> CaptureResult<Box<dyn ScreenCapture>> {
        let core_graphics = core_graphics_capture::CoreGraphicsCapture::new()?;

        Ok(Box::new(Self {
            use_screencapturekit: false,
            core_graphics,
            screencapturekit: None,
        }))
    }

    /// Create with hardware acceleration (ScreenCaptureKit on macOS 12.3+)
    pub fn with_hardware_acceleration() -> CaptureResult<Box<dyn ScreenCapture>> {
        let core_graphics = core_graphics_capture::CoreGraphicsCapture::new()?;

        // Try to initialize ScreenCaptureKit (macOS 12.3+)
        let screencapturekit = if screencapturekit::ScreenCaptureKit::is_available() {
            screencapturekit::ScreenCaptureKit::new().ok()
        } else {
            None
        };

        Ok(Box::new(Self {
            use_screencapturekit: screencapturekit.is_some(),
            core_graphics,
            screencapturekit,
        }))
    }

    /// Check and request screen recording permissions
    pub fn check_permissions() -> CaptureResult<bool> {
        #[cfg(target_os = "macos")]
        {
            use objc::{class, msg_send, sel, sel_impl};
            use objc::runtime::BOOL;

            unsafe {
                let cls = class!(CGWindowListCreateImage);
                if cls.is_null() {
                    return Ok(true); // Assume permitted on older macOS
                }

                // Check if we can access screen content
                let can_record: BOOL = msg_send![cls, canRecordScreen];
                if can_record == objc::runtime::NO {
                    // Request permission
                    let _: () = msg_send![cls, requestScreenCaptureAccess];
                    return Ok(false);
                }

                Ok(true)
            }
        }

        #[cfg(not(target_os = "macos"))]
        Ok(true)
    }

    /// Enumerate all displays using CoreGraphics
    fn enumerate_displays() -> CaptureResult<Vec<DisplayInfo>> {
        let mut displays = Vec::new();

        unsafe {
            // Get all online displays
            let max_displays = 32;
            let mut display_ids = vec![0u32; max_displays];
            let mut display_count = 0u32;

            let result = CGGetOnlineDisplayList(
                max_displays as u32,
                display_ids.as_mut_ptr(),
                &mut display_count,
            );

            if result != 0 {
                return Err(CaptureError::DisplayEnumerationFailed(
                    "CGGetOnlineDisplayList failed".to_string(),
                ));
            }

            for i in 0..display_count as usize {
                let display_id = display_ids[i];

                // Get display bounds
                let bounds = CGDisplayBounds(display_id);

                // Get display mode for refresh rate and color depth
                let mode = CGDisplayCopyDisplayMode(display_id);
                let refresh_rate = if !mode.is_null() {
                    let rate = CGDisplayModeGetRefreshRate(mode);
                    CGDisplayModeRelease(mode);
                    if rate > 0.0 { rate as u32 } else { 60 }
                } else {
                    60
                };

                // Check if main display
                let is_main = CGDisplayIsMain(display_id) != 0;

                // Get scale factor (for Retina displays)
                let scale_factor = Self::get_display_scale_factor(display_id);

                displays.push(DisplayInfo {
                    index: i,
                    name: format!("Display {}", i + 1),
                    width: bounds.size.width as u32,
                    height: bounds.size.height as u32,
                    x: bounds.origin.x as i32,
                    y: bounds.origin.y as i32,
                    scale_factor,
                    is_primary: is_main,
                    refresh_rate,
                    color_depth: 32, // macOS typically uses 32-bit color
                });
            }
        }

        if displays.is_empty() {
            return Err(CaptureError::DisplayEnumerationFailed(
                "No displays found".to_string(),
            ));
        }

        Ok(displays)
    }

    /// Get display scale factor for Retina displays
    fn get_display_scale_factor(display_id: CGDirectDisplayID) -> f32 {
        #[cfg(target_os = "macos")]
        unsafe {
            use core_foundation::number::{CFNumber, CFNumberRef};
            use core_graphics::display::CGDisplayCopyDisplayMode;

            let mode = CGDisplayCopyDisplayMode(display_id);
            if mode.is_null() {
                return 1.0;
            }

            // Try to get pixel width vs point width to determine scale
            let pixel_width = CGDisplayModeGetPixelWidth(mode);
            let point_width = CGDisplayModeGetWidth(mode);

            CGDisplayModeRelease(mode);

            if point_width > 0 {
                (pixel_width as f32) / (point_width as f32)
            } else {
                1.0
            }
        }

        #[cfg(not(target_os = "macos"))]
        1.0
    }
}

#[cfg(target_os = "macos")]
impl ScreenCapture for MacOSCapture {
    fn get_displays(&self) -> CaptureResult<Vec<DisplayInfo>> {
        Self::enumerate_displays()
    }

    fn capture_display(&self, display_index: usize) -> CaptureResult<RawImage> {
        // Check permissions first
        if !Self::check_permissions()? {
            return Err(CaptureError::PermissionDenied(
                "Screen recording permission required".to_string(),
            ));
        }

        // Try ScreenCaptureKit first if available
        if let Some(ref sck) = self.screencapturekit {
            if let Ok(image) = sck.capture_display(display_index) {
                return Ok(image);
            }
        }

        // Fallback to CoreGraphics
        self.core_graphics.capture_display(display_index)
    }

    fn capture_region(&self, region: CaptureRegion) -> CaptureResult<RawImage> {
        // Check permissions first
        if !Self::check_permissions()? {
            return Err(CaptureError::PermissionDenied(
                "Screen recording permission required".to_string(),
            ));
        }

        // Try ScreenCaptureKit first if available
        if let Some(ref sck) = self.screencapturekit {
            if let Ok(image) = sck.capture_region(region) {
                return Ok(image);
            }
        }

        // Fallback to CoreGraphics
        self.core_graphics.capture_region(region)
    }

    fn implementation_name(&self) -> String {
        if self.use_screencapturekit {
            "macOS ScreenCaptureKit".to_string()
        } else {
            "macOS CoreGraphics".to_string()
        }
    }

    fn is_hardware_accelerated(&self) -> bool {
        self.use_screencapturekit
    }

    fn capabilities(&self) -> CaptureCapabilities {
        CaptureCapabilities {
            supports_cursor: true,
            supports_window_capture: true,
            supports_hdr: self.use_screencapturekit,
            max_resolution: (0, 0), // No limit
            supports_multi_display: true,
            supports_gpu_acceleration: self.use_screencapturekit,
            estimated_latency_ms: if self.use_screencapturekit { 5 } else { 15 },
        }
    }
}

// Stub for non-macOS platforms
#[cfg(not(target_os = "macos"))]
pub struct MacOSCapture;

#[cfg(not(target_os = "macos"))]
impl MacOSCapture {
    pub fn new() -> CaptureResult<Box<dyn ScreenCapture>> {
        Err(CaptureError::PlatformError(
            "macOS capture is only available on macOS".to_string(),
        ))
    }

    pub fn with_hardware_acceleration() -> CaptureResult<Box<dyn ScreenCapture>> {
        Err(CaptureError::PlatformError(
            "macOS capture is only available on macOS".to_string(),
        ))
    }
}

// CoreGraphics FFI declarations
#[cfg(target_os = "macos")]
extern "C" {
    fn CGDisplayBounds(display: CGDirectDisplayID) -> CGRect;
    fn CGDisplayIsMain(display: CGDirectDisplayID) -> i32;
    fn CGGetOnlineDisplayList(
        max_displays: u32,
        online_displays: *mut CGDirectDisplayID,
        display_count: *mut u32,
    ) -> i32;
    fn CGDisplayCopyDisplayMode(display: CGDirectDisplayID) -> CGDisplayModeRef;
    fn CGDisplayModeGetRefreshRate(mode: CGDisplayModeRef) -> f64;
    fn CGDisplayModeGetWidth(mode: CGDisplayModeRef) -> usize;
    fn CGDisplayModeGetPixelWidth(mode: CGDisplayModeRef) -> usize;
    fn CGDisplayModeRelease(mode: CGDisplayModeRef);
}

#[cfg(target_os = "macos")]
type CGDirectDisplayID = u32;
#[cfg(target_os = "macos")]
type CGDisplayModeRef = *mut std::ffi::c_void;

#[cfg(target_os = "macos")]
#[repr(C)]
struct CGRect {
    origin: CGPoint,
    size: CGSize,
}

#[cfg(target_os = "macos")]
#[repr(C)]
struct CGPoint {
    x: f64,
    y: f64,
}

#[cfg(target_os = "macos")]
#[repr(C)]
struct CGSize {
    width: f64,
    height: f64,
}