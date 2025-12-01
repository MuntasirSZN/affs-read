//! Variable block size AFFS reader for hard disk partitions.
//!
//! AFFS on hard disks can use block sizes of 512, 1024, 2048, 4096, or 8192 bytes.
//! The block size is not stored in the filesystem and must be determined by
//! probing: try reading the root block at each possible block size until
//! the checksum validates.

use crate::checksum::{boot_sum, normal_sum_slice, read_i32_be_slice, read_u32_be_slice};
use crate::constants::*;
use crate::date::AmigaDate;
use crate::error::{AffsError, Result};
use crate::symlink::read_symlink_target_with_block_size;
use crate::types::{EntryType, FsFlags, FsType, SectorDevice};

/// Maximum block size supported (8192 bytes = 16 sectors).
pub const MAX_BLOCK_SIZE: usize = 8192;

/// Variable block size AFFS reader.
///
/// This reader supports AFFS filesystems with block sizes from 512 to 8192 bytes,
/// as used on Amiga hard disk partitions. The block size is determined by probing
/// the root block at different sizes until the checksum validates.
pub struct AffsReaderVar<'a, D: SectorDevice> {
    device: &'a D,
    /// Filesystem type (OFS or FFS).
    fs_type: FsType,
    /// Filesystem flags.
    fs_flags: FsFlags,
    /// Root block number (in filesystem blocks).
    root_block: u32,
    /// Total blocks on device.
    total_blocks: u32,
    /// Log2 of block size relative to 512 (0=512, 1=1024, ..., 4=8192).
    log_blocksize: u8,
    /// Actual block size in bytes.
    block_size: usize,
    /// Hash table size (entries per directory).
    hash_table_size: u32,
    /// Boot block sector offset (0 or 1).
    #[allow(dead_code)]
    boot_sector: u32,
    /// Disk name from root block.
    disk_name: [u8; MAX_NAME_LEN],
    /// Disk name length.
    disk_name_len: u8,
    /// Volume creation date.
    creation_date: AmigaDate,
    /// Volume last modification date.
    last_modified: AmigaDate,
}

/// Probe result for mount operation.
struct ProbeResult {
    fs_type: FsType,
    fs_flags: FsFlags,
    root_block: u32,
    log_blocksize: u8,
    block_size: usize,
    hash_table_size: u32,
    boot_sector: u32,
    disk_name: [u8; MAX_NAME_LEN],
    disk_name_len: u8,
    creation_date: AmigaDate,
    last_modified: AmigaDate,
}

impl<'a, D: SectorDevice> AffsReaderVar<'a, D> {
    /// Create a new variable block size AFFS reader.
    ///
    /// This probes the filesystem to determine the block size by trying
    /// different block sizes until the root block checksum validates.
    ///
    /// # Arguments
    /// * `device` - Sector device to read from
    /// * `total_sectors` - Total number of 512-byte sectors on the device
    pub fn new(device: &'a D, total_sectors: u64) -> Result<Self> {
        let result = Self::probe(device, total_sectors)?;

        Ok(Self {
            device,
            fs_type: result.fs_type,
            fs_flags: result.fs_flags,
            root_block: result.root_block,
            total_blocks: (total_sectors >> result.log_blocksize) as u32,
            log_blocksize: result.log_blocksize,
            block_size: result.block_size,
            hash_table_size: result.hash_table_size,
            boot_sector: result.boot_sector,
            disk_name: result.disk_name,
            disk_name_len: result.disk_name_len,
            creation_date: result.creation_date,
            last_modified: result.last_modified,
        })
    }

