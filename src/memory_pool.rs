//! Memory pool for efficient buffer management

use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::error::{MemoryPoolError, MemoryPoolResult};

/// A buffer that automatically returns to the pool when dropped
pub struct PooledBuffer {
    data: Option<Vec<u8>>,
    size: usize,
    pool: Option<Arc<MemoryPool>>,
}

impl PooledBuffer {
    /// Get the buffer data
    pub fn data(&self) -> &[u8] {
        self.data.as_ref().map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Get mutable buffer data
    pub fn data_mut(&mut self) -> &mut [u8] {
        self.data.as_mut().map(|v| v.as_mut_slice()).unwrap_or(&mut [])
    }

    /// Get the buffer size
    pub fn size(&self) -> usize {
        self.size
    }

    /// Take ownership of the buffer data (removes from pool management)
    pub fn into_vec(mut self) -> Vec<u8> {
        // CRITICAL: Decrement current_memory_usage since buffer is leaving pool management
        // The caller takes ownership and the memory will be freed by the OS when Vec<u8> is dropped
        if let Some(ref pool) = self.pool {
            pool.stats.current_memory_usage.fetch_sub(
                self.size,
                Ordering::Relaxed,
            );
        }

        self.pool = None; // Prevent returning to pool
        self.data.take().unwrap_or_default()
    }
}

impl Drop for PooledBuffer {
    fn drop(&mut self) {
        if let (Some(pool), Some(data)) = (self.pool.take(), self.data.take()) {
            pool.release_internal(data, self.size);
        }
    }
}

/// Buffer metadata for pool management
#[derive(Debug)]
struct BufferEntry {
    buffer: Vec<u8>,
    size: usize,
    last_used: Instant,
    use_count: u32,
}

impl BufferEntry {
    fn new(buffer: Vec<u8>, size: usize) -> Self {
        Self {
            buffer,
            size,
            last_used: Instant::now(),
            use_count: 0,
        }
    }

    fn matches_size(&self, requested_size: usize) -> bool {
        self.size >= requested_size && self.size <= requested_size * 2
    }

    fn is_expired(&self, timeout: Duration) -> bool {
        self.last_used.elapsed() > timeout
    }
}

/// Statistics for the memory pool
#[derive(Debug, Clone, Default)]
pub struct PoolStats {
    pub available_buffers: usize,
    pub total_buffers_created: u64,
    pub total_memory_allocated: usize,
    pub peak_memory_usage: usize,
    pub memory_reuse_count: u64,
    pub current_memory_usage: usize,  // Active (in-use) allocations only
    pub pooled_memory: usize,          // Memory held in pool buffers
    pub buffer_hits: u64,
    pub buffer_misses: u64,
}

/// Memory pool for efficient buffer reuse
pub struct MemoryPool {
    inner: Arc<Mutex<MemoryPoolInner>>,
    stats: Arc<PoolStatsAtomic>,
    config: PoolConfig,
}

struct MemoryPoolInner {
    buffers: VecDeque<BufferEntry>,
}

struct PoolStatsAtomic {
    total_buffers_created: AtomicU64,
    memory_reuse_count: AtomicU64,
    peak_memory_usage: AtomicUsize,
    current_memory_usage: AtomicUsize,
    buffer_hits: AtomicU64,
    buffer_misses: AtomicU64,
}

/// Configuration for the memory pool
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Maximum number of buffers to keep in the pool
    pub max_buffers: usize,
    /// Maximum total memory to keep allocated (bytes)
    pub max_memory: usize,
    /// Buffer expiration timeout
    pub buffer_timeout: Duration,
    /// Whether to pre-allocate buffers
    pub preallocate: bool,
    /// Default buffer size for pre-allocation
    pub default_buffer_size: usize,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_buffers: 10,
            max_memory: 500 * 1024 * 1024, // 500 MB (2x to account for OCR processing time)
            buffer_timeout: Duration::from_secs(60),
            preallocate: false,
            default_buffer_size: 1920 * 1080 * 4, // Full HD RGBA
        }
    }
}

impl MemoryPool {
    /// Create a new memory pool with default configuration
    pub fn new() -> Arc<Self> {
        Self::with_config(PoolConfig::default())
    }

    /// Create a new memory pool with custom configuration
    pub fn with_config(config: PoolConfig) -> Arc<Self> {
        let pool = Arc::new(Self {
            inner: Arc::new(Mutex::new(MemoryPoolInner {
                buffers: VecDeque::new(),
            })),
            stats: Arc::new(PoolStatsAtomic {
                total_buffers_created: AtomicU64::new(0),
                memory_reuse_count: AtomicU64::new(0),
                peak_memory_usage: AtomicUsize::new(0),
                current_memory_usage: AtomicUsize::new(0),
                buffer_hits: AtomicU64::new(0),
                buffer_misses: AtomicU64::new(0),
            }),
            config,
        });

        if pool.config.preallocate {
            pool.preallocate_buffers();
        }

        pool
    }

