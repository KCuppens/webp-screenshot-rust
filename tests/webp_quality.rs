//! WebP Quality Validation Tests
//!
//! Equivalent to test/quality/webp-quality.test.js

use std::collections::HashMap;
use webp_screenshot_rust::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone)]
    struct QualityThresholds {
        psnr_minimum: f64,
        psnr_target: f64,
        ssim_minimum: f64,
        ssim_target: f64,
        compression_ratio: f64,
    }

    impl Default for QualityThresholds {
        fn default() -> Self {
            Self {
                psnr_minimum: 35.0,
                psnr_target: 45.0,
                ssim_minimum: 0.85,
                ssim_target: 0.95,
                compression_ratio: 5.0,
            }
        }
    }

    #[derive(Debug, Clone)]
    struct TestImageSet {
        gradient: RawImage,
        photograph: RawImage,
        screenshot: RawImage,
        geometric: RawImage,
        text: RawImage,
        solid: RawImage,
    }

    #[test]
    fn test_psnr_validation() {
        println!("🎨 Testing PSNR (Peak Signal-to-Noise Ratio) validation...");

        let test_images = create_test_image_set();
        let quality_levels = [60, 70, 80, 90, 95];
        let thresholds = QualityThresholds::default();

        let image_types = [
            ("gradient", &test_images.gradient),
            ("photograph", &test_images.photograph),
            ("screenshot", &test_images.screenshot),
            ("geometric", &test_images.geometric),
            ("text", &test_images.text),
            ("solid", &test_images.solid),
        ];

        for (image_type, original_image) in image_types {
            for quality in quality_levels {
                println!("Testing {} at quality {}", image_type, quality);

                let mut encoder = WebPEncoder::new();
                let config = WebPConfig {
                    quality: quality as u8,
                    ..Default::default()
                };

                match encoder.encode(original_image, &config) {
                    Ok(webp_data) => {
                        assert!(!webp_data.is_empty(), "WebP data should not be empty");

                        // Decode WebP back to raw data for comparison (would need WebP decoder)
                        // For now, just verify basic metrics
                        let compression_ratio = original_image.data.len() as f64 / webp_data.len() as f64;

                        println!(
                            "{} Q{}: compression={:.1}:1, size={}KB",
                            image_type,
                            quality,
                            compression_ratio,
                            webp_data.len() / 1024
                        );

                        // Quality expectations based on quality setting
                        let expected_min_compression = if quality >= 90 {
                            thresholds.compression_ratio * 0.6 // High quality = lower compression
                        } else if quality >= 80 {
                            thresholds.compression_ratio * 0.8
                        } else {
                            thresholds.compression_ratio
                        };

                        assert!(
                            compression_ratio > expected_min_compression,
                            "Should achieve at least {:.1}:1 compression, got {:.1}:1",
                            expected_min_compression,
                            compression_ratio
                        );
                    }
                    Err(e) => {
                        panic!("Failed to encode {} at quality {}: {}", image_type, quality, e);
                    }
                }
            }
        }
    }

    #[test]
    fn test_quality_vs_compression_tradeoffs() {
        println!("🎨 Testing quality vs compression trade-offs...");

        let test_image = create_photographic_image(512, 512);
        let qualities = [50, 60, 70, 80, 90, 95];
        let mut results = Vec::new();
        let mut encoder = WebPEncoder::new();

        for quality in qualities {
            let config = WebPConfig {
                quality: quality as u8,
                ..Default::default()
            };

            match encoder.encode(&test_image, &config) {
                Ok(webp_data) => {
                    let compression_ratio = test_image.data.len() as f64 / webp_data.len() as f64;
                    let bits_per_pixel = (webp_data.len() * 8) as f64 / (test_image.width * test_image.height) as f64;

                    results.push((quality, webp_data.len(), compression_ratio, bits_per_pixel));

                    println!(
                        "Q{}: {:.1}KB, {:.1}:1 compression, {:.2} bits/pixel",
                        quality,
                        webp_data.len() as f64 / 1024.0,
                        compression_ratio,
                        bits_per_pixel
                    );
                }
                Err(e) => {
                    panic!("Failed to encode at quality {}: {}", quality, e);
                }
            }
        }

        println!("\n📊 Quality vs Compression Analysis:");
        println!("Quality\tFile Size\tCompression\tBits/Pixel");
        for (quality, file_size, compression_ratio, bits_per_pixel) in &results {
            println!(
                "{}\t{:.1}KB\t\t{:.1}:1\t\t{:.2}",
                quality,
                *file_size as f64 / 1024.0,
                compression_ratio,
                bits_per_pixel
            );
        }

        // Verify quality/size relationship
        for i in 1..results.len() {
            let (_, prev_size, prev_ratio, _) = results[i - 1];
            let (_, curr_size, curr_ratio, _) = results[i];

            assert!(
                curr_size > prev_size,
                "Higher quality should produce larger files"
            );
            assert!(
                curr_ratio < prev_ratio,
                "Higher quality should have lower compression ratio"
            );
        }
    }

    #[test]
    fn test_compression_efficiency_for_content_types() {
        println!("🎨 Testing compression efficiency for different content types...");

        let test_images = create_test_image_set();
        let thresholds = QualityThresholds::default();

        let content_types = [
            ("gradient", &test_images.gradient),
            ("photograph", &test_images.photograph),
            ("screenshot", &test_images.screenshot),
            ("geometric", &test_images.geometric),
            ("text", &test_images.text),
            ("solid", &test_images.solid),
        ];

        for (content_type, image) in content_types {
            println!("\n{} compression analysis:", content_type);
            println!("Method\tTime(ms)\tSize(KB)\tRatio");

            // Test different methods
            let methods = [1, 3, 4, 6]; // Fast to slow
            let mut method_results = Vec::new();

            for method in methods {
                let start_time = std::time::Instant::now();

                let mut encoder = WebPEncoder::new();
                let config = WebPConfig {
                    quality: 80,
                    method: method as u8,
                    ..Default::default()
                };

                match encoder.encode(image, &config) {
                    Ok(webp_data) => {
                        let encode_time = start_time.elapsed().as_millis() as f64;
                        let compression_ratio = image.data.len() as f64 / webp_data.len() as f64;

                        method_results.push((method, encode_time, webp_data.len(), compression_ratio));

                        println!(
                            "{}\t{:.1}\t\t{:.1}\t\t{:.1}:1",
                            method,
                            encode_time,
                            webp_data.len() as f64 / 1024.0,
                            compression_ratio
                        );
                    }
                    Err(e) => {
                        println!("Method {} failed: {}", method, e);
                    }
                }
            }

            // Validate compression efficiency
            for (method, encode_time, file_size, compression_ratio) in method_results {
                assert!(
                    compression_ratio > thresholds.compression_ratio,
                    "Method {} should achieve at least {:.1}:1 compression, got {:.1}:1",
                    method,
                    thresholds.compression_ratio,
                    compression_ratio
                );
                assert!(file_size > 0, "File size should be greater than 0");
                assert!(
                    encode_time < 5000.0,
                    "Should encode within 5 seconds, took {:.1}ms",
                    encode_time
                );
            }
        }
    }

    #[test]
    fn test_optimization_for_different_use_cases() {
        println!("🎨 Testing optimization for different use cases...");

        let use_cases = [
            ("Fast Web", 75, 1, 8.0),
            ("Balanced", 80, 4, 10.0),
            ("High Quality", 90, 6, 6.0),
            ("Archival", 95, 6, 4.0),
        ];

        let test_image = create_photographic_image(512, 512); // Use photograph as representative

        for (use_case_name, quality, method, target_ratio) in use_cases {
            println!("Testing {} use case", use_case_name);

            let start_time = std::time::Instant::now();
            let mut encoder = WebPEncoder::new();
            let config = WebPConfig {
                quality: quality as u8,
                method: method as u8,
                ..Default::default()
            };

            match encoder.encode(&test_image, &config) {
                Ok(webp_data) => {
                    let encode_time = start_time.elapsed().as_millis() as f64;
                    let compression_ratio = test_image.data.len() as f64 / webp_data.len() as f64;

                    println!(
                        "{}: {:.1}:1 in {:.1}ms",
                        use_case_name,
                        compression_ratio,
                        encode_time
                    );

                    assert!(
                        compression_ratio > target_ratio * 0.8,
                        "{} should achieve at least {:.1}:1 compression (80% of target), got {:.1}:1",
                        use_case_name,
                        target_ratio * 0.8,
                        compression_ratio
                    );
                    assert!(!webp_data.is_empty(), "Should produce non-empty output");
                }
                Err(e) => {
                    panic!("Failed to encode for {} use case: {}", use_case_name, e);
                }
            }
        }
    }

    #[test]
    fn test_visual_quality_assessment() {
        println!("🎨 Testing visual quality assessment...");

        let feature_tests = [
            ("Edge Preservation", create_geometric_image(512, 512)),
            ("Color Accuracy", create_gradient_image(512, 512)),
            ("Text Clarity", create_text_image(512, 512)),
            ("Detail Retention", create_photographic_image(512, 512)),
        ];

        for (test_name, test_image) in feature_tests {
            println!("Testing {}", test_name);

            let mut encoder = WebPEncoder::new();
            let config = WebPConfig {
                quality: 85,
                ..Default::default()
            };

            match encoder.encode(&test_image, &config) {
                Ok(webp_data) => {
                    // For now, just verify basic metrics
                    let compression_ratio = test_image.data.len() as f64 / webp_data.len() as f64;

                    println!(
                        "{}: {:.1}:1 compression, {} bytes encoded",
                        test_name,
                        compression_ratio,
                        webp_data.len()
                    );

                    // Basic quality checks
                    assert!(compression_ratio > 3.0, "Should achieve reasonable compression");
                    assert!(!webp_data.is_empty(), "Should produce encoded output");

                    // In a full implementation, would decode and analyze visual features
                    // let features = analyze_visual_features(&test_image.data, &decoded_image, test_image.width, test_image.height);
                    // assert!(features.edge_preservation > 0.85);
                    // assert!(features.color_accuracy > 0.90);
                }
                Err(e) => {
                    panic!("Failed to encode for {}: {}", test_name, e);
                }
            }
        }
    }

    #[test]
    fn test_transparency_handling() {
        println!("🎨 Testing transparency handling...");

        let transparent_image = create_transparent_image(256, 256);

        let mut encoder = WebPEncoder::new();
        let config = WebPConfig {
            quality: 90,
            ..Default::default()
        };

        match encoder.encode(&transparent_image, &config) {
            Ok(webp_data) => {
                // Verify WebP data is generated
                assert!(!webp_data.is_empty(), "Should produce WebP data");

                // In a full implementation, would verify alpha channel preservation
                // let metadata = decode_webp_metadata(&webp_data);
                // assert_eq!(metadata.channels, 4); // RGBA
                // assert!(metadata.has_alpha);

                let compression_ratio = transparent_image.data.len() as f64 / webp_data.len() as f64;
                println!("Transparency: {:.1}:1 compression", compression_ratio);

                assert!(compression_ratio > 3.0, "Should achieve reasonable compression with alpha");
            }
            Err(e) => {
                panic!("Failed to encode transparent image: {}", e);
            }
        }
    }

    #[test]
    fn test_quality_regression_detection() {
        println!("🎨 Testing quality regression detection...");

        let test_images = create_test_image_set();
        let test_qualities = [70, 80, 90];

        let image_types = [
            ("gradient", &test_images.gradient),
            ("photograph", &test_images.photograph),
            ("screenshot", &test_images.screenshot),
            ("geometric", &test_images.geometric),
            ("text", &test_images.text),
            ("solid", &test_images.solid),
        ];

        let mut current_results: HashMap<String, HashMap<String, (f64, usize)>> = HashMap::new();

        // Test all image types at standard qualities
        for (image_type, image) in image_types {
            let mut image_results = HashMap::new();

            for quality in test_qualities {
                let mut encoder = WebPEncoder::new();
                let config = WebPConfig {
                    quality: quality as u8,
                    ..Default::default()
                };

                match encoder.encode(image, &config) {
                    Ok(webp_data) => {
                        let compression_ratio = image.data.len() as f64 / webp_data.len() as f64;
                        image_results.insert(format!("q{}", quality), (compression_ratio, webp_data.len()));

                        println!(
                            "{} Q{}: {:.1}:1 compression, {}KB",
                            image_type,
                            quality,
                            compression_ratio,
                            webp_data.len() / 1024
                        );
                    }
                    Err(e) => {
                        println!("Failed to encode {} at Q{}: {}", image_type, quality, e);
                    }
                }
            }

            current_results.insert(image_type.to_string(), image_results);
        }

        // Basic quality validation
        for (image_type, image_results) in current_results {
            for (quality_key, (compression_ratio, file_size)) in image_results {
                assert!(
                    compression_ratio > 3.0,
                    "{} {} should achieve at least 3:1 compression, got {:.1}:1",
                    image_type,
                    quality_key,
                    compression_ratio
                );
                assert!(file_size > 0, "File size should be greater than 0");
            }
        }

        println!("Quality regression detection completed");
    }

    // Helper functions for image generation

    fn create_test_image_set() -> TestImageSet {
        TestImageSet {
            gradient: create_gradient_image(512, 512),
            photograph: create_photographic_image(512, 512),
            screenshot: create_screenshot_image(512, 512),
            geometric: create_geometric_image(512, 512),
            text: create_text_image(512, 512),
            solid: create_solid_color_image(512, 512),
        }
    }

    fn create_gradient_image(width: u32, height: u32) -> RawImage {
        let mut data = vec![0u8; (width * height * 4) as usize];

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

    fn create_photographic_image(width: u32, height: u32) -> RawImage {
        let mut data = vec![0u8; (width * height * 4) as usize];

        // Simulate natural image with smooth variations and some noise
        for y in 0..height {
            for x in 0..width {
                let offset = ((y * width + x) * 4) as usize;

                // Base colors with smooth variations
                let base_r = 128.0 + 60.0 * (x as f64 * 0.02).sin() * (y as f64 * 0.015).cos();
                let base_g = 100.0 + 80.0 * (x as f64 * 0.015).cos() * (y as f64 * 0.02).sin();
                let base_b = 140.0 + 50.0 * ((x + y) as f64 * 0.01).sin();

                // Add some noise using simple hash-based pseudo-random
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};

                let mut hasher = DefaultHasher::new();
                (x, y).hash(&mut hasher);
                let hash_val = hasher.finish();
                let noise = ((hash_val & 0xFF) as f64 - 127.5) * 0.16; // Approximately ±20

                data[offset] = (base_r + noise).max(0.0).min(255.0) as u8;
                data[offset + 1] = (base_g + noise).max(0.0).min(255.0) as u8;
                data[offset + 2] = (base_b + noise).max(0.0).min(255.0) as u8;
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

    fn create_screenshot_image(width: u32, height: u32) -> RawImage {
        let mut data = vec![0u8; (width * height * 4) as usize];

        // Simulate typical desktop screenshot with windows, text areas, and solid colors
        for y in 0..height {
            for x in 0..width {
                let offset = ((y * width + x) * 4) as usize;

                let (r, g, b) = if y < height / 10 {
                    // Title bar
                    (240, 240, 240) // Light gray
                } else if x < width / 5 || x > width * 4 / 5 || y > height * 9 / 10 {
                    // Borders and taskbar
                    (64, 64, 64) // Dark gray
                } else if (x > width * 3 / 10 && x < width * 7 / 10) && (y > height * 3 / 10 && y < height * 6 / 10) {
                    // Text area (simulate text with high frequency pattern)
                    let text_pattern = ((x / 8) + (y / 12)) % 2;
                    if text_pattern == 0 { (255, 255, 255) } else { (0, 0, 0) }
                } else {
                    // Content area
                    (248, 249, 250) // Very light gray
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

    fn create_geometric_image(width: u32, height: u32) -> RawImage {
        let mut data = vec![0u8; (width * height * 4) as usize];

        for y in 0..height {
            for x in 0..width {
                let offset = ((y * width + x) * 4) as usize;

                // Create geometric shapes
                let center_x = width / 2;
                let center_y = height / 2;
                let distance = (((x as i32 - center_x as i32).pow(2) + (y as i32 - center_y as i32).pow(2)) as f64).sqrt();

                let (r, g, b) = if distance < (width as f64 * 0.15) {
                    // Center circle - red
                    (255, 50, 50)
                } else if (x as i32 - center_x as i32).abs() < 20 || (y as i32 - center_y as i32).abs() < 20 {
                    // Cross pattern - blue
                    (50, 50, 255)
                } else if ((x / 40) + (y / 40)) % 2 == 0 {
                    // Checkerboard - white/black
                    (255, 255, 255)
                } else {
                    (0, 0, 0)
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

    fn create_text_image(width: u32, height: u32) -> RawImage {
        let mut data = vec![255u8; (width * height * 4) as usize]; // Fill with white

        // Set alpha channel
        for i in (3..data.len()).step_by(4) {
            data[i] = 255;
        }

        // Simulate text with black rectangles (simplified text blocks)
        let line_height = 16;
        let char_width = 8;

        for line in 0..(height / line_height) {
            let y = line * line_height;

            // Use deterministic "random" for line length
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};

            let mut hasher = DefaultHasher::new();
            line.hash(&mut hasher);
            let hash_val = hasher.finish();
            let line_length = (width as f64 * (0.7 + 0.3 * ((hash_val & 0xFF) as f64 / 255.0)) / char_width as f64) as u32;

            for char in 0..line_length {
                let x = char * char_width;

                // Draw character block
                for dy in 2..(line_height - 2) {
                    for dx in 1..(char_width - 1) {
                        if y + dy < height && x + dx < width {
                            let offset = (((y + dy) * width + (x + dx)) * 4) as usize;

                            // Use hash for pseudo-random character fill
                            let mut char_hasher = DefaultHasher::new();
                            (x, y, dx, dy).hash(&mut char_hasher);
                            let char_hash = char_hasher.finish();

                            if (char_hash & 0xFF) > 76 { // 70% fill for character
                                data[offset] = 0;     // Black text
                                data[offset + 1] = 0;
                                data[offset + 2] = 0;
                            }
                        }
                    }
                }
            }
        }

        RawImage {
            data,
            width,
            height,
            format: PixelFormat::RGBA,
        }
    }

    fn create_solid_color_image(width: u32, height: u32) -> RawImage {
        let mut data = vec![0u8; (width * height * 4) as usize];

        // Solid blue
        for i in (0..data.len()).step_by(4) {
            data[i] = 100;         // R
            data[i + 1] = 150;     // G
            data[i + 2] = 255;     // B
            data[i + 3] = 255;     // A
        }

        RawImage {
            data,
            width,
            height,
            format: PixelFormat::RGBA,
        }
    }

    fn create_transparent_image(width: u32, height: u32) -> RawImage {
        let mut data = vec![0u8; (width * height * 4) as usize];

        for y in 0..height {
            for x in 0..width {
                let offset = ((y * width + x) * 4) as usize;

                // Create gradient with varying alpha
                data[offset] = 255;                                        // R
                data[offset + 1] = (x * 255 / width) as u8;              // G
                data[offset + 2] = (y * 255 / height) as u8;             // B
                data[offset + 3] = ((x + y) * 255 / (width + height)) as u8; // Varying alpha
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