    /// Probe the filesystem to determine block size.
    fn probe(device: &'a D, _total_sectors: u64) -> Result<ProbeResult> {
        // Buffer for reading - we need max block size
        let mut buf = [0u8; MAX_BLOCK_SIZE];

        // Try boot block at sector 0 and sector 1
        for boot_sector in 0..=MAX_BOOT_BLOCK {
            // Read boot block (2 sectors)
            if Self::read_sectors(device, boot_sector as u64, &mut buf[..BOOT_BLOCK_SIZE]).is_err()
            {
                continue;
            }

            // Check DOS signature
            if &buf[0..3] != b"DOS" {
                continue;
            }

            // Check FFS flag (we only support FFS like GRUB)
            let flags = buf[3];
            if (flags & DOSFS_FFS) == 0 {
                continue; // OFS not supported for variable block size
            }

            let fs_type = FsType::Ffs;
            let fs_flags = FsFlags::from_dos_type(flags);

            // Verify boot checksum if boot code is present
            if buf[12] != 0 {
                let checksum = read_u32_be_slice(&buf, 4);
                let boot_buf: &[u8; BOOT_BLOCK_SIZE] = buf[..BOOT_BLOCK_SIZE].try_into().unwrap();
                let calculated = boot_sum(boot_buf);
                if checksum != calculated {
                    continue;
                }
            }

            let root_block_num = read_u32_be_slice(&buf, 8);

            // Try each block size
            for log_blocksize in 0..=MAX_LOG_BLOCK_SIZE {
                let block_size = 512usize << log_blocksize;

                // Read root block
                let root_sector = (root_block_num as u64) << log_blocksize;
                if Self::read_sectors(device, root_sector, &mut buf[..block_size]).is_err() {
                    continue;
                }

                // Validate root block type
                let block_type = read_i32_be_slice(&buf, 0);
                if block_type != T_HEADER {
                    continue;
                }

                // Validate secondary type (at end of block)
                let sec_type = read_i32_be_slice(&buf, block_size - 4);
                if sec_type != ST_ROOT {
                    continue;
                }

                // Validate hash table size
                let hash_table_size = read_u32_be_slice(&buf, 12);
                if hash_table_size == 0 {
                    continue;
                }

                // Validate checksum
                let checksum = read_u32_be_slice(&buf, 20);
                let calculated = normal_sum_slice(&buf[..block_size], 20);
                if checksum != calculated {
                    continue;
                }

                // Parse root block data
                let name_offset = block_size - FILE_LOCATION + 108; // 0x1B0 relative to end
                let name_len = buf[name_offset].min(MAX_NAME_LEN as u8);
                let mut disk_name = [0u8; MAX_NAME_LEN];
                disk_name[..name_len as usize]
                    .copy_from_slice(&buf[name_offset + 1..name_offset + 1 + name_len as usize]);

                // Creation date is at offset 0x1A4 from start in 512-byte block
                // For variable blocks, it's at block_size - FILE_LOCATION + 0x1A4 - (512 - FILE_LOCATION)
                // Actually for root block, dates are at fixed offsets from end
                let date_offset = block_size - FILE_LOCATION + 0x1A4 - (BLOCK_SIZE - FILE_LOCATION);
                let creation_date = AmigaDate::new(
                    read_i32_be_slice(&buf, date_offset),
                    read_i32_be_slice(&buf, date_offset + 4),
                    read_i32_be_slice(&buf, date_offset + 8),
                );

                let mod_offset = block_size - FILE_LOCATION + 0x1D8 - (BLOCK_SIZE - FILE_LOCATION);
                let last_modified = AmigaDate::new(
                    read_i32_be_slice(&buf, mod_offset),
                    read_i32_be_slice(&buf, mod_offset + 4),
                    read_i32_be_slice(&buf, mod_offset + 8),
                );

                return Ok(ProbeResult {
                    fs_type,
                    fs_flags,
                    root_block: root_block_num,
                    log_blocksize,
                    block_size,
                    hash_table_size,
                    boot_sector,
                    disk_name,
                    disk_name_len: name_len,
                    creation_date,
                    last_modified,
                });
            }
        }

        Err(AffsError::InvalidDosType)
    }

