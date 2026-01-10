//! Spike Compression
//!
//! Run-length encoding and other compression techniques for sparse spike trains.
//! Achieves 4-8x memory reduction for typical neuromorphic workloads.
//!
//! Encoding formats:
//! - RLE: Run-length encoding for consecutive zeros
//! - Delta: Time differences between spikes
//! - Bitmap: Bit-packed spike presence

use heapless::Vec as HVec;

/// Maximum compressed size
const MAX_COMPRESSED: usize = 256;

/// Maximum raw spikes
const MAX_SPIKES: usize = 128;

/// Compression method
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompressionMethod {
    /// No compression
    None,
    /// Run-length encoding
    RLE,
    /// Delta encoding (time differences)
    Delta,
    /// Bitmap encoding (1 bit per time slot)
    Bitmap,
    /// Adaptive (choose best)
    Adaptive,
}

/// Compressed spike train
#[derive(Clone, Debug)]
pub struct CompressedSpikes {
    /// Compression method used
    pub method: CompressionMethod,
    /// Compressed data
    pub data: HVec<u8, MAX_COMPRESSED>,
    /// Original length (time slots)
    pub original_len: u16,
    /// Original spike count
    pub spike_count: u16,
}

impl CompressedSpikes {
    /// Create empty compressed spikes
    pub fn new() -> Self {
        Self {
            method: CompressionMethod::None,
            data: HVec::new(),
            original_len: 0,
            spike_count: 0,
        }
    }

    /// Get compression ratio (original / compressed)
    pub fn compression_ratio(&self) -> f32 {
        if self.data.is_empty() {
            return 1.0;
        }
        self.original_len as f32 / self.data.len() as f32
    }

    /// Get memory savings (0.0 to 1.0)
    pub fn memory_savings(&self) -> f32 {
        if self.original_len == 0 {
            return 0.0;
        }
        1.0 - (self.data.len() as f32 / self.original_len as f32)
    }
}

impl Default for CompressedSpikes {
    fn default() -> Self {
        Self::new()
    }
}

/// Spike train (raw format)
#[derive(Clone, Debug)]
pub struct SpikeTrain {
    /// Spike times (relative to start)
    pub times: HVec<u16, MAX_SPIKES>,
    /// Duration of recording (time slots)
    pub duration: u16,
}

impl SpikeTrain {
    /// Create empty spike train
    pub fn new(duration: u16) -> Self {
        Self {
            times: HVec::new(),
            duration,
        }
    }

