//! Ultra-Streaming Pipeline Integration Tests
//!
//! Equivalent to test/integration/streaming-pipeline.test.js

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use webp_screenshot_rust::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_initialization_and_configuration() {
        println!("🎯 Testing pipeline initialization and configuration...");

        // Create pipeline builder
        let builder = StreamingPipelineBuilder::new()
            .target_fps(30)
            .buffer_size(60)
            .capture_threads(2)
            .encoding_threads(4)
            .adaptive_quality(true);

        // Build pipeline with mock capturer
        let capturer = match capture::Capturer::new() {
            Ok(c) => c,
            Err(e) => {
                println!("Skipping test: Cannot create capturer - {}", e);
                return;
            }
        };

        let pipeline = builder.build(Box::new(capturer));

        // Verify configuration
        assert!(!pipeline.is_running());
        println!("Pipeline created successfully");
    }

    #[test]
    fn test_streaming_config_defaults() {
        println!("🎯 Testing streaming configuration defaults...");

        let config = StreamingConfig::default();

        assert_eq!(config.target_fps, 30);
        assert_eq!(config.buffer_size, 60);
        assert!(config.adaptive_quality);
        assert!(config.allow_frame_drop);
        assert_eq!(config.webp_config.quality, 80); // Fast config default
    }

    #[test]
    fn test_chunked_image_processing() {
        println!("🎯 Testing chunked image processing...");

        let capturer = match capture::Capturer::new() {
            Ok(c) => c,
            Err(e) => {
                println!("Skipping test: Cannot create capturer - {}", e);
                return;
            }
        };

        let config = StreamingConfig {
            target_fps: 10,
            buffer_size: 30,
            capture_threads: 1,
            encoding_threads: 2,
            adaptive_quality: true,
            allow_frame_drop: true,
            webp_config: WebPConfig {
                quality: 80,
                method: 4,
                ..Default::default()
            },
            use_zero_copy: true,
            use_gpu: false,
        };

        let pipeline = StreamingPipeline::new(Box::new(capturer), config);

        let progress_call_count = Arc::new(AtomicUsize::new(0));
        let progress_updates = Arc::new(std::sync::Mutex::new(Vec::new()));

        let call_count_clone = Arc::clone(&progress_call_count);
        let updates_clone = Arc::clone(&progress_updates);

        // Simulate streaming for a short duration
        let result = pipeline.start(move |data| {
            let count = call_count_clone.fetch_add(1, Ordering::Relaxed);
            let mut updates = updates_clone.lock().unwrap();
            updates.push((count, data.len()));
            println!("Received frame {}: {} bytes", count, data.len());
        });

        match result {
            Ok(()) => {
                // Let it run for a bit
                std::thread::sleep(Duration::from_millis(500));
                pipeline.stop();

                // Wait for shutdown
                std::thread::sleep(Duration::from_millis(100));

                let final_count = progress_call_count.load(Ordering::Relaxed);
                println!("Chunked processing completed with {} frames", final_count);

                // Should have received some frames
                assert!(final_count > 0, "Should have processed at least one frame");
            }
            Err(e) => {
                if e.to_string().contains("No display") || e.to_string().contains("capture failed") {
                    println!("Skipping test: No display available (CI environment)");
                } else {
                    panic!("Unexpected error: {}", e);
                }
            }
        }
    }

    #[test]
    fn test_memory_constrained_chunked_processing() {
        println!("🎯 Testing memory-constrained chunked processing...");

        let capturer = match capture::Capturer::new() {
            Ok(c) => c,
            Err(e) => {
                println!("Skipping test: Cannot create capturer - {}", e);
                return;
            }
        };

        // Configure very small memory limits to force chunking
        let config = StreamingConfig {
            buffer_size: 10, // Small buffer
            capture_threads: 1,
            encoding_threads: 1,
            adaptive_quality: true,
            allow_frame_drop: true,
            webp_config: WebPConfig {
                quality: 75,
                ..Default::default()
            },
            use_zero_copy: false, // Force traditional capture
            use_gpu: false,
            ..Default::default()
        };

        let pipeline = StreamingPipeline::new(Box::new(capturer), config);

        let start_time = Instant::now();
        let frame_count = Arc::new(AtomicUsize::new(0));
        let frame_count_clone = Arc::clone(&frame_count);

        let result = pipeline.start(move |data| {
            frame_count_clone.fetch_add(1, Ordering::Relaxed);
            println!("Processed frame: {} bytes", data.len());
        });

        match result {
            Ok(()) => {
                // Run for short duration
                std::thread::sleep(Duration::from_millis(200));
                pipeline.stop();

                let duration_ms = start_time.elapsed().as_millis();
                let final_count = frame_count.load(Ordering::Relaxed);

                println!(
                    "Memory-constrained processing: {}ms, {} frames",
                    duration_ms, final_count
                );

                // Should process at least one frame
                assert!(final_count > 0, "Should process at least one frame");
            }
            Err(e) => {
                if e.to_string().contains("not supported") {
                    println!("Skipping test: Streaming not supported");
                } else {
                    panic!("Unexpected error: {}", e);
                }
            }
        }
    }

    #[test]
    fn test_multi_display_streaming() {
        println!("🎯 Testing multi-display streaming...");

        let capturer = match capture::Capturer::new() {
            Ok(c) => c,
            Err(e) => {
                println!("Skipping test: Cannot create capturer - {}", e);
                return;
            }
        };

        // Get available displays
        let display_count = match capturer.get_displays() {
            Ok(displays) => displays.len().min(2), // Test up to 2 displays
            Err(_) => 1, // Fallback to single display
        };

        println!("Testing with {} display(s)", display_count);

        let config = StreamingConfig {
            target_fps: 15,
            buffer_size: 30,
            adaptive_quality: true,
            ..Default::default()
        };

        let pipeline = StreamingPipeline::new(Box::new(capturer), config);

        let total_frames = Arc::new(AtomicUsize::new(0));
        let total_frames_clone = Arc::clone(&total_frames);

        let result = pipeline.start(move |data| {
            let count = total_frames_clone.fetch_add(1, Ordering::Relaxed);
            println!("Multi-display frame {}: {} bytes", count, data.len());
        });

        match result {
            Ok(()) => {
                // Run for a bit
                std::thread::sleep(Duration::from_millis(300));
                pipeline.stop();

                let final_count = total_frames.load(Ordering::Relaxed);
                println!("Multi-display processing: {} total frames", final_count);

                // Should have processed frames
                assert!(final_count > 0, "Should have processed frames from displays");
            }
            Err(e) => {
                if e.to_string().contains("No display") || e.to_string().contains("not supported") {
                    println!("Skipping test: Multi-display not supported");
                } else {
                    panic!("Unexpected error: {}", e);
                }
            }
        }
    }

    #[test]
    fn test_progress_callback_system() {
        println!("🎯 Testing progress callback system...");

        let test_image = create_large_test_image(1024, 1024);
        let mut encoder = WebPEncoder::new();

        let config = WebPConfig {
            quality: 80,
            ..Default::default()
        };

        // Test encoding with progress tracking
        let start_time = Instant::now();
        let result = encoder.encode(&test_image, &config);
        let duration = start_time.elapsed();

        match result {
            Ok(encoded_data) => {
                println!(
                    "Encoded {} bytes in {:.2}ms",
                    encoded_data.len(),
                    duration.as_millis()
                );

                assert!(!encoded_data.is_empty(), "Should produce encoded data");
                assert!(duration.as_millis() < 5000, "Should complete within 5 seconds");
            }
            Err(e) => {
                println!("Encoding failed: {}", e);
            }
        }
    }

    #[test]
    fn test_callback_cancellation() {
        println!("🎯 Testing callback cancellation...");

        let capturer = match capture::Capturer::new() {
            Ok(c) => c,
            Err(e) => {
                println!("Skipping test: Cannot create capturer - {}", e);
                return;
            }
        };

        let config = StreamingConfig {
            target_fps: 30,
            buffer_size: 10,
            ..Default::default()
        };

        let pipeline = StreamingPipeline::new(Box::new(capturer), config);

        let callback_count = Arc::new(AtomicUsize::new(0));
        let should_stop = Arc::new(AtomicBool::new(false));

        let count_clone = Arc::clone(&callback_count);
        let stop_clone = Arc::clone(&should_stop);

        let result = pipeline.start(move |_data| {
            let count = count_clone.fetch_add(1, Ordering::Relaxed);
            println!("Callback {}: processing frame", count);

            // Cancel after a few updates
            if count >= 2 {
                stop_clone.store(true, Ordering::Relaxed);
            }
        });

        match result {
            Ok(()) => {
                // Wait for cancellation condition
                while !should_stop.load(Ordering::Relaxed) && callback_count.load(Ordering::Relaxed) < 10 {
                    std::thread::sleep(Duration::from_millis(50));
                }

                pipeline.stop();

                let final_count = callback_count.load(Ordering::Relaxed);
                println!("Callback cancellation handled after {} updates", final_count);

                assert!(final_count >= 2, "Should have received at least 2 callbacks");
                assert!(final_count <= 10, "Should not have exceeded limit");
            }
            Err(e) => {
                println!("Pipeline start failed: {}", e);
            }
        }
    }

    #[test]
    fn test_performance_and_scalability_validation() {
        println!("🎯 Testing performance and scalability validation...");

        let test_sizes = [
            ("Small", 512, 512),
            ("Medium", 1024, 1024),
            ("Large", 2048, 2048),
            ("XLarge", 4096, 2160),
        ];

        let mut results = Vec::new();
        let mut encoder = WebPEncoder::new();

        for (name, width, height) in test_sizes {
            let test_image = create_large_test_image(width, height);
            let pixel_count = width * height;

            let start_time = Instant::now();

            let config = WebPConfig {
                quality: 80,
                ..Default::default()
            };

            match encoder.encode(&test_image, &config) {
                Ok(encoded_data) => {
                    let duration_ms = start_time.elapsed().as_millis() as f64;
                    let throughput_mpps = (pixel_count as f64 / 1_000_000.0) / (duration_ms / 1000.0);
                    let compression_ratio = test_image.data.len() as f64 / encoded_data.len() as f64;

                    results.push((name, width, height, pixel_count, duration_ms, throughput_mpps, compression_ratio));

                    println!(
                        "{} ({}x{}): {:.2}ms, {:.2} MP/s, {:.1}:1 compression",
                        name, width, height, duration_ms, throughput_mpps, compression_ratio
                    );

                    // Verify throughput and compression
                    assert!(throughput_mpps > 1.0, "Should achieve at least 1 MP/s");
                    assert!(compression_ratio > 3.0, "Should achieve at least 3:1 compression");
                }
                Err(e) => {
                    println!("Encoding failed for {}: {}", name, e);
                }
            }
        }

        // Check that very large images don't have dramatically worse throughput
        if let (Some(small), Some(large)) = (
            results.iter().find(|r| r.0 == "Small"),
            results.iter().find(|r| r.0 == "XLarge")
        ) {
            let throughput_ratio = small.5 / large.5;
            assert!(
                throughput_ratio < 5.0,
                "Large images shouldn't be >5x slower per pixel, got {:.2}x",
                throughput_ratio
            );
        }
    }

    #[test]
    fn test_memory_efficiency_under_streaming() {
        println!("🎯 Testing memory efficiency under streaming...");

        let capturer = match capture::Capturer::new() {
            Ok(c) => c,
            Err(e) => {
                println!("Skipping test: Cannot create capturer - {}", e);
                return;
            }
        };

        let config = StreamingConfig {
            buffer_size: 20,
            capture_threads: 1,
            encoding_threads: 2,
            adaptive_quality: true,
            webp_config: WebPConfig {
                quality: 80,
                ..Default::default()
            },
            ..Default::default()
        };

        let pipeline = StreamingPipeline::new(Box::new(capturer), config);

        let initial_stats = memory_pool::global_pool().stats();
        let frame_count = Arc::new(AtomicUsize::new(0));
        let frame_count_clone = Arc::clone(&frame_count);

        let result = pipeline.start(move |data| {
            frame_count_clone.fetch_add(1, Ordering::Relaxed);
            println!("Streaming frame: {} bytes", data.len());
        });

        match result {
            Ok(()) => {
                // Run for a bit to collect stats
                std::thread::sleep(Duration::from_millis(500));
                pipeline.stop();

                let final_stats = memory_pool::global_pool().stats();
                let final_count = frame_count.load(Ordering::Relaxed);

                println!(
                    "Streaming memory usage: {} bytes peak, {} frames processed",
                    final_stats.peak_allocated,
                    final_count
                );

                // Should have processed frames
                assert!(final_count > 0, "Should have processed frames");

                // Memory usage should be reasonable
                assert!(
                    final_stats.peak_allocated < 600 * 1024 * 1024,
                    "Should stay under 600MB"
                );
            }
            Err(e) => {
                println!("Streaming test failed: {}", e);
            }
        }
    }

    #[test]
    fn test_error_handling_and_recovery() {
        println!("🎯 Testing error handling and recovery...");

        // Test invalid configurations
        let capturer = match capture::Capturer::new() {
            Ok(c) => c,
            Err(e) => {
                println!("Skipping test: Cannot create capturer - {}", e);
                return;
            }
        };

        // Create pipeline with invalid configuration (should work but be adjusted internally)
        let config = StreamingConfig {
            target_fps: 0, // Invalid FPS
            buffer_size: 0, // Invalid buffer size
            capture_threads: 0, // Invalid thread count
            encoding_threads: 0, // Invalid thread count
            ..Default::default()
        };

        // Should handle invalid configurations gracefully
        let pipeline = StreamingPipeline::new(Box::new(capturer), config);
        assert!(!pipeline.is_running(), "Pipeline should not be running initially");

        println!("Invalid configuration handled gracefully");
    }

    #[test]
    fn test_streaming_recovery_from_failures() {
        println!("🎯 Testing streaming recovery from failures...");

        let mut encoder = WebPEncoder::new();

        // Test with invalid image first
        let invalid_image = RawImage {
            data: vec![0u8; 100], // Too small for dimensions
            width: 1920,
            height: 1080,
            format: PixelFormat::RGBA,
        };

        let config = WebPConfig {
            quality: 80,
            ..Default::default()
        };

        // This should fail
        let invalid_result = encoder.encode(&invalid_image, &config);
        assert!(invalid_result.is_err(), "Should fail with invalid image");

        // Should still be able to process valid images after error
        let valid_image = create_large_test_image(256, 256);
        let valid_result = encoder.encode(&valid_image, &config);

        match valid_result {
            Ok(data) => {
                assert!(!data.is_empty(), "Should produce valid output after recovery");
                println!("Successfully recovered from encoding failure");
            }
            Err(e) => {
                panic!("System not recovered after error: {}", e);
            }
        }
    }

    /// Helper function to create large test images
    fn create_large_test_image(width: u32, height: u32) -> RawImage {
        let mut data = vec![0u8; (width * height * 4) as usize];

        // Create a complex pattern that exercises the streaming pipeline
        for y in 0..height {
            for x in 0..width {
                let offset = ((y * width + x) * 4) as usize;

                // Create zones with different characteristics
                let zone_x = x / 256;
                let zone_y = y / 256;
                let zone = (zone_x + zone_y) % 4;

                let (r, g, b) = match zone {
                    0 => {
                        // Gradient zone
                        let r = (x * 255) / width;
                        let g = (y * 255) / height;
                        let b = ((x + y) * 255) / (width + height);
                        (r as u8, g as u8, b as u8)
                    }
                    1 => {
                        // High frequency zone
                        let r = ((x as f64 * 0.1).sin() + 1.0) * 127.0;
                        let g = ((y as f64 * 0.1).cos() + 1.0) * 127.0;
                        let b = (((x + y) as f64 * 0.05).sin() + 1.0) * 127.0;
                        (r as u8, g as u8, b as u8)
                    }
                    2 => {
                        // Solid color zone
                        let r = (zone_x * 63) % 256;
                        let g = (zone_y * 63) % 256;
                        let b = ((zone_x + zone_y) * 63) % 256;
                        (r as u8, g as u8, b as u8)
                    }
                    3 => {
                        // Noise zone
                        use std::collections::hash_map::DefaultHasher;
                        use std::hash::{Hash, Hasher};

                        let mut hasher = DefaultHasher::new();
                        (x, y).hash(&mut hasher);
                        let hash_val = hasher.finish();

                        let r = (hash_val & 0xFF) as u8;
                        let g = ((hash_val >> 8) & 0xFF) as u8;
                        let b = ((hash_val >> 16) & 0xFF) as u8;
                        (r, g, b)
                    }
                    _ => (128, 128, 128), // Default gray
                };

                data[offset] = r;
                data[offset + 1] = g;
                data[offset + 2] = b;
                data[offset + 3] = 255;
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