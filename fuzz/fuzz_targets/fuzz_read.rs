#![no_main]

use affs_read::{AffsReader, BlockDevice};
use libfuzzer_sys::fuzz_target;

/// A mock block device backed by fuzzed data.
struct FuzzDevice<'a> {
    data: &'a [u8],
}

impl<'a> FuzzDevice<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data }
    }
}

impl BlockDevice for FuzzDevice<'_> {
    fn read_block(&self, block: u32, buf: &mut [u8; 512]) -> Result<(), ()> {
        let offset = (block as usize) * 512;
        if offset + 512 <= self.data.len() {
            buf.copy_from_slice(&self.data[offset..offset + 512]);
            Ok(())
        } else if offset < self.data.len() {
            // Partial block - fill with zeros
            buf.fill(0);
            let available = self.data.len() - offset;
            buf[..available].copy_from_slice(&self.data[offset..]);
            Ok(())
        } else {
            Err(())
        }
    }
}

fuzz_target!(|data: &[u8]| {
    // Need at least 2 blocks (boot block) + 1 block (root) = 1536 bytes minimum
    if data.len() < 1536 {
        return;
    }

    let device = FuzzDevice::new(data);
    let num_blocks = (data.len() / 512) as u32;

    // Try to create the reader
    let reader = match AffsReader::with_size(&device, num_blocks) {
        Ok(r) => r,
        Err(_) => return,
    };

    // Try to read root directory
    for entry in reader.read_root_dir().flatten() {
        // Try to read the entry name
        let _ = entry.name();
        let _ = entry.name_str();
        let _ = entry.comment();
        let _ = entry.comment_str();
        let _ = entry.is_dir();
        let _ = entry.is_file();

        // If it's a file, try to read it
        if entry.is_file() {
            if let Ok(mut file_reader) = reader.read_file(entry.block) {
                let mut buf = [0u8; 1024];
                // Try to read some data
                let _ = file_reader.read(&mut buf);
            }
        }

        // If it's a directory, try to read it
        if entry.is_dir() {
            if let Ok(subdir) = reader.read_dir(entry.block) {
                for subentry in subdir {
                    let _ = subentry;
                }
            }
        }
    }
});
