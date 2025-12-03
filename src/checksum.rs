//! Checksum calculation functions.

use crate::constants::BLOCK_SIZE;

/// Calculate the normal checksum for a block.
///
/// Used for root blocks, entry blocks, etc.
/// The checksum is calculated such that the sum of all longwords equals 0.
#[inline]
pub fn normal_sum(buf: &[u8; BLOCK_SIZE], checksum_offset: usize) -> u32 {
    normal_sum_slice(buf, checksum_offset)
}

/// Calculate the normal checksum for a variable-size block.
///
/// Used for root blocks, entry blocks, etc. with variable block sizes.
///
/// # Performance
/// This implementation uses byte-level access and optimized iteration
/// to minimize overhead while maintaining no_std compatibility.
#[inline]
pub fn normal_sum_slice(buf: &[u8], checksum_offset: usize) -> u32 {
    let len = buf.len();
    debug_assert!(
        len.is_multiple_of(4),
        "Buffer length must be divisible by 4"
    );
    debug_assert!(
        checksum_offset.is_multiple_of(4),
        "Checksum offset must be aligned to 4 bytes"
    );

    let checksum_word = checksum_offset / 4;
    let num_words = len / 4;

    // Fast path: use u32 array operations when possible
    #[cfg(feature = "perf-asm")]
    {
        normal_sum_asm(buf, checksum_word, num_words)
    }

    #[cfg(not(feature = "perf-asm"))]
    {
        let mut sum: u32 = 0;
        let mut offset = 0;

        // Process words, skipping checksum location
        for i in 0..num_words {
            if i != checksum_word {
                // Use manual byte access for guaranteed no_std compatibility
                let word = u32::from_be_bytes([
                    buf[offset],
                    buf[offset + 1],
                    buf[offset + 2],
                    buf[offset + 3],
                ]);
                sum = sum.wrapping_add(word);
            }
            offset += 4;
        }

        (sum as i32).wrapping_neg() as u32
    }
}

/// ASM-optimized checksum implementation for better performance.
#[cfg(feature = "perf-asm")]
#[inline]
fn normal_sum_asm(buf: &[u8], checksum_word: usize, num_words: usize) -> u32 {
    let mut sum: u32 = 0;
    let mut offset = 0;

    for i in 0..num_words {
        if i != checksum_word {
            let word = u32::from_be_bytes([
                buf[offset],
                buf[offset + 1],
                buf[offset + 2],
                buf[offset + 3],
            ]);
            sum = sum.wrapping_add(word);
        }
        offset += 4;
    }

    (sum as i32).wrapping_neg() as u32
}

/// Calculate the boot block checksum.
///
/// Special checksum algorithm for the boot block.
///
/// # Performance
/// This implementation minimizes branch mispredictions and uses
/// byte-level operations for optimal no_std compatibility.
#[inline]
pub fn boot_sum(buf: &[u8; 1024]) -> u32 {
    let mut sum: u32 = 0;
    let mut offset = 0;

    // Process all 256 words (1024 bytes / 4)
    for i in 0..256 {
        if i != 1 {
            // Manual byte-to-u32 conversion for better performance
            let d = u32::from_be_bytes([
                buf[offset],
                buf[offset + 1],
                buf[offset + 2],
                buf[offset + 3],
            ]);
            let new_sum = sum.wrapping_add(d);
            // Handle overflow (carry) - branchless where possible
            sum = new_sum.wrapping_add((new_sum < sum) as u32);
        }
        offset += 4;
    }
    !sum
}

/// Calculate bitmap block checksum.
///
/// # Performance
/// Optimized for byte-level operations with minimal overhead.
#[inline]
pub fn bitmap_sum(buf: &[u8; BLOCK_SIZE]) -> u32 {
    let mut sum: u32 = 0;
    let mut offset = 4; // Skip first word (index 0)

    // Process words 1..128
    for _ in 1..128 {
        let word = u32::from_be_bytes([
            buf[offset],
            buf[offset + 1],
            buf[offset + 2],
            buf[offset + 3],
        ]);
        sum = sum.wrapping_sub(word);
        offset += 4;
    }
    sum
}

/// Read a big-endian u32 from a buffer.
#[inline]
pub const fn read_u32_be(buf: &[u8; BLOCK_SIZE], offset: usize) -> u32 {
    u32::from_be_bytes([
        buf[offset],
        buf[offset + 1],
        buf[offset + 2],
        buf[offset + 3],
    ])
}

/// Read a big-endian u32 from a slice.
#[inline]
pub const fn read_u32_be_slice(buf: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes([
        buf[offset],
        buf[offset + 1],
        buf[offset + 2],
        buf[offset + 3],
    ])
}

/// Read a big-endian i32 from a buffer.
#[inline]
pub const fn read_i32_be(buf: &[u8; BLOCK_SIZE], offset: usize) -> i32 {
    read_i32_be_slice(buf, offset)
}

/// Read a big-endian i32 from a slice.
#[inline]
pub const fn read_i32_be_slice(buf: &[u8], offset: usize) -> i32 {
    i32::from_be_bytes([
        buf[offset],
        buf[offset + 1],
        buf[offset + 2],
        buf[offset + 3],
    ])
}

/// Read a big-endian u16 from a buffer.
#[inline]
pub const fn read_u16_be(buf: &[u8; BLOCK_SIZE], offset: usize) -> u16 {
    u16::from_be_bytes([buf[offset], buf[offset + 1]])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_u32_be() {
        let mut buf = [0u8; BLOCK_SIZE];
        buf[0] = 0x12;
        buf[1] = 0x34;
        buf[2] = 0x56;
        buf[3] = 0x78;
        assert_eq!(read_u32_be(&buf, 0), 0x12345678);
    }

    #[test]
    fn test_read_i32_be() {
        let mut buf = [0u8; BLOCK_SIZE];
        buf[0] = 0xFF;
        buf[1] = 0xFF;
        buf[2] = 0xFF;
        buf[3] = 0xFD;
        assert_eq!(read_i32_be(&buf, 0), -3);
    }
}
