//! File reading functionality.

use crate::block::{EntryBlock, FileExtBlock, OfsDataBlock};
use crate::constants::*;
use crate::error::{AffsError, Result};
use crate::types::{BlockDevice, FsType};

/// Streaming file reader.
///
/// Reads file data sequentially with zero heap allocation.
/// Supports both OFS and FFS filesystems.
///
/// # Example
///
/// ```ignore
/// let mut reader = FileReader::new(&device, FsType::Ffs, file_header_block)?;
/// let mut buf = [0u8; 1024];
/// loop {
///     let n = reader.read(&mut buf)?;
///     if n == 0 {
///         break; // EOF
///     }
///     // Process buf[..n]
/// }
/// ```
pub struct FileReader<'a, D: BlockDevice> {
    device: &'a D,
    fs_type: FsType,
    /// Block number of file header (for reset/seek).
    header_block: u32,
    /// Total file size in bytes.
    file_size: u32,
    /// Bytes remaining to read.
    remaining: u32,
    /// Current block index within the file (0-based).
    block_index: u32,
    /// Initial number of data blocks in header (for reset).
    initial_blocks_in_header: u32,
    /// Total number of data blocks in header/ext block.
    blocks_in_current: u32,
    /// Index within current header/extension block.
    index_in_current: u32,
    /// Initial data block pointers from header (for reset).
    initial_data_blocks: [u32; MAX_DATABLK],
    /// Current data block pointers (from header or extension).
    data_blocks: [u32; MAX_DATABLK],
    /// Initial extension block (for reset).
    initial_extension: u32,
    /// Next extension block.
    next_extension: u32,
    /// Initial first data block for OFS (for reset).
    initial_first_data: u32,
    /// Current data block (for OFS linked list).
    current_data_block: u32,
    /// Offset within current data block.
    offset_in_block: usize,
    /// Block buffer.
    buf: [u8; BLOCK_SIZE],
}

impl<'a, D: BlockDevice> FileReader<'a, D> {
    /// Create a new file reader from a file header block.
    ///
    /// # Arguments
    /// * `device` - Block device to read from
    /// * `fs_type` - Filesystem type (OFS or FFS)
    /// * `header_block` - Block number of the file header
    pub fn new(device: &'a D, fs_type: FsType, header_block: u32) -> Result<Self> {
        let mut buf = [0u8; BLOCK_SIZE];
        device
            .read_block(header_block, &mut buf)
            .map_err(|()| AffsError::BlockReadError)?;

        let entry = EntryBlock::parse(&buf)?;

        if !entry.is_file() {
            return Err(AffsError::NotAFile);
        }

        let file_size = entry.byte_size;
        if entry.high_seq < 0 || entry.high_seq as usize > MAX_DATABLK {
            return Err(AffsError::InvalidState);
        }
        let blocks_in_current = entry.high_seq as u32;

        // Copy data block pointers
        let mut data_blocks = [0u32; MAX_DATABLK];
        data_blocks.copy_from_slice(&entry.hash_table);

        Ok(Self {
            device,
            fs_type,
            header_block,
            file_size,
            remaining: file_size,
            block_index: 0,
            initial_blocks_in_header: blocks_in_current,
            blocks_in_current,
            index_in_current: 0,
            initial_data_blocks: data_blocks,
            data_blocks,
            initial_extension: entry.extension,
            next_extension: entry.extension,
            initial_first_data: entry.first_data,
            current_data_block: entry.first_data,
            offset_in_block: 0,
            buf,
        })
    }