    /// Pre-allocate buffers for better performance
    fn preallocate_buffers(&self) {
        let mut inner = self.inner.lock();
        let num_buffers = self.config.max_buffers.min(4);

        for _ in 0..num_buffers {
            let buffer = vec![0u8; self.config.default_buffer_size];
            let entry = BufferEntry::new(buffer, self.config.default_buffer_size);
            inner.buffers.push_back(entry);

            self.stats
                .total_buffers_created
                .fetch_add(1, Ordering::Relaxed);
            // Pre-allocated buffers are in the pool, so they're not "active"
            // Don't add to current_memory_usage
        }
    }

    /// Acquire a buffer from the pool
    pub fn acquire(self: &Arc<Self>, size: usize) -> MemoryPoolResult<PooledBuffer> {
        if size == 0 {
            return Err(MemoryPoolError::InvalidBufferSize { size });
        }

        let mut inner = self.inner.lock();

        // Clean up expired buffers
        self.cleanup_expired_buffers(&mut inner);

        // Diagnostic logging for large allocations
        if size > 10 * 1024 * 1024 {
            let current = self.stats.current_memory_usage.load(Ordering::Relaxed);
            eprintln!(
                "[MemoryPool] Large allocation requested: {:.2} MB, current pool usage: {:.2} MB / {:.2} MB",
                size as f64 / (1024.0 * 1024.0),
                current as f64 / (1024.0 * 1024.0),
                self.config.max_memory as f64 / (1024.0 * 1024.0)
            );
        }

        // Try to find a suitable buffer
        if let Some(index) = self.find_suitable_buffer(&inner, size) {
            let mut entry = inner.buffers.remove(index).unwrap();
            entry.last_used = Instant::now();
            entry.use_count += 1;

            // When taking buffer from pool, it becomes "active" again
            // Increment current_memory_usage to track active allocations
            self.stats.current_memory_usage.fetch_add(
                entry.size,
                Ordering::Relaxed,
            );

            // Resize if necessary
            if entry.buffer.len() < size {
                let size_diff = size - entry.size;
                entry.buffer.resize(size, 0);
                self.stats.current_memory_usage.fetch_add(
                    size_diff,
                    Ordering::Relaxed,
                );
                entry.size = size;
            }

            self.stats.buffer_hits.fetch_add(1, Ordering::Relaxed);
            self.stats.memory_reuse_count.fetch_add(1, Ordering::Relaxed);

            return Ok(PooledBuffer {
                data: Some(entry.buffer),
                size,
                pool: Some(Arc::clone(self)),
            });
        }

        drop(inner); // Release lock before allocation

        // No suitable buffer found, allocate new one
        self.stats.buffer_misses.fetch_add(1, Ordering::Relaxed);
        self.allocate_new_buffer(size)
    }

    /// Find a suitable buffer in the pool
    fn find_suitable_buffer(&self, inner: &MemoryPoolInner, size: usize) -> Option<usize> {
        inner
            .buffers
            .iter()
            .position(|entry| entry.matches_size(size))
    }

    /// Allocate a new buffer
    fn allocate_new_buffer(self: &Arc<Self>, size: usize) -> MemoryPoolResult<PooledBuffer> {
        // Check if we can allocate more memory
        let current_memory = self.stats.current_memory_usage.load(Ordering::Relaxed);
        if current_memory + size > self.config.max_memory {
            // Pool is full - allow direct allocation without tracking in pool
            // This provides a fallback when pool is exhausted
            eprintln!(
                "[MemoryPool] Pool limit reached ({} MB), allocating {:.2} MB directly without pooling",
                self.config.max_memory / (1024 * 1024),
                size as f64 / (1024.0 * 1024.0)
            );

            let buffer = vec![0u8; size];
            return Ok(PooledBuffer {
                data: Some(buffer),
                size,
                pool: None, // No pool reference - buffer will be dropped normally
            });
        }

        let buffer = vec![0u8; size];
        self.stats
            .total_buffers_created
            .fetch_add(1, Ordering::Relaxed);
        self.stats
            .current_memory_usage
            .fetch_add(size, Ordering::Relaxed);
        self.update_peak_memory();

        Ok(PooledBuffer {
            data: Some(buffer),
            size,
            pool: Some(Arc::clone(self)),
        })
    }