    /// Read multiple sectors into a buffer.
    fn read_sectors(device: &D, start_sector: u64, buf: &mut [u8]) -> Result<()> {
        let num_sectors = buf.len() / BLOCK_SIZE;
        let mut sector_buf = [0u8; BLOCK_SIZE];

        for i in 0..num_sectors {
            device
                .read_sector(start_sector + i as u64, &mut sector_buf)
                .map_err(|()| AffsError::BlockReadError)?;
            buf[i * BLOCK_SIZE..(i + 1) * BLOCK_SIZE].copy_from_slice(&sector_buf);
        }

        Ok(())
    }

    /// Read a filesystem block into a buffer.
    fn read_block_into(&self, block: u32, buf: &mut [u8]) -> Result<()> {
        let start_sector = (block as u64) << self.log_blocksize;
        Self::read_sectors(self.device, start_sector, &mut buf[..self.block_size])
    }

    /// Get the filesystem type (OFS or FFS).
    #[inline]
    pub const fn fs_type(&self) -> FsType {
        self.fs_type
    }

    /// Get filesystem flags.
    #[inline]
    pub const fn fs_flags(&self) -> FsFlags {
        self.fs_flags
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

    /// Get the block size in bytes.
    #[inline]
    pub const fn block_size(&self) -> usize {
        self.block_size
    }

    /// Get the log2 block size (relative to 512).
    #[inline]
    pub const fn log_blocksize(&self) -> u8 {
        self.log_blocksize
    }

    /// Get the disk name (volume label) as bytes.
    #[inline]
    pub fn disk_name(&self) -> &[u8] {
        &self.disk_name[..self.disk_name_len as usize]
    }

    /// Get the disk name (volume label) as a string (if valid UTF-8).
    #[inline]
    pub fn disk_name_str(&self) -> Option<&str> {
        core::str::from_utf8(self.disk_name()).ok()
    }

    /// Get the volume label (alias for disk_name).
    #[inline]
    pub fn label(&self) -> &[u8] {
        self.disk_name()
    }

    /// Get the volume label as string (alias for disk_name_str).
    #[inline]
    pub fn label_str(&self) -> Option<&str> {
        self.disk_name_str()
    }

    /// Get the volume creation date.
    #[inline]
    pub const fn creation_date(&self) -> AmigaDate {
        self.creation_date
    }

    /// Get the volume last modification date.
    #[inline]
    pub const fn last_modified(&self) -> AmigaDate {
        self.last_modified
    }

    /// Get the volume modification time as Unix timestamp.
    ///
    /// This matches GRUB's `grub_affs_mtime()` behavior:
    /// - days * 86400 + min * 60 + hz / 50 + epoch offset
    #[inline]
    pub fn mtime(&self) -> i64 {
        self.last_modified.to_unix_timestamp()
    }

    /// Get the hash table size.
    #[inline]
    pub const fn hash_table_size(&self) -> u32 {
        self.hash_table_size
    }

    /// Check if international mode is enabled.
    #[inline]
    pub const fn is_intl(&self) -> bool {
        self.fs_flags.intl || self.fs_flags.dircache
    }

    /// Read a symlink target.
    ///
    /// # Arguments
    /// * `block` - Block number of the symlink entry
    /// * `out` - Buffer to write the UTF-8 symlink target into
    ///
    /// # Returns
    /// The number of bytes written to `out`.
    pub fn read_symlink(&self, block: u32, out: &mut [u8]) -> Result<usize> {
        let mut buf = [0u8; MAX_BLOCK_SIZE];
        self.read_block_into(block, &mut buf)?;

        // Verify this is a symlink
        let sec_type = read_i32_be_slice(&buf, self.block_size - 4);
        if sec_type != ST_LSOFT {
            return Err(AffsError::NotASymlink);
        }

        Ok(read_symlink_target_with_block_size(
            &buf[..self.block_size],
            self.block_size,
            out,
        ))
    }

    /// Iterate over entries in the root directory.
    pub fn read_root_dir(&self) -> Result<VarDirIter<'_, D>> {
        let mut buf = [0u8; MAX_BLOCK_SIZE];
        self.read_block_into(self.root_block, &mut buf)?;

        // Read hash table
        let mut hash_table = [0u32; 256]; // Max possible hash table size
        let ht_size = self.hash_table_size as usize;
        for (i, slot) in hash_table.iter_mut().enumerate().take(ht_size.min(256)) {
            *slot = read_u32_be_slice(&buf, SYMLINK_OFFSET + i * 4);
        }

        Ok(VarDirIter::new(
            self.device,
            hash_table,
            ht_size,
            self.is_intl(),
            self.log_blocksize,
            self.block_size,
        ))
    }

