// Test multi-monitor capture
use webp_screenshot_rust::{WebPScreenshot, CaptureConfig, CaptureRegion};

fn main() {
    println!("Testing multi-monitor screenshot capture...\n");

    let config = CaptureConfig {
        include_cursor: false,
        ..Default::default()
    };

    let mut screenshot = WebPScreenshot::with_config(config).expect("Failed to create screenshot");

    // Get all displays
    let displays = screenshot.get_displays().expect("Failed to get displays");
    println!("Found {} display(s):", displays.len());
    for (i, display) in displays.iter().enumerate() {
        println!("  Display {}: {}x{} at ({}, {})",
                 i, display.width, display.height, display.x, display.y);
    }

    if displays.len() > 1 {
        println!("\n✅ Multiple monitors detected!");

        // Try to capture from the second monitor
        let display1 = &displays[1];
        println!("\nTesting capture from Display 1:");
        println!("  Position: ({}, {})", display1.x, display1.y);
        println!("  Size: {}x{}", display1.width, display1.height);

        // Capture a 400x400 region from near the top-left of the secondary monitor
        // Add a small offset to ensure we're clearly in the secondary monitor
        let test_x = display1.x + 100;
        let test_y = display1.y + 100;
        let region = CaptureRegion::new(test_x, test_y, 400, 400);

        println!("\nCapturing 400x400 test region:");
        println!("  x={}, y={}", region.x, region.y);

        let mut config_with_region = screenshot.config().clone();
        config_with_region.region = Some(region);
        screenshot.set_config(config_with_region);

        match screenshot.capture_display(0) {
            Ok(result) => {
                println!("\n✅ Capture successful!");
                println!("  Size: {} bytes", result.data.len());

                // Save to file
                let filename = format!("test_monitor1_at_{}_{}.webp", test_x, test_y);
                match result.save(&filename) {
                    Ok(_) => {
                        println!("  💾 Saved to: {}", filename);
                        println!("\n📋 Please verify the screenshot shows content from your SECONDARY monitor!");
                    }
                    Err(e) => println!("  ❌ Failed to save: {}", e),
                }
            }
            Err(e) => println!("\n❌ Capture failed: {}", e),
        }

        // Also test primary monitor for comparison
        println!("\n\nTesting capture from Display 0 (primary) for comparison:");
        let display0 = &displays[0];
        let test_x = display0.x + 100;
        let test_y = display0.y + 100;
        let region = CaptureRegion::new(test_x, test_y, 400, 400);

        let mut config_with_region = screenshot.config().clone();
        config_with_region.region = Some(region);
        screenshot.set_config(config_with_region);

        match screenshot.capture_display(0) {
            Ok(result) => {
                let filename = format!("test_monitor0_at_{}_{}.webp", test_x, test_y);
                match result.save(&filename) {
                    Ok(_) => println!("  💾 Saved primary monitor test to: {}", filename),
                    Err(e) => println!("  ❌ Failed to save: {}", e),
                }
            }
            Err(e) => println!("\n❌ Capture failed: {}", e),
        }
    } else {
        println!("\n⚠️  Only one monitor detected. Multi-monitor test skipped.");
    }
}
