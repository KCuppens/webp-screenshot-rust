//! FFI/C API for WebP Screenshot library
//!
//! Provides C-compatible interface for use from other languages

#![allow(non_camel_case_types)]

use crate::{
    CaptureConfig, WebPConfig, WebPScreenshot,
};
use libc::{c_char, c_int, c_uint, c_void, size_t};
use std::{
    ffi::CString,
    ptr,
};

/// Opaque handle for WebPScreenshot instance
pub struct webp_screenshot_handle {
    inner: Box<WebPScreenshot>,
}

/// Display information structure for C API
#[repr(C)]
pub struct webp_display_info {
    pub index: c_uint,
    pub width: c_uint,
    pub height: c_uint,
    pub x: c_int,
    pub y: c_int,
    pub scale_factor: f32,
    pub is_primary: c_int,
    pub refresh_rate: c_uint,
    pub name: *const c_char,
}

/// WebP configuration for C API
#[repr(C)]
pub struct webp_config {
    pub quality: c_uint,
    pub method: c_uint,
    pub lossless: c_int,
    pub near_lossless: c_uint,
    pub segments: c_uint,
    pub sns_strength: c_uint,
    pub filter_strength: c_uint,
    pub filter_sharpness: c_uint,
    pub auto_filter: c_int,
    pub alpha_compression: c_int,
    pub alpha_filtering: c_uint,
    pub alpha_quality: c_uint,
    pub pass: c_uint,
    pub thread_count: c_uint,
    pub low_memory: c_int,
    pub exact: c_int,
}

/// Capture options for C API
#[repr(C)]
pub struct capture_options {
    pub webp_config: webp_config,
    pub include_cursor: c_int,
    pub use_hardware_acceleration: c_int,
    pub max_retries: c_uint,
    pub retry_delay_ms: c_uint,
}

/// Screenshot result for C API
#[repr(C)]
pub struct screenshot_result {
    pub data: *mut c_void,
    pub size: size_t,
    pub width: c_uint,
    pub height: c_uint,
    pub success: c_int,
    pub error_message: *const c_char,
}

/// Statistics for C API
#[repr(C)]
pub struct performance_stats {
    pub total_captures: u64,
    pub successful_captures: u64,
    pub failed_captures: u64,
    pub total_bytes_captured: u64,
    pub total_bytes_encoded: u64,
    pub average_capture_time_ms: f64,
    pub average_compression_ratio: f64,
}

// Error codes
const SUCCESS: c_int = 0;
const ERROR_NULL_POINTER: c_int = -1;
#[allow(dead_code)]
const ERROR_INVALID_PARAMETER: c_int = -2;
const ERROR_OUT_OF_MEMORY: c_int = -3;
const ERROR_CAPTURE_FAILED: c_int = -4;
#[allow(dead_code)]
const ERROR_ENCODING_FAILED: c_int = -5;
#[allow(dead_code)]
const ERROR_PERMISSION_DENIED: c_int = -6;
#[allow(dead_code)]
const ERROR_DISPLAY_NOT_FOUND: c_int = -7;
#[allow(dead_code)]
const ERROR_NOT_SUPPORTED: c_int = -8;

/// Create a new WebPScreenshot instance
#[no_mangle]
pub extern "C" fn webp_screenshot_create() -> *mut webp_screenshot_handle {
    match WebPScreenshot::new() {
        Ok(screenshot) => {
            let handle = Box::new(webp_screenshot_handle {
                inner: Box::new(screenshot),
            });
            Box::into_raw(handle)
        }
        Err(_) => ptr::null_mut(),
    }
}

/// Create with custom options
#[no_mangle]
pub extern "C" fn webp_screenshot_create_with_options(
    options: *const capture_options,
) -> *mut webp_screenshot_handle {
    if options.is_null() {
        return ptr::null_mut();
    }

    unsafe {
        let opts = &*options;
        let config = convert_capture_options(opts);

        match WebPScreenshot::with_config(config) {
            Ok(screenshot) => {
                let handle = Box::new(webp_screenshot_handle {
                    inner: Box::new(screenshot),
                });
                Box::into_raw(handle)
            }
            Err(_) => ptr::null_mut(),
        }
    }
}