    /// Iterate over entries in a directory.
    pub fn read_dir(&self, block: u32) -> Result<VarDirIter<'_, D>> {
        if block == self.root_block {
            return self.read_root_dir();
        }

        let mut buf = [0u8; MAX_BLOCK_SIZE];
        self.read_block_into(block, &mut buf)?;

        // Validate block type
        let block_type = read_i32_be_slice(&buf, 0);
        if block_type != T_HEADER {
            return Err(AffsError::InvalidBlockType);
        }

        // Validate this is a directory
        let sec_type = read_i32_be_slice(&buf, self.block_size - 4);
        if sec_type != ST_DIR && sec_type != ST_LDIR {
            return Err(AffsError::NotADirectory);
        }

        // Read hash table
        let mut hash_table = [0u32; 256];
        let ht_size = self.hash_table_size as usize;
        for (i, slot) in hash_table.iter_mut().enumerate().take(ht_size.min(256)) {
            *slot = read_u32_be_slice(&buf, SYMLINK_OFFSET + i * 4);
        }

        Ok(VarDirIter::new(
            self.device,
            hash_table,
            ht_size,
            self.is_intl(),
            self.log_blocksize,
            self.block_size,
        ))
    }
}

/// Directory entry for variable block size filesystem.
#[derive(Debug, Clone)]
pub struct VarDirEntry {
    /// Entry name.
    pub name: [u8; MAX_NAME_LEN],
    /// Name length.
    pub name_len: u8,
    /// Entry type.
    pub entry_type: EntryType,
    /// Block number.
    pub block: u32,
    /// Parent block.
    pub parent: u32,
    /// File size.
    pub size: u32,
    /// Modification date.
    pub date: AmigaDate,
}

impl VarDirEntry {
    /// Get entry name as bytes.
    #[inline]
    pub fn name(&self) -> &[u8] {
        &self.name[..self.name_len as usize]
    }

    /// Get entry name as string.
    #[inline]
    pub fn name_str(&self) -> Option<&str> {
        core::str::from_utf8(self.name()).ok()
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

/// Directory iterator for variable block size filesystem.
pub struct VarDirIter<'a, D: SectorDevice> {
    device: &'a D,
    hash_table: [u32; 256],
    hash_table_size: usize,
    hash_index: usize,
    current_chain: u32,
    #[allow(dead_code)]
    intl: bool,
    log_blocksize: u8,
    block_size: usize,
    buf: [u8; MAX_BLOCK_SIZE],
}

impl<'a, D: SectorDevice> VarDirIter<'a, D> {
    fn new(
        device: &'a D,
        hash_table: [u32; 256],
        hash_table_size: usize,
        intl: bool,
        log_blocksize: u8,
        block_size: usize,
    ) -> Self {
        Self {
            device,
            hash_table,
            hash_table_size,
            hash_index: 0,
            current_chain: 0,
            intl,
            log_blocksize,
            block_size,
            buf: [0u8; MAX_BLOCK_SIZE],
        }
    }

    fn read_block_into(&mut self, block: u32) -> Result<()> {
        let start_sector = (block as u64) << self.log_blocksize;
        let num_sectors = 1usize << self.log_blocksize;
        let mut sector_buf = [0u8; BLOCK_SIZE];

        for i in 0..num_sectors {
            self.device
                .read_sector(start_sector + i as u64, &mut sector_buf)
                .map_err(|()| AffsError::BlockReadError)?;
            self.buf[i * BLOCK_SIZE..(i + 1) * BLOCK_SIZE].copy_from_slice(&sector_buf);
        }

        Ok(())
    }

