//! Lightweight obfuscation for the on-disk runtime dictionary cache.
//!
//! The runtime cache (`dictionary_runtime_cache.zst`) is gzip + bincode. If an
//! attacker obtains the file (e.g. from a dev build or a decrypted temp dir),
//! it is trivially decompressible. This module adds a cheap XOR-scramble layer
//! keyed by a compile-time constant so the file is not immediately readable.
//!
//! This is a deterrent, not a hard boundary: the key is a fixed constant in the
//! binary. Its purpose is to raise the cost of casual extraction. The cache is
//! additionally protected at rest by the FFI master-key encryption of the
//! embedded blob.

use std::io::{self, Read};

/// A fixed, obfuscated scramble key. Stored as a byte array so it does not
/// appear as a contiguous ASCII string in the binary.
const CACHE_SCRAMBLE_KEY: [u8; 32] = [
    0x3c, 0x8f, 0x1a, 0x7d, 0x52, 0xe4, 0x09, 0xb6, 0x2d, 0x71, 0xc8, 0x4f, 0x93, 0x0a, 0x65, 0xde,
    0x47, 0x1b, 0xa2, 0x8c, 0x30, 0xf5, 0x6e, 0x19, 0x84, 0x5b, 0xc1, 0x27, 0x9e, 0x40, 0x73, 0x0c,
];

/// XOR-scramble a byte buffer in place using the cache key.
///
/// The key is applied with a rolling offset so repeated bytes do not produce a
/// repeating pattern. This is a symmetric operation: applying it twice restores
/// the original data.
pub fn scramble(data: &mut [u8]) {
    for (i, byte) in data.iter_mut().enumerate() {
        *byte ^= scramble_mask(i);
    }
}

fn scramble_mask(position: usize) -> u8 {
    let key = CACHE_SCRAMBLE_KEY[position % CACHE_SCRAMBLE_KEY.len()];
    let offset = (position as u8).wrapping_mul(0x1f).wrapping_add(key);
    key.wrapping_add(offset)
}

/// Applies the same position-dependent XOR while bytes are read, avoiding a
/// second full-size input buffer during runtime cache deserialization.
pub struct ScrambleReader<R> {
    inner: R,
    position: usize,
}

impl<R> ScrambleReader<R> {
    pub fn new(inner: R) -> Self {
        Self { inner, position: 0 }
    }
}

impl<R: Read> Read for ScrambleReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let count = self.inner.read(buffer)?;
        for (offset, byte) in buffer[..count].iter_mut().enumerate() {
            *byte ^= scramble_mask(self.position + offset);
        }
        self.position += count;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scramble_is_symmetric() {
        let original = b"hello world, this is a test payload for the cache".to_vec();
        let mut data = original.clone();
        scramble(&mut data);
        // Scrambled data must differ from the original.
        assert_ne!(data, original, "scramble must change the data");
        // Applying scramble twice restores the original.
        scramble(&mut data);
        assert_eq!(data, original, "scramble must be symmetric");
    }

    #[test]
    fn scramble_handles_empty() {
        let mut data: Vec<u8> = Vec::new();
        scramble(&mut data);
        assert!(data.is_empty());
    }

    #[test]
    fn streaming_reader_matches_in_place_scramble_across_short_reads() {
        let original = b"streaming cache payload that spans several reads".to_vec();
        let mut scrambled = original.clone();
        scramble(&mut scrambled);
        let source = std::io::Cursor::new(scrambled);
        let mut reader = ScrambleReader::new(source);
        let mut restored = Vec::new();
        let mut chunk = [0u8; 7];
        loop {
            let count = reader.read(&mut chunk).expect("stream read");
            if count == 0 {
                break;
            }
            restored.extend_from_slice(&chunk[..count]);
        }
        assert_eq!(restored, original);
    }
}
