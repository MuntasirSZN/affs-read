#![no_main]

use affs_read::{BootBlock, EntryBlock, FileExtBlock, OfsDataBlock, RootBlock};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Fuzz block parsing directly

    // Boot block parsing (needs 1024 bytes)
    if data.len() >= 1024 {
        let boot_buf: &[u8; 1024] = data[..1024].try_into().unwrap();
        let _ = BootBlock::parse(boot_buf);
    }

    // Single block parsing (needs 512 bytes)
    if data.len() >= 512 {
        let block_buf: &[u8; 512] = data[..512].try_into().unwrap();

        // Try parsing as different block types
        let _ = RootBlock::parse(block_buf);
        let _ = EntryBlock::parse(block_buf);
        let _ = FileExtBlock::parse(block_buf);
        let _ = OfsDataBlock::parse(block_buf);
    }
});