/// Destroy a WebPScreenshot instance
#[no_mangle]
pub extern "C" fn webp_screenshot_destroy(handle: *mut webp_screenshot_handle) {
    if !handle.is_null() {
        unsafe {
            let _ = Box::from_raw(handle);
        }
    }
}

/// Get available displays
#[no_mangle]
pub extern "C" fn webp_screenshot_get_displays(
    handle: *mut webp_screenshot_handle,
    displays: *mut webp_display_info,
    count: *mut c_uint,
) -> c_int {
    if handle.is_null() || count.is_null() {
        return ERROR_NULL_POINTER;
    }

    unsafe {
        let screenshot = &(*handle).inner;

        match screenshot.get_displays() {
            Ok(display_list) => {
                let display_count = display_list.len() as c_uint;

                if displays.is_null() {
                    // Just return count
                    *count = display_count;
                    return SUCCESS;
                }

                let max_count = (*count).min(display_count);
                *count = max_count;

                for i in 0..max_count as usize {
                    let info = &display_list[i];
                    let c_info = webp_display_info {
                        index: i as c_uint,
                        width: info.width,
                        height: info.height,
                        x: info.x,
                        y: info.y,
                        scale_factor: info.scale_factor,
                        is_primary: if info.is_primary { 1 } else { 0 },
                        refresh_rate: info.refresh_rate,
                        name: CString::new(info.name.clone())
                            .unwrap_or_default()
                            .into_raw(),
                    };
                    ptr::write(displays.add(i), c_info);
                }

                SUCCESS
            }
            Err(_) => ERROR_CAPTURE_FAILED,
        }
    }
}

/// Capture a display
#[no_mangle]
pub extern "C" fn webp_screenshot_capture_display(
    handle: *mut webp_screenshot_handle,
    display_index: c_uint,
    result: *mut screenshot_result,
) -> c_int {
    if handle.is_null() || result.is_null() {
        return ERROR_NULL_POINTER;
    }

    unsafe {
        let screenshot = &mut (*handle).inner;

        match screenshot.capture_display(display_index as usize) {
            Ok(capture) => {
                let data_size = capture.data.len();
                let data_ptr = libc::malloc(data_size) as *mut u8;

                if data_ptr.is_null() {
                    return ERROR_OUT_OF_MEMORY;
                }

                ptr::copy_nonoverlapping(capture.data.as_ptr(), data_ptr, data_size);

                *result = screenshot_result {
                    data: data_ptr as *mut c_void,
                    size: data_size,
                    width: capture.width,
                    height: capture.height,
                    success: 1,
                    error_message: ptr::null(),
                };

                SUCCESS
            }
            Err(e) => {
                let error_msg = CString::new(e.to_string()).unwrap_or_default();

                *result = screenshot_result {
                    data: ptr::null_mut(),
                    size: 0,
                    width: 0,
                    height: 0,
                    success: 0,
                    error_message: error_msg.into_raw(),
                };

                ERROR_CAPTURE_FAILED
            }
        }
    }
}

/// Free screenshot result
#[no_mangle]
pub extern "C" fn webp_screenshot_free_result(result: *mut screenshot_result) {
    if result.is_null() {
        return;
    }

    unsafe {
        let res = &mut *result;

        if !res.data.is_null() {
            libc::free(res.data);
            res.data = ptr::null_mut();
        }

        if !res.error_message.is_null() {
            let _ = CString::from_raw(res.error_message as *mut c_char);
            res.error_message = ptr::null();
        }
    }
}

