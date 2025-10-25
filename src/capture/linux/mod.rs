//! Linux screen capture implementation

#[cfg(target_os = "linux")]
mod x11_capture;

#[cfg(all(target_os = "linux", feature = "wayland"))]
mod wayland_capture;

#[cfg(target_os = "linux")]
use crate::{
    capture::traits::{CaptureCapabilities, ScreenCapture},
    error::{CaptureError, CaptureResult},
    types::{CaptureRegion, DisplayInfo, RawImage},
};

/// Linux screen capture implementation
#[cfg(target_os = "linux")]
pub struct LinuxCapture {
    backend: LinuxBackend,
}

#[cfg(target_os = "linux")]
enum LinuxBackend {
    X11(x11_capture::X11Capture),
    #[cfg(feature = "wayland")]
    Wayland(wayland_capture::WaylandCapture),
}

#[cfg(target_os = "linux")]
impl LinuxCapture {
    /// Create a new Linux capturer
    pub fn new() -> CaptureResult<Box<dyn ScreenCapture>> {
        // Detect which backend to use
        let backend = Self::detect_backend()?;

        Ok(Box::new(Self { backend }))
    }

    /// Detect whether to use X11 or Wayland
    fn detect_backend() -> CaptureResult<LinuxBackend> {
        // Check for Wayland session
        #[cfg(feature = "wayland")]
        {
            if std::env::var("WAYLAND_DISPLAY").is_ok() {
                if let Ok(wayland) = wayland_capture::WaylandCapture::new() {
                    return Ok(LinuxBackend::Wayland(wayland));
                }
            }
        }

        // Check for X11 session
        if std::env::var("DISPLAY").is_ok() {
            let x11 = x11_capture::X11Capture::new()?;
            return Ok(LinuxBackend::X11(x11));
        }

        Err(CaptureError::PlatformError(
            "No X11 or Wayland display found".to_string(),
        ))
    }
}

#[cfg(target_os = "linux")]
impl ScreenCapture for LinuxCapture {
    fn get_displays(&self) -> CaptureResult<Vec<DisplayInfo>> {
        match &self.backend {
            LinuxBackend::X11(x11) => x11.get_displays(),
            #[cfg(feature = "wayland")]
            LinuxBackend::Wayland(wayland) => wayland.get_displays(),
        }
    }

    fn capture_display(&self, display_index: usize) -> CaptureResult<RawImage> {
        match &self.backend {
            LinuxBackend::X11(x11) => x11.capture_display(display_index),
            #[cfg(feature = "wayland")]
            LinuxBackend::Wayland(wayland) => wayland.capture_display(display_index),
        }
    }

    fn capture_region(&self, region: CaptureRegion) -> CaptureResult<RawImage> {
        match &self.backend {
            LinuxBackend::X11(x11) => x11.capture_region(region),
            #[cfg(feature = "wayland")]
            LinuxBackend::Wayland(wayland) => wayland.capture_region(region),
        }
    }

    fn implementation_name(&self) -> String {
        match &self.backend {
            LinuxBackend::X11(_) => "Linux X11".to_string(),
            #[cfg(feature = "wayland")]
            LinuxBackend::Wayland(_) => "Linux Wayland".to_string(),
        }
    }

    fn capabilities(&self) -> CaptureCapabilities {
        CaptureCapabilities {
            supports_cursor: true,
            supports_window_capture: true,
            supports_hdr: false,
            max_resolution: (0, 0), // No limit
            supports_multi_display: true,
            supports_gpu_acceleration: false,
            estimated_latency_ms: 20,
        }
    }
}

// Stub for non-Linux platforms
#[cfg(not(target_os = "linux"))]
pub struct LinuxCapture;

#[cfg(not(target_os = "linux"))]
impl LinuxCapture {
    pub fn new() -> CaptureResult<Box<dyn ScreenCapture>> {
        Err(CaptureError::PlatformError(
            "Linux capture is only available on Linux".to_string(),
        ))
    }
}