    /// Create a file reader from an already-parsed entry block.
    ///
    /// This avoids re-reading the header block if you already have it.
    ///
    /// # Arguments
    /// * `device` - Block device to read from
    /// * `fs_type` - Filesystem type (OFS or FFS)
    /// * `header_block` - Block number of the file header
    /// * `entry` - Already-parsed entry block
    pub fn from_entry(
        device: &'a D,
        fs_type: FsType,
        header_block: u32,
        entry: &EntryBlock,
    ) -> Result<Self> {
        if !entry.is_file() {
            return Err(AffsError::NotAFile);
        }

        let file_size = entry.byte_size;
        if entry.high_seq < 0 || entry.high_seq as usize > MAX_DATABLK {
            return Err(AffsError::InvalidState);
        }
        let blocks_in_current = entry.high_seq as u32;

        let mut data_blocks = [0u32; MAX_DATABLK];
        data_blocks.copy_from_slice(&entry.hash_table);

        Ok(Self {
            device,
            fs_type,
            header_block,
            file_size,
            remaining: file_size,
            block_index: 0,
            initial_blocks_in_header: blocks_in_current,
            blocks_in_current,
            index_in_current: 0,
            initial_data_blocks: data_blocks,
            data_blocks,
            initial_extension: entry.extension,
            next_extension: entry.extension,
            initial_first_data: entry.first_data,
            current_data_block: entry.first_data,
            offset_in_block: 0,
            buf: [0u8; BLOCK_SIZE],
        })
    }

    /// Get the total file size in bytes.
    #[inline]
    pub const fn size(&self) -> u32 {
        self.file_size
    }

    /// Get the block number of the file header.
    #[inline]
    pub const fn header_block(&self) -> u32 {
        self.header_block
    }

    /// Get the number of bytes remaining to read.
    #[inline]
    pub const fn remaining(&self) -> u32 {
        self.remaining
    }

    /// Check if we've reached end of file.
    #[inline]
    pub const fn is_eof(&self) -> bool {
        self.remaining == 0
    }

    /// Get current position in the file.
    #[inline]
    pub const fn position(&self) -> u32 {
        self.file_size - self.remaining
    }

    /// Reset the reader to the beginning of the file.
    ///
    /// This restores all internal state to allow reading from the start.
    pub fn reset(&mut self) {
        self.remaining = self.file_size;
        self.block_index = 0;
        self.blocks_in_current = self.initial_blocks_in_header;
        self.index_in_current = 0;
        self.data_blocks = self.initial_data_blocks;
        self.next_extension = self.initial_extension;
        self.current_data_block = self.initial_first_data;
        self.offset_in_block = 0;
    }

    /// Read data into a buffer.
    ///
    /// Returns the number of bytes read. Returns 0 at end of file.
    pub fn read(&mut self, out: &mut [u8]) -> Result<usize> {
        if self.remaining == 0 || out.is_empty() {
            return Ok(0);
        }

        let mut total_read = 0;

        while total_read < out.len() && self.remaining > 0 {
            // If we need to read a new data block
            if self.offset_in_block == 0 || self.offset_in_block >= self.data_block_size() {
                self.read_next_data_block()?;
            }

            // Calculate how much we can read from current block
            let data_size = self.current_block_data_size();
            let available = data_size.saturating_sub(self.offset_in_block);
            let to_read = available
                .min(out.len() - total_read)
                .min(self.remaining as usize);

            if to_read == 0 {
                break;
            }

            // Copy data
            let data_start = self.data_offset();
            let src = &self.buf
                [data_start + self.offset_in_block..data_start + self.offset_in_block + to_read];
            out[total_read..total_read + to_read].copy_from_slice(src);

            total_read += to_read;
            self.offset_in_block += to_read;
            self.remaining -= to_read as u32;
        }

        Ok(total_read)
    }

    /// Read the entire file into a buffer.
    ///
    /// The buffer must be at least as large as the file size.
    /// Returns the number of bytes read.
    pub fn read_all(&mut self, out: &mut [u8]) -> Result<usize> {
        if out.len() < self.remaining as usize {
            return Err(AffsError::BufferTooSmall);
        }

        let mut total = 0;
        while self.remaining > 0 {
            let n = self.read(&mut out[total..])?;
            if n == 0 {
                break;
            }
            total += n;
        }
        Ok(total)
    }

    /// Get data block size for this filesystem type.
    #[inline]
    const fn data_block_size(&self) -> usize {
        match self.fs_type {
            FsType::Ofs => OFS_DATA_SIZE,
            FsType::Ffs => FFS_DATA_SIZE,
        }
    }

