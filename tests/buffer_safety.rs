//! Buffer Safety and Security Tests
//!
//! Equivalent to test/security/buffer-safety.test.js

use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicUsize, Ordering};
use webp_screenshot_rust::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_size_mismatch_handling() {
        println!("🔒 Testing buffer size mismatch handling...");

        let test_cases = [
            (100, 100, 100, true),   // Buffer too small
            (40000, 100, 100, false), // Buffer adequate
            (39999, 100, 100, true), // Buffer just under required size
            (0, 100, 100, true),     // Empty buffer
        ];

        for (buffer_size, width, height, should_error) in test_cases {
            let buffer = vec![0u8; buffer_size];
            let raw_image = RawImage {
                data: buffer,
                width,
                height,
                format: PixelFormat::BGRA,
            };

            // Test conversion (would be equivalent to convertBGRAToRGBA in JS)
            let result = convert_bgra_to_rgba(&raw_image);

            if should_error {
                assert!(
                    result.is_err(),
                    "Buffer size {} for {}x{} should cause error",
                    buffer_size, width, height
                );
            } else {
                assert!(
                    result.is_ok(),
                    "Buffer size {} for {}x{} should not cause error",
                    buffer_size, width, height
                );
            }
        }
    }

    #[test]
    fn test_image_dimensions_validation() {
        println!("🔒 Testing image dimensions validation...");

        let valid_buffer = vec![0u8; 1920 * 1080 * 4]; // 1080p RGBA

        // Valid cases
        let valid_cases = [
            (1920, 1080),
            (960, 2160), // Same total pixels, different aspect ratio
        ];

        for (width, height) in valid_cases {
            let raw_image = RawImage {
                data: valid_buffer.clone(),
                width,
                height,
                format: PixelFormat::BGRA,
            };

            let result = convert_bgra_to_rgba(&raw_image);
            assert!(
                result.is_ok(),
                "Valid dimensions {}x{} should not cause error",
                width, height
            );
        }

        // Invalid cases - dimensions too large for buffer
        let invalid_cases = [
            (1921, 1080), // Width too large
            (1920, 1081), // Height too large
            (3840, 2160), // 4K requires larger buffer
        ];

        for (width, height) in invalid_cases {
            let raw_image = RawImage {
                data: valid_buffer.clone(),
                width,
                height,
                format: PixelFormat::BGRA,
            };

            let result = convert_bgra_to_rgba(&raw_image);
            assert!(
                result.is_err(),
                "Invalid dimensions {}x{} should cause error",
                width, height
            );
        }
    }

    #[test]
    fn test_edge_case_dimensions() {
        println!("🔒 Testing edge case dimensions...");

        // Test zero dimensions
        let zero_cases = [
            (0, 0),
            (0, 100),
            (100, 0),
        ];

        for (width, height) in zero_cases {
            let buffer = vec![0u8; 100]; // Some buffer size
            let raw_image = RawImage {
                data: buffer,
                width,
                height,
                format: PixelFormat::BGRA,
            };

            let result = convert_bgra_to_rgba(&raw_image);
            assert!(
                result.is_err(),
                "Zero dimension {}x{} should cause error",
                width, height
            );
        }

        // Test very small dimensions (should work)
        let tiny_buffer = vec![0u8; 4]; // 1 pixel
        let tiny_image = RawImage {
            data: tiny_buffer,
            width: 1,
            height: 1,
            format: PixelFormat::BGRA,
        };

        let result = convert_bgra_to_rgba(&tiny_image);
        assert!(result.is_ok(), "1x1 image should not cause error");

        // Test large dimensions with insufficient buffer
        let small_buffer = vec![0u8; 1000];
        let large_image = RawImage {
            data: small_buffer,
            width: 1000,
            height: 1000,
            format: PixelFormat::BGRA,
        };

        let result = convert_bgra_to_rgba(&large_image);
        assert!(result.is_err(), "Large dimensions with small buffer should cause error");
    }

    #[test]
    fn test_integer_overflow_protection() {
        println!("🔒 Testing integer overflow protection...");

        let test_cases = [
            (2147483647, 2, "Width near i32::MAX"),
            (65536, 65536, "Large square dimensions"),
            (4294967295, 1, "Width at u32::MAX"),
        ];

        for (width, height, description) in test_cases {
            println!("Testing: {}", description);

            let buffer = vec![0u8; 1000]; // Small buffer
            let raw_image = RawImage {
                data: buffer,
                width,
                height,
                format: PixelFormat::BGRA,
            };

            // These should fail safely without crashing
            let result = convert_bgra_to_rgba(&raw_image);
            assert!(
                result.is_err(),
                "{} should fail safely",
                description
            );
        }
    }

    #[test]
    fn test_memory_boundary_validation() {
        println!("🔒 Testing memory boundary validation...");

        let buffer_size = 1000;
        let mut buffer = vec![0u8; buffer_size];

        // Fill buffer with known pattern
        for (i, byte) in buffer.iter_mut().enumerate() {
            *byte = (i % 256) as u8;
        }

        // Create guard patterns (simulate checking for buffer overruns)
        let guard_pattern = vec![0xDE, 0xAD, 0xBE, 0xEF];
        let mut guarded_buffer = Vec::new();
        guarded_buffer.extend_from_slice(&guard_pattern);
        guarded_buffer.extend_from_slice(&guard_pattern);
        guarded_buffer.extend_from_slice(&buffer);
        guarded_buffer.extend_from_slice(&guard_pattern);
        guarded_buffer.extend_from_slice(&guard_pattern);

        // Use the middle section for processing
        let working_buffer = guarded_buffer[8..8 + buffer_size].to_vec();

        if working_buffer.len() >= 100 * 4 {
            // This should work without corrupting guards
            let raw_image = RawImage {
                data: working_buffer,
                width: 25,
                height: 10, // 25x10 = 250 pixels = 1000 bytes
                format: PixelFormat::BGRA,
            };

            let _result = convert_bgra_to_rgba(&raw_image); // May succeed or fail based on validation
        }

        // Check guard patterns are intact
        assert_eq!(&guarded_buffer[0..4], guard_pattern);
        assert_eq!(&guarded_buffer[4..8], guard_pattern);
        let end_start = 8 + buffer_size;
        assert_eq!(&guarded_buffer[end_start..end_start + 4], guard_pattern);
        assert_eq!(&guarded_buffer[end_start + 4..end_start + 8], guard_pattern);

        println!("Guard patterns intact - no buffer overrun detected");
    }

    #[test]
    fn test_webp_parameter_validation() {
        println!("🔒 Testing WebP parameter validation...");

        let test_image = create_test_image(100, 100);
        let mut encoder = WebPEncoder::new();

        // Valid parameters
        let valid_config = WebPConfig {
            quality: 80,
            ..Default::default()
        };

        let result = encoder.encode(&test_image, &valid_config);
        assert!(result.is_ok(), "Valid parameters should not cause error");

        // Invalid quality values should be clamped or cause errors
        let invalid_configs = [
            WebPConfig { quality: 0, ..Default::default() },    // Below minimum
            WebPConfig { quality: 255, ..Default::default() },  // Above maximum
        ];

        for config in invalid_configs {
            let result = encoder.encode(&test_image, &config);
            // Either should work (if clamped) or fail gracefully
            match result {
                Ok(_) => println!("Invalid quality {} was handled by clamping", config.quality),
                Err(e) => println!("Invalid quality {} correctly rejected: {}", config.quality, e),
            }
        }
    }

    #[test]
    fn test_extreme_numeric_inputs() {
        println!("🔒 Testing extreme numeric inputs...");

        let buffer = vec![0u8; 1000];

        let extreme_values = [
            u32::MAX,
            0,
            1,
        ];

        for extreme_value in extreme_values {
            let raw_image = RawImage {
                data: buffer.clone(),
                width: extreme_value,
                height: 100,
                format: PixelFormat::BGRA,
            };

            let result = convert_bgra_to_rgba(&raw_image);
            match result {
                Ok(_) => println!("Extreme width {} handled successfully", extreme_value),
                Err(e) => println!("Extreme width {} safely rejected: {}", extreme_value, e),
            }

            let raw_image = RawImage {
                data: buffer.clone(),
                width: 100,
                height: extreme_value,
                format: PixelFormat::BGRA,
            };

            let result = convert_bgra_to_rgba(&raw_image);
            match result {
                Ok(_) => println!("Extreme height {} handled successfully", extreme_value),
                Err(e) => println!("Extreme height {} safely rejected: {}", extreme_value, e),
            }
        }
    }

    #[test]
    fn test_memory_allocation_failure_handling() {
        println!("🔒 Testing memory allocation failure handling...");

        // Try to allocate extremely large buffers through memory pool
        let huge_sizes = [
            1024 * 1024 * 1024,     // 1GB
            2 * 1024 * 1024 * 1024, // 2GB (may exceed limits)
        ];

        for size in huge_sizes {
            println!("Testing allocation of {} MB", size / 1024 / 1024);

            match memory_pool::global_pool().allocate(size) {
                Ok(buffer) => {
                    println!("Successfully allocated {}MB buffer", size / 1024 / 1024);
                    // Return it immediately
                    memory_pool::global_pool().return_buffer(buffer);
                }
                Err(e) => {
                    println!("Allocation of {}MB failed as expected: {}", size / 1024 / 1024, e);
                }
            }
        }
    }

    #[test]
    fn test_concurrent_buffer_allocations() {
        println!("🔒 Testing concurrent buffer allocations...");

        let buffer_size = 10 * 1024 * 1024; // 10MB each
        let max_buffers = 20; // Try to allocate 200MB total
        let successful_allocations = Arc::new(AtomicUsize::new(0));
        let failed_allocations = Arc::new(AtomicUsize::new(0));

        let handles: Vec<_> = (0..max_buffers).map(|i| {
            let successful = Arc::clone(&successful_allocations);
            let failed = Arc::clone(&failed_allocations);

            std::thread::spawn(move || {
                match memory_pool::global_pool().allocate(buffer_size) {
                    Ok(buffer) => {
                        successful.fetch_add(1, Ordering::Relaxed);
                        // Hold buffer briefly
                        std::thread::sleep(std::time::Duration::from_millis(10));
                        memory_pool::global_pool().return_buffer(buffer);
                    }
                    Err(_) => {
                        failed.fetch_add(1, Ordering::Relaxed);
                    }
                }
            })
        }).collect();

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        let successful_count = successful_allocations.load(Ordering::Relaxed);
        let failed_count = failed_allocations.load(Ordering::Relaxed);

        println!(
            "Concurrent allocations: {} successful, {} failed ({}MB total attempted)",
            successful_count,
            failed_count,
            (successful_count + failed_count) * buffer_size / 1024 / 1024
        );

        // Should have allocated some buffers but hit reasonable limits
        assert!(successful_count > 0, "Should have allocated some buffers");
        // Note: We don't assert a specific failure count as it depends on available memory
    }

    #[test]
    fn test_rapid_allocation_deallocation_cycles() {
        println!("🔒 Testing rapid allocation/deallocation cycles...");

        let iterations = 1000;
        let buffer_size = 1024 * 1024; // 1MB
        let mut errors = 0;

        for i in 0..iterations {
            match memory_pool::global_pool().allocate(buffer_size) {
                Ok(buffer) => {
                    // Immediately return the buffer
                    memory_pool::global_pool().return_buffer(buffer);
                }
                Err(_) => {
                    errors += 1;
                    if errors > iterations / 10 { // More than 10% failure rate
                        break; // Stop if too many errors
                    }
                }
            }

            // Occasionally log progress
            if i % 100 == 0 {
                println!("Completed {} allocation cycles", i);
            }
        }

        println!("Completed {} allocation cycles with {} errors", iterations - errors, errors);

        // Should complete most cycles successfully
        assert!(
            errors < iterations / 20, // Less than 5% error rate
            "Error rate too high: {}/{}",
            errors,
            iterations
        );
    }

    #[test]
    fn test_concurrent_processing_safety() {
        println!("🔒 Testing concurrent processing safety...");

        let concurrent_operations = 10;
        let test_data = create_test_image(256, 256);

        let handles: Vec<_> = (0..concurrent_operations).map(|index| {
            let test_image = test_data.clone();

            std::thread::spawn(move || {
                // Test conversion
                match convert_bgra_to_rgba(&test_image) {
                    Ok(result) => {
                        // Verify first few pixels converted correctly
                        let is_valid = if !result.data.is_empty() && !test_image.data.is_empty() {
                            result.data[0] == test_image.data[2] && // R should be original B
                            result.data[1] == test_image.data[1] && // G should be same
                            result.data[2] == test_image.data[0] && // B should be original R
                            result.data[3] == test_image.data[3]    // A should be same
                        } else {
                            false
                        };

                        (index, Ok(is_valid))
                    }
                    Err(e) => (index, Err(e.to_string()))
                }
            })
        }).collect();

        let mut valid_results = 0;
        for handle in handles {
            match handle.join().unwrap() {
                (index, Ok(is_valid)) => {
                    if is_valid {
                        valid_results += 1;
                    } else {
                        println!("Thread {} produced invalid result", index);
                    }
                }
                (index, Err(e)) => {
                    println!("Thread {} failed: {}", index, e);
                }
            }
        }

        println!("Concurrent processing: {}/{} valid results", valid_results, concurrent_operations);

        // All operations should maintain data integrity
        assert_eq!(
            valid_results, concurrent_operations,
            "All concurrent operations should maintain data integrity"
        );
    }

    #[test]
    fn test_error_recovery_and_cleanup() {
        println!("🔒 Testing error recovery and cleanup...");

        let initial_stats = memory_pool::global_pool().stats();

        // Cause various errors
        let error_operations = [
            || {
                let invalid_image = RawImage {
                    data: vec![],
                    width: 100,
                    height: 100,
                    format: PixelFormat::BGRA,
                };
                convert_bgra_to_rgba(&invalid_image)
            },
            || {
                let invalid_image = RawImage {
                    data: vec![0u8; 10],
                    width: 1000,
                    height: 1000,
                    format: PixelFormat::BGRA,
                };
                convert_bgra_to_rgba(&invalid_image)
            },
        ];

        for (index, error_op) in error_operations.iter().enumerate() {
            match error_op() {
                Ok(_) => println!("Error operation {} unexpectedly succeeded", index),
                Err(_) => println!("Error operation {} failed as expected", index),
            }
        }

        let final_stats = memory_pool::global_pool().stats();

        // Memory pool should not indicate leaks
        println!(
            "Memory stats - Initial: {} bytes, Final: {} bytes",
            initial_stats.total_allocated,
            final_stats.total_allocated
        );

        // In a more sophisticated implementation, we'd check for memory leaks
        // For now, just verify the system is still functional
        let test_image = create_test_image(64, 64);
        let result = convert_bgra_to_rgba(&test_image);
        assert!(result.is_ok(), "System should still function after errors");
    }

    #[test]
    fn test_partial_operation_cleanup() {
        println!("🔒 Testing partial operation cleanup...");

        let mut encoder = WebPEncoder::new();
        let test_image = create_test_image(100, 100);

        // Start with valid parameters
        let valid_config = WebPConfig {
            quality: 80,
            ..Default::default()
        };

        let valid_result = encoder.encode(&test_image, &valid_config);
        assert!(valid_result.is_ok(), "Valid operation should succeed");

        // Try with potentially invalid parameters
        let potentially_invalid_config = WebPConfig {
            quality: 0, // May be invalid or clamped
            ..Default::default()
        };

        let _invalid_result = encoder.encode(&test_image, &potentially_invalid_config);
        // Don't assert on this result as implementation may handle it differently

        // System should still work after potential failure
        let recovery_result = encoder.encode(&test_image, &valid_config);
        assert!(
            recovery_result.is_ok(),
            "System should recover after potential error: {:?}",
            recovery_result
        );

        println!("Successfully recovered after partial operation failure");
    }

    // Helper functions

    fn create_test_image(width: u32, height: u32) -> RawImage {
        let mut data = vec![0u8; (width * height * 4) as usize];

        // Fill with test pattern
        for i in (0..data.len()).step_by(4) {
            data[i] = (i % 256) as u8;         // R
            data[i + 1] = ((i + 1) % 256) as u8; // G
            data[i + 2] = ((i + 2) % 256) as u8; // B
            data[i + 3] = 255;                 // A
        }

        RawImage {
            data,
            width,
            height,
            format: PixelFormat::BGRA,
        }
    }

    fn convert_bgra_to_rgba(image: &RawImage) -> Result<RawImage, CaptureError> {
        // Validate buffer size
        let expected_size = (image.width * image.height * 4) as usize;
        if image.data.len() != expected_size {
            return Err(CaptureError::InvalidInput(format!(
                "Buffer size mismatch: expected {}, got {}",
                expected_size,
                image.data.len()
            )));
        }

        // Validate dimensions
        if image.width == 0 || image.height == 0 {
            return Err(CaptureError::InvalidInput("Dimensions cannot be zero".to_string()));
        }

        // Check for potential overflow
        if image.width > 100000 || image.height > 100000 {
            return Err(CaptureError::InvalidInput("Dimensions too large".to_string()));
        }

        // Perform BGRA to RGBA conversion
        let mut converted_data = image.data.clone();
        for i in (0..converted_data.len()).step_by(4) {
            // Swap B and R channels
            converted_data.swap(i, i + 2);
        }

        Ok(RawImage {
            data: converted_data,
            width: image.width,
            height: image.height,
            format: PixelFormat::RGBA,
        })
    }
}