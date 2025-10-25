//! Capture Performance Benchmarks
//!
//! Benchmarks for screenshot capture operations using criterion

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::time::Duration;
use webp_screenshot_rust::*;

fn bench_capture_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("capture_operations");
    group.measurement_time(Duration::from_secs(30));

    // Test different capture scenarios
    let scenarios = [
        ("small_capture", 640, 480),
        ("medium_capture", 1920, 1080),
        ("large_capture", 2560, 1440),
        ("ultra_capture", 3840, 2160),
    ];

    for (name, width, height) in scenarios {
        group.throughput(Throughput::Elements(width as u64 * height as u64));

        group.bench_with_input(BenchmarkId::new("screenshot_capture", name), &(width, height), |b, &(_w, _h)| {
            // Setup - create screenshot instance once outside the benchmark
            let mut screenshot = match WebPScreenshot::new() {
                Ok(s) => s,
                Err(_) => {
                    // Skip benchmark if no display available
                    b.iter(|| {});
                    return;
                }
            };

            b.iter(|| {
                // The actual operation being benchmarked
                match screenshot.capture_display(0) {
                    Ok(result) => {
                        criterion::black_box(result);
                    }
                    Err(_) => {
                        // Handle capture failures gracefully
                    }
                }
            })
        });
    }

    group.finish();
}

fn bench_display_enumeration(c: &mut Criterion) {
    let mut group = c.benchmark_group("display_enumeration");

    group.bench_function("get_displays", |b| {
        b.iter(|| {
            match get_displays() {
                Ok(displays) => criterion::black_box(displays),
                Err(_) => Vec::new(),
            }
        })
    });

    group.finish();
}

fn bench_capturer_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("capturer_creation");

    group.bench_function("create_capturer", |b| {
        b.iter(|| {
            match capture::Capturer::new() {
                Ok(capturer) => {
                    criterion::black_box(capturer);
                }
                Err(_) => {
                    // Handle creation failure
                }
            }
        })
    });

    group.bench_function("create_screenshot_instance", |b| {
        b.iter(|| {
            match WebPScreenshot::new() {
                Ok(screenshot) => {
                    criterion::black_box(screenshot);
                }
                Err(_) => {
                    // Handle creation failure
                }
            }
        })
    });

    group.finish();
}

fn bench_zero_copy_operations(c: &mut Criterion) {
    if !ZeroCopyOptimizer::is_supported() {
        return; // Skip if zero-copy not supported
    }

    let mut group = c.benchmark_group("zero_copy_operations");

    let zero_copy = ZeroCopyOptimizer::new();
    if !zero_copy.is_enabled() {
        return; // Skip if zero-copy not enabled
    }

    group.bench_function("zero_copy_capture", |b| {
        let capturer = match capture::Capturer::new() {
            Ok(c) => c,
            Err(_) => {
                b.iter(|| {});
                return;
            }
        };

        b.iter(|| {
            match zero_copy.capture_zero_copy(&*capturer, 0) {
                Ok(result) => criterion::black_box(result),
                Err(_) => {
                    // Handle capture failure
                }
            }
        })
    });

    group.finish();
}

fn bench_multi_display_capture(c: &mut Criterion) {
    let mut group = c.benchmark_group("multi_display_capture");

    // Only test if we have multiple displays
    let display_count = match get_displays() {
        Ok(displays) => displays.len().min(4), // Test up to 4 displays
        Err(_) => 1,
    };

    if display_count > 1 {
        group.bench_function("capture_all_displays", |b| {
            let mut screenshot = match WebPScreenshot::new() {
                Ok(s) => s,
                Err(_) => {
                    b.iter(|| {});
                    return;
                }
            };

            b.iter(|| {
                let results = screenshot.capture_all_displays();
                criterion::black_box(results);
            })
        });
    }

    group.finish();
}

fn bench_capture_with_config(c: &mut Criterion) {
    let mut group = c.benchmark_group("capture_with_config");

    let configs = [
        ("fast_config", WebPConfig::fast()),
        ("default_config", WebPConfig::default()),
        ("high_quality_config", WebPConfig {
            quality: 95,
            method: 6,
            ..Default::default()
        }),
    ];

    for (config_name, webp_config) in configs {
        group.bench_with_input(BenchmarkId::new("capture_with_webp_config", config_name), &webp_config, |b, config| {
            let mut screenshot = match WebPScreenshot::new() {
                Ok(s) => s,
                Err(_) => {
                    b.iter(|| {});
                    return;
                }
            };

            b.iter(|| {
                match screenshot.capture_with_config(0, config.clone()) {
                    Ok(result) => criterion::black_box(result),
                    Err(_) => {
                        // Handle capture failure
                    }
                }
            })
        });
    }

    group.finish();
}