    fn parse_entry(&self, block: u32) -> Option<VarDirEntry> {
        let buf = &self.buf[..self.block_size];

        // Entry type is at end of block - 4
        let sec_type = read_i32_be_slice(buf, self.block_size - 4);
        let entry_type = EntryType::from_sec_type(sec_type)?;

        // Name is at block_size - FILE_LOCATION + offset
        let name_offset = self.block_size - FILE_LOCATION + 108;
        let name_len = buf[name_offset].min(MAX_NAME_LEN as u8);
        let mut name = [0u8; MAX_NAME_LEN];
        name[..name_len as usize]
            .copy_from_slice(&buf[name_offset + 1..name_offset + 1 + name_len as usize]);

        // Size at offset 0x144 relative to start in standard block
        // For variable blocks: block_size - FILE_LOCATION + 12
        let size_offset = self.block_size - FILE_LOCATION + 12;
        let size = read_u32_be_slice(buf, size_offset);

        // Parent at block_size - 12
        let parent = read_u32_be_slice(buf, self.block_size - 12);

        // Date at block_size - FILE_LOCATION + 0x1A4 - (512 - FILE_LOCATION)
        let date_offset = self.block_size - FILE_LOCATION + 0x1A4 - (BLOCK_SIZE - FILE_LOCATION);
        let date = AmigaDate::new(
            read_i32_be_slice(buf, date_offset),
            read_i32_be_slice(buf, date_offset + 4),
            read_i32_be_slice(buf, date_offset + 8),
        );

        Some(VarDirEntry {
            name,
            name_len,
            entry_type,
            block,
            parent,
            size,
            date,
        })
    }
}

impl<D: SectorDevice> Iterator for VarDirIter<'_, D> {
    type Item = Result<VarDirEntry>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            // If we're in a hash chain, continue it
            if self.current_chain != 0 {
                if let Err(e) = self.read_block_into(self.current_chain) {
                    return Some(Err(e));
                }

                let block = self.current_chain;

                // Next in chain at block_size - 16
                self.current_chain = read_u32_be_slice(&self.buf, self.block_size - 16);

                if let Some(entry) = self.parse_entry(block) {
                    return Some(Ok(entry));
                }
                continue;
            }

            // Find next non-empty hash slot
            while self.hash_index < self.hash_table_size {
                let block = self.hash_table[self.hash_index];
                self.hash_index += 1;

                if block != 0 {
                    self.current_chain = block;
                    break;
                }
            }

