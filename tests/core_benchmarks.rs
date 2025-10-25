//! Core Performance Benchmarks
//!
//! Equivalent to test/performance/core-benchmarks.test.js

use std::time::Instant;
use webp_screenshot_rust::*;

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_TIMEOUT_MS: u64 = 60000;

    #[derive(Debug, Clone)]
    struct BenchmarkResults {
        capture_performance: std::collections::HashMap<String, CapturePerformanceResult>,
        encode_performance: std::collections::HashMap<String, EncodePerformanceResult>,
        memory_performance: MemoryPerformanceResult,
        scalability_performance: std::collections::HashMap<String, ScalabilityResult>,
    }

    #[derive(Debug, Clone)]
    struct CapturePerformanceResult {
        avg_duration_ms: f64,
        p95_duration_ms: f64,
        avg_data_size_mb: f64,
        iterations: usize,
    }

    #[derive(Debug, Clone)]
    struct EncodePerformanceResult {
        avg_duration_ms: f64,
        avg_compression_ratio: f64,
        throughput_megapixels_per_second: f64,
        iterations: usize,
    }

    #[derive(Debug, Clone)]
    struct MemoryPerformanceResult {
        pool_efficiency: Vec<MemoryEfficiencyResult>,
        memory_reuse_count: u64,
        peak_memory_usage_mb: f64,
        zero_copy_speedup: Option<f64>,
    }

    #[derive(Debug, Clone)]
    struct MemoryEfficiencyResult {
        size: usize,
        alloc_duration: f64,
        return_duration: f64,
        reuse_duration: f64,
        reuse_speedup: f64,
    }

    #[derive(Debug, Clone)]
    struct ScalabilityResult {
        avg_duration_ms: f64,
        threads_used: usize,
    }

    struct TestResolution {
        name: &'static str,
        width: u32,
        height: u32,
    }

    const RESOLUTIONS: &[TestResolution] = &[
        TestResolution { name: "1080p", width: 1920, height: 1080 },
        TestResolution { name: "1440p", width: 2560, height: 1440 },
        TestResolution { name: "4K", width: 3840, height: 2160 },
        TestResolution { name: "8K", width: 7680, height: 4320 },
    ];

    #[test]
    fn test_screenshot_capture_performance() {
        println!("🎯 Testing screenshot capture performance...");

        for resolution in RESOLUTIONS {
            let iterations = match resolution.name {
                "8K" => 3,
                "4K" => 5,
                _ => 10,
            };

            println!("Testing {} ({} iterations)", resolution.name, iterations);

            // Create WebPScreenshot instance
            let mut screenshot = match WebPScreenshot::new() {
                Ok(s) => s,
                Err(e) => {
                    println!("Skipping {}: Cannot create screenshot instance - {}", resolution.name, e);
                    continue;
                }
            };

            let mut capture_results = Vec::new();

            // Warm up
            if let Err(_) = screenshot.capture_display(0) {
                println!("Skipping {}: No display available", resolution.name);
                continue;
            }

            for i in 0..iterations {
                let start_time = Instant::now();

                match screenshot.capture_display(0) {
                    Ok(result) => {
                        let duration_ms = start_time.elapsed().as_millis() as f64;

                        capture_results.push((
                            duration_ms,
                            result.data.len(),
                            result.width,
                            result.height,
                        ));
                    }
                    Err(e) => {
                        println!("Capture iteration {} failed: {}", i, e);
                    }
                }
            }

            if capture_results.is_empty() {
                println!("No successful captures for {}", resolution.name);
                continue;
            }

            let avg_duration = capture_results.iter().map(|r| r.0).sum::<f64>() / capture_results.len() as f64;
            let avg_data_size = capture_results.iter().map(|r| r.1).sum::<usize>() as f64 / capture_results.len() as f64;

            // Calculate 95th percentile
            let mut durations: Vec<f64> = capture_results.iter().map(|r| r.0).collect();
            durations.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let p95_index = ((capture_results.len() as f64) * 0.95) as usize;
            let p95_duration = durations.get(p95_index).copied().unwrap_or(avg_duration);

            println!(
                "{} Capture: {:.2}ms avg, {:.2}ms p95, {:.2}MB",
                resolution.name,
                avg_duration,
                p95_duration,
                avg_data_size / (1024.0 * 1024.0)
            );

            // Performance expectations
            let expected_max_duration = match resolution.name {
                "1080p" => 100.0,
                "1440p" => 150.0,
                "4K" => 300.0,
                "8K" => 800.0,
                _ => 200.0,
            };

            assert!(
                avg_duration < expected_max_duration,
                "{} capture should be under {}ms, got {:.2}ms",
                resolution.name,
                expected_max_duration,
                avg_duration
            );
        }
    }

    #[test]
    fn test_webp_encoding_performance() {
        println!("🎯 Testing WebP encoding performance...");

        let quality_levels = [60, 80, 90, 95];
        let test_sizes = [
            ("Medium", 1024, 768),
            ("Large", 1920, 1080),
            ("Ultra", 3840, 2160),
        ];

        for (name, width, height) in test_sizes {
            for quality in quality_levels {
                println!("Testing {} images at quality {}", name, quality);

                let pixel_count = width * height;
                let test_image = create_test_image(width, height);
                let iterations = (1000000 / pixel_count).max(1);

                let mut encode_results = Vec::new();
                let mut encoder = WebPEncoder::new();

                for _i in 0..iterations {
                    let start_time = Instant::now();

                    let webp_config = WebPConfig {
                        quality: quality as u8,
                        method: 4,
                        ..Default::default()
                    };

                    match encoder.encode(&test_image, &webp_config) {
                        Ok(webp_data) => {
                            let duration_ms = start_time.elapsed().as_millis() as f64;

                            encode_results.push((
                                duration_ms,
                                test_image.data.len(),
                                webp_data.len(),
                                test_image.data.len() as f64 / webp_data.len() as f64,
                            ));
                        }
                        Err(e) => {
                            eprintln!("Encoding failed: {}", e);
                        }
                    }
                }

                if !encode_results.is_empty() {
                    let avg_duration = encode_results.iter().map(|r| r.0).sum::<f64>() / encode_results.len() as f64;
                    let avg_compression_ratio = encode_results.iter().map(|r| r.3).sum::<f64>() / encode_results.len() as f64;
                    let throughput_mpps = (pixel_count as f64 / 1_000_000.0) / (avg_duration / 1000.0);

                    println!(
                        "{} Q{}: {:.2}ms, {:.1}:1 compression, {:.2} MP/s",
                        name, quality, avg_duration, avg_compression_ratio, throughput_mpps
                    );

                    // Performance expectations
                    assert!(throughput_mpps > 5.0, "Should achieve at least 5 MP/s");
                    assert!(avg_compression_ratio > 3.0, "Should achieve at least 3:1 compression");
                }
            }
        }
    }

    #[test]
    fn test_memory_pool_efficiency() {
        println!("🎯 Testing memory pool efficiency...");

        let test_sizes = [1024, 4096, 16384, 65536, 262144];
        let iterations_per_size = 100;

        for size in test_sizes {
            let start_time = Instant::now();
            let mut buffers = Vec::new();

            // Allocation phase
            for _i in 0..iterations_per_size {
                if let Ok(buffer) = memory_pool::global_pool().allocate(size) {
                    buffers.push(buffer);
                }
            }

            let alloc_end_time = Instant::now();

            // Return phase
            for buffer in buffers.drain(..) {
                memory_pool::global_pool().return_buffer(buffer);
            }

            let return_end_time = Instant::now();

            // Reallocation phase (should reuse buffers)
            for _i in 0..iterations_per_size {
                if let Ok(buffer) = memory_pool::global_pool().allocate(size) {
                    buffers.push(buffer);
                }
            }

            let reuse_end_time = Instant::now();

            let alloc_duration = (alloc_end_time - start_time).as_millis() as f64;
            let return_duration = (return_end_time - alloc_end_time).as_millis() as f64;
            let reuse_duration = (reuse_end_time - return_end_time).as_millis() as f64;

            let reuse_speedup = if reuse_duration > 0.0 {
                alloc_duration / reuse_duration
            } else {
                1.0
            };

            println!("Size {}: {:.2}x faster reuse", size, reuse_speedup);

            // Verify memory pool is working efficiently
            assert!(reuse_speedup > 1.1, "Should be at least 10% faster on reuse");

            // Clean up
            for buffer in buffers {
                memory_pool::global_pool().return_buffer(buffer);
            }
        }
    }

    #[test]
    fn test_zero_copy_optimization_benefits() {
        println!("🎯 Testing zero-copy optimization benefits...");

        if !ZeroCopyOptimizer::is_supported() {
            println!("Zero-copy not supported on this platform, skipping test");
            return;
        }

        let zero_copy = ZeroCopyOptimizer::new();
        if !zero_copy.is_enabled() {
            println!("Zero-copy not enabled, skipping test");
            return;
        }

        let iterations = 10;
        let mut zero_copy_results = Vec::new();
        let mut traditional_results = Vec::new();

        // Test zero-copy capture
        for _i in 0..iterations {
            let start_time = Instant::now();

            if let Ok(capturer) = capture::Capturer::new() {
                if let Ok(result) = zero_copy.capture_zero_copy(&*capturer, 0) {
                    let duration_ms = start_time.elapsed().as_millis() as f64;
                    zero_copy_results.push((duration_ms, result.size()));
                }
            }
        }

        // Test traditional capture for comparison
        if let Ok(mut screenshot) = WebPScreenshot::new() {
            for _i in 0..iterations {
                let start_time = Instant::now();

                if let Ok(result) = screenshot.capture_display(0) {
                    let duration_ms = start_time.elapsed().as_millis() as f64;
                    traditional_results.push((duration_ms, result.data.len()));
                }
            }
        }

        if !zero_copy_results.is_empty() && !traditional_results.is_empty() {
            let zero_copy_avg = zero_copy_results.iter().map(|r| r.0).sum::<f64>() / zero_copy_results.len() as f64;
            let traditional_avg = traditional_results.iter().map(|r| r.0).sum::<f64>() / traditional_results.len() as f64;
            let speedup_ratio = traditional_avg / zero_copy_avg;

            println!("Zero-copy speedup: {:.2}x faster", speedup_ratio);
            assert!(speedup_ratio > 1.2, "Should be at least 20% improvement");
        }
    }

    #[test]
    fn test_multi_threading_scalability() {
        println!("🎯 Testing multi-threading scalability...");

        let thread_counts = [1, 2, 4, 8];
        let test_image_size = (2048, 2048); // 4MP test image
        let mut results = std::collections::HashMap::new();

        for thread_count in thread_counts {
            println!("Testing with {} threads", thread_count);

            let test_image = create_test_image(test_image_size.0, test_image_size.1);
            let iterations = 3;
            let mut durations = Vec::new();

            for _i in 0..iterations {
                let start_time = Instant::now();

                let webp_config = WebPConfig {
                    quality: 80,
                    method: 4,
                    thread_count,
                    ..Default::default()
                };

                let mut encoder = WebPEncoder::new();
                if let Ok(_webp_data) = encoder.encode(&test_image, &webp_config) {
                    let duration_ms = start_time.elapsed().as_millis() as f64;
                    durations.push(duration_ms);
                }
            }

            if !durations.is_empty() {
                let avg_duration = durations.iter().sum::<f64>() / durations.len() as f64;
                results.insert(format!("threads_{}", thread_count), avg_duration);

                println!("{} threads: {:.2}ms average", thread_count, avg_duration);

                // Sanity check - should complete in reasonable time
                assert!(avg_duration < 5000.0, "Should complete in less than 5 seconds");
            }
        }

        // Demonstrate threading scalability
        if results.len() >= 2 {
            let baseline_key = "threads_1";
            if let Some(&baseline_time) = results.get(baseline_key) {
                for thread_count in thread_counts {
                    if thread_count > 1 {
                        let key = format!("threads_{}", thread_count);
                        if let Some(&current_time) = results.get(&key) {
                            let speedup_ratio = baseline_time / current_time;
                            let efficiency = speedup_ratio / thread_count as f64;

                            println!(
                                "{} threads: {:.2}x speedup, {:.1}% efficiency",
                                thread_count,
                                speedup_ratio,
                                efficiency * 100.0
                            );

                            // Threading should provide some benefit (at least 1.2x with 2 threads)
                            if thread_count == 2 {
                                assert!(speedup_ratio > 1.2, "Should get at least 1.2x speedup with 2 threads");
                            }
                        }
                    }
                }
            }
        }
    }

    /// Helper function to create test images
    fn create_test_image(width: u32, height: u32) -> RawImage {
        let mut data = vec![0u8; (width * height * 4) as usize];

        // Create a complex pattern that's realistic for compression testing
        for y in 0..height {
            for x in 0..width {
                let offset = ((y * width + x) * 4) as usize;

                // Gradient with some noise
                let gradient_x = (x as f64 / width as f64) * 255.0;
                let gradient_y = (y as f64 / height as f64) * 255.0;
                let noise = ((x as f64 * 0.1).sin() + (y as f64 * 0.1).cos()) * 30.0;

                data[offset] = (gradient_x + noise).max(0.0).min(255.0) as u8;     // R
                data[offset + 1] = (gradient_y + noise).max(0.0).min(255.0) as u8; // G
                data[offset + 2] = ((gradient_x + gradient_y) / 2.0) as u8;         // B
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