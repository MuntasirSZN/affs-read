//! Main AFFS reader interface.

use crate::block::{BootBlock, EntryBlock, RootBlock};
use crate::constants::*;
use crate::dir::{DirEntry, DirIter};
use crate::error::{AffsError, Result};
use crate::file::FileReader;
use crate::types::{BlockDevice, FsFlags, FsType};

/// Main AFFS filesystem reader.
///
/// Provides read-only access to an AFFS/OFS filesystem image.
///
/// # Example
///
/// ```ignore
/// use affs_read::{AffsReader, BlockDevice};
///
/// struct MyDevice { data: Vec<u8> }
///
/// impl BlockDevice for MyDevice {
///     fn read_block(&self, block: u32, buf: &mut [u8; 512]) -> Result<(), ()> {
///         let offset = block as usize * 512;
///         buf.copy_from_slice(&self.data[offset..offset + 512]);
///         Ok(())
///     }
/// }
///
/// let device = MyDevice { data: adf_data };
/// let reader = AffsReader::new(&device)?;
///
/// // Get disk info
/// println!("Disk: {:?}", reader.disk_name());
/// println!("Type: {:?}", reader.fs_type());
///
/// // List root directory
/// for entry in reader.read_dir(reader.root_block())? {
///     let entry = entry?;
///     println!("{:?}: {} bytes", entry.name(), entry.size);
/// }
/// ```
pub struct AffsReader<'a, D: BlockDevice> {
    device: &'a D,
    /// Boot block info.
    boot: BootBlock,
    /// Root block info.
    root: RootBlock,
    /// Calculated root block number.
    root_block: u32,
    /// Total blocks on device.
    total_blocks: u32,
}

impl<'a, D: BlockDevice> AffsReader<'a, D> {
    /// Create a new AFFS reader for a standard DD floppy (880KB).
    pub fn new(device: &'a D) -> Result<Self> {
        Self::with_size(device, FLOPPY_DD_SECTORS)
    }

    /// Create a new AFFS reader for an HD floppy (1.76MB).
    pub fn new_hd(device: &'a D) -> Result<Self> {
        Self::with_size(device, FLOPPY_HD_SECTORS)
    }

    /// Create a new AFFS reader with a specific block count.
    pub fn with_size(device: &'a D, total_blocks: u32) -> Result<Self> {
        // Read boot block (2 sectors)
        let mut boot_buf = [0u8; BOOT_BLOCK_SIZE];
        device
            .read_block(0, array_ref_mut(&mut boot_buf, 0))
            .map_err(|()| AffsError::BlockReadError)?;
        device
            .read_block(1, array_ref_mut(&mut boot_buf, BLOCK_SIZE))
            .map_err(|()| AffsError::BlockReadError)?;

        let boot = BootBlock::parse(&boot_buf)?;

        // Calculate root block position (middle of disk)
        let root_block = if boot.root_block != 0 {
            boot.root_block
        } else {
            total_blocks / 2
        };

        // Validate root block is in range
        if root_block >= total_blocks {
            return Err(AffsError::BlockOutOfRange);
        }

        // Read root block
        let mut root_buf = [0u8; BLOCK_SIZE];
        device
            .read_block(root_block, &mut root_buf)
            .map_err(|()| AffsError::BlockReadError)?;

        let root = RootBlock::parse(&root_buf)?;

        Ok(Self {
            device,
            boot,
            root,
            root_block,
            total_blocks,
        })
    }

    /// Get the filesystem type (OFS or FFS).
    #[inline]
    pub const fn fs_type(&self) -> FsType {
        self.boot.fs_type()
    }

    /// Get filesystem flags.
    #[inline]
    pub const fn fs_flags(&self) -> FsFlags {
        self.boot.fs_flags()
    }

    /// Check if international mode is enabled.
    #[inline]
    pub const fn is_intl(&self) -> bool {
        self.boot.fs_flags().intl
    }

    /// Get the root block number.
    #[inline]
    pub const fn root_block(&self) -> u32 {
        self.root_block
    }

    /// Get the total number of blocks.
    #[inline]
    pub const fn total_blocks(&self) -> u32 {
        self.total_blocks
    }

    /// Get the disk name as bytes.
    #[inline]
    pub fn disk_name(&self) -> &[u8] {
        self.root.name()
    }

    /// Get the disk name as a string (if valid UTF-8).
    #[inline]
    pub fn disk_name_str(&self) -> Option<&str> {
        core::str::from_utf8(self.disk_name()).ok()
    }