/// Get performance statistics
#[no_mangle]
pub extern "C" fn webp_screenshot_get_stats(
    handle: *mut webp_screenshot_handle,
    stats: *mut performance_stats,
) -> c_int {
    if handle.is_null() || stats.is_null() {
        return ERROR_NULL_POINTER;
    }

    unsafe {
        let screenshot = &(*handle).inner;
        let perf_stats = screenshot.stats();

        *stats = performance_stats {
            total_captures: perf_stats.total_captures,
            successful_captures: perf_stats.successful_captures,
            failed_captures: perf_stats.failed_captures,
            total_bytes_captured: perf_stats.total_bytes_captured,
            total_bytes_encoded: perf_stats.total_bytes_encoded,
            average_capture_time_ms: perf_stats.average_capture_time().as_millis() as f64,
            average_compression_ratio: perf_stats.average_compression_ratio(),
        };

        SUCCESS
    }
}

/// Get library version
#[no_mangle]
pub extern "C" fn webp_screenshot_version() -> *const c_char {
    static VERSION: once_cell::sync::Lazy<CString> =
        once_cell::sync::Lazy::new(|| CString::new(crate::version()).unwrap_or_default());
    VERSION.as_ptr()
}

/// Check if hardware acceleration is available
#[no_mangle]
pub extern "C" fn webp_screenshot_is_hardware_accelerated(
    handle: *mut webp_screenshot_handle,
) -> c_int {
    if handle.is_null() {
        return 0;
    }

    unsafe {
        let screenshot = &(*handle).inner;
        if screenshot.is_hardware_accelerated() {
            1
        } else {
            0
        }
    }
}

/// Get implementation name
#[no_mangle]
pub extern "C" fn webp_screenshot_implementation_name(
    handle: *mut webp_screenshot_handle,
) -> *const c_char {
    if handle.is_null() {
        return ptr::null();
    }

    unsafe {
        let screenshot = &(*handle).inner;
        let name = CString::new(screenshot.implementation_name()).unwrap_or_default();
        name.into_raw()
    }
}

/// Free string returned by the library
#[no_mangle]
pub extern "C" fn webp_screenshot_free_string(str: *mut c_char) {
    if !str.is_null() {
        unsafe {
            let _ = CString::from_raw(str);
        }
    }
}

// Helper functions

fn convert_capture_options(opts: &capture_options) -> CaptureConfig {
    CaptureConfig {
        webp_config: WebPConfig {
            quality: opts.webp_config.quality as u8,
            method: opts.webp_config.method as u8,
            lossless: opts.webp_config.lossless != 0,
            near_lossless: opts.webp_config.near_lossless as u8,
            segments: opts.webp_config.segments as u8,
            sns_strength: opts.webp_config.sns_strength as u8,
            filter_strength: opts.webp_config.filter_strength as u8,
            filter_sharpness: opts.webp_config.filter_sharpness as u8,
            auto_filter: opts.webp_config.auto_filter != 0,
            alpha_compression: opts.webp_config.alpha_compression != 0,
            alpha_filtering: opts.webp_config.alpha_filtering as u8,
            alpha_quality: opts.webp_config.alpha_quality as u8,
            pass: opts.webp_config.pass as u8,
            thread_count: opts.webp_config.thread_count as usize,
            low_memory: opts.webp_config.low_memory != 0,
            exact: opts.webp_config.exact != 0,
        },
        include_cursor: opts.include_cursor != 0,
        use_hardware_acceleration: opts.use_hardware_acceleration != 0,
        max_retries: opts.max_retries,
        retry_delay: std::time::Duration::from_millis(opts.retry_delay_ms as u64),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    #[test]
    fn test_c_api_create_destroy() {
        let handle = webp_screenshot_create();
        assert!(!handle.is_null());
        webp_screenshot_destroy(handle);
    }

    #[test]
    fn test_c_api_version() {
        let version = webp_screenshot_version();
        assert!(!version.is_null());

        unsafe {
            let version_str = CStr::from_ptr(version);
            assert!(!version_str.to_bytes().is_empty());
        }
    }
}