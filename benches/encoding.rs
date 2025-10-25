//! WebP Encoding Performance Benchmarks
//!
//! Benchmarks for WebP encoding operations using criterion

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::time::Duration;
use webp_screenshot_rust::*;

fn bench_webp_encoding_quality_levels(c: &mut Criterion) {
    let mut group = c.benchmark_group("webp_encoding_quality");
    group.measurement_time(Duration::from_secs(20));

    let image_sizes = [
        ("small", 256, 256),
        ("medium", 512, 512),
        ("large", 1024, 1024),
        ("xlarge", 2048, 1536),
    ];

    let quality_levels = [50, 75, 90, 95];

    for (size_name, width, height) in image_sizes {
        let test_image = create_test_image(width, height, ImagePattern::Photographic);
        let pixel_count = width as u64 * height as u64;

        group.throughput(Throughput::Elements(pixel_count));

        for quality in quality_levels {
            let bench_name = format!("{}_q{}", size_name, quality);

            group.bench_with_input(BenchmarkId::new("encode_quality", bench_name), &(test_image.clone(), quality), |b, (image, q)| {
                let mut encoder = WebPEncoder::new();
                let config = WebPConfig {
                    quality: *q as u8,
                    ..Default::default()
                };

                b.iter(|| {
                    match encoder.encode(image, &config) {
                        Ok(data) => criterion::black_box(data),
                        Err(_) => Vec::new(),
                    }
                })
            });
        }
    }

    group.finish();
}

fn bench_webp_encoding_methods(c: &mut Criterion) {
    let mut group = c.benchmark_group("webp_encoding_methods");
    group.measurement_time(Duration::from_secs(15));

    let test_image = create_test_image(1024, 768, ImagePattern::Mixed);
    let pixel_count = (1024 * 768) as u64;
    group.throughput(Throughput::Elements(pixel_count));

    let methods = [
        ("fastest", 0),
        ("fast", 1),
        ("balanced", 4),
        ("high_quality", 6),
    ];

    for (method_name, method_value) in methods {
        group.bench_with_input(BenchmarkId::new("encode_method", method_name), &method_value, |b, &method| {
            let mut encoder = WebPEncoder::new();
            let config = WebPConfig {
                quality: 80,
                method: method as u8,
                ..Default::default()
            };

            b.iter(|| {
                match encoder.encode(&test_image, &config) {
                    Ok(data) => criterion::black_box(data),
                    Err(_) => Vec::new(),
                }
            })
        });
    }

    group.finish();
}

fn bench_webp_encoding_image_types(c: &mut Criterion) {
    let mut group = c.benchmark_group("webp_encoding_image_types");
    group.measurement_time(Duration::from_secs(15));

    let image_types = [
        ("gradient", ImagePattern::Gradient),
        ("photographic", ImagePattern::Photographic),
        ("screenshot", ImagePattern::Screenshot),
        ("geometric", ImagePattern::Geometric),
        ("text", ImagePattern::Text),
        ("solid", ImagePattern::SolidColor),
        ("noise", ImagePattern::Noise),
    ];

    let width = 512;
    let height = 512;
    let pixel_count = (width * height) as u64;
    group.throughput(Throughput::Elements(pixel_count));

    for (type_name, pattern) in image_types {
        let test_image = create_test_image(width, height, pattern);

        group.bench_with_input(BenchmarkId::new("encode_image_type", type_name), &test_image, |b, image| {
            let mut encoder = WebPEncoder::new();
            let config = WebPConfig {
                quality: 80,
                method: 4,
                ..Default::default()
            };

            b.iter(|| {
                match encoder.encode(image, &config) {
                    Ok(data) => criterion::black_box(data),
                    Err(_) => Vec::new(),
                }
            })
        });
    }

    group.finish();
}

fn bench_webp_encoding_threading(c: &mut Criterion) {
    let mut group = c.benchmark_group("webp_encoding_threading");
    group.measurement_time(Duration::from_secs(20));

    let test_image = create_test_image(2048, 1536, ImagePattern::Photographic);
    let pixel_count = (2048 * 1536) as u64;
    group.throughput(Throughput::Elements(pixel_count));

    let thread_counts = [1, 2, 4, 8, 16];

    for thread_count in thread_counts {
        group.bench_with_input(BenchmarkId::new("encode_threads", thread_count), &thread_count, |b, &threads| {
            let mut encoder = WebPEncoder::new();
            let config = WebPConfig {
                quality: 80,
                method: 4,
                thread_count: threads,
                ..Default::default()
            };

            b.iter(|| {
                match encoder.encode(&test_image, &config) {
                    Ok(data) => criterion::black_box(data),
                    Err(_) => Vec::new(),
                }
            })
        });
    }

    group.finish();
}

