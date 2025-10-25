//! Ultra-high performance streaming pipeline for real-time capture and encoding
//!
//! Features:
//! - Multi-threaded capture and encoding
//! - Ring buffer for frame management
//! - Adaptive quality based on performance
//! - Frame dropping for consistent FPS

use crate::{
    capture::ScreenCapture,
    encoder::{WebPEncoder, simd::SimdConverter},
    error::{CaptureError, CaptureResult},
    memory_pool::MemoryPool,
    pipeline::zero_copy::ZeroCopyOptimizer,
    types::{RawImage, WebPConfig},
};

use crossbeam_channel::{bounded, Receiver, Sender};
use parking_lot::Mutex;
use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

/// Frame data in the pipeline
#[derive(Clone)]
struct Frame {
    #[allow(dead_code)]
    id: u64,
    image: RawImage,
    #[allow(dead_code)]
    timestamp: Instant,
    capture_duration: Duration,
}

/// Streaming pipeline configuration
#[derive(Debug, Clone)]
pub struct StreamingConfig {
    /// Target frames per second
    pub target_fps: u32,
    /// Maximum frames in buffer
    pub buffer_size: usize,
    /// Number of capture threads
    pub capture_threads: usize,
    /// Number of encoding threads
    pub encoding_threads: usize,
    /// Enable adaptive quality
    pub adaptive_quality: bool,
    /// Enable frame dropping
    pub allow_frame_drop: bool,
    /// Initial WebP configuration
    pub webp_config: WebPConfig,
    /// Use zero-copy optimizations
    pub use_zero_copy: bool,
    /// Use GPU encoding if available
    pub use_gpu: bool,
}

impl Default for StreamingConfig {
    fn default() -> Self {
        let cpu_count = num_cpus::get();

        Self {
            target_fps: 30,
            buffer_size: 60, // 2 seconds at 30fps
            capture_threads: 1,
            encoding_threads: (cpu_count / 2).max(2),
            adaptive_quality: true,
            allow_frame_drop: true,
            webp_config: WebPConfig::fast(),
            use_zero_copy: true,
            use_gpu: false,
        }
    }
}

/// Streaming statistics
#[derive(Debug, Clone, Default)]
pub struct StreamingStats {
    pub frames_captured: u64,
    pub frames_encoded: u64,
    pub frames_dropped: u64,
    pub bytes_encoded: u64,
    pub total_capture_time: Duration,
    pub total_encode_time: Duration,
    pub current_fps: f32,
    pub current_bitrate: u64,
    pub avg_capture_time: Duration,
    pub avg_encode_time: Duration,
}

/// Ultra streaming pipeline for high-performance capture
pub struct StreamingPipeline {
    config: StreamingConfig,
    capturer: Arc<Box<dyn ScreenCapture>>,
    running: Arc<AtomicBool>,
    stats: Arc<Mutex<StreamingStats>>,
    frame_counter: Arc<AtomicU64>,
    memory_pool: Arc<MemoryPool>,
    zero_copy: Arc<ZeroCopyOptimizer>,
    #[allow(dead_code)]
    simd_converter: Arc<SimdConverter>,
}

impl StreamingPipeline {
    /// Create a new streaming pipeline
    pub fn new(
        capturer: Box<dyn ScreenCapture>,
        config: StreamingConfig,
    ) -> Self {
        Self {
            config,
            capturer: Arc::new(capturer),
            running: Arc::new(AtomicBool::new(false)),
            stats: Arc::new(Mutex::new(StreamingStats::default())),
            frame_counter: Arc::new(AtomicU64::new(0)),
            memory_pool: MemoryPool::new(),
            zero_copy: Arc::new(ZeroCopyOptimizer::new()),
            simd_converter: Arc::new(SimdConverter::new()),
        }
    }

    /// Start the streaming pipeline
    pub fn start<F>(&self, callback: F) -> CaptureResult<()>
    where
        F: FnMut(Vec<u8>) + Send + 'static,
    {
        if self.running.load(Ordering::Relaxed) {
            return Err(CaptureError::CaptureFailed(
                "Pipeline already running".to_string(),
            ));
        }

        self.running.store(true, Ordering::Relaxed);

        // Create channels for frame passing
        let (capture_tx, capture_rx) = bounded::<Frame>(self.config.buffer_size);
        let (encode_tx, encode_rx) = bounded::<Vec<u8>>(self.config.buffer_size);

        // Start capture thread(s)
        self.start_capture_threads(capture_tx);

        // Start encoding threads
        self.start_encoding_threads(capture_rx, encode_tx);

        // Start output thread
        self.start_output_thread(encode_rx, callback);

        // Start statistics thread
        self.start_stats_thread();

        Ok(())
    }

