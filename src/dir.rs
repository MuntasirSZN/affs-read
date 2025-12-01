//! Directory traversal.

use crate::block::{EntryBlock, hash_name, names_equal};
use crate::constants::*;
use crate::date::AmigaDate;
use crate::error::{AffsError, Result};
use crate::types::{Access, BlockDevice, EntryType};

/// Directory entry information.
#[derive(Debug, Clone)]
pub struct DirEntry {
    /// Entry name (up to 30 bytes).
    pub(crate) name: [u8; MAX_NAME_LEN],
    /// Name length.
    pub(crate) name_len: u8,
    /// Entry type.
    pub entry_type: EntryType,
    /// Block number of this entry.
    pub block: u32,
    /// Parent block number.
    pub parent: u32,
    /// File size (0 for directories).
    pub size: u32,
    /// Access permissions.
    pub access: Access,
    /// Last modification date.
    pub date: AmigaDate,
    /// Real entry (for hard links).
    pub real_entry: u32,
    /// Comment (if any).
    pub(crate) comment: [u8; MAX_COMMENT_LEN],
    /// Comment length.
    pub(crate) comment_len: u8,
}

impl DirEntry {
    /// Create from an entry block.
    pub(crate) fn from_entry_block(block_num: u32, entry: &EntryBlock) -> Option<Self> {
        let entry_type = entry.entry_type()?;

        let mut name = [0u8; MAX_NAME_LEN];
        let name_len = entry.name_len.min(MAX_NAME_LEN as u8);
        name[..name_len as usize].copy_from_slice(&entry.name[..name_len as usize]);

        let mut comment = [0u8; MAX_COMMENT_LEN];
        let comment_len = entry.comment_len.min(MAX_COMMENT_LEN as u8);
        comment[..comment_len as usize].copy_from_slice(&entry.comment[..comment_len as usize]);

        Some(Self {
            name,
            name_len,
            entry_type,
            block: block_num,
            parent: entry.parent,
            size: entry.byte_size,
            access: Access::new(entry.access),
            date: entry.date,
            real_entry: entry.real_entry,
            comment,
            comment_len,
        })
    }

    /// Get entry name as byte slice.
    #[inline]
    pub fn name(&self) -> &[u8] {
        &self.name[..self.name_len as usize]
    }

    /// Get entry name as str (if valid UTF-8).
    #[inline]
    pub fn name_str(&self) -> Option<&str> {
        core::str::from_utf8(self.name()).ok()
    }

    /// Get comment as byte slice.
    #[inline]
    pub fn comment(&self) -> &[u8] {
        &self.comment[..self.comment_len as usize]
    }

    /// Get comment as str (if valid UTF-8).
    #[inline]
    pub fn comment_str(&self) -> Option<&str> {
        core::str::from_utf8(self.comment()).ok()
    }

    /// Check if this is a directory.
    #[inline]
    pub const fn is_dir(&self) -> bool {
        self.entry_type.is_dir()
    }

    /// Check if this is a file.
    #[inline]
    pub const fn is_file(&self) -> bool {
        self.entry_type.is_file()
    }

    /// Check if this is a symlink.
    #[inline]
    pub const fn is_symlink(&self) -> bool {
        matches!(self.entry_type, EntryType::SoftLink)
    }
}

/// Iterator over directory entries.
///
/// This iterator reads entries lazily from the hash table.
pub struct DirIter<'a, D: BlockDevice> {
    device: &'a D,
    hash_table: [u32; HASH_TABLE_SIZE],
    hash_index: usize,
    current_chain: u32,
    intl: bool,
    buf: [u8; BLOCK_SIZE],
}

impl<'a, D: BlockDevice> DirIter<'a, D> {
    /// Create a new directory iterator.
    pub(crate) fn new(device: &'a D, hash_table: [u32; HASH_TABLE_SIZE], intl: bool) -> Self {
        Self {
            device,
            hash_table,
            hash_index: 0,
            current_chain: 0,
            intl,
            buf: [0u8; BLOCK_SIZE],
        }
    }

    /// Find an entry by name in this directory.
    pub fn find(mut self, name: &[u8]) -> Result<DirEntry> {
        if name.len() > MAX_NAME_LEN {
            return Err(AffsError::NameTooLong);
        }

        let hash = hash_name(name, self.intl);
        let mut block = self.hash_table[hash];

        while block != 0 {
            self.device
                .read_block(block, &mut self.buf)
                .map_err(|()| AffsError::BlockReadError)?;

            let entry = EntryBlock::parse(&self.buf)?;

            if names_equal(entry.name(), name, self.intl) {
                return DirEntry::from_entry_block(block, &entry).ok_or(AffsError::InvalidSecType);
            }

            block = entry.next_same_hash;
        }

        Err(AffsError::EntryNotFound)
    }
}

impl<D: BlockDevice> Iterator for DirIter<'_, D> {
    type Item = Result<DirEntry>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            // If we're in a hash chain, continue it
            if self.current_chain != 0 {
                let result = self.device.read_block(self.current_chain, &mut self.buf);
                if result.is_err() {
                    return Some(Err(AffsError::BlockReadError));
                }

                match EntryBlock::parse(&self.buf) {
                    Ok(entry) => {
                        let block = self.current_chain;
                        self.current_chain = entry.next_same_hash;

                        match DirEntry::from_entry_block(block, &entry) {
                            Some(dir_entry) => return Some(Ok(dir_entry)),
                            None => continue, // Skip invalid entries
                        }
                    }
                    Err(e) => return Some(Err(e)),
                }
            }

            // Find next non-empty hash slot
            while self.hash_index < HASH_TABLE_SIZE {
                let block = self.hash_table[self.hash_index];
                self.hash_index += 1;

                if block != 0 {
                    self.current_chain = block;
                    break;
                }
            }

            // No more entries
            if self.current_chain == 0 {
                return None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dir_entry_name() {
        let mut entry = DirEntry {
            name: [0u8; MAX_NAME_LEN],
            name_len: 4,
            entry_type: EntryType::File,
            block: 100,
            parent: 880,
            size: 1024,
            access: Access::new(0),
            date: AmigaDate::default(),
            real_entry: 0,
            comment: [0u8; MAX_COMMENT_LEN],
            comment_len: 0,
        };
        entry.name[..4].copy_from_slice(b"test");

        assert_eq!(entry.name(), b"test");
        assert_eq!(entry.name_str(), Some("test"));
    }
}