fn bench_webp_encoding_advanced_features(c: &mut Criterion) {
    let mut group = c.benchmark_group("webp_encoding_advanced");
    group.measurement_time(Duration::from_secs(15));

    let test_image = create_test_image(1024, 768, ImagePattern::Mixed);
    let pixel_count = (1024 * 768) as u64;
    group.throughput(Throughput::Elements(pixel_count));

    // Test lossless encoding
    group.bench_function("lossless_encoding", |b| {
        let mut encoder = WebPEncoder::new();
        let config = WebPConfig {
            lossless: true,
            method: 4,
            ..Default::default()
        };

        b.iter(|| {
            match encoder.encode(&test_image, &config) {
                Ok(data) => criterion::black_box(data),
                Err(_) => Vec::new(),
            }
        })
    });

    // Test near-lossless encoding
    group.bench_function("near_lossless_encoding", |b| {
        let mut encoder = WebPEncoder::new();
        let config = WebPConfig {
            lossless: true,
            near_lossless: 90,
            method: 4,
            ..Default::default()
        };

        b.iter(|| {
            match encoder.encode(&test_image, &config) {
                Ok(data) => criterion::black_box(data),
                Err(_) => Vec::new(),
            }
        })
    });

    // Test with alpha channel
    let transparent_image = create_test_image_with_alpha(512, 512);
    group.bench_function("alpha_encoding", |b| {
        let mut encoder = WebPEncoder::new();
        let config = WebPConfig {
            quality: 80,
            method: 4,
            alpha_quality: 90,
            ..Default::default()
        };

        b.iter(|| {
            match encoder.encode(&transparent_image, &config) {
                Ok(data) => criterion::black_box(data),
                Err(_) => Vec::new(),
            }
        })
    });

    group.finish();
}

fn bench_simd_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("simd_operations");
    group.measurement_time(Duration::from_secs(10));

    let image_sizes = [
        ("small", 256, 256),
        ("medium", 1024, 768),
        ("large", 2048, 1536),
    ];

    for (size_name, width, height) in image_sizes {
        let bgra_image = create_test_image_bgra(width, height);
        let pixel_count = (width * height) as u64;

        group.throughput(Throughput::Elements(pixel_count));

        group.bench_with_input(BenchmarkId::new("bgra_to_rgba_conversion", size_name), &bgra_image, |b, image| {
            b.iter(|| {
                let mut converted = image.clone();
                // Convert BGRA to RGBA in-place (simplified SIMD operation simulation)
                for chunk in converted.data.chunks_exact_mut(4) {
                    chunk.swap(0, 2); // Swap B and R channels
                }
                criterion::black_box(converted);
            })
        });

        // Benchmark SIMD converter if available
        #[cfg(feature = "simd")]
        {
            group.bench_with_input(BenchmarkId::new("simd_conversion", size_name), &bgra_image, |b, image| {
                b.iter(|| {
                    let converter = encoder::simd::global_simd_converter();
                    match converter.convert_bgra_to_rgba(&image.data, image.width, image.height) {
                        Ok(converted) => criterion::black_box(converted),
                        Err(_) => Vec::new(),
                    }
                })
            });
        }
    }

    group.finish();
}

fn bench_encoder_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("encoder_creation");

    group.bench_function("create_webp_encoder", |b| {
        b.iter(|| {
            let encoder = WebPEncoder::new();
            criterion::black_box(encoder);
        })
    });

    #[cfg(feature = "gpu")]
    group.bench_function("create_gpu_encoder", |b| {
        b.iter(|| {
            let gpu_encoder = encoder::gpu::GpuWebPEncoder::new();
            criterion::black_box(gpu_encoder);
        })
    });

    group.finish();
}

fn bench_encoding_streaming(c: &mut Criterion) {
    let mut group = c.benchmark_group("encoding_streaming");
    group.measurement_time(Duration::from_secs(25));

    // Test streaming pipeline encoding
    let large_image = create_test_image(3840, 2160, ImagePattern::Mixed); // 4K image
    let pixel_count = (3840 * 2160) as u64;
    group.throughput(Throughput::Elements(pixel_count));

    group.bench_function("streaming_pipeline_encode", |b| {
        let capturer = match capture::Capturer::new() {
            Ok(c) => c,
            Err(_) => {
                b.iter(|| {});
                return;
            }
        };

        let config = StreamingConfig {
            target_fps: 30,
            buffer_size: 10,
            capture_threads: 1,
            encoding_threads: 2,
            adaptive_quality: false,
            allow_frame_drop: false,
            webp_config: WebPConfig {
                quality: 80,
                method: 1, // Fast method for streaming
                ..Default::default()
            },
            use_zero_copy: false,
            use_gpu: false,
        };

        let pipeline = StreamingPipeline::new(Box::new(capturer), config);

        b.iter(|| {
            // Simulate streaming encoding by starting and immediately stopping
            let frame_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let frame_count_clone = frame_count.clone();

            if pipeline.start(move |data| {
                frame_count_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                criterion::black_box(data);
            }).is_ok() {
                std::thread::sleep(Duration::from_millis(10)); // Brief operation
                pipeline.stop();
            }

            let final_count = frame_count.load(std::sync::atomic::Ordering::Relaxed);
            criterion::black_box(final_count);
        })
    });

    group.finish();
}

