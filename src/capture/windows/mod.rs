//! Windows screen capture implementation

#[cfg(target_os = "windows")]
mod gdi;

#[cfg(target_os = "windows")]
mod capture_api;

#[cfg(target_os = "windows")]
use crate::{
    capture::traits::{CaptureCapabilities, ScreenCapture},
    error::{CaptureError, CaptureResult},
    types::{CaptureRegion, DisplayInfo, RawImage},
};

#[cfg(target_os = "windows")]
use windows::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        UI::WindowsAndMessaging::*,
    },
};

/// Windows screen capture implementation
#[cfg(target_os = "windows")]
pub struct WindowsCapture {
    use_hardware_acceleration: bool,
    gdi_capturer: gdi::GdiCapture,
    capture_api: Option<capture_api::WindowsCaptureApi>,
}

#[cfg(target_os = "windows")]
impl WindowsCapture {
    /// Create a new Windows capturer
    pub fn new() -> CaptureResult<Box<dyn ScreenCapture>> {
        let gdi_capturer = gdi::GdiCapture::new()?;

        Ok(Box::new(Self {
            use_hardware_acceleration: false,
            gdi_capturer,
            capture_api: None,
        }))
    }

    /// Create a Windows capturer with hardware acceleration
    pub fn with_hardware_acceleration() -> CaptureResult<Box<dyn ScreenCapture>> {
        let gdi_capturer = gdi::GdiCapture::new()?;

        // Try to initialize Windows.Graphics.Capture API (Windows 10 1903+)
        let capture_api = capture_api::WindowsCaptureApi::new().ok();

        Ok(Box::new(Self {
            use_hardware_acceleration: capture_api.is_some(),
            gdi_capturer,
            capture_api,
        }))
    }

    /// Enumerate all displays
    fn enumerate_displays() -> CaptureResult<Vec<DisplayInfo>> {
        let mut displays = Vec::new();
        let mut monitor_index: usize = 0;  // Use usize directly to ensure proper alignment

        unsafe {
            // Callback data structure
            struct EnumData {
                displays: *mut Vec<DisplayInfo>,
                index: *mut usize,
            }

            // Enum callback function
            unsafe extern "system" fn monitor_enum_proc(
                hmonitor: HMONITOR,
                _hdc: HDC,
                _rect: *mut RECT,
                lparam: LPARAM,
            ) -> BOOL {
                let data = &mut *(lparam.0 as *mut EnumData);
                let displays = &mut *data.displays;
                let index = *data.index;

                let mut info = MONITORINFOEXW {
                    monitorInfo: MONITORINFO {
                        cbSize: std::mem::size_of::<MONITORINFOEXW>() as u32,
                        ..Default::default()
                    },
                    ..Default::default()
                };

                if GetMonitorInfoW(hmonitor, &mut info.monitorInfo as *mut _ as *mut MONITORINFO).as_bool() {
                    let rect = info.monitorInfo.rcMonitor;
                    let _work_rect = info.monitorInfo.rcWork;

                    // Get device name
                    let device_name = String::from_utf16_lossy(
                        &info.szDevice[..info.szDevice.iter().position(|&c| c == 0).unwrap_or(0)]
                    );

                    displays.push(DisplayInfo {
                        index,
                        name: device_name,
                        width: (rect.right - rect.left) as u32,
                        height: (rect.bottom - rect.top) as u32,
                        x: rect.left,
                        y: rect.top,
                        scale_factor: 1.0, // Will be updated with DPI info
                        is_primary: (info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY) != 0,
                        refresh_rate: 60, // Default, will be updated
                        color_depth: 32,  // Default
                    });

                    *data.index += 1;
                }

                TRUE
            }

            let mut enum_data = EnumData {
                displays: &mut displays as *mut Vec<DisplayInfo>,
                index: &mut monitor_index as *mut usize,  // No cast needed, already usize
            };

            let _ = EnumDisplayMonitors(
                HDC::default(),
                None,
                Some(monitor_enum_proc),
                LPARAM(&mut enum_data as *mut EnumData as isize),
            );
        }

        if displays.is_empty() {
            return Err(CaptureError::DisplayEnumerationFailed(
                "No displays found".to_string(),
            ));
        }

        Ok(displays)
    }
}

#[cfg(target_os = "windows")]
impl ScreenCapture for WindowsCapture {
    fn get_displays(&self) -> CaptureResult<Vec<DisplayInfo>> {
        Self::enumerate_displays()
    }

    fn capture_display(&self, display_index: usize) -> CaptureResult<RawImage> {
        // Try hardware acceleration first if available
        if let Some(ref capture_api) = self.capture_api {
            if let Ok(image) = capture_api.capture_display(display_index) {
                return Ok(image);
            }
        }

        // Fallback to GDI
        self.gdi_capturer.capture_display(display_index)
    }

    fn capture_region(&self, region: CaptureRegion) -> CaptureResult<RawImage> {
        eprintln!("[WINDOWS] capture_region called with region: {:?}", region);

        // Try hardware acceleration first if available
        if let Some(ref capture_api) = self.capture_api {
            eprintln!("[WINDOWS] Trying hardware acceleration (Windows.Graphics.Capture)...");
            if let Ok(image) = capture_api.capture_region(region) {
                eprintln!("[WINDOWS] ✅ Hardware acceleration succeeded!");
                return Ok(image);
            }
            eprintln!("[WINDOWS] Hardware acceleration failed, falling back to GDI");
        } else {
            eprintln!("[WINDOWS] No hardware acceleration available, using GDI");
        }

        // Fallback to GDI
        eprintln!("[WINDOWS] Calling gdi_capturer.capture_region()...");
        let result = self.gdi_capturer.capture_region(region);
        if result.is_ok() {
            eprintln!("[WINDOWS] ✅ GDI capture succeeded!");
        } else {
            eprintln!("[WINDOWS] ❌ GDI capture failed: {:?}", result.as_ref().err());
        }
        result
    }

    fn implementation_name(&self) -> String {
        if self.use_hardware_acceleration {
            "Windows Graphics Capture API".to_string()
        } else {
            "Windows GDI".to_string()
        }
    }

    fn is_hardware_accelerated(&self) -> bool {
        self.use_hardware_acceleration
    }

    fn capabilities(&self) -> CaptureCapabilities {
        CaptureCapabilities {
            supports_cursor: true,
            supports_window_capture: true,
            supports_hdr: self.use_hardware_acceleration,
            max_resolution: (0, 0), // No limit
            supports_multi_display: true,
            supports_gpu_acceleration: self.use_hardware_acceleration,
            estimated_latency_ms: if self.use_hardware_acceleration { 10 } else { 20 },
        }
    }
}

// Stub implementation for non-Windows platforms
#[cfg(not(target_os = "windows"))]
pub struct WindowsCapture;

#[cfg(not(target_os = "windows"))]
impl WindowsCapture {
    pub fn new() -> CaptureResult<Box<dyn ScreenCapture>> {
        Err(CaptureError::PlatformError(
            "Windows capture is only available on Windows".to_string(),
        ))
    }

    pub fn with_hardware_acceleration() -> CaptureResult<Box<dyn ScreenCapture>> {
        Err(CaptureError::PlatformError(
            "Windows capture is only available on Windows".to_string(),
        ))
    }
}