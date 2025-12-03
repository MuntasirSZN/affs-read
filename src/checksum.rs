//! Checksum calculation functions.

use crate::constants::BLOCK_SIZE;

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
#[allow(dead_code)]
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
#[cfg(feature = "simd")]
#[inline]
fn normal_sum_slice_simd(buf: &[u8], checksum_offset: usize) -> u32 {
    let checksum_word = checksum_offset / 4;
    let num_words = buf.len() / 4;

    // Use SIMD for accumulation
    let mut sum_vec = u32x4::ZERO;
    let mut i = 0;
    let mut offset = 0;

    // Process 4 words at a time with SIMD
    while i + 4 <= num_words {
        // Check if any of the next 4 words is the checksum word
        let skip_0 = i == checksum_word;
        let skip_1 = i + 1 == checksum_word;
        let skip_2 = i + 2 == checksum_word;
        let skip_3 = i + 3 == checksum_word;

        let w0 = if skip_0 {
            0
        } else {
            u32::from_be_bytes([
                buf[offset],
                buf[offset + 1],
                buf[offset + 2],
                buf[offset + 3],
            ])
        };
        let w1 = if skip_1 {
            0
        } else {
            u32::from_be_bytes([
                buf[offset + 4],
                buf[offset + 5],
                buf[offset + 6],
                buf[offset + 7],
            ])
        };
        let w2 = if skip_2 {
            0
        } else {
            u32::from_be_bytes([
                buf[offset + 8],
                buf[offset + 9],
                buf[offset + 10],
                buf[offset + 11],
            ])
        };
        let w3 = if skip_3 {
            0
        } else {
            u32::from_be_bytes([
                buf[offset + 12],
                buf[offset + 13],
                buf[offset + 14],
                buf[offset + 15],
            ])
        };

        let words = u32x4::new([w0, w1, w2, w3]);
        sum_vec += words;

        i += 4;
        offset += 16;
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
            let word = u32::from_be_bytes([
                buf[offset],
                buf[offset + 1],
                buf[offset + 2],
                buf[offset + 3],
            ]);
            sum = sum.wrapping_add(word);
        }
        i += 1;
        offset += 4;
    }

    (sum as i32).wrapping_neg() as u32
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
#[allow(dead_code)]
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
#[cfg(feature = "simd")]
#[inline]
fn boot_sum_simd(buf: &[u8; 1024]) -> u32 {
    let mut sum: u32 = 0;

    // Process first word (index 0)
    let d = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]);
    let new_sum = sum.wrapping_add(d);
    sum = new_sum.wrapping_add((new_sum < sum) as u32);

    // Process words 2-255 in batches of 4 using SIMD (skip word at index 1)
    for i in (2..256).step_by(4) {
        let base = i * 4;

        let words = if i + 3 < 256 {
            u32x4::new([
                u32::from_be_bytes([buf[base], buf[base + 1], buf[base + 2], buf[base + 3]]),
                u32::from_be_bytes([buf[base + 4], buf[base + 5], buf[base + 6], buf[base + 7]]),
                u32::from_be_bytes([buf[base + 8], buf[base + 9], buf[base + 10], buf[base + 11]]),
                u32::from_be_bytes([
                    buf[base + 12],
                    buf[base + 13],
                    buf[base + 14],
                    buf[base + 15],
                ]),
            ])
        } else {
            // Handle remaining words
            let mut arr = [0u32; 4];
            for (j, item) in arr.iter_mut().enumerate().take((256 - i).min(4)) {
                let offset = base + j * 4;
                *item = u32::from_be_bytes([
                    buf[offset],
                    buf[offset + 1],
                    buf[offset + 2],
                    buf[offset + 3],
                ]);
            }
            u32x4::new(arr)
        };

        // Add each word with carry
        let words_array = words.to_array();
        for &d in &words_array {
            if d != 0 {
                let new_sum = sum.wrapping_add(d);
                sum = new_sum.wrapping_add((new_sum < sum) as u32);
            }
        }
    }

    !sum
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
#[allow(dead_code)]
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
#[cfg(feature = "simd")]
#[inline]
fn bitmap_sum_simd(buf: &[u8; BLOCK_SIZE]) -> u32 {
    let mut sum_vec = u32x4::ZERO;
    let mut offset = 4;

    // Process 4 words at a time with SIMD (words 1-124 in groups of 4)
    for _ in (1..125).step_by(4) {
        let words = u32x4::new([
            u32::from_be_bytes([
                buf[offset],
                buf[offset + 1],
                buf[offset + 2],
                buf[offset + 3],
            ]),
            u32::from_be_bytes([
                buf[offset + 4],
                buf[offset + 5],
                buf[offset + 6],
                buf[offset + 7],
            ]),
            u32::from_be_bytes([
                buf[offset + 8],
                buf[offset + 9],
                buf[offset + 10],
                buf[offset + 11],
            ]),
            u32::from_be_bytes([
                buf[offset + 12],
                buf[offset + 13],
                buf[offset + 14],
                buf[offset + 15],
            ]),
        ]);
        sum_vec -= words;
        offset += 16;
    }

    // Process remaining words 125, 126, 127
    let mut sum: u32 = 0;
    for i in 125..128 {
        let word = u32::from_be_bytes([buf[i * 4], buf[i * 4 + 1], buf[i * 4 + 2], buf[i * 4 + 3]]);
        sum = sum.wrapping_sub(word);
    }

    // Sum the SIMD lanes
    let sum_array = sum_vec.to_array();
    sum.wrapping_add(sum_array[0])
        .wrapping_add(sum_array[1])
        .wrapping_add(sum_array[2])
        .wrapping_add(sum_array[3])
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