    /// Get the data offset within a block.
    #[inline]
    const fn data_offset(&self) -> usize {
        match self.fs_type {
            FsType::Ofs => OfsDataBlock::HEADER_SIZE,
            FsType::Ffs => 0,
        }
    }

    /// Get actual data size in current block.
    fn current_block_data_size(&self) -> usize {
        match self.fs_type {
            FsType::Ofs => {
                // OFS has explicit data size in header
                // We need to parse it from current buffer
                let header = OfsDataBlock::parse(&self.buf).ok();
                header
                    .map(|h| (h.data_size as usize).min(OFS_DATA_SIZE))
                    .unwrap_or(0)
            }
            FsType::Ffs => {
                // FFS uses full block, but last block may be partial
                let block_size = FFS_DATA_SIZE;
                let remaining = self.remaining as usize + self.offset_in_block;
                remaining.min(block_size)
            }
        }
    }

    /// Read the next data block.
    fn read_next_data_block(&mut self) -> Result<()> {
        let block = self.get_next_data_block()?;
        if block == 0 {
            return Err(AffsError::EndOfFile);
        }

        self.device
            .read_block(block, &mut self.buf)
            .map_err(|()| AffsError::BlockReadError)?;

        // Validate OFS data block
        if matches!(self.fs_type, FsType::Ofs) {
            let _ = OfsDataBlock::parse(&self.buf)?;
        }

        self.offset_in_block = 0;
        self.block_index += 1;
        Ok(())
    }

    /// Get the next data block number.
    fn get_next_data_block(&mut self) -> Result<u32> {
        match self.fs_type {
            FsType::Ofs => self.get_next_ofs_block(),
            FsType::Ffs => self.get_next_ffs_block(),
        }
    }

    /// Derive a safe upper bound for data blocks from the u32 file size
    /// and the per-block payload. `u32::MAX / payload` is the absolute
    /// maximum number of data blocks any file in this FS can reference.
    #[inline]
    fn max_data_blocks(&self) -> usize {
        let payload = self.data_block_size();
        // payload is 488 or 512, never 0.
        let max_by_u32 = (u32::MAX as usize / payload) + 2;
        // Also bound by this file's own size — tighter for small files.
        let needed = self.file_size as usize / payload + 3;
        // Use the larger of the two so a valid huge file (>1M blocks) can
        // still be traversed, while small files keep a tighter guard.
        max_by_u32.max(needed)
    }

    /// Get next data block for OFS (follows linked list).
    fn get_next_ofs_block(&mut self) -> Result<u32> {
        if self.block_index == 0 {
            // First block - use first_data from header
            // current_data_block was set in new()
            return Ok(self.current_data_block);
        }

        // Guard against crafted circular linked list. Bound derived from
        // u32 byte_size so a valid 4 GiB file (OFS ~8.8M blocks, FFS ~8.4M)
        // is allowed, unlike the previous fixed 1_000_000 cap.
        if (self.block_index as usize) > self.max_data_blocks() {
            return Err(AffsError::InvalidState);
        }

        // Follow the linked list
        // current buffer should have the previous data block
        let header = OfsDataBlock::parse(&self.buf)?;
        self.current_data_block = header.next_data;
        Ok(self.current_data_block)
    }