    /// Check if the bitmap is valid.
    #[inline]
    pub const fn bitmap_valid(&self) -> bool {
        self.root.bitmap_valid()
    }

    /// Get the root directory hash table.
    #[inline]
    pub fn root_hash_table(&self) -> &[u32; HASH_TABLE_SIZE] {
        &self.root.hash_table
    }

    /// Get a reference to the block device.
    #[inline]
    pub const fn device(&self) -> &'a D {
        self.device
    }

    /// Iterate over entries in the root directory.
    pub fn read_root_dir(&self) -> DirIter<'_, D> {
        DirIter::new(self.device, self.root.hash_table, self.is_intl())
    }

    /// Iterate over entries in a directory.
    ///
    /// # Arguments
    /// * `block` - Block number of the directory entry
    pub fn read_dir(&self, block: u32) -> Result<DirIter<'_, D>> {
        if block == self.root_block {
            return Ok(self.read_root_dir());
        }

        let mut buf = [0u8; BLOCK_SIZE];
        self.device
            .read_block(block, &mut buf)
            .map_err(|()| AffsError::BlockReadError)?;

        let entry = EntryBlock::parse(&buf)?;

        if !entry.is_dir() {
            return Err(AffsError::NotADirectory);
        }

        Ok(DirIter::new(self.device, entry.hash_table, self.is_intl()))
    }

    /// Find an entry by name in a directory.
    ///
    /// # Arguments
    /// * `dir_block` - Block number of the directory
    /// * `name` - Name to search for
    pub fn find_entry(&self, dir_block: u32, name: &[u8]) -> Result<DirEntry> {
        let dir = self.read_dir(dir_block)?;
        dir.find(name)
    }

    /// Find an entry by path from the root.
    ///
    /// Path components are separated by '/'.
    pub fn find_path(&self, path: &[u8]) -> Result<DirEntry> {
        let mut current_block = self.root_block;
        let mut final_entry: Option<DirEntry> = None;

        for component in path.split(|&b| b == b'/') {
            if component.is_empty() {
                continue;
            }

            let entry = self.find_entry(current_block, component)?;

            if entry.is_dir() {
                current_block = entry.block;
            }

            final_entry = Some(entry);
        }

        final_entry.ok_or(AffsError::EntryNotFound)
    }

    /// Read a file's contents.
    ///
    /// # Arguments
    /// * `block` - Block number of the file header
    pub fn read_file(&self, block: u32) -> Result<FileReader<'_, D>> {
        FileReader::new(self.device, self.fs_type(), block)
    }

    /// Read an entry block.
    pub fn read_entry(&self, block: u32) -> Result<EntryBlock> {
        let mut buf = [0u8; BLOCK_SIZE];
        self.device
            .read_block(block, &mut buf)
            .map_err(|()| AffsError::BlockReadError)?;
        EntryBlock::parse(&buf)
    }

    /// Get a DirEntry for the root directory.
    pub fn root_entry(&self) -> DirEntry {
        DirEntry::from_root(&self.root, self.root_block)
    }
}

/// Helper to get a mutable array reference from a slice.
#[inline]
fn array_ref_mut(slice: &mut [u8], offset: usize) -> &mut [u8; BLOCK_SIZE] {
    (&mut slice[offset..offset + BLOCK_SIZE])
        .try_into()
        .expect("slice size mismatch")
}

// Extension of DirEntry to support root
impl crate::dir::DirEntry {
    /// Create a DirEntry representing the root directory.
    pub(crate) fn from_root(root: &RootBlock, block: u32) -> Self {
        let mut name = [0u8; MAX_NAME_LEN];
        let name_len = root.name_len.min(MAX_NAME_LEN as u8);
        name[..name_len as usize].copy_from_slice(&root.disk_name[..name_len as usize]);

        Self {
            name,
            name_len,
            entry_type: crate::types::EntryType::Root,
            block,
            parent: 0,
            size: 0,
            access: crate::types::Access::new(0),
            date: root.last_modified,
            real_entry: 0,
            comment: [0u8; MAX_COMMENT_LEN],
            comment_len: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DummyDevice;

    impl BlockDevice for DummyDevice {
        fn read_block(&self, _block: u32, _buf: &mut [u8; 512]) -> core::result::Result<(), ()> {
            Err(())
        }
    }

    #[test]
    fn test_reader_error_on_bad_device() {
        let device = DummyDevice;
        let result = AffsReader::new(&device);
        assert!(result.is_err());
    }
}