            if self.current_chain == 0 {
                return None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DummySectorDevice;

    impl SectorDevice for DummySectorDevice {
        fn read_sector(&self, _sector: u64, _buf: &mut [u8; 512]) -> core::result::Result<(), ()> {
            Err(())
        }
    }

    #[test]
    fn test_var_reader_error_on_bad_device() {
        let device = DummySectorDevice;
        let result = AffsReaderVar::new(&device, 1760);
        assert!(result.is_err());
    }

    /// Good device that returns a valid boot block and root block and one
    /// directory entry block so we can exercise probing and iteration.
    struct DummyGoodDevice;

    impl DummyGoodDevice {
        fn write_u32_be(buf: &mut [u8], offset: usize, val: u32) {
            let bytes = val.to_be_bytes();
            buf[offset..offset + 4].copy_from_slice(&bytes);
        }

        fn write_i32_be(buf: &mut [u8], offset: usize, val: i32) {
            let bytes = val.to_be_bytes();
            buf[offset..offset + 4].copy_from_slice(&bytes);
        }
    }

    impl SectorDevice for DummyGoodDevice {
        fn read_sector(&self, sector: u64, buf: &mut [u8; 512]) -> core::result::Result<(), ()> {
            // Sector mapping:
            // 0..=1 -> boot block (1024 bytes split)
            // 2 -> root block (512 bytes)
            // 5 -> directory entry block (512 bytes)
            for b in buf.iter_mut() {
                *b = 0;
            }

            match sector {
                0 => {
                    // First half of boot block
                    let mut boot = [0u8; 1024];
                    boot.fill(0);
                    boot[0..3].copy_from_slice(b"DOS");
                    boot[3] = DOSFS_FFS; // FFS flag
                    // buf[12] = 0 => skip boot checksum validation
                    DummyGoodDevice::write_u32_be(&mut boot, 8, 2); // root block = 2
                    buf.copy_from_slice(&boot[0..512]);
                    Ok(())
                }
                1 => {
                    // Second half of boot block
                    let mut boot = [0u8; 1024];
                    boot.fill(0);
                    boot[0..3].copy_from_slice(b"DOS");
                    boot[3] = DOSFS_FFS;
                    DummyGoodDevice::write_u32_be(&mut boot, 8, 2);
                    buf.copy_from_slice(&boot[512..1024]);
                    Ok(())
                }
                2 => {
                    // Root block (512 bytes)
                    let mut rb = [0u8; 512];
                    rb.fill(0);
                    // Block type header
                    DummyGoodDevice::write_i32_be(&mut rb, 0, T_HEADER);
                    // hash table size at offset 12
                    DummyGoodDevice::write_u32_be(&mut rb, 12, 4);
                    // We'll set checksum at offset 20 later
                    // Secondary type at end
                    DummyGoodDevice::write_i32_be(&mut rb, 512 - 4, ST_ROOT);
                    // Set hash table first slot to point to block 5 at SYMLINK_OFFSET
                    DummyGoodDevice::write_u32_be(&mut rb, SYMLINK_OFFSET, 5);
                    // Name offset and name
                    let name_offset = 512 - FILE_LOCATION + 108;
                    rb[name_offset] = 4; // length
                    rb[name_offset + 1..name_offset + 1 + 4].copy_from_slice(b"test");
                    // Date fields (three i32) - leave zero
                    // Calculate checksum excluding offset 20
                    let checksum = normal_sum_slice(&rb[..512], 20);
                    DummyGoodDevice::write_u32_be(&mut rb, 20, checksum);
                    buf.copy_from_slice(&rb);
                    Ok(())
                }
                5 => {
                    // Directory entry block for block number 5
                    let mut eb = [0u8; 512];
                    eb.fill(0);
                    DummyGoodDevice::write_i32_be(&mut eb, 0, T_HEADER);
                    // Secondary type -> file
                    DummyGoodDevice::write_i32_be(&mut eb, 512 - 4, ST_FILE);
                    // Name
                    let name_offset = 512 - FILE_LOCATION + 108;
                    eb[name_offset] = 4;
                    eb[name_offset + 1..name_offset + 1 + 4].copy_from_slice(b"file");
                    // Size at size_offset = block_size - FILE_LOCATION + 12
                    let size_offset = 512 - FILE_LOCATION + 12;
                    DummyGoodDevice::write_u32_be(&mut eb, size_offset, 123);
                    // Parent at block_size - 12
                    DummyGoodDevice::write_u32_be(&mut eb, 512 - 12, 2);
                    buf.copy_from_slice(&eb);
                    Ok(())
                }
                _ => Err(()),
            }
        }
    }

    #[test]
    fn test_var_probe_and_dir_iter() {
        let device = DummyGoodDevice;
        // total sectors arbitrary but >= 6
        let reader = AffsReaderVar::new(&device, 100).expect("probe should succeed");
        assert_eq!(reader.block_size(), 512);
        assert_eq!(reader.root_block(), 2);
        assert_eq!(reader.disk_name_str(), Some("test"));

        // Read root dir and iterate
        let mut iter = reader.read_root_dir().expect("read_root_dir");
        let first = iter.next().expect("entry").expect("ok entry");
        assert_eq!(first.name_str(), Some("file"));
        assert_eq!(first.size, 123);
        assert_eq!(first.block, 5);
    }
}
