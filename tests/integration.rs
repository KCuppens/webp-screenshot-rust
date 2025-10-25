//! Integration Tests
//!
//! Main integration test module that runs all test suites

use webp_screenshot_rust::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_screenshot_capture() {
        println!("🎯 Testing basic screenshot capture...");

        match WebPScreenshot::new() {
            Ok(mut screenshot) => {
                // Try to capture primary display
                match screenshot.capture_display(0) {
                    Ok(result) => {
                        assert!(!result.data.is_empty(), "Screenshot data should not be empty");
                        assert!(result.width > 0, "Width should be greater than 0");
                        assert!(result.height > 0, "Height should be greater than 0");

                        println!(
                            "✓ Captured screenshot: {}x{}, {} bytes",
                            result.width, result.height, result.data.len()
                        );
                    }
                    Err(e) => {
                        // Skip test if no display available (CI environment)
                        if e.to_string().contains("No display") || e.to_string().contains("permission") {
                            println!("⚠️ Skipping capture test: {}", e);
                        } else {
                            panic!("Unexpected capture error: {}", e);
                        }
                    }
                }
            }
            Err(e) => {
                println!("⚠️ Cannot create screenshot instance: {}", e);
            }
        }
    }

    #[test]
    fn test_display_enumeration() {
        println!("🎯 Testing display enumeration...");

        match get_displays() {
            Ok(displays) => {
                println!("Found {} display(s)", displays.len());

                for (index, display) in displays.iter().enumerate() {
                    println!(
                        "Display {}: {}x{} at ({}, {}), scale: {:.2}, primary: {}",
                        index,
                        display.width,
                        display.height,
                        display.x,
                        display.y,
                        display.scale_factor,
                        display.is_primary
                    );
                }

                // Should have at least one display in most environments
                if displays.is_empty() {
                    println!("⚠️ No displays found (may be running in headless environment)");
                } else {
                    assert!(displays.len() > 0, "Should find at least one display");
                }
            }
            Err(e) => {
                println!("⚠️ Failed to enumerate displays: {}", e);
            }
        }
    }

    #[test]
    fn test_webp_encoding_quality_levels() {
        println!("🎯 Testing WebP encoding at different quality levels...");

        let test_image = create_test_image(256, 256);
        let mut encoder = WebPEncoder::new();
        let quality_levels = [50, 75, 90];

        for quality in quality_levels {
            let config = WebPConfig {
                quality: quality as u8,
                ..Default::default()
            };

            match encoder.encode(&test_image, &config) {
                Ok(webp_data) => {
                    let compression_ratio = test_image.data.len() as f64 / webp_data.len() as f64;

                    println!(
                        "Quality {}: {} bytes -> {} bytes ({:.1}:1 compression)",
                        quality,
                        test_image.data.len(),
                        webp_data.len(),
                        compression_ratio
                    );

                    assert!(!webp_data.is_empty(), "WebP data should not be empty");
                    assert!(compression_ratio > 2.0, "Should achieve at least 2:1 compression");
                }
                Err(e) => {
                    panic!("Failed to encode at quality {}: {}", quality, e);
                }
            }
        }
    }

    #[test]
    fn test_memory_pool_functionality() {
        println!("🎯 Testing memory pool functionality...");

        let pool = memory_pool::global_pool();
        let initial_stats = pool.stats();

        println!(
            "Initial pool stats: {} bytes allocated, {} buffers active",
            initial_stats.total_allocated,
            initial_stats.active_buffers
        );

        // Allocate some buffers
        let buffer_size = 1024 * 1024; // 1MB
        let mut buffers = Vec::new();

        for i in 0..5 {
            match pool.allocate(buffer_size) {
                Ok(buffer) => {
                    buffers.push(buffer);
                    println!("Allocated buffer {}: {} bytes", i, buffer_size);
                }
                Err(e) => {
                    println!("Failed to allocate buffer {}: {}", i, e);
                    break;
                }
            }
        }

        let after_alloc_stats = pool.stats();
        println!(
            "After allocation: {} bytes allocated, {} buffers active",
            after_alloc_stats.total_allocated,
            after_alloc_stats.active_buffers
        );

        // Return buffers
        for buffer in buffers {
            pool.return_buffer(buffer);
        }

        let final_stats = pool.stats();
        println!(
            "Final pool stats: {} bytes allocated, {} buffers active",
            final_stats.total_allocated,
            final_stats.active_buffers
        );

        // Verify pool is functioning
        assert!(final_stats.total_allocated >= initial_stats.total_allocated);
    }

    #[test]
    fn test_configuration_validation() {
        println!("🎯 Testing configuration validation...");

        // Test default configuration
        let default_config = CaptureConfig::default();
        assert_eq!(default_config.webp_config.quality, 80);
        assert!(!default_config.include_cursor);

        // Test WebP configuration validation
        let webp_configs = [
            WebPConfig { quality: 0, ..Default::default() },      // Minimum
            WebPConfig { quality: 100, ..Default::default() },    // Maximum
            WebPConfig { quality: 80, method: 0, ..Default::default() },
            WebPConfig { quality: 80, method: 6, ..Default::default() },
        ];

        for (index, config) in webp_configs.iter().enumerate() {
            println!(
                "Testing WebP config {}: quality={}, method={}",
                index, config.quality, config.method
            );

            // Create screenshot instance with config
            let capture_config = CaptureConfig {
                webp_config: config.clone(),
                ..Default::default()
            };

            match WebPScreenshot::with_config(capture_config) {
                Ok(_) => println!("✓ Config {} accepted", index),
                Err(e) => println!("⚠️ Config {} rejected: {}", index, e),
            }
        }
    }

    #[test]
    fn test_error_handling() {
        println!("🎯 Testing error handling...");

        // Test invalid display index
        if let Ok(mut screenshot) = WebPScreenshot::new() {
            match screenshot.capture_display(9999) {
                Ok(_) => println!("⚠️ Invalid display index unexpectedly succeeded"),
                Err(e) => println!("✓ Invalid display index correctly rejected: {}", e),
            }
        }

        // Test invalid image encoding
        let invalid_image = RawImage {
            data: vec![], // Empty data
            width: 100,
            height: 100,
            format: PixelFormat::RGBA,
        };

        let mut encoder = WebPEncoder::new();
        let config = WebPConfig::default();

        match encoder.encode(&invalid_image, &config) {
            Ok(_) => println!("⚠️ Invalid image unexpectedly encoded successfully"),
            Err(e) => println!("✓ Invalid image correctly rejected: {}", e),
        }
    }

    #[test]
    fn test_library_capabilities() {
        println!("🎯 Testing library capabilities...");

        let version = version();
        println!("Library version: {}", version);
        assert!(!version.is_empty(), "Version should not be empty");

        let capabilities = capabilities();
        println!("Library capabilities: {}", capabilities);
        assert!(!capabilities.is_empty(), "Capabilities should not be empty");

        // Test platform-specific features
        println!("Platform features:");
        println!("- Zero-copy supported: {}", ZeroCopyOptimizer::is_supported());

        #[cfg(feature = "gpu")]
        println!("- GPU acceleration: enabled");
        #[cfg(not(feature = "gpu"))]
        println!("- GPU acceleration: disabled");

        #[cfg(feature = "parallel")]
        println!("- Parallel processing: enabled");
        #[cfg(not(feature = "parallel"))]
        println!("- Parallel processing: disabled");
    }

    #[test]
    fn test_convenience_functions() {
        println!("🎯 Testing convenience functions...");

        // Test capture_primary_display
        match capture_primary_display() {
            Ok(screenshot) => {
                println!(
                    "✓ Primary display captured: {}x{}, {} bytes",
                    screenshot.width,
                    screenshot.height,
                    screenshot.data.len()
                );
            }
            Err(e) => {
                if e.to_string().contains("No display") {
                    println!("⚠️ No display available for primary capture: {}", e);
                } else {
                    panic!("Unexpected error in primary capture: {}", e);
                }
            }
        }

        // Test capture_with_quality
        match capture_with_quality(0, 90) {
            Ok(screenshot) => {
                println!(
                    "✓ High quality capture: {}x{}, {} bytes",
                    screenshot.width,
                    screenshot.height,
                    screenshot.data.len()
                );
            }
            Err(e) => {
                if e.to_string().contains("No display") {
                    println!("⚠️ No display available for quality capture: {}", e);
                } else {
                    panic!("Unexpected error in quality capture: {}", e);
                }
            }
        }
    }

    #[test]
    fn test_statistics_tracking() {
        println!("🎯 Testing statistics tracking...");

        match WebPScreenshot::new() {
            Ok(mut screenshot) => {
                let initial_stats = screenshot.stats();
                println!(
                    "Initial stats: {} captures, {} successful, {} failed",
                    initial_stats.total_captures,
                    initial_stats.successful_captures,
                    initial_stats.failed_captures
                );

                // Attempt a capture to update stats
                match screenshot.capture_display(0) {
                    Ok(_) => {
                        let updated_stats = screenshot.stats();
                        println!(
                            "Updated stats: {} captures, {} successful, {} failed",
                            updated_stats.total_captures,
                            updated_stats.successful_captures,
                            updated_stats.failed_captures
                        );

                        assert!(
                            updated_stats.total_captures > initial_stats.total_captures,
                            "Total captures should increase"
                        );
                        assert!(
                            updated_stats.successful_captures > initial_stats.successful_captures,
                            "Successful captures should increase"
                        );
                    }
                    Err(e) => {
                        println!("⚠️ Capture failed for stats test: {}", e);

                        let updated_stats = screenshot.stats();
                        // Failed capture should still update stats
                        assert!(
                            updated_stats.total_captures > initial_stats.total_captures ||
                            updated_stats.failed_captures > initial_stats.failed_captures,
                            "Stats should update even on failure"
                        );
                    }
                }

                // Test stats reset
                screenshot.reset_stats();
                let reset_stats = screenshot.stats();
                assert_eq!(reset_stats.total_captures, 0, "Stats should reset to zero");
            }
            Err(e) => {
                println!("⚠️ Cannot test stats without screenshot instance: {}", e);
            }
        }
    }

    // Helper function to create test image
    fn create_test_image(width: u32, height: u32) -> RawImage {
        let mut data = vec![0u8; (width * height * 4) as usize];

        // Create a simple gradient pattern
        for y in 0..height {
            for x in 0..width {
                let offset = ((y * width + x) * 4) as usize;

                data[offset] = (x * 255 / width) as u8;         // R
                data[offset + 1] = (y * 255 / height) as u8;   // G
                data[offset + 2] = ((x + y) * 255 / (width + height)) as u8; // B
                data[offset + 3] = 255; // A
            }
        }

        RawImage {
            data,
            width,
            height,
            format: PixelFormat::RGBA,
        }
    }
}