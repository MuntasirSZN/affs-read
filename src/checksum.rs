//! Checksum calculation functions.

use crate::constants::BLOCK_SIZE;

#[cfg(feature = "simd")]
use bytemuck::try_cast_slice;
#[cfg(feature = "simd")]
use wide::u32x4;

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

    #[cfg(feature = "simd")]
    {
        normal_sum_slice_simd(buf, checksum_offset)
    }

    #[cfg(not(feature = "simd"))]
    {
        normal_sum_slice_scalar(buf, checksum_offset)
    }
}

/// Scalar implementation of normal_sum_slice.
#[inline]
fn normal_sum_slice_scalar(buf: &[u8], checksum_offset: usize) -> u32 {
    let checksum_word = checksum_offset / 4;
    let num_words = buf.len() / 4;

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

/// SIMD-optimized implementation of normal_sum_slice.
///
/// Uses bytemuck for safe byte slice casting when alignment permits,
/// falls back to scalar implementation otherwise.
#[cfg(feature = "simd")]
#[inline]
fn normal_sum_slice_simd(buf: &[u8], checksum_offset: usize) -> u32 {
    // Try to use bytemuck for aligned access when possible
    if let Ok(words_slice) = try_cast_slice::<u8, u32>(buf) {
        let checksum_word = checksum_offset / 4;
        let num_words = buf.len() / 4;

        // Use SIMD for accumulation
        let mut sum_vec = u32x4::ZERO;
        let mut i = 0;

        // Aligned path: use bytemuck-cast slice
        while i + 4 <= num_words {
            let skip_0 = i == checksum_word;
            let skip_1 = i + 1 == checksum_word;
            let skip_2 = i + 2 == checksum_word;
            let skip_3 = i + 3 == checksum_word;

            let w0 = if skip_0 {
                0
            } else {
                u32::from_be(words_slice[i])
            };
            let w1 = if skip_1 {
                0
            } else {
                u32::from_be(words_slice[i + 1])
            };
            let w2 = if skip_2 {
                0
            } else {
                u32::from_be(words_slice[i + 2])
            };
            let w3 = if skip_3 {
                0
            } else {
                u32::from_be(words_slice[i + 3])
            };

            let words = u32x4::new([w0, w1, w2, w3]);
            sum_vec += words;
            i += 4;
        }

        // Sum the SIMD lanes
        let sum_array = sum_vec.to_array();
        let mut sum: u32 = sum_array[0]
            .wrapping_add(sum_array[1])
            .wrapping_add(sum_array[2])
            .wrapping_add(sum_array[3]);

        // Process remaining words
        while i < num_words {
            if i != checksum_word {
                let word = u32::from_be(words_slice[i]);
                sum = sum.wrapping_add(word);
            }
            i += 1;
        }

        (sum as i32).wrapping_neg() as u32
    } else {
        // Unaligned fallback: use scalar implementation
        normal_sum_slice_scalar(buf, checksum_offset)
    }
}

/// Calculate the boot block checksum.
///
/// Special checksum algorithm for the boot block.
#[inline]
pub fn boot_sum(buf: &[u8; 1024]) -> u32 {
    #[cfg(feature = "simd")]
    {
        boot_sum_simd(buf)
    }

    #[cfg(not(feature = "simd"))]
    {
        boot_sum_scalar(buf)
    }
}

/// Scalar implementation of boot_sum.
#[inline]
fn boot_sum_scalar(buf: &[u8; 1024]) -> u32 {
    let mut sum: u32 = 0;
    let mut offset = 0;

    for i in 0..256 {
        if i != 1 {
            let d = u32::from_be_bytes([
                buf[offset],
                buf[offset + 1],
                buf[offset + 2],
                buf[offset + 3],
            ]);
            let new_sum = sum.wrapping_add(d);
            sum = new_sum.wrapping_add((new_sum < sum) as u32);
        }
        offset += 4;
    }
    !sum
}

/// SIMD-optimized implementation of boot_sum.
///
/// Uses bytemuck for safe byte slice casting when alignment permits,
/// falls back to scalar implementation otherwise.
#[cfg(feature = "simd")]
#[inline]
fn boot_sum_simd(buf: &[u8; 1024]) -> u32 {
    // Try to use bytemuck for aligned access when possible
    if let Ok(words_slice) = try_cast_slice::<u8, u32>(buf) {
        let mut sum: u32 = 0;

        // Aligned path: use bytemuck-cast slice
        // Process first word (index 0)
        let d = u32::from_be(words_slice[0]);
        let new_sum = sum.wrapping_add(d);
        sum = new_sum.wrapping_add((new_sum < sum) as u32);

        // Process words 2-255 in batches of 4 using SIMD (skip word at index 1)
        for i in (2..256).step_by(4) {
            let words = if i + 3 < 256 {
                u32x4::new([
                    u32::from_be(words_slice[i]),
                    u32::from_be(words_slice[i + 1]),
                    u32::from_be(words_slice[i + 2]),
                    u32::from_be(words_slice[i + 3]),
                ])
            } else {
                let mut arr = [0u32; 4];
                for (j, item) in arr.iter_mut().enumerate().take((256 - i).min(4)) {
                    *item = u32::from_be(words_slice[i + j]);
                }
                u32x4::new(arr)
            };

            let words_array = words.to_array();
            for &d in &words_array {
                if d != 0 {
                    let new_sum = sum.wrapping_add(d);
                    sum = new_sum.wrapping_add((new_sum < sum) as u32);
                }
            }
        }

        !sum
    } else {
        // Unaligned fallback: use scalar implementation
        boot_sum_scalar(buf)
    }
}

/// Calculate bitmap block checksum.
#[inline]
pub fn bitmap_sum(buf: &[u8; BLOCK_SIZE]) -> u32 {
    #[cfg(feature = "simd")]
    {
        bitmap_sum_simd(buf)
    }

    #[cfg(not(feature = "simd"))]
    {
        bitmap_sum_scalar(buf)
    }
}

/// Scalar implementation of bitmap_sum.
#[inline]
fn bitmap_sum_scalar(buf: &[u8; BLOCK_SIZE]) -> u32 {
    let mut sum: u32 = 0;
    let mut offset = 4;

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

/// SIMD-optimized implementation of bitmap_sum.
///
/// Uses bytemuck for safe byte slice casting when alignment permits,
/// falls back to scalar implementation otherwise.
#[cfg(feature = "simd")]
#[inline]
fn bitmap_sum_simd(buf: &[u8; BLOCK_SIZE]) -> u32 {
    // Try to use bytemuck for aligned access when possible
    if let Ok(words_slice) = try_cast_slice::<u8, u32>(buf) {
        let mut sum_vec = u32x4::ZERO;

        // Aligned path: use bytemuck-cast slice
        for i in (1..125).step_by(4) {
            let words = u32x4::new([
                u32::from_be(words_slice[i]),
                u32::from_be(words_slice[i + 1]),
                u32::from_be(words_slice[i + 2]),
                u32::from_be(words_slice[i + 3]),
            ]);
            sum_vec -= words;
        }

        // Process remaining words 125, 126, 127
        let mut sum: u32 = 0;
        for &word in &words_slice[125..128] {
            sum = sum.wrapping_sub(u32::from_be(word));
        }

        // Sum the SIMD lanes
        let sum_array = sum_vec.to_array();
        sum.wrapping_add(sum_array[0])
            .wrapping_add(sum_array[1])
            .wrapping_add(sum_array[2])
            .wrapping_add(sum_array[3])
    } else {
        // Unaligned fallback: use scalar implementation
        bitmap_sum_scalar(buf)
    }
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