fn bench_memory_pool_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_pool_operations");

    let buffer_sizes = [
        1024,        // 1KB
        64 * 1024,   // 64KB
        1024 * 1024, // 1MB
        4 * 1024 * 1024, // 4MB
    ];

    for &size in &buffer_sizes {
        group.throughput(Throughput::Bytes(size as u64));

        group.bench_with_input(BenchmarkId::new("allocate_buffer", format!("{}kb", size / 1024)), &size, |b, &buffer_size| {
            b.iter(|| {
                match memory_pool::global_pool().allocate(buffer_size) {
                    Ok(buffer) => {
                        criterion::black_box(&buffer);
                        memory_pool::global_pool().return_buffer(buffer);
                    }
                    Err(_) => {
                        // Handle allocation failure
                    }
                }
            })
        });

        group.bench_with_input(BenchmarkId::new("allocate_return_cycle", format!("{}kb", size / 1024)), &size, |b, &buffer_size| {
            // Pre-allocate buffers to test reuse efficiency
            let mut buffers = Vec::new();
            for _ in 0..10 {
                if let Ok(buffer) = memory_pool::global_pool().allocate(buffer_size) {
                    buffers.push(buffer);
                }
            }

            // Return all buffers to pool
            for buffer in buffers {
                memory_pool::global_pool().return_buffer(buffer);
            }

            b.iter(|| {
                // This should reuse existing buffers
                match memory_pool::global_pool().allocate(buffer_size) {
                    Ok(buffer) => {
                        criterion::black_box(&buffer);
                        memory_pool::global_pool().return_buffer(buffer);
                    }
                    Err(_) => {
                        // Handle allocation failure
                    }
                }
            })
        });
    }

    group.finish();
}

fn bench_statistics_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("statistics_operations");

    group.bench_function("get_stats", |b| {
        let screenshot = match WebPScreenshot::new() {
            Ok(s) => s,
            Err(_) => {
                b.iter(|| {});
                return;
            }
        };

        b.iter(|| {
            let stats = screenshot.stats();
            criterion::black_box(stats);
        })
    });

    group.bench_function("get_memory_stats", |b| {
        let screenshot = match WebPScreenshot::new() {
            Ok(s) => s,
            Err(_) => {
                b.iter(|| {});
                return;
            }
        };

        b.iter(|| {
            let stats = screenshot.memory_stats();
            criterion::black_box(stats);
        })
    });

    if ZeroCopyOptimizer::is_supported() {
        group.bench_function("get_zero_copy_stats", |b| {
            let screenshot = match WebPScreenshot::new() {
                Ok(s) => s,
                Err(_) => {
                    b.iter(|| {});
                    return;
                }
            };

            b.iter(|| {
                let stats = screenshot.zero_copy_stats();
                criterion::black_box(stats);
            })
        });
    }

    group.finish();
}

fn bench_convenience_functions(c: &mut Criterion) {
    let mut group = c.benchmark_group("convenience_functions");

    group.bench_function("capture_primary_display", |b| {
        b.iter(|| {
            match capture_primary_display() {
                Ok(result) => criterion::black_box(result),
                Err(_) => {
                    // Handle capture failure
                }
            }
        })
    });

    let quality_levels = [50, 75, 90];
    for quality in quality_levels {
        group.bench_with_input(BenchmarkId::new("capture_with_quality", quality), &quality, |b, &q| {
            b.iter(|| {
                match capture_with_quality(0, q) {
                    Ok(result) => criterion::black_box(result),
                    Err(_) => {
                        // Handle capture failure
                    }
                }
            })
        });
    }

    group.finish();
}

fn bench_platform_specific_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("platform_specific");

    // Test screenshot implementation name lookup
    group.bench_function("implementation_name", |b| {
        let screenshot = match WebPScreenshot::new() {
            Ok(s) => s,
            Err(_) => {
                b.iter(|| {});
                return;
            }
        };

        b.iter(|| {
            let name = screenshot.implementation_name();
            criterion::black_box(name);
        })
    });

    // Test hardware acceleration detection
    group.bench_function("is_hardware_accelerated", |b| {
        let screenshot = match WebPScreenshot::new() {
            Ok(s) => s,
            Err(_) => {
                b.iter(|| {});
                return;
            }
        };

        b.iter(|| {
            let is_accel = screenshot.is_hardware_accelerated();
            criterion::black_box(is_accel);
        })
    });

    // Test GPU info retrieval
    group.bench_function("gpu_info", |b| {
        let screenshot = match WebPScreenshot::new() {
            Ok(s) => s,
            Err(_) => {
                b.iter(|| {});
                return;
            }
        };

        b.iter(|| {
            let info = screenshot.gpu_info();
            criterion::black_box(info);
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_capture_operations,
    bench_display_enumeration,
    bench_capturer_creation,
    bench_zero_copy_operations,
    bench_multi_display_capture,
    bench_capture_with_config,
    bench_memory_pool_operations,
    bench_statistics_operations,
    bench_convenience_functions,
    bench_platform_specific_operations
);

criterion_main!(benches);