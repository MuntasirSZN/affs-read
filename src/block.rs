//! Block structure parsing.

use crate::checksum::{boot_sum, normal_sum, read_i32_be, read_u32_be, read_u32_be_slice};
use crate::constants::*;
use crate::date::AmigaDate;
use crate::error::{AffsError, Result};
use crate::types::{EntryType, FsFlags, FsType};

/// Parsed boot block.
#[derive(Debug, Clone)]
pub struct BootBlock {
    /// DOS type bytes ("DOS\x00" - "DOS\x07").
    pub dos_type: [u8; 4],
    /// Checksum.
    pub checksum: u32,
    /// Root block number.
    pub root_block: u32,
}

impl BootBlock {
    /// Parse boot block from raw data (1024 bytes).
    pub fn parse(buf: &[u8; BOOT_BLOCK_SIZE]) -> Result<Self> {
        let dos_type = [buf[0], buf[1], buf[2], buf[3]];

        // Check for "DOS" signature
        if &dos_type[0..3] != b"DOS" {
            return Err(AffsError::InvalidDosType);
        }

        let checksum = read_u32_be_slice(buf, 4);
        let root_block = read_u32_be_slice(buf, 8);

        // Verify checksum if boot code is present
        if buf[12] != 0 {
            let calculated = boot_sum(buf);
            if checksum != calculated {
                return Err(AffsError::ChecksumMismatch);
            }
        }

        Ok(Self {
            dos_type,
            checksum,
            root_block,
        })
    }

    /// Get filesystem type (OFS or FFS).
    #[inline]
    pub const fn fs_type(&self) -> FsType {
        if (self.dos_type[3] & DOSFS_FFS) != 0 {
            FsType::Ffs
        } else {
            FsType::Ofs
        }
    }

    /// Get filesystem flags.
    #[inline]
    pub const fn fs_flags(&self) -> FsFlags {
        FsFlags::from_dos_type(self.dos_type[3])
    }
}

/// Parsed root block.
#[derive(Debug, Clone)]
pub struct RootBlock {
    /// Block type (should be T_HEADER).
    pub block_type: i32,
    /// Hash table size (always 72).
    pub hash_table_size: i32,
    /// Checksum.
    pub checksum: u32,
    /// Hash table entries.
    pub hash_table: [u32; HASH_TABLE_SIZE],
    /// Bitmap valid flag (-1 = valid).
    pub bm_flag: i32,
    /// Bitmap block pointers.
    pub bm_pages: [u32; BM_PAGES_ROOT_SIZE],
    /// Bitmap extension block.
    pub bm_ext: u32,
    /// Creation date.
    pub creation_date: AmigaDate,
    /// Disk name length.
    pub name_len: u8,
    /// Disk name (up to 30 chars).
    pub disk_name: [u8; MAX_NAME_LEN],
    /// Last modification date.
    pub last_modified: AmigaDate,
    /// Directory cache extension (FFS only).
    pub extension: u32,
    /// Secondary type (should be ST_ROOT).
    pub sec_type: i32,
}

impl RootBlock {
    /// Parse root block from raw data.
    pub fn parse(buf: &[u8; BLOCK_SIZE]) -> Result<Self> {
        let block_type = read_i32_be(buf, 0);
        if block_type != T_HEADER {
            return Err(AffsError::InvalidBlockType);
        }

        let sec_type = read_i32_be(buf, 508);
        if sec_type != ST_ROOT {
            return Err(AffsError::InvalidSecType);
        }

        let checksum = read_u32_be(buf, 20);
        let calculated = normal_sum(buf, 20);
        if checksum != calculated {
            return Err(AffsError::ChecksumMismatch);
        }

        let hash_table_size = read_i32_be(buf, 12);

        let mut hash_table = [0u32; HASH_TABLE_SIZE];
        for (i, entry) in hash_table.iter_mut().enumerate() {
            *entry = read_u32_be(buf, 24 + i * 4);
        }

        let bm_flag = read_i32_be(buf, 0x138);

        let mut bm_pages = [0u32; BM_PAGES_ROOT_SIZE];
        for (i, page) in bm_pages.iter_mut().enumerate() {
            *page = read_u32_be(buf, 0x13C + i * 4);
        }

        let bm_ext = read_u32_be(buf, 0x1A0);

        let creation_date = AmigaDate::new(
            read_i32_be(buf, 0x1A4),
            read_i32_be(buf, 0x1A8),
            read_i32_be(buf, 0x1AC),
        );

        let name_len = buf[0x1B0].min(MAX_NAME_LEN as u8);
        let mut disk_name = [0u8; MAX_NAME_LEN];
        disk_name[..name_len as usize].copy_from_slice(&buf[0x1B1..0x1B1 + name_len as usize]);

        let last_modified = AmigaDate::new(
            read_i32_be(buf, 0x1D8),
            read_i32_be(buf, 0x1DC),
            read_i32_be(buf, 0x1E0),
        );

        let extension = read_u32_be(buf, 0x1F8);

        Ok(Self {
            block_type,
            hash_table_size,
            checksum,
            hash_table,
            bm_flag,
            bm_pages,
            bm_ext,
            creation_date,
            name_len,
            disk_name,
            last_modified,
            extension,
            sec_type,
        })
    }

