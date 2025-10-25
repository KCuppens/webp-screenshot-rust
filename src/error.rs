//! Error types for the webp-screenshot library

use thiserror::Error;

/// Main error type for screenshot capture operations
#[derive(Error, Debug)]
pub enum CaptureError {
    /// Display not found or invalid index
    #[error("Display not found: index {0}")]
    DisplayNotFound(usize),

    /// Display enumeration failed
    #[error("Failed to enumerate displays: {0}")]
    DisplayEnumerationFailed(String),

    /// Capture operation failed
    #[error("Capture failed: {0}")]
    CaptureFailed(String),

    /// Permission denied for screen capture
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    /// Platform-specific error
    #[error("Platform error: {0}")]
    PlatformError(String),

    /// Hardware acceleration not available
    #[error("Hardware acceleration not available: {0}")]
    HardwareAccelerationUnavailable(String),

    /// Invalid configuration
    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),

    /// Memory allocation failed
    #[error("Memory allocation failed: requested {size} bytes")]
    MemoryAllocationFailed { size: usize },

    /// Timeout occurred during capture
    #[error("Capture timeout: exceeded {timeout_ms}ms")]
    CaptureTimeout { timeout_ms: u64 },

    /// IO error
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// Windows-specific error
    #[cfg(windows)]
    #[error("Windows error: {0}")]
    WindowsError(#[from] windows::core::Error),

    /// Encoding error
    #[error("Encoding error: {0}")]
    EncodingError(String),

    /// Other errors
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Error type for WebP encoding operations
#[derive(Error, Debug)]
pub enum EncodingError {
    /// Invalid image dimensions
    #[error("Invalid image dimensions: {width}x{height}")]
    InvalidDimensions { width: u32, height: u32 },

    /// Invalid pixel format
    #[error("Invalid pixel format: {0}")]
    InvalidPixelFormat(String),

    /// Invalid configuration
    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),

    /// Unsupported format
    #[error("Unsupported format: {0}")]
    UnsupportedFormat(String),

    /// Encoding failed
    #[error("WebP encoding failed: {0}")]
    EncodingFailed(String),

    /// Invalid quality parameter
    #[error("Invalid quality parameter: {0} (must be 0-100)")]
    InvalidQuality(u8),

    /// Invalid compression method
    #[error("Invalid compression method: {0} (must be 0-6)")]
    InvalidMethod(u8),

    /// Buffer too small
    #[error("Output buffer too small: need {required} bytes, got {provided}")]
    BufferTooSmall { required: usize, provided: usize },

    /// Unsupported feature
    #[error("Unsupported feature: {0}")]
    UnsupportedFeature(String),

    /// Memory allocation failed
    #[error("Memory allocation failed during encoding")]
    MemoryAllocationFailed,

    /// Other encoding errors
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Error type for memory pool operations
#[derive(Error, Debug)]
pub enum MemoryPoolError {
    /// Pool is full
    #[error("Memory pool is full: max capacity {capacity} reached")]
    PoolFull { capacity: usize },

    /// Invalid buffer size
    #[error("Invalid buffer size: {size}")]
    InvalidBufferSize { size: usize },

    /// Buffer not found in pool
    #[error("Buffer not found in pool")]
    BufferNotFound,

    /// Pool is poisoned (mutex error)
    #[error("Memory pool is poisoned")]
    PoolPoisoned,
}

/// Combined result type for capture operations
pub type CaptureResult<T> = Result<T, CaptureError>;

/// Combined result type for encoding operations
pub type EncodingResult<T> = Result<T, EncodingError>;

/// Combined result type for memory pool operations
pub type MemoryPoolResult<T> = Result<T, MemoryPoolError>;

/// Convert Windows HRESULT to CaptureError
#[cfg(windows)]
pub fn from_hresult(hr: windows::core::HRESULT) -> CaptureError {
    CaptureError::WindowsError(windows::core::Error::from(hr))
}

/// Convert error code to human-readable string
pub fn error_code_to_string(code: i32) -> String {
    match code {
        -1 => "Generic error".to_string(),
        -2 => "Invalid parameter".to_string(),
        -3 => "Out of memory".to_string(),
        -4 => "Not supported".to_string(),
        -5 => "Permission denied".to_string(),
        -6 => "Timeout".to_string(),
        _ => format!("Unknown error code: {}", code),
    }
}

impl CaptureError {
    /// Check if the error is recoverable (worth retrying)
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            CaptureError::CaptureTimeout { .. } | CaptureError::MemoryAllocationFailed { .. }
        )
    }

    /// Get error code for FFI
    pub fn to_error_code(&self) -> i32 {
        match self {
            CaptureError::DisplayNotFound(_) => -1001,
            CaptureError::DisplayEnumerationFailed(_) => -1002,
            CaptureError::CaptureFailed(_) => -1003,
            CaptureError::PermissionDenied(_) => -1004,
            CaptureError::PlatformError(_) => -1005,
            CaptureError::HardwareAccelerationUnavailable(_) => -1006,
            CaptureError::InvalidConfiguration(_) => -1007,
            CaptureError::MemoryAllocationFailed { .. } => -1008,
            CaptureError::CaptureTimeout { .. } => -1009,
            CaptureError::IoError(_) => -1010,
            #[cfg(windows)]
            CaptureError::WindowsError(_) => -1011,
            CaptureError::EncodingError(_) => -1012,
            CaptureError::Other(_) => -1999,
        }
    }
}

impl EncodingError {
    /// Check if the error is related to invalid parameters
    pub fn is_parameter_error(&self) -> bool {
        matches!(
            self,
            EncodingError::InvalidDimensions { .. }
                | EncodingError::InvalidPixelFormat(_)
                | EncodingError::InvalidQuality(_)
                | EncodingError::InvalidMethod(_)
        )
    }

    /// Get error code for FFI
    pub fn to_error_code(&self) -> i32 {
        match self {
            EncodingError::InvalidDimensions { .. } => -2001,
            EncodingError::InvalidPixelFormat(_) => -2002,
            EncodingError::InvalidConfiguration(_) => -2003,
            EncodingError::UnsupportedFormat(_) => -2004,
            EncodingError::EncodingFailed(_) => -2005,
            EncodingError::InvalidQuality(_) => -2006,
            EncodingError::InvalidMethod(_) => -2007,
            EncodingError::BufferTooSmall { .. } => -2008,
            EncodingError::UnsupportedFeature(_) => -2009,
            EncodingError::MemoryAllocationFailed => -2010,
            EncodingError::Other(_) => -2999,
        }
    }
}

impl From<EncodingError> for CaptureError {
    fn from(err: EncodingError) -> Self {
        CaptureError::EncodingError(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capture_error_display() {
        let err = CaptureError::DisplayNotFound(2);
        assert_eq!(err.to_string(), "Display not found: index 2");
    }

    #[test]
    fn test_encoding_error_display() {
        let err = EncodingError::InvalidQuality(150);
        assert_eq!(
            err.to_string(),
            "Invalid quality parameter: 150 (must be 0-100)"
        );
    }

    #[test]
    fn test_error_code_conversion() {
        let err = CaptureError::PermissionDenied("Screen recording".to_string());
        assert_eq!(err.to_error_code(), -1004);
    }

    #[test]
    fn test_is_recoverable() {
        let timeout_err = CaptureError::CaptureTimeout { timeout_ms: 5000 };
        assert!(timeout_err.is_recoverable());

        let perm_err = CaptureError::PermissionDenied("test".to_string());
        assert!(!perm_err.is_recoverable());
    }
}