    /// Stop the streaming pipeline
    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }

    /// Check if pipeline is running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Get current statistics
    pub fn stats(&self) -> StreamingStats {
        self.stats.lock().clone()
    }

    /// Start capture threads
    fn start_capture_threads(&self, tx: Sender<Frame>) {
        for _thread_id in 0..self.config.capture_threads {
            let capturer = Arc::clone(&self.capturer);
            let running = Arc::clone(&self.running);
            let frame_counter = Arc::clone(&self.frame_counter);
            let _memory_pool = Arc::clone(&self.memory_pool);
            let zero_copy = Arc::clone(&self.zero_copy);
            let tx = tx.clone();
            let target_fps = self.config.target_fps;
            let use_zero_copy = self.config.use_zero_copy;

            thread::spawn(move || {
                let frame_duration = Duration::from_micros(1_000_000 / target_fps as u64);
                let mut next_frame_time = Instant::now();

                while running.load(Ordering::Relaxed) {
                    let capture_start = Instant::now();

                    // Capture frame
                    let image = if use_zero_copy {
                        zero_copy.capture_zero_copy(&**capturer, 0)
                    } else {
                        capturer.capture_display(0)
                    };

                    if let Ok(image) = image {
                        let capture_duration = capture_start.elapsed();
                        let frame_id = frame_counter.fetch_add(1, Ordering::Relaxed);

                        let frame = Frame {
                            id: frame_id,
                            image,
                            timestamp: Instant::now(),
                            capture_duration,
                        };

                        // Send frame to encoding pipeline
                        if tx.send(frame).is_err() {
                            // Channel full or closed
                            break;
                        }
                    }

                    // Maintain target FPS
                    next_frame_time += frame_duration;
                    let now = Instant::now();
                    if next_frame_time > now {
                        thread::sleep(next_frame_time - now);
                    }
                }
            });
        }
    }

    /// Start encoding threads
    fn start_encoding_threads(&self, rx: Receiver<Frame>, tx: Sender<Vec<u8>>) {
        for _thread_id in 0..self.config.encoding_threads {
            let rx = rx.clone();
            let tx = tx.clone();
            let running = Arc::clone(&self.running);
            let stats = Arc::clone(&self.stats);
            let webp_config = self.config.webp_config.clone();
            let adaptive_quality = self.config.adaptive_quality;
            let allow_frame_drop = self.config.allow_frame_drop;

            thread::spawn(move || {
                let mut encoder = WebPEncoder::new();
                let mut current_config = webp_config;

                while running.load(Ordering::Relaxed) {
                    // Receive frame
                    let frame = match rx.recv_timeout(Duration::from_millis(100)) {
                        Ok(frame) => frame,
                        Err(_) => continue,
                    };

                    let encode_start = Instant::now();

                    // Check if frame should be dropped
                    if allow_frame_drop && rx.len() > 10 {
                        // Skip encoding if buffer is backing up
                        let mut stats = stats.lock();
                        stats.frames_dropped += 1;
                        continue;
                    }

                    // Adaptive quality adjustment
                    if adaptive_quality {
                        current_config = Self::adjust_quality(
                            current_config,
                            frame.capture_duration,
                            rx.len(),
                        );
                    }

                    // Encode frame
                    match encoder.encode(&frame.image, &current_config) {
                        Ok(webp_data) => {
                            let encode_duration = encode_start.elapsed();

                            // Update stats
                            {
                                let mut stats = stats.lock();
                                stats.frames_encoded += 1;
                                stats.bytes_encoded += webp_data.len() as u64;
                                stats.total_encode_time += encode_duration;
                            }

                            // Send encoded frame
                            if tx.send(webp_data).is_err() {
                                break;
                            }
                        }
                        Err(e) => {
                            eprintln!("Encoding error: {}", e);
                        }
                    }
                }
            });
        }
    }

    /// Start output thread
    fn start_output_thread<F>(&self, rx: Receiver<Vec<u8>>, mut callback: F)
    where
        F: FnMut(Vec<u8>) + Send + 'static,
    {
        let running = Arc::clone(&self.running);

        thread::spawn(move || {
            while running.load(Ordering::Relaxed) {
                match rx.recv_timeout(Duration::from_millis(100)) {
                    Ok(data) => callback(data),
                    Err(_) => continue,
                }
            }
        });
    }

    /// Start statistics thread
    fn start_stats_thread(&self) {
        let running = Arc::clone(&self.running);
        let stats = Arc::clone(&self.stats);
        let frame_counter = Arc::clone(&self.frame_counter);

        thread::spawn(move || {
            let mut last_frame_count = 0u64;
            let mut last_bytes = 0u64;
            let mut last_time = Instant::now();

            while running.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_secs(1));

                let current_frames = frame_counter.load(Ordering::Relaxed);
                let elapsed = last_time.elapsed();

                let mut stats = stats.lock();

                // Calculate FPS
                let frames_delta = current_frames - last_frame_count;
                stats.current_fps = frames_delta as f32 / elapsed.as_secs_f32();

                // Calculate bitrate
                let bytes_delta = stats.bytes_encoded - last_bytes;
                stats.current_bitrate = (bytes_delta * 8) / elapsed.as_secs().max(1);

                // Calculate averages
                if stats.frames_captured > 0 {
                    stats.avg_capture_time =
                        stats.total_capture_time / stats.frames_captured as u32;
                }
                if stats.frames_encoded > 0 {
                    stats.avg_encode_time =
                        stats.total_encode_time / stats.frames_encoded as u32;
                }

                stats.frames_captured = current_frames;

                last_frame_count = current_frames;
                last_bytes = stats.bytes_encoded;
                last_time = Instant::now();
            }
        });
    }

    /// Adjust quality based on performance
    fn adjust_quality(
        mut config: WebPConfig,
        capture_duration: Duration,
        buffer_depth: usize,
    ) -> WebPConfig {
        // If capture is slow or buffer is filling, reduce quality
        if capture_duration > Duration::from_millis(20) || buffer_depth > 30 {
            config.quality = (config.quality - 5).max(60);
            config.method = (config.method - 1).max(0);
        } else if capture_duration < Duration::from_millis(10) && buffer_depth < 10 {
            // If performance is good, increase quality
            config.quality = (config.quality + 2).min(90);
            config.method = (config.method + 1).min(4);
        }

        config
    }
}

