//! Build script for webp-screenshot-rust

use std::env;

fn main() {
    // Print cargo instructions for conditional compilation
    println!("cargo:rerun-if-changed=build.rs");

    // Get target OS
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();

    // Platform-specific configuration
    match target_os.as_str() {
        "windows" => configure_windows(),
        "macos" => configure_macos(),
        "linux" => configure_linux(),
        _ => {
            println!("cargo:warning=Unsupported target OS: {}", target_os);
        }
    }

    // Architecture-specific configuration
    match target_arch.as_str() {
        "x86_64" => {
            println!("cargo:rustc-cfg=arch_x86_64");
            // Enable SIMD features if available
            if is_x86_feature_detected("avx2") {
                println!("cargo:rustc-cfg=has_avx2");
            }
            if is_x86_feature_detected("sse4.1") {
                println!("cargo:rustc-cfg=has_sse41");
            }
        }
        "aarch64" => {
            println!("cargo:rustc-cfg=arch_aarch64");
            println!("cargo:rustc-cfg=has_neon"); // NEON is standard on aarch64
        }
        _ => {}
    }

    // Feature detection
    detect_features();
}

#[cfg(target_os = "windows")]
fn configure_windows() {
    println!("cargo:rustc-cfg=platform_windows");

    // Link Windows libraries
    println!("cargo:rustc-link-lib=gdi32");
    println!("cargo:rustc-link-lib=user32");
    println!("cargo:rustc-link-lib=kernel32");

    // Check for Windows SDK version
    if let Ok(sdk_version) = env::var("WindowsSDKVersion") {
        println!("cargo:rustc-cfg=windows_sdk_version=\"{}\"", sdk_version);

        // Enable Windows Graphics Capture if SDK is recent enough
        if is_windows_version_supported(&sdk_version) {
            println!("cargo:rustc-cfg=has_windows_capture_api");
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn configure_windows() {}

#[cfg(target_os = "macos")]
fn configure_macos() {
    println!("cargo:rustc-cfg=platform_macos");

    // Link macOS frameworks
    println!("cargo:rustc-link-lib=framework=CoreGraphics");
    println!("cargo:rustc-link-lib=framework=CoreFoundation");
    println!("cargo:rustc-link-lib=framework=AppKit");
    println!("cargo:rustc-link-lib=framework=IOKit");
    println!("cargo:rustc-link-lib=framework=IOSurface");

    // Check macOS version for ScreenCaptureKit support (macOS 12.3+)
    if let Ok(version) = macos_version() {
        if version >= (12, 3) {
            println!("cargo:rustc-cfg=has_screencapturekit");
            println!("cargo:rustc-link-lib=framework=ScreenCaptureKit");
        }
    }

    // Metal support for GPU acceleration
    if env::var("CARGO_FEATURE_GPU").is_ok() {
        println!("cargo:rustc-link-lib=framework=Metal");
        println!("cargo:rustc-link-lib=framework=MetalPerformanceShaders");
    }
}

#[cfg(not(target_os = "macos"))]
fn configure_macos() {}

#[cfg(target_os = "linux")]
fn configure_linux() {
    println!("cargo:rustc-cfg=platform_linux");

    // Use pkg-config to find X11 libraries
    match pkg_config::probe_library("x11") {
        Ok(_) => {
            println!("cargo:rustc-cfg=has_x11");
        }
        Err(e) => {
            println!("cargo:warning=X11 not found: {}", e);
        }
    }

    match pkg_config::probe_library("xrandr") {
        Ok(_) => {
            println!("cargo:rustc-cfg=has_xrandr");
        }
        Err(_) => {}
    }

    match pkg_config::probe_library("xfixes") {
        Ok(_) => {
            println!("cargo:rustc-cfg=has_xfixes");
        }
        Err(_) => {}
    }

    // Check for Wayland support
    if env::var("CARGO_FEATURE_WAYLAND").is_ok() {
        match pkg_config::probe_library("wayland-client") {
            Ok(_) => {
                println!("cargo:rustc-cfg=has_wayland");
            }
            Err(e) => {
                println!("cargo:warning=Wayland not found: {}", e);
            }
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn configure_linux() {}

fn detect_features() {
    // Detect CPU features at build time
    #[cfg(target_arch = "x86_64")]
    {
        use std::arch::x86_64::__cpuid;

        unsafe {
            // Check for AVX2
            let cpuid = __cpuid(7);
            if cpuid.ebx & (1 << 5) != 0 {
                println!("cargo:rustc-cfg=cpu_avx2");
            }

            // Check for SSE4.1
            let cpuid = __cpuid(1);
            if cpuid.ecx & (1 << 19) != 0 {
                println!("cargo:rustc-cfg=cpu_sse41");
            }
        }
    }
}

fn is_x86_feature_detected(feature: &str) -> bool {
    // This is a simplified version - actual implementation would use cpuid
    match feature {
        "avx2" => cfg!(target_feature = "avx2"),
        "sse4.1" => cfg!(target_feature = "sse4.1"),
        _ => false,
    }
}

#[cfg(target_os = "windows")]
fn is_windows_version_supported(sdk_version: &str) -> bool {
    // Windows Graphics Capture API requires Windows 10 1903 (10.0.18362) or later
    if let Some(version) = sdk_version.split('.').nth(2) {
        if let Ok(build) = version.parse::<u32>() {
            return build >= 18362;
        }
    }
    false
}

#[cfg(target_os = "macos")]
fn macos_version() -> Result<(u32, u32), Box<dyn std::error::Error>> {
    use std::process::Command;

    let output = Command::new("sw_vers").arg("-productVersion").output()?;

    let version_str = String::from_utf8(output.stdout)?;
    let parts: Vec<&str> = version_str.trim().split('.').collect();

    if parts.len() >= 2 {
        let major = parts[0].parse::<u32>()?;
        let minor = parts[1].parse::<u32>()?;
        Ok((major, minor))
    } else {
        Err("Invalid macOS version format".into())
    }
}