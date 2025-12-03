//! Symlink reading functionality.

use crate::constants::*;

/// Maximum symlink target length.
///
/// For a 512-byte block, symlink data starts at offset 24 and ends before
/// the file header structure at offset 312 (512 - 200), giving 288 bytes.
/// For larger block sizes, this grows proportionally.
pub const MAX_SYMLINK_LEN: usize = BLOCK_SIZE - SYMLINK_OFFSET - FILE_LOCATION;

/// Read symlink target from a block buffer.
///
/// The symlink target is stored as a Latin1 string starting at offset 24
/// (GRUB_AFFS_SYMLINK_OFFSET) in the entry block.
///
/// # Arguments
/// * `buf` - The entry block data (512 bytes for standard block size)
/// * `out` - Output buffer for UTF-8 converted target
///
/// # Returns
/// The number of bytes written to `out`, or an error.
///
/// # Notes
/// - The target is null-terminated in the block
/// - Latin1 characters are converted to UTF-8
/// - Leading `:` is replaced with `/` (Amiga volume reference)
pub fn read_symlink_target(buf: &[u8; BLOCK_SIZE], out: &mut [u8]) -> usize {
    read_symlink_target_with_block_size(buf, BLOCK_SIZE, out)
}

/// Read symlink target with variable block size support.
///
/// # Arguments
/// * `buf` - The entry block data
/// * `block_size` - The filesystem block size
/// * `out` - Output buffer for UTF-8 converted target
///
/// # Returns
/// The number of bytes written to `out`.
pub fn read_symlink_target_with_block_size(buf: &[u8], block_size: usize, out: &mut [u8]) -> usize {
    // Calculate symlink data region
    let symlink_start = SYMLINK_OFFSET;
    let symlink_end = block_size.saturating_sub(FILE_LOCATION);

    if symlink_start >= symlink_end || symlink_start >= buf.len() {
        return 0;
    }

    let symlink_end = symlink_end.min(buf.len());
    let latin1 = &buf[symlink_start..symlink_end];

    let len = memchr::memchr(0, latin1).unwrap_or(latin1.len());
    let latin1 = &latin1[..len];

    // Convert Latin1 to UTF-8 with `:` -> `/` replacement
    latin1_to_utf8_symlink(latin1, out)
}

/// Convert Latin1 bytes to UTF-8, replacing leading `:` with `/`.
///
/// In Amiga paths, `:` refers to the volume root. GRUB replaces this
/// with `/` for Unix compatibility.
///
/// # Arguments
/// * `latin1` - Input Latin1 bytes
/// * `out` - Output buffer for UTF-8
///
/// # Returns
/// Number of bytes written to `out`.
fn latin1_to_utf8_symlink(latin1: &[u8], out: &mut [u8]) -> usize {
    let mut out_pos = 0;

    for (i, &byte) in latin1.iter().enumerate() {
        // Replace leading `:` with `/`
        let byte = if i == 0 && byte == b':' { b'/' } else { byte };

        if byte < 0x80 {
            // ASCII - direct copy
            if out_pos >= out.len() {
                break;
            }
            out[out_pos] = byte;
            out_pos += 1;
        } else {
            // Latin1 high byte (0x80-0xFF) -> UTF-8 two-byte sequence
            // UTF-8: 110xxxxx 10xxxxxx
            if out_pos + 1 >= out.len() {
                break;
            }
            out[out_pos] = 0xC0 | (byte >> 6);
            out[out_pos + 1] = 0x80 | (byte & 0x3F);
            out_pos += 2;
        }
    }

    out_pos
}

/// Calculate maximum UTF-8 length for a Latin1 string.
///
/// Each Latin1 byte can expand to at most 2 UTF-8 bytes.
#[inline]
pub const fn max_utf8_len(latin1_len: usize) -> usize {
    latin1_len * 2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_latin1_to_utf8_ascii() {
        let input = b"hello";
        let mut out = [0u8; 32];
        let len = latin1_to_utf8_symlink(input, &mut out);
        assert_eq!(len, 5);
        assert_eq!(&out[..len], b"hello");
    }

    #[test]
    fn test_latin1_to_utf8_high_bytes() {
        // Latin1: 0xE9 = 'e' with accent (e-acute)
        // UTF-8: 0xC3 0xA9
        let input = [0xE9];
        let mut out = [0u8; 32];
        let len = latin1_to_utf8_symlink(&input, &mut out);
        assert_eq!(len, 2);
        assert_eq!(&out[..len], &[0xC3, 0xA9]);
    }

    #[test]
    fn test_colon_replacement() {
        let input = b":path/to/file";
        let mut out = [0u8; 32];
        let len = latin1_to_utf8_symlink(input, &mut out);
        assert_eq!(len, 13);
        assert_eq!(&out[..len], b"/path/to/file");
    }

    #[test]
    fn test_colon_not_at_start() {
        let input = b"path:to/file";
        let mut out = [0u8; 32];
        let len = latin1_to_utf8_symlink(input, &mut out);
        assert_eq!(len, 12);
        assert_eq!(&out[..len], b"path:to/file");
    }

    #[test]
    fn test_read_symlink_target() {
        let mut buf = [0u8; BLOCK_SIZE];
        // Put a symlink target at offset 24
        buf[SYMLINK_OFFSET..SYMLINK_OFFSET + 5].copy_from_slice(b"test\0");

        let mut out = [0u8; 32];
        let len = read_symlink_target(&buf, &mut out);
        assert_eq!(len, 4);
        assert_eq!(&out[..len], b"test");
    }

    #[test]
    fn test_read_symlink_with_colon() {
        let mut buf = [0u8; BLOCK_SIZE];
        buf[SYMLINK_OFFSET..SYMLINK_OFFSET + 6].copy_from_slice(b":boot\0");

        let mut out = [0u8; 32];
        let len = read_symlink_target(&buf, &mut out);
        assert_eq!(len, 5);
        assert_eq!(&out[..len], b"/boot");
    }
}
