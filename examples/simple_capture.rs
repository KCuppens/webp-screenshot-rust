//! Simple screenshot capture example

use webp_screenshot_rust::{WebPScreenshot, CaptureConfig, WebPConfig};
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logger
    env_logger::init();

    println!("WebP Screenshot Capture - Simple Example");
    println!("========================================\n");

    // Create screenshot instance with default configuration
    let mut screenshot = WebPScreenshot::new()?;

    // Get available displays
    let displays = screenshot.get_displays()?;
    println!("Found {} display(s):", displays.len());
    for display in &displays {
        println!(
            "  Display {}: {}x{} at ({}, {}), Primary: {}",
            display.index,
            display.width,
            display.height,
            display.x,
            display.y,
            display.is_primary
        );
    }
    println!();

    // Capture primary display
    println!("Capturing primary display...");
    let start = Instant::now();

    let result = screenshot.capture_display(0)?;

    let duration = start.elapsed();
    println!("Capture completed in {:?}", duration);

    // Display capture information
    println!("\nCapture Information:");
    println!("  Resolution: {}x{}", result.width, result.height);
    println!("  WebP Size: {} bytes", result.size());
    println!("  Original Size: {} bytes", result.metadata.original_size);
    println!(
        "  Compression Ratio: {:.2}%",
        result.metadata.compression_ratio() * 100.0
    );
    println!(
        "  Space Saved: {:.2}%",
        result.metadata.space_savings_percent()
    );
    println!(
        "  Capture Time: {:?}",
        result.metadata.capture_duration
    );
    println!(
        "  Encoding Time: {:?}",
        result.metadata.encoding_duration
    );
    println!(
        "  Implementation: {}",
        result.metadata.implementation
    );

    // Save to file
    let filename = "screenshot_simple.webp";
    result.save(filename)?;
    println!("\nScreenshot saved to: {}", filename);

    // Display performance statistics
    let stats = screenshot.stats();
    println!("\nPerformance Statistics:");
    println!("  Total Captures: {}", stats.total_captures);
    println!("  Success Rate: {:.2}%", stats.success_rate());
    println!(
        "  Average Capture Time: {:?}",
        stats.average_capture_time()
    );
    println!(
        "  Average Compression: {:.2}%",
        stats.average_compression_ratio() * 100.0
    );

    // Memory pool statistics
    let mem_stats = screenshot.memory_stats();
    println!("\nMemory Pool Statistics:");
    println!("  Available Buffers: {}", mem_stats.available_buffers);
    println!("  Total Buffers Created: {}", mem_stats.total_buffers_created);
    println!("  Memory Reuse Count: {}", mem_stats.memory_reuse_count);
    println!("  Current Memory Usage: {} MB", mem_stats.current_memory_usage / 1024 / 1024);
    println!("  Peak Memory Usage: {} MB", mem_stats.peak_memory_usage / 1024 / 1024);
    println!("  Hit Rate: {:.2}%", mem_stats.buffer_hits as f64 / (mem_stats.buffer_hits + mem_stats.buffer_misses) as f64 * 100.0);

    Ok(())
}