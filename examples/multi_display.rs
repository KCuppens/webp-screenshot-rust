//! Multi-display screenshot capture example

use webp_screenshot_rust::{WebPScreenshot, CaptureConfig, WebPConfig};
use std::time::Instant;
use std::thread;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("WebP Screenshot Capture - Multi-Display Example");
    println!("================================================\n");

    // Create screenshot instance with custom configuration
    let config = CaptureConfig {
        webp_config: WebPConfig::balanced(),
        include_cursor: false,
        use_hardware_acceleration: true,
        max_retries: 3,
        retry_delay: Duration::from_millis(100),
        timeout: Duration::from_secs(5),
        ..Default::default()
    };

    let mut screenshot = WebPScreenshot::with_config(config)?;
    println!("Using implementation: {}", screenshot.implementation_name());
    println!(
        "Hardware acceleration: {}",
        if screenshot.is_hardware_accelerated() {
            "Enabled"
        } else {
            "Disabled"
        }
    );

    // Get all displays
    let displays = screenshot.get_displays()?;
    println!("\nDetected {} display(s):", displays.len());

    for display in &displays {
        println!("\nDisplay {} - '{}':", display.index, display.name);
        println!("  Resolution: {}x{}", display.width, display.height);
        println!("  Position: ({}, {})", display.x, display.y);
        println!("  Scale Factor: {}", display.scale_factor);
        println!("  Refresh Rate: {} Hz", display.refresh_rate);
        println!("  Color Depth: {} bits", display.color_depth);
        println!("  Primary: {}", display.is_primary);
        println!("  Total Pixels: {}", display.pixel_count());
    }

    // Capture all displays
    println!("\n" + &"=".repeat(50));
    println!("Capturing all displays...\n");

    let start_time = Instant::now();
    let results = screenshot.capture_all_displays();
    let total_time = start_time.elapsed();

    // Process results
    let mut success_count = 0;
    let mut total_size = 0;
    let mut total_original_size = 0;

    for (index, result) in results.iter().enumerate() {
        match result {
            Ok(capture) => {
                success_count += 1;
                total_size += capture.size();
                total_original_size += capture.metadata.original_size;

                println!("Display {} captured successfully:", index);
                println!("  Resolution: {}x{}", capture.width, capture.height);
                println!("  WebP Size: {} KB", capture.size() / 1024);
                println!(
                    "  Compression: {:.1}%",
                    capture.metadata.compression_ratio() * 100.0
                );
                println!(
                    "  Capture Time: {:.2}ms",
                    capture.metadata.capture_duration.as_secs_f64() * 1000.0
                );
                println!(
                    "  Encoding Time: {:.2}ms",
                    capture.metadata.encoding_duration.as_secs_f64() * 1000.0
                );

                // Save to file
                let filename = format!("screenshot_display_{}.webp", index);
                capture.save(&filename)?;
                println!("  Saved to: {}", filename);
            }
            Err(e) => {
                println!("Display {} capture failed: {}", index, e);
            }
        }
        println!();
    }

    // Summary
    println!(&"=".repeat(50));
    println!("Capture Summary:");
    println!("  Displays Captured: {}/{}", success_count, displays.len());
    println!("  Total Time: {:?}", total_time);
    println!(
        "  Average Time per Display: {:?}",
        total_time / displays.len() as u32
    );
    println!("  Total WebP Size: {} MB", total_size / 1024 / 1024);
    println!(
        "  Total Original Size: {} MB",
        total_original_size / 1024 / 1024
    );

    if total_original_size > 0 {
        let overall_compression = total_size as f64 / total_original_size as f64;
        println!(
            "  Overall Compression: {:.1}%",
            overall_compression * 100.0
        );
        println!(
            "  Space Saved: {} MB ({:.1}%)",
            (total_original_size - total_size) / 1024 / 1024,
            (1.0 - overall_compression) * 100.0
        );
    }

    // Test different quality settings
    println!("\n" + &"=".repeat(50));
    println!("Testing Different Quality Settings...\n");

    let quality_levels = vec![
        ("Low", WebPConfig::fast()),
        ("Default", WebPConfig::default()),
        ("High", WebPConfig::high_quality()),
        ("Lossless", WebPConfig::lossless()),
    ];

    for (name, config) in quality_levels {
        println!("Testing {} quality...", name);
        let start = Instant::now();

        match screenshot.capture_with_config(0, config) {
            Ok(result) => {
                let duration = start.elapsed();
                println!("  Size: {} KB", result.size() / 1024);
                println!(
                    "  Compression: {:.1}%",
                    result.metadata.compression_ratio() * 100.0
                );
                println!("  Time: {:?}", duration);

                let filename = format!("screenshot_quality_{}.webp", name.to_lowercase());
                result.save(&filename)?;
            }
            Err(e) => println!("  Failed: {}", e),
        }
    }

    // Final statistics
    let stats = screenshot.stats();
    println!("\n" + &"=".repeat(50));
    println!("Session Statistics:");
    println!("  Total Captures: {}", stats.total_captures);
    println!("  Successful: {}", stats.successful_captures);
    println!("  Failed: {}", stats.failed_captures);
    println!("  Success Rate: {:.1}%", stats.success_rate());
    println!("  Fastest Capture: {:?}", stats.fastest_capture);
    println!("  Slowest Capture: {:?}", stats.slowest_capture);
    println!(
        "  Average Time: {:?}",
        stats.average_capture_time()
    );

    Ok(())
}