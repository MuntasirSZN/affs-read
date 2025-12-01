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
#[inline]
pub fn normal_sum_slice(buf: &[u8], checksum_offset: usize) -> u32 {
    let mut sum: u32 = 0;
    for i in 0..(buf.len() / 4) {
        if i != checksum_offset / 4 {
            sum = sum.wrapping_add(read_u32_be_slice(buf, i * 4));
        }
    }
    (sum as i32).wrapping_neg() as u32
}

/// Calculate the boot block checksum.
///
/// Special checksum algorithm for the boot block.
#[inline]
pub fn boot_sum(buf: &[u8; 1024]) -> u32 {
    let mut sum: u32 = 0;
    for i in 0..256 {
        if i != 1 {
            let d = read_u32_be_slice(buf, i * 4);
            let new_sum = sum.wrapping_add(d);
            // Handle overflow (carry)
            if new_sum < sum {
                sum = new_sum.wrapping_add(1);
            } else {
                sum = new_sum;
            }
        }
    }
    !sum
}

/// Calculate bitmap block checksum.
#[inline]
pub fn bitmap_sum(buf: &[u8; BLOCK_SIZE]) -> u32 {
    let mut sum: u32 = 0;
    for i in 1..128 {
        sum = sum.wrapping_sub(read_u32_be(buf, i * 4));
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
