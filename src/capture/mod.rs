//! Screen capture module with platform-specific implementations

pub mod traits;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "linux")]
pub mod linux;

pub use traits::ScreenCapture;

use crate::error::CaptureResult;

/// Platform-specific capturer factory
pub struct Capturer;

impl Capturer {
    /// Create a new platform-specific capturer
    pub fn new() -> CaptureResult<Box<dyn ScreenCapture>> {
        #[cfg(target_os = "windows")]
        {
            windows::WindowsCapture::new()
        }

        #[cfg(target_os = "macos")]
        {
            macos::MacOSCapture::new()
        }

        #[cfg(target_os = "linux")]
        {
            linux::LinuxCapture::new()
        }

        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        {
            Err(crate::error::CaptureError::PlatformError(
                "Unsupported platform".to_string(),
            ))
        }
    }

    /// Create a capturer with hardware acceleration if available
    pub fn with_hardware_acceleration() -> CaptureResult<Box<dyn ScreenCapture>> {
        #[cfg(target_os = "windows")]
        {
            windows::WindowsCapture::with_hardware_acceleration()
        }

        #[cfg(target_os = "macos")]
        {
            macos::MacOSCapture::with_hardware_acceleration()
        }

        #[cfg(target_os = "linux")]
        {
            // Linux typically doesn't have specific hardware acceleration for capture
            linux::LinuxCapture::new()
        }

        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        {
            Err(crate::error::CaptureError::PlatformError(
                "Unsupported platform".to_string(),
            ))
        }
    }
}