//! Streaming screenshot capture example with real-time monitoring

use webp_screenshot_rust::{WebPScreenshot, CaptureConfig, WebPConfig, CaptureRegion};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("WebP Screenshot Capture - Streaming Example");
    println!("============================================\n");

    // Create high-performance configuration
    let config = CaptureConfig {
        webp_config: WebPConfig::fast(),
        include_cursor: true,
        use_hardware_acceleration: true,
        max_retries: 1,
        retry_delay: Duration::from_millis(10),
        timeout: Duration::from_secs(1),
        ..Default::default()
    };

    let mut screenshot = WebPScreenshot::with_config(config)?;

    // Set up Ctrl+C handler
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        println!("\nStopping capture...");
        r.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");

    println!("Starting streaming capture (Press Ctrl+C to stop)");
    println!("Configuration:");
    println!("  Quality: {} (fast preset)", screenshot.config().webp_config.quality);
    println!("  Method: {}", screenshot.config().webp_config.method);
    println!("  Include Cursor: {}", screenshot.config().include_cursor);
    println!("  Hardware Acceleration: {}", screenshot.is_hardware_accelerated());
    println!();

    // Streaming parameters
    let target_fps = 10;
    let frame_duration = Duration::from_millis(1000 / target_fps);
    let mut frame_count = 0;
    let mut total_size = 0usize;
    let start_time = Instant::now();
    let mut last_stats_time = Instant::now();

    // Performance tracking
    let mut min_capture_time = Duration::MAX;
    let mut max_capture_time = Duration::ZERO;
    let mut total_capture_time = Duration::ZERO;

    // Optional: Define a capture region (e.g., top-left quarter of screen)
    let displays = screenshot.get_displays()?;
    if let Some(primary) = displays.first() {
        println!("Primary display: {}x{}", primary.width, primary.height);

        // Uncomment to capture only a region
        // let region = CaptureRegion::new(0, 0, primary.width / 2, primary.height / 2);
        // screenshot.config_mut().region = Some(region);
    }

    println!("Starting capture loop at {} FPS target...\n", target_fps);

    while running.load(Ordering::SeqCst) {
        let frame_start = Instant::now();

        // Capture frame
        match screenshot.capture_display(0) {
            Ok(result) => {
                frame_count += 1;
                total_size += result.size();

                // Track performance
                let capture_time = result.metadata.capture_duration;
                min_capture_time = min_capture_time.min(capture_time);
                max_capture_time = max_capture_time.max(capture_time);
                total_capture_time += capture_time;

                // Save every 10th frame (optional)
                if frame_count % 10 == 0 {
                    let filename = format!("stream_frame_{:04}.webp", frame_count);
                    if let Err(e) = result.save(&filename) {
                        eprintln!("Failed to save frame: {}", e);
                    }
                }

                // Print stats every second
                if last_stats_time.elapsed() >= Duration::from_secs(1) {
                    let elapsed = start_time.elapsed();
                    let actual_fps = frame_count as f64 / elapsed.as_secs_f64();
                    let avg_size = total_size / frame_count;
                    let avg_capture = total_capture_time / frame_count as u32;
                    let throughput_mbps = (total_size as f64 * 8.0) / elapsed.as_secs_f64() / 1_000_000.0;

                    // Clear previous line and print stats
                    print!("\r");
                    print!(
                        "Frame {} | FPS: {:.1} | Avg Size: {} KB | Capture: {:.1}ms | Throughput: {:.2} Mbps",
                        frame_count,
                        actual_fps,
                        avg_size / 1024,
                        avg_capture.as_secs_f64() * 1000.0,
                        throughput_mbps
                    );

                    use std::io::{self, Write};
                    io::stdout().flush().unwrap();

                    last_stats_time = Instant::now();
                }
            }
            Err(e) => {
                eprintln!("\nCapture error: {}", e);
                if !e.is_recoverable() {
                    break;
                }
            }
        }

        // Maintain target frame rate
        let frame_time = frame_start.elapsed();
        if frame_time < frame_duration {
            thread::sleep(frame_duration - frame_time);
        }
    }

    // Print final statistics
    println!("\n\n" + &"=".repeat(50));
    println!("Streaming Session Complete");
    println!(&"=".repeat(50));

    let total_duration = start_time.elapsed();
    let actual_fps = frame_count as f64 / total_duration.as_secs_f64();

    println!("\nSession Statistics:");
    println!("  Duration: {:?}", total_duration);
    println!("  Frames Captured: {}", frame_count);
    println!("  Average FPS: {:.2}", actual_fps);
    println!("  Total Data: {} MB", total_size / 1024 / 1024);
    println!(
        "  Average Frame Size: {} KB",
        total_size / frame_count / 1024
    );
    println!(
        "  Average Bitrate: {:.2} Mbps",
        (total_size as f64 * 8.0) / total_duration.as_secs_f64() / 1_000_000.0
    );

    println!("\nCapture Performance:");
    println!(
        "  Min Capture Time: {:.2}ms",
        min_capture_time.as_secs_f64() * 1000.0
    );
    println!(
        "  Max Capture Time: {:.2}ms",
        max_capture_time.as_secs_f64() * 1000.0
    );
    println!(
        "  Avg Capture Time: {:.2}ms",
        (total_capture_time / frame_count as u32).as_secs_f64() * 1000.0
    );

    // Memory statistics
    let mem_stats = screenshot.memory_stats();
    println!("\nMemory Pool Performance:");
    println!("  Buffer Reuse: {}", mem_stats.memory_reuse_count);
    println!(
        "  Hit Rate: {:.1}%",
        (mem_stats.buffer_hits as f64 / (mem_stats.buffer_hits + mem_stats.buffer_misses) as f64) * 100.0
    );
    println!(
        "  Peak Memory: {} MB",
        mem_stats.peak_memory_usage / 1024 / 1024
    );

    // Overall performance stats
    let stats = screenshot.stats();
    println!("\nOverall Performance:");
    println!("  Success Rate: {:.1}%", stats.success_rate());
    println!(
        "  Compression Ratio: {:.1}%",
        stats.average_compression_ratio() * 100.0
    );

    Ok(())
}

// Helper module for cross-platform Ctrl+C handling
mod ctrlc {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    pub fn set_handler<F>(handler: F) -> std::io::Result<()>
    where
        F: Fn() + Send + 'static,
    {
        // Simplified version - in production use the ctrlc crate
        std::thread::spawn(move || {
            // This is a placeholder - actual implementation would set up signal handlers
            std::thread::sleep(std::time::Duration::from_secs(60));
            handler();
        });
        Ok(())
    }
}