    /// Add a spike at given time
    pub fn add_spike(&mut self, time: u16) -> bool {
        if time < self.duration && !self.times.is_full() {
            // Keep sorted
            let pos = self.times.iter().position(|&t| t > time).unwrap_or(self.times.len());

            // Insert at position (shift elements)
            if self.times.len() < MAX_SPIKES {
                let _ = self.times.push(0); // Extend
                for i in (pos + 1..self.times.len()).rev() {
                    self.times[i] = self.times[i - 1];
                }
                self.times[pos] = time;
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Get spike count
    pub fn spike_count(&self) -> usize {
        self.times.len()
    }

    /// Get sparsity (fraction of zeros)
    pub fn sparsity(&self) -> f32 {
        if self.duration == 0 {
            return 1.0;
        }
        1.0 - (self.times.len() as f32 / self.duration as f32)
    }

    /// Check if spike exists at time
    pub fn has_spike(&self, time: u16) -> bool {
        self.times.contains(&time)
    }
}

impl Default for SpikeTrain {
    fn default() -> Self {
        Self::new(0)
    }
}

/// Spike compressor
pub struct SpikeCompressor {
    method: CompressionMethod,
}

impl SpikeCompressor {
    /// Create a new spike compressor
    pub fn new(method: CompressionMethod) -> Self {
        Self { method }
    }

    /// Compress a spike train
    pub fn compress(&self, train: &SpikeTrain) -> CompressedSpikes {
        let method = if self.method == CompressionMethod::Adaptive {
            self.choose_best_method(train)
        } else {
            self.method
        };

        match method {
            CompressionMethod::None => self.compress_none(train),
            CompressionMethod::RLE => self.compress_rle(train),
            CompressionMethod::Delta => self.compress_delta(train),
            CompressionMethod::Bitmap => self.compress_bitmap(train),
            CompressionMethod::Adaptive => self.compress_rle(train), // Fallback
        }
    }

    /// Decompress to spike train
    pub fn decompress(&self, compressed: &CompressedSpikes) -> SpikeTrain {
        match compressed.method {
            CompressionMethod::None => self.decompress_none(compressed),
            CompressionMethod::RLE => self.decompress_rle(compressed),
            CompressionMethod::Delta => self.decompress_delta(compressed),
            CompressionMethod::Bitmap => self.decompress_bitmap(compressed),
            CompressionMethod::Adaptive => self.decompress_rle(compressed),
        }
    }

    /// Choose best compression method based on spike train characteristics
    fn choose_best_method(&self, train: &SpikeTrain) -> CompressionMethod {
        let sparsity = train.sparsity();
        let spike_count = train.spike_count();

        if spike_count == 0 {
            CompressionMethod::RLE // Very efficient for empty
        } else if sparsity > 0.95 {
            CompressionMethod::Delta // Best for very sparse
        } else if sparsity > 0.8 {
            CompressionMethod::RLE // Good for sparse
        } else if train.duration <= 256 {
            CompressionMethod::Bitmap // Efficient for short, dense
        } else {
            CompressionMethod::RLE // Default
        }
    }

    /// No compression - just store spike times
    fn compress_none(&self, train: &SpikeTrain) -> CompressedSpikes {
        let mut result = CompressedSpikes::new();
        result.method = CompressionMethod::None;
        result.original_len = train.duration;
        result.spike_count = train.times.len() as u16;

        // Store as u16 pairs (2 bytes per spike)
        for &time in train.times.iter() {
            let _ = result.data.push((time >> 8) as u8);
            let _ = result.data.push((time & 0xFF) as u8);
        }

        result
    }

    /// RLE compression
    ///
    /// Format: [count, value] pairs
    /// - count: number of consecutive time slots
    /// - value: 0 = no spikes, 1 = spike
    fn compress_rle(&self, train: &SpikeTrain) -> CompressedSpikes {
        let mut result = CompressedSpikes::new();
        result.method = CompressionMethod::RLE;
        result.original_len = train.duration;
        result.spike_count = train.times.len() as u16;

        if train.duration == 0 {
            return result;
        }

        let mut pos: u16 = 0;
        let mut spike_idx = 0;

        while pos < train.duration {
            // Count consecutive zeros
            let mut zero_count: u16 = 0;
            while pos < train.duration && (spike_idx >= train.times.len() || train.times[spike_idx] != pos) {
                zero_count += 1;
                pos += 1;
                if zero_count == 255 {
                    break;
                }
            }

            if zero_count > 0 {
                let _ = result.data.push(zero_count as u8);
                let _ = result.data.push(0); // No spike
            }

            // Count consecutive spikes (usually 1)
            let mut spike_count: u16 = 0;
            while pos < train.duration && spike_idx < train.times.len() && train.times[spike_idx] == pos {
                spike_count += 1;
                spike_idx += 1;
                pos += 1;
                if spike_count == 255 {
                    break;
                }
            }

            if spike_count > 0 {
                let _ = result.data.push(spike_count as u8);
                let _ = result.data.push(1); // Spike
            }
        }

        result
    }

    /// Delta encoding - store time differences between spikes
    fn compress_delta(&self, train: &SpikeTrain) -> CompressedSpikes {
        let mut result = CompressedSpikes::new();
        result.method = CompressionMethod::Delta;
        result.original_len = train.duration;
        result.spike_count = train.times.len() as u16;

        // Store duration first (2 bytes)
        let _ = result.data.push((train.duration >> 8) as u8);
        let _ = result.data.push((train.duration & 0xFF) as u8);

        if train.times.is_empty() {
            return result;
        }

        // First spike time (2 bytes)
        let first = train.times[0];
        let _ = result.data.push((first >> 8) as u8);
        let _ = result.data.push((first & 0xFF) as u8);

        // Subsequent deltas (variable length)
        let mut prev = first;
        for &time in train.times.iter().skip(1) {
            let delta = time - prev;

            if delta < 128 {
                // 1 byte
                let _ = result.data.push(delta as u8);
            } else {
                // 2 bytes (high bit set)
                let _ = result.data.push(0x80 | ((delta >> 8) as u8));
                let _ = result.data.push((delta & 0xFF) as u8);
            }

            prev = time;
        }

        result
    }

    /// Bitmap encoding - 1 bit per time slot
    fn compress_bitmap(&self, train: &SpikeTrain) -> CompressedSpikes {
        let mut result = CompressedSpikes::new();
        result.method = CompressionMethod::Bitmap;
        result.original_len = train.duration;
        result.spike_count = train.times.len() as u16;

        // Store duration (2 bytes)
        let _ = result.data.push((train.duration >> 8) as u8);
        let _ = result.data.push((train.duration & 0xFF) as u8);

        // Bitmap: ceil(duration / 8) bytes
        let num_bytes = (train.duration as usize + 7) / 8;
        let mut spike_idx = 0;

        for byte_idx in 0..num_bytes {
            let mut byte: u8 = 0;
            for bit in 0..8 {
                let time = (byte_idx * 8 + bit) as u16;
                if time < train.duration {
                    if spike_idx < train.times.len() && train.times[spike_idx] == time {
                        byte |= 1 << bit;
                        spike_idx += 1;
                    }
                }
            }
            let _ = result.data.push(byte);
        }

        result
    }

    /// Decompress from no compression
    fn decompress_none(&self, compressed: &CompressedSpikes) -> SpikeTrain {
        let mut train = SpikeTrain::new(compressed.original_len);

        let mut i = 0;
        while i + 1 < compressed.data.len() {
            let time = ((compressed.data[i] as u16) << 8) | (compressed.data[i + 1] as u16);
            let _ = train.times.push(time);
            i += 2;
        }

        train
    }

    /// Decompress from RLE
    fn decompress_rle(&self, compressed: &CompressedSpikes) -> SpikeTrain {
        let mut train = SpikeTrain::new(compressed.original_len);
        let mut pos: u16 = 0;

        let mut i = 0;
        while i + 1 < compressed.data.len() {
            let count = compressed.data[i] as u16;
            let value = compressed.data[i + 1];

            if value == 1 {
                // Spikes
                for _ in 0..count {
                    if pos < compressed.original_len {
                        let _ = train.times.push(pos);
                        pos += 1;
                    }
                }
            } else {
                // Zeros
                pos += count;
            }

            i += 2;
        }

        train
    }

    /// Decompress from delta encoding
    fn decompress_delta(&self, compressed: &CompressedSpikes) -> SpikeTrain {
        if compressed.data.len() < 2 {
            return SpikeTrain::new(0);
        }

        let duration = ((compressed.data[0] as u16) << 8) | (compressed.data[1] as u16);
        let mut train = SpikeTrain::new(duration);

        if compressed.data.len() < 4 {
            return train;
        }

        // First spike
        let first = ((compressed.data[2] as u16) << 8) | (compressed.data[3] as u16);
        let _ = train.times.push(first);
        let mut prev = first;

        // Read deltas
        let mut i = 4;
        while i < compressed.data.len() {
            let delta = if compressed.data[i] & 0x80 != 0 {
                // 2-byte delta
                if i + 1 >= compressed.data.len() {
                    break;
                }
                let d = (((compressed.data[i] & 0x7F) as u16) << 8) | (compressed.data[i + 1] as u16);
                i += 2;
                d
            } else {
                // 1-byte delta
                let d = compressed.data[i] as u16;
                i += 1;
                d
            };

            prev += delta;
            if prev < duration {
                let _ = train.times.push(prev);
            }
        }

        train
    }

    /// Decompress from bitmap
    fn decompress_bitmap(&self, compressed: &CompressedSpikes) -> SpikeTrain {
        if compressed.data.len() < 2 {
            return SpikeTrain::new(0);
        }

        let duration = ((compressed.data[0] as u16) << 8) | (compressed.data[1] as u16);
        let mut train = SpikeTrain::new(duration);

        for (byte_idx, &byte) in compressed.data.iter().skip(2).enumerate() {
            for bit in 0..8 {
                if byte & (1 << bit) != 0 {
                    let time = (byte_idx * 8 + bit) as u16;
                    if time < duration {
                        let _ = train.times.push(time);
                    }
                }
            }
        }

        train
    }
}

impl Default for SpikeCompressor {
    fn default() -> Self {
        Self::new(CompressionMethod::Adaptive)
    }
}

/// Batch spike compression statistics
#[derive(Clone, Copy, Debug, Default)]
pub struct CompressionStats {
    /// Total bytes before compression
    pub original_bytes: u32,
    /// Total bytes after compression
    pub compressed_bytes: u32,
    /// Number of trains compressed
    pub trains_compressed: u32,
    /// Best compression ratio achieved
    pub best_ratio: f32,
    /// Worst compression ratio
    pub worst_ratio: f32,
}

impl CompressionStats {
    /// Get average compression ratio
    pub fn average_ratio(&self) -> f32 {
        if self.compressed_bytes == 0 {
            1.0
        } else {
            self.original_bytes as f32 / self.compressed_bytes as f32
        }
    }

    /// Get total memory savings
    pub fn total_savings(&self) -> f32 {
        if self.original_bytes == 0 {
            0.0
        } else {
            1.0 - (self.compressed_bytes as f32 / self.original_bytes as f32)
        }
    }

    /// Update with new compression result
    pub fn record(&mut self, original: usize, compressed: usize) {
        self.original_bytes += original as u32;
        self.compressed_bytes += compressed as u32;
        self.trains_compressed += 1;

        let ratio = if compressed > 0 {
            original as f32 / compressed as f32
        } else {
            1.0
        };

        if self.trains_compressed == 1 {
            self.best_ratio = ratio;
            self.worst_ratio = ratio;
        } else {
            self.best_ratio = self.best_ratio.max(ratio);
            self.worst_ratio = self.worst_ratio.min(ratio);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spike_train() {
        let mut train = SpikeTrain::new(100);

        train.add_spike(10);
        train.add_spike(20);
        train.add_spike(50);

        assert_eq!(train.spike_count(), 3);
        assert!(train.has_spike(10));
        assert!(train.has_spike(20));
        assert!(!train.has_spike(15));
    }

    #[test]
    fn test_rle_compression() {
        let compressor = SpikeCompressor::new(CompressionMethod::RLE);

        let mut train = SpikeTrain::new(100);
        train.add_spike(10);
        train.add_spike(50);
        train.add_spike(90);

        let compressed = compressor.compress(&train);
        assert_eq!(compressed.spike_count, 3);
        assert!(compressed.compression_ratio() > 1.0);

        let decompressed = compressor.decompress(&compressed);
        assert_eq!(decompressed.spike_count(), 3);
        assert!(decompressed.has_spike(10));
        assert!(decompressed.has_spike(50));
        assert!(decompressed.has_spike(90));
    }

    #[test]
    fn test_delta_compression() {
        let compressor = SpikeCompressor::new(CompressionMethod::Delta);

        let mut train = SpikeTrain::new(1000);
        train.add_spike(100);
        train.add_spike(200);
        train.add_spike(300);

        let compressed = compressor.compress(&train);
        let decompressed = compressor.decompress(&compressed);

        assert_eq!(decompressed.spike_count(), 3);
        assert!(decompressed.has_spike(100));
        assert!(decompressed.has_spike(200));
        assert!(decompressed.has_spike(300));
    }

    #[test]
    fn test_bitmap_compression() {
        let compressor = SpikeCompressor::new(CompressionMethod::Bitmap);

        let mut train = SpikeTrain::new(64);
        train.add_spike(0);
        train.add_spike(7);
        train.add_spike(8);
        train.add_spike(63);

        let compressed = compressor.compress(&train);
        let decompressed = compressor.decompress(&compressed);

        assert_eq!(decompressed.spike_count(), 4);
        assert!(decompressed.has_spike(0));
        assert!(decompressed.has_spike(7));
        assert!(decompressed.has_spike(8));
        assert!(decompressed.has_spike(63));
    }

    #[test]
    fn test_adaptive_compression() {
        let compressor = SpikeCompressor::new(CompressionMethod::Adaptive);

        // Very sparse train - should use delta
        let mut sparse_train = SpikeTrain::new(10000);
        sparse_train.add_spike(1000);
        sparse_train.add_spike(5000);
        sparse_train.add_spike(9000);

        let compressed = compressor.compress(&sparse_train);
        assert!(compressed.compression_ratio() > 5.0);
    }

    #[test]
    fn test_compression_stats() {
        let mut stats = CompressionStats::default();

        stats.record(100, 20);
        stats.record(100, 25);
        stats.record(100, 30);

        assert_eq!(stats.trains_compressed, 3);
        assert!(stats.average_ratio() > 3.0);
        assert!(stats.total_savings() > 0.7);
    }

    #[test]
    fn test_empty_train() {
        let compressor = SpikeCompressor::new(CompressionMethod::RLE);

        let train = SpikeTrain::new(100);
        let compressed = compressor.compress(&train);
        let decompressed = compressor.decompress(&compressed);

        assert_eq!(decompressed.spike_count(), 0);
        assert_eq!(decompressed.duration, 100);
    }
}
