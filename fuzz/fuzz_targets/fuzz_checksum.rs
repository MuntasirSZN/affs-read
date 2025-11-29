#![no_main]

use affs_read::{bitmap_sum, boot_sum, normal_sum};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Fuzz checksum functions

    // normal_sum needs 512 bytes
    if data.len() >= 512 {
        let block_buf: &[u8; 512] = data[..512].try_into().unwrap();

        // Try different checksum offsets
        for offset in [0, 4, 8, 12, 16, 20, 24, 508].iter() {
            let _ = normal_sum(block_buf, *offset);
        }

        // bitmap_sum
        let _ = bitmap_sum(block_buf);
    }

    // boot_sum needs 1024 bytes
    if data.len() >= 1024 {
        let boot_buf: &[u8; 1024] = data[..1024].try_into().unwrap();
        let _ = boot_sum(boot_buf);
    }
});