    /// Internal method to release a buffer back to the pool
    fn release_internal(&self, buffer: Vec<u8>, size: usize) {
        let mut inner = self.inner.lock();

        // IMPORTANT: When buffer is returned to pool, it's no longer "active"
        // Decrement current_memory_usage as it only tracks ACTIVE allocations
        // NOTE: This is also done in into_vec() when buffer leaves pool management entirely
        // Pooled buffers don't count toward the limit for new allocations
        self.stats
            .current_memory_usage
            .fetch_sub(size, Ordering::Relaxed);

        // Check if we should keep this buffer in the pool
        if inner.buffers.len() >= self.config.max_buffers {
            // Pool is full, drop the buffer (already decremented memory above)
            return;
        }

        // Keep buffer in pool for reuse
        let entry = BufferEntry::new(buffer, size);
        inner.buffers.push_back(entry);
    }

    /// Clean up expired buffers
    fn cleanup_expired_buffers(&self, inner: &mut MemoryPoolInner) {
        let timeout = self.config.buffer_timeout;

        // Simply remove expired buffers from pool
        // No need to adjust current_memory_usage since pooled buffers aren't counted
        inner.buffers.retain(|entry| !entry.is_expired(timeout));
    }

    /// Update peak memory usage
    fn update_peak_memory(&self) {
        let current = self.stats.current_memory_usage.load(Ordering::Relaxed);
        let mut peak = self.stats.peak_memory_usage.load(Ordering::Relaxed);

        while current > peak {
            match self.stats.peak_memory_usage.compare_exchange(
                peak,
                current,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(x) => peak = x,
            }
        }
    }

    /// Clear all buffers from the pool
    pub fn clear(&self) {
        let mut inner = self.inner.lock();
        // Simply clear pooled buffers
        // No need to adjust current_memory_usage since pooled buffers aren't counted
        inner.buffers.clear();
    }

    /// Get current pool statistics
    pub fn stats(&self) -> PoolStats {
        let inner = self.inner.lock();

        let pooled_memory: usize = inner.buffers.iter().map(|e| e.size).sum();

        PoolStats {
            available_buffers: inner.buffers.len(),
            total_buffers_created: self.stats.total_buffers_created.load(Ordering::Relaxed),
            total_memory_allocated: pooled_memory + self.stats.current_memory_usage.load(Ordering::Relaxed),
            peak_memory_usage: self.stats.peak_memory_usage.load(Ordering::Relaxed),
            memory_reuse_count: self.stats.memory_reuse_count.load(Ordering::Relaxed),
            current_memory_usage: self.stats.current_memory_usage.load(Ordering::Relaxed),
            pooled_memory,
            buffer_hits: self.stats.buffer_hits.load(Ordering::Relaxed),
            buffer_misses: self.stats.buffer_misses.load(Ordering::Relaxed),
        }
    }

    /// Get hit rate percentage
    pub fn hit_rate(&self) -> f64 {
        let hits = self.stats.buffer_hits.load(Ordering::Relaxed);
        let misses = self.stats.buffer_misses.load(Ordering::Relaxed);
        let total = hits + misses;

        if total == 0 {
            0.0
        } else {
            (hits as f64 / total as f64) * 100.0
        }
    }
}

impl Default for MemoryPool {
    fn default() -> Self {
        Arc::try_unwrap(Self::new()).unwrap_or_else(|arc| (*arc).clone())
    }
}

impl Clone for MemoryPool {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            stats: Arc::clone(&self.stats),
            config: self.config.clone(),
        }
    }
}

/// Global memory pool instance
static GLOBAL_POOL: once_cell::sync::Lazy<Arc<MemoryPool>> =
    once_cell::sync::Lazy::new(|| MemoryPool::new());

/// Get the global memory pool instance
pub fn global_pool() -> Arc<MemoryPool> {
    Arc::clone(&GLOBAL_POOL)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_acquire_release() {
        let pool = MemoryPool::new();
        let buffer = pool.acquire(1024).unwrap();
        assert_eq!(buffer.size(), 1024);
        drop(buffer);

        let stats = pool.stats();
        assert_eq!(stats.available_buffers, 1);
    }

    #[test]
    fn test_pool_reuse() {
        let pool = MemoryPool::new();

        let buffer1 = pool.acquire(1024).unwrap();
        drop(buffer1);

        let buffer2 = pool.acquire(1024).unwrap();
        drop(buffer2);

        let stats = pool.stats();
        assert_eq!(stats.memory_reuse_count, 1);
        assert_eq!(stats.total_buffers_created, 1);
    }

    #[test]
    fn test_pool_size_matching() {
        let pool = MemoryPool::new();

        let buffer1 = pool.acquire(1024).unwrap();
        drop(buffer1);

        // Should reuse the 1024 buffer for 512 request
        let buffer2 = pool.acquire(512).unwrap();
        drop(buffer2);

        let stats = pool.stats();
        assert_eq!(stats.memory_reuse_count, 1);
    }

    #[test]
    fn test_hit_rate() {
        let pool = MemoryPool::new();

        let _buffer1 = pool.acquire(1024).unwrap();
        drop(_buffer1);
        let _buffer2 = pool.acquire(1024).unwrap();

        assert!(pool.hit_rate() > 0.0);
    }
}