// Helper functions for creating test images

#[derive(Clone, Copy)]
enum ImagePattern {
    Gradient,
    Photographic,
    Screenshot,
    Geometric,
    Text,
    SolidColor,
    Noise,
    Mixed,
}

fn create_test_image(width: u32, height: u32, pattern: ImagePattern) -> RawImage {
    let mut data = vec![0u8; (width * height * 4) as usize];

    match pattern {
        ImagePattern::Gradient => {
            for y in 0..height {
                for x in 0..width {
                    let offset = ((y * width + x) * 4) as usize;
                    data[offset] = (x * 255 / width) as u8;         // R
                    data[offset + 1] = (y * 255 / height) as u8;   // G
                    data[offset + 2] = ((x + y) * 255 / (width + height)) as u8; // B
                    data[offset + 3] = 255; // A
                }
            }
        }
        ImagePattern::Photographic => {
            for y in 0..height {
                for x in 0..width {
                    let offset = ((y * width + x) * 4) as usize;

                    // Simulate natural image with smooth variations
                    let base_r = 128.0 + 60.0 * (x as f64 * 0.02).sin() * (y as f64 * 0.015).cos();
                    let base_g = 100.0 + 80.0 * (x as f64 * 0.015).cos() * (y as f64 * 0.02).sin();
                    let base_b = 140.0 + 50.0 * ((x + y) as f64 * 0.01).sin();

                    // Add pseudo-random noise
                    let noise_seed = (x.wrapping_mul(73) ^ y.wrapping_mul(37)) as u64;
                    let noise = ((noise_seed % 40) as f64 - 20.0);

                    data[offset] = (base_r + noise).max(0.0).min(255.0) as u8;
                    data[offset + 1] = (base_g + noise).max(0.0).min(255.0) as u8;
                    data[offset + 2] = (base_b + noise).max(0.0).min(255.0) as u8;
                    data[offset + 3] = 255;
                }
            }
        }
        ImagePattern::Screenshot => {
            for y in 0..height {
                for x in 0..width {
                    let offset = ((y * width + x) * 4) as usize;

                    let (r, g, b) = if y < height / 10 {
                        (240, 240, 240) // Title bar
                    } else if x < width / 5 || x > width * 4 / 5 || y > height * 9 / 10 {
                        (64, 64, 64) // Borders and taskbar
                    } else if (x > width * 3 / 10 && x < width * 7 / 10) && (y > height * 3 / 10 && y < height * 6 / 10) {
                        // Text area with high frequency pattern
                        if ((x / 8) + (y / 12)) % 2 == 0 { (255, 255, 255) } else { (0, 0, 0) }
                    } else {
                        (248, 249, 250) // Content area
                    };

                    data[offset] = r;
                    data[offset + 1] = g;
                    data[offset + 2] = b;
                    data[offset + 3] = 255;
                }
            }
        }
        ImagePattern::Geometric => {
            for y in 0..height {
                for x in 0..width {
                    let offset = ((y * width + x) * 4) as usize;

                    let center_x = width / 2;
                    let center_y = height / 2;
                    let distance = (((x as i32 - center_x as i32).pow(2) + (y as i32 - center_y as i32).pow(2)) as f64).sqrt();

                    let (r, g, b) = if distance < (width as f64 * 0.15) {
                        (255, 50, 50) // Center circle - red
                    } else if (x as i32 - center_x as i32).abs() < 20 || (y as i32 - center_y as i32).abs() < 20 {
                        (50, 50, 255) // Cross pattern - blue
                    } else if ((x / 40) + (y / 40)) % 2 == 0 {
                        (255, 255, 255) // Checkerboard - white
                    } else {
                        (0, 0, 0) // Black
                    };

                    data[offset] = r;
                    data[offset + 1] = g;
                    data[offset + 2] = b;
                    data[offset + 3] = 255;
                }
            }
        }
        ImagePattern::Text => {
            // Fill with white background
            for i in (0..data.len()).step_by(4) {
                data[i] = 255;     // R
                data[i + 1] = 255; // G
                data[i + 2] = 255; // B
                data[i + 3] = 255; // A
            }

            // Add text-like patterns
            let line_height = 16;
            let char_width = 8;

            for line in 0..(height / line_height) {
                let y = line * line_height;
                let line_length = (width * 7 / 10) / char_width; // 70% line fill

                for char in 0..line_length {
                    let x = char * char_width;

                    // Draw character block
                    for dy in 2..(line_height - 2) {
                        for dx in 1..(char_width - 1) {
                            if y + dy < height && x + dx < width {
                                let char_offset = (((y + dy) * width + (x + dx)) * 4) as usize;
                                let char_seed = (x.wrapping_mul(7) ^ y.wrapping_mul(11) ^ dx.wrapping_mul(3) ^ dy.wrapping_mul(5)) as u64;

                                if (char_seed % 10) > 2 { // 70% fill for character
                                    data[char_offset] = 0;     // Black text
                                    data[char_offset + 1] = 0;
                                    data[char_offset + 2] = 0;
                                }
                            }
                        }
                    }
                }
            }
        }
        ImagePattern::SolidColor => {
            for i in (0..data.len()).step_by(4) {
                data[i] = 100;     // R
                data[i + 1] = 150; // G
                data[i + 2] = 255; // B
                data[i + 3] = 255; // A
            }
        }
        ImagePattern::Noise => {
            for y in 0..height {
                for x in 0..width {
                    let offset = ((y * width + x) * 4) as usize;

                    // Generate pseudo-random values
                    let seed = (x.wrapping_mul(73) ^ y.wrapping_mul(37) ^ (x + y).wrapping_mul(17)) as u64;
                    let r = (seed & 0xFF) as u8;
                    let g = ((seed >> 8) & 0xFF) as u8;
                    let b = ((seed >> 16) & 0xFF) as u8;

                    data[offset] = r;
                    data[offset + 1] = g;
                    data[offset + 2] = b;
                    data[offset + 3] = 255;
                }
            }
        }
        ImagePattern::Mixed => {
            // Combination of different patterns in regions
            for y in 0..height {
                for x in 0..width {
                    let offset = ((y * width + x) * 4) as usize;

                    let region_x = (x * 4) / width;
                    let region_y = (y * 4) / height;
                    let region = (region_x + region_y * 2) % 4;

                    let (r, g, b) = match region {
                        0 => {
                            // Gradient region
                            let r = (x * 255 / width) as u8;
                            let g = (y * 255 / height) as u8;
                            let b = ((x + y) * 255 / (width + height)) as u8;
                            (r, g, b)
                        }
                        1 => {
                            // High frequency region
                            let val = ((x as f64 * 0.1).sin() + (y as f64 * 0.1).cos()) * 127.0 + 128.0;
                            let v = val.max(0.0).min(255.0) as u8;
                            (v, v, v)
                        }
                        2 => {
                            // Geometric region
                            if ((x / 20) + (y / 20)) % 2 == 0 { (255, 0, 0) } else { (0, 0, 255) }
                        }
                        3 => {
                            // Solid region
                            (128, 128, 128)
                        }
                        _ => (0, 0, 0),
                    };

                    data[offset] = r;
                    data[offset + 1] = g;
                    data[offset + 2] = b;
                    data[offset + 3] = 255;
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

fn create_test_image_bgra(width: u32, height: u32) -> RawImage {
    let mut data = vec![0u8; (width * height * 4) as usize];

    for y in 0..height {
        for x in 0..width {
            let offset = ((y * width + x) * 4) as usize;
            data[offset] = (y * 255 / height) as u8;     // B
            data[offset + 1] = ((x + y) * 255 / (width + height)) as u8; // G
            data[offset + 2] = (x * 255 / width) as u8; // R
            data[offset + 3] = 255; // A
        }
    }

    RawImage {
        data,
        width,
        height,
        format: PixelFormat::BGRA,
    }
}

fn create_test_image_with_alpha(width: u32, height: u32) -> RawImage {
    let mut data = vec![0u8; (width * height * 4) as usize];

    for y in 0..height {
        for x in 0..width {
            let offset = ((y * width + x) * 4) as usize;

            // Create gradient with varying alpha
            data[offset] = 255; // R
            data[offset + 1] = (x * 255 / width) as u8; // G
            data[offset + 2] = (y * 255 / height) as u8; // B
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

criterion_group!(
    benches,
    bench_webp_encoding_quality_levels,
    bench_webp_encoding_methods,
    bench_webp_encoding_image_types,
    bench_webp_encoding_threading,
    bench_webp_encoding_advanced_features,
    bench_simd_operations,
    bench_encoder_creation,
    bench_encoding_streaming
);

criterion_main!(benches);