/// Builder for streaming pipeline
pub struct StreamingPipelineBuilder {
    config: StreamingConfig,
}

impl StreamingPipelineBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            config: StreamingConfig::default(),
        }
    }

    /// Set target FPS
    pub fn target_fps(mut self, fps: u32) -> Self {
        self.config.target_fps = fps;
        self
    }

    /// Set buffer size
    pub fn buffer_size(mut self, size: usize) -> Self {
        self.config.buffer_size = size;
        self
    }

    /// Set number of capture threads
    pub fn capture_threads(mut self, count: usize) -> Self {
        self.config.capture_threads = count;
        self
    }

    /// Set number of encoding threads
    pub fn encoding_threads(mut self, count: usize) -> Self {
        self.config.encoding_threads = count;
        self
    }

    /// Enable adaptive quality
    pub fn adaptive_quality(mut self, enabled: bool) -> Self {
        self.config.adaptive_quality = enabled;
        self
    }

    /// Enable frame dropping
    pub fn allow_frame_drop(mut self, enabled: bool) -> Self {
        self.config.allow_frame_drop = enabled;
        self
    }

    /// Set WebP configuration
    pub fn webp_config(mut self, config: WebPConfig) -> Self {
        self.config.webp_config = config;
        self
    }

    /// Enable zero-copy
    pub fn use_zero_copy(mut self, enabled: bool) -> Self {
        self.config.use_zero_copy = enabled;
        self
    }

    /// Enable GPU encoding
    pub fn use_gpu(mut self, enabled: bool) -> Self {
        self.config.use_gpu = enabled;
        self
    }

    /// Build the pipeline
    pub fn build(self, capturer: Box<dyn ScreenCapture>) -> StreamingPipeline {
        StreamingPipeline::new(capturer, self.config)
    }
}

impl Default for StreamingPipelineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_builder() {
        let config = StreamingPipelineBuilder::new()
            .target_fps(60)
            .buffer_size(120)
            .capture_threads(2)
            .encoding_threads(4)
            .adaptive_quality(true)
            .build(crate::capture::Capturer::new().unwrap());

        assert_eq!(config.config.target_fps, 60);
        assert_eq!(config.config.buffer_size, 120);
    }

    #[test]
    fn test_quality_adjustment() {
        let config = WebPConfig {
            quality: 80,
            method: 4,
            ..Default::default()
        };

        // Test quality reduction
        let adjusted = StreamingPipeline::adjust_quality(
            config.clone(),
            Duration::from_millis(25),
            40,
        );
        assert!(adjusted.quality < config.quality);

        // Test quality increase
        let adjusted = StreamingPipeline::adjust_quality(
            config.clone(),
            Duration::from_millis(5),
            5,
        );
        assert!(adjusted.quality > config.quality);
    }
}