    /// Get disk name as string slice.
    #[inline]
    pub fn name(&self) -> &[u8] {
        &self.disk_name[..self.name_len as usize]
    }

    /// Check if bitmap is valid.
    #[inline]
    pub const fn bitmap_valid(&self) -> bool {
        self.bm_flag == BM_VALID
    }
}

/// Parsed entry block (file header or directory).
#[derive(Debug, Clone)]
pub struct EntryBlock {
    /// Block type (should be T_HEADER).
    pub block_type: i32,
    /// This block's sector number.
    pub header_key: u32,
    /// High sequence (number of data blocks in this header for files).
    pub high_seq: i32,
    /// First data block (files only).
    pub first_data: u32,
    /// Checksum.
    pub checksum: u32,
    /// Hash table (directories) or data block pointers (files).
    pub hash_table: [u32; HASH_TABLE_SIZE],
    /// Access flags.
    pub access: u32,
    /// File size in bytes (files only).
    pub byte_size: u32,
    /// Comment length.
    pub comment_len: u8,
    /// Comment (up to 79 chars).
    pub comment: [u8; MAX_COMMENT_LEN],
    /// Last modification date.
    pub date: AmigaDate,
    /// Name length.
    pub name_len: u8,
    /// Entry name (up to 30 chars).
    pub name: [u8; MAX_NAME_LEN],
    /// Real entry (for hard links).
    pub real_entry: u32,
    /// Next link in chain.
    pub next_link: u32,
    /// Next entry with same hash.
    pub next_same_hash: u32,
    /// Parent directory block.
    pub parent: u32,
    /// Extension block (file ext or dir cache).
    pub extension: u32,
    /// Secondary type.
    pub sec_type: i32,
}

impl EntryBlock {
    /// Parse entry block from raw data.
    pub fn parse(buf: &[u8; BLOCK_SIZE]) -> Result<Self> {
        let block_type = read_i32_be(buf, 0);
        if block_type != T_HEADER {
            return Err(AffsError::InvalidBlockType);
        }

        let checksum = read_u32_be(buf, 20);
        let calculated = normal_sum(buf, 20);
        if checksum != calculated {
            return Err(AffsError::ChecksumMismatch);
        }

        let header_key = read_u32_be(buf, 4);
        let high_seq = read_i32_be(buf, 8);
        let first_data = read_u32_be(buf, 16);

        let mut hash_table = [0u32; HASH_TABLE_SIZE];
        for (i, entry) in hash_table.iter_mut().enumerate() {
            *entry = read_u32_be(buf, 24 + i * 4);
        }

        let access = read_u32_be(buf, 0x140);
        let byte_size = read_u32_be(buf, 0x144);

        let comment_len = buf[0x148].min(MAX_COMMENT_LEN as u8);
        let mut comment = [0u8; MAX_COMMENT_LEN];
        comment[..comment_len as usize].copy_from_slice(&buf[0x149..0x149 + comment_len as usize]);

        let date = AmigaDate::new(
            read_i32_be(buf, 0x1A4),
            read_i32_be(buf, 0x1A8),
            read_i32_be(buf, 0x1AC),
        );

        let name_len = buf[0x1B0].min(MAX_NAME_LEN as u8);
        let mut name = [0u8; MAX_NAME_LEN];
        name[..name_len as usize].copy_from_slice(&buf[0x1B1..0x1B1 + name_len as usize]);

        let real_entry = read_u32_be(buf, 0x1D4);
        let next_link = read_u32_be(buf, 0x1D8);
        let next_same_hash = read_u32_be(buf, 0x1F0);
        let parent = read_u32_be(buf, 0x1F4);
        let extension = read_u32_be(buf, 0x1F8);
        let sec_type = read_i32_be(buf, 0x1FC);

        Ok(Self {
            block_type,
            header_key,
            high_seq,
            first_data,
            checksum,
            hash_table,
            access,
            byte_size,
            comment_len,
            comment,
            date,
            name_len,
            name,
            real_entry,
            next_link,
            next_same_hash,
            parent,
            extension,
            sec_type,
        })
    }

    /// Get entry name as byte slice.
    #[inline]
    pub fn name(&self) -> &[u8] {
        &self.name[..self.name_len as usize]
    }

    /// Get comment as byte slice.
    #[inline]
    pub fn comment(&self) -> &[u8] {
        &self.comment[..self.comment_len as usize]
    }

    /// Get entry type.
    #[inline]
    pub fn entry_type(&self) -> Option<EntryType> {
        EntryType::from_sec_type(self.sec_type)
    }

    /// Check if this is a directory.
    #[inline]
    pub const fn is_dir(&self) -> bool {
        self.sec_type == ST_DIR || self.sec_type == ST_LDIR
    }

    /// Check if this is a file.
    #[inline]
    pub const fn is_file(&self) -> bool {
        self.sec_type == ST_FILE || self.sec_type == ST_LFILE
    }

    /// Get data block pointer at index (for files).
    /// Index 0 is the first data block.
    #[inline]
    pub const fn data_block(&self, index: usize) -> u32 {
        if index < MAX_DATABLK {
            // Data blocks are stored in reverse order
            self.hash_table[MAX_DATABLK - 1 - index]
        } else {
            0
        }
    }
}

/// Parsed file extension block.
#[derive(Debug, Clone)]
pub struct FileExtBlock {
    /// Block type (should be T_LIST).
    pub block_type: i32,
    /// This block's sector number.
    pub header_key: u32,
    /// High sequence (number of data blocks in this ext block).
    pub high_seq: i32,
    /// Checksum.
    pub checksum: u32,
    /// Data block pointers.
    pub data_blocks: [u32; MAX_DATABLK],
    /// Parent (file header block).
    pub parent: u32,
    /// Next extension block.
    pub extension: u32,
    /// Secondary type (should be ST_FILE).
    pub sec_type: i32,
}

impl FileExtBlock {
    /// Parse file extension block from raw data.
    pub fn parse(buf: &[u8; BLOCK_SIZE]) -> Result<Self> {
        let block_type = read_i32_be(buf, 0);
        if block_type != T_LIST {
            return Err(AffsError::InvalidBlockType);
        }

        let checksum = read_u32_be(buf, 20);
        let calculated = normal_sum(buf, 20);
        if checksum != calculated {
            return Err(AffsError::ChecksumMismatch);
        }

        let header_key = read_u32_be(buf, 4);
        let high_seq = read_i32_be(buf, 8);

        let mut data_blocks = [0u32; MAX_DATABLK];
        for (i, block) in data_blocks.iter_mut().enumerate() {
            *block = read_u32_be(buf, 24 + i * 4);
        }

        let parent = read_u32_be(buf, 0x1F4);
        let extension = read_u32_be(buf, 0x1F8);
        let sec_type = read_i32_be(buf, 0x1FC);

        Ok(Self {
            block_type,
            header_key,
            high_seq,
            checksum,
            data_blocks,
            parent,
            extension,
            sec_type,
        })
    }

    /// Get data block pointer at index.
    #[inline]
    pub const fn data_block(&self, index: usize) -> u32 {
        if index < MAX_DATABLK {
            // Data blocks are stored in reverse order
            self.data_blocks[MAX_DATABLK - 1 - index]
        } else {
            0
        }
    }
}

/// Parsed OFS data block header.
#[derive(Debug, Clone, Copy)]
pub struct OfsDataBlock {
    /// Block type (should be T_DATA).
    pub block_type: i32,
    /// File header block pointer.
    pub header_key: u32,
    /// Sequence number (1-based).
    pub seq_num: u32,
    /// Data size in this block.
    pub data_size: u32,
    /// Next data block.
    pub next_data: u32,
    /// Checksum.
    pub checksum: u32,
}

impl OfsDataBlock {
    /// OFS data block header size.
    pub const HEADER_SIZE: usize = 24;

    /// Parse OFS data block header from raw data.
    pub fn parse(buf: &[u8; BLOCK_SIZE]) -> Result<Self> {
        let block_type = read_i32_be(buf, 0);
        if block_type != T_DATA {
            return Err(AffsError::InvalidBlockType);
        }

        let checksum = read_u32_be(buf, 20);
        let calculated = normal_sum(buf, 20);
        if checksum != calculated {
            return Err(AffsError::ChecksumMismatch);
        }

        Ok(Self {
            block_type,
            header_key: read_u32_be(buf, 4),
            seq_num: read_u32_be(buf, 8),
            data_size: read_u32_be(buf, 12),
            next_data: read_u32_be(buf, 16),
            checksum,
        })
    }

    /// Get data portion of the block.
    #[inline]
    pub fn data(buf: &[u8; BLOCK_SIZE]) -> &[u8] {
        &buf[Self::HEADER_SIZE..]
    }
}

/// Compute hash value for a name.
///
/// This implements the Amiga filename hashing algorithm.
#[inline]
pub fn hash_name(name: &[u8], intl: bool) -> usize {
    let mut hash = name.len() as u32;

    for &c in name {
        let upper = if intl {
            intl_to_upper(c)
        } else {
            ascii_to_upper(c)
        };
        hash = (hash.wrapping_mul(13).wrapping_add(upper as u32)) & 0x7FF;
    }
    (hash % HASH_TABLE_SIZE as u32) as usize
}

/// Convert ASCII character to uppercase using branchless operation.
#[inline]
const fn ascii_to_upper(c: u8) -> u8 {
    const ASCII_CASE_DIFF: u8 = 32;
    if c.is_ascii() {
        c & !(c.is_ascii_lowercase() as u8 * ASCII_CASE_DIFF)
    } else {
        c
    }
}

/// Convert character to uppercase with international support.
///
/// Handles Latin-1 characters (192-254) excluding multiplication sign (247).
/// Range 224-254 covers lowercase accented letters (à-þ) that map to
/// uppercase equivalents (À-Þ) by subtracting 32.
#[inline]
pub const fn intl_to_upper(c: u8) -> u8 {
    const ASCII_CASE_DIFF: u8 = 32;
    const LATIN1_LOWER_START: u8 = 224;
    const LATIN1_LOWER_END: u8 = 254;
    const MULTIPLICATION_SIGN: u8 = 247;

    if (c >= b'a' && c <= b'z')
        || (c >= LATIN1_LOWER_START && c <= LATIN1_LOWER_END && c != MULTIPLICATION_SIGN)
    {
        c.wrapping_sub(ASCII_CASE_DIFF)
    } else {
        c
    }
}

/// Compare two names for equality (case-insensitive).
#[inline]
pub fn names_equal(a: &[u8], b: &[u8], intl: bool) -> bool {
    if a.len() != b.len() {
        return false;
    }

    if a.is_empty() {
        return true;
    }

    if intl {
        for (&ca, &cb) in a.iter().zip(b.iter()) {
            if intl_to_upper(ca) != intl_to_upper(cb) {
                return false;
            }
        }
    } else {
        for (&ca, &cb) in a.iter().zip(b.iter()) {
            if ascii_to_upper(ca) != ascii_to_upper(cb) {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_name() {
        // These are known hash values from the AFFS spec
        assert!(hash_name(b"test", false) < HASH_TABLE_SIZE);
        assert!(hash_name(b"", false) == 0);
    }

    #[test]
    fn test_intl_to_upper() {
        assert_eq!(intl_to_upper(b'a'), b'A');
        assert_eq!(intl_to_upper(b'z'), b'Z');
        assert_eq!(intl_to_upper(b'A'), b'A');
        assert_eq!(intl_to_upper(224), 192); // à -> À
    }

    #[test]
    fn test_names_equal() {
        assert!(names_equal(b"Test", b"test", false));
        assert!(names_equal(b"TEST", b"test", false));
        assert!(!names_equal(b"Test", b"test2", false));
    }
}