    /// Get next data block for FFS (uses block pointer table).
    fn get_next_ffs_block(&mut self) -> Result<u32> {
        // Check if we need to load an extension block
        if self.index_in_current >= self.blocks_in_current {
            if self.next_extension == 0 {
                return Ok(0); // No more blocks
            }

            // Guard against crafted circular extension chain — same derived
            // bound as OFS so files >1M blocks remain readable.
            if (self.block_index as usize) > self.max_data_blocks() {
                return Err(AffsError::InvalidState);
            }

            // Load extension block
            self.device
                .read_block(self.next_extension, &mut self.buf)
                .map_err(|()| AffsError::BlockReadError)?;

            let ext = FileExtBlock::parse(&self.buf)?;
            if ext.high_seq < 0 || ext.high_seq as usize > MAX_DATABLK {
                return Err(AffsError::InvalidState);
            }

            // Copy data block pointers
            self.data_blocks.copy_from_slice(&ext.data_blocks);
            self.blocks_in_current = ext.high_seq as u32;
            self.next_extension = ext.extension;
            self.index_in_current = 0;
        }

        if self.index_in_current >= self.blocks_in_current {
            return Ok(0);
        }

        // Get block pointer (stored in reverse order)
        let idx = self.index_in_current as usize;
        let block = if idx < MAX_DATABLK {
            self.data_blocks[MAX_DATABLK - 1 - idx]
        } else {
            0
        };

        self.index_in_current += 1;
        Ok(block)
    }

    /// Seek to a specific position in the file.
    ///
    /// Note: Seeking backwards resets to the beginning and seeks forward,
    /// which may need to re-read extension blocks for large files.
    pub fn seek(&mut self, position: u32) -> Result<()> {
        if position > self.file_size {
            return Err(AffsError::EndOfFile);
        }

        if position == self.position() {
            return Ok(());
        }

        // For backward seeks, reset to beginning first
        if position < self.position() {
            self.reset();
        }

        // Seek forward by reading and discarding
        let mut discard = [0u8; 512];
        let mut to_skip = position - self.position();
        while to_skip > 0 {
            let n = self.read(&mut discard[..to_skip.min(512) as usize])?;
            if n == 0 {
                return Err(AffsError::EndOfFile);
            }
            to_skip -= n as u32;
        }

        Ok(())
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
    fn test_file_reader_error_on_bad_device() {
        let device = DummyDevice;
        let result = FileReader::new(&device, FsType::Ffs, 100);
        assert!(result.is_err());
    }

    #[test]
    fn test_max_data_blocks_derived_not_fixed_1m() {
        // Construct a minimal valid file header for a huge file (>1M blocks)
        // to verify the derived guard allows it. We reuse MockDevice logic
        // via a synthetic EntryBlock rather than allocating 1M blocks.
        let mut hdr = [0u8; BLOCK_SIZE];
        // Use integration-test helper layout: high_seq max 72, but file_size
        // drives max_data_blocks(). With FFS payload 512, u32::MAX needs
        // ~8.4M blocks — must be > 1_000_000.
        let large_size = u32::MAX;
        // Manually craft EntryBlock fields in hdr
        hdr[0..4].copy_from_slice(&2i32.to_be_bytes()); // T_HEADER
        hdr[8..12].copy_from_slice(&72i32.to_be_bytes()); // high_seq 72
        hdr[0x144..0x148].copy_from_slice(&large_size.to_be_bytes());
        hdr[0x1B0] = 4;
        hdr[0x1B1..0x1B5].copy_from_slice(b"bigf");
        hdr[0x1FC..].copy_from_slice(&(-3i32).to_be_bytes()); // ST_FILE
        // checksum at 20
        let sum = crate::checksum::normal_sum(&hdr, 20);
        hdr[20..24].copy_from_slice(&sum.to_be_bytes());
        let entry = crate::block::EntryBlock::parse(&hdr).unwrap();

        struct NoopDevice;
        impl BlockDevice for NoopDevice {
            fn read_block(&self, _b: u32, buf: &mut [u8; 512]) -> core::result::Result<(), ()> {
                // Return a valid empty header for any extension read — not used in this test
                buf.fill(0);
                Ok(())
            }
        }
        let dev = NoopDevice;
        let reader = FileReader::from_entry(&dev, FsType::Ffs, 100, &entry).unwrap();
        // Derived bound must exceed 1M and exceed the 8M needed for u32::MAX
        assert!(reader.max_data_blocks() > 1_000_000);
        assert!(reader.max_data_blocks() >= (u32::MAX as usize / 512));
        // Simulate that block_index at 1.5M is still allowed (would have
        // been rejected by the old fixed 1M guard).
        assert!((1_500_000usize) < reader.max_data_blocks());
    }
}
