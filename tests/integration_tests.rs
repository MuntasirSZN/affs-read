//! Integration tests for affs-read with mock ADF data.

use affs_read::*;

/// Mock block device that holds an in-memory disk image.
struct MockDevice {
    blocks: Vec<[u8; 512]>,
}

impl MockDevice {
    fn new(num_blocks: usize) -> Self {
        Self {
            blocks: vec![[0u8; 512]; num_blocks],
        }
    }

    fn set_block(&mut self, block: u32, data: &[u8; 512]) {
        self.blocks[block as usize] = *data;
    }

    fn get_block_mut(&mut self, block: u32) -> &mut [u8; 512] {
        &mut self.blocks[block as usize]
    }
}

impl BlockDevice for MockDevice {
    fn read_block(&self, block: u32, buf: &mut [u8; 512]) -> Result<(), ()> {
        if (block as usize) < self.blocks.len() {
            *buf = self.blocks[block as usize];
            Ok(())
        } else {
            Err(())
        }
    }
}

/// Helper to write a big-endian u32.
fn write_u32_be(buf: &mut [u8], offset: usize, val: u32) {
    buf[offset..offset + 4].copy_from_slice(&val.to_be_bytes());
}

/// Helper to write a big-endian i32.
fn write_i32_be(buf: &mut [u8], offset: usize, val: i32) {
    buf[offset..offset + 4].copy_from_slice(&val.to_be_bytes());
}

/// Calculate and set the normal checksum for a block.
fn set_checksum(buf: &mut [u8; 512], checksum_offset: usize) {
    let mut sum: u32 = 0;
    for i in 0..(512 / 4) {
        if i != checksum_offset / 4 {
            let val =
                u32::from_be_bytes([buf[i * 4], buf[i * 4 + 1], buf[i * 4 + 2], buf[i * 4 + 3]]);
            sum = sum.wrapping_add(val);
        }
    }
    let checksum = (-(sum as i32)) as u32;
    write_u32_be(buf, checksum_offset, checksum);
}

/// Create a valid boot block for FFS.
fn create_boot_block() -> ([u8; 512], [u8; 512]) {
    let mut block0 = [0u8; 512];
    let block1 = [0u8; 512];

    // DOS signature + FFS flag
    block0[0] = b'D';
    block0[1] = b'O';
    block0[2] = b'S';
    block0[3] = 1; // FFS

    // Root block at 880 (middle of DD floppy)
    write_u32_be(&mut block0, 8, 880);

    (block0, block1)
}

/// Create a valid root block.
fn create_root_block(disk_name: &[u8]) -> [u8; 512] {
    let mut buf = [0u8; 512];

    // Block type = T_HEADER (2)
    write_i32_be(&mut buf, 0, 2);

    // Hash table size
    write_i32_be(&mut buf, 12, 72);

    // Bitmap valid flag
    write_i32_be(&mut buf, 0x138, -1);

    // Bitmap pages (at least one)
    write_u32_be(&mut buf, 0x13C, 881);

    // Disk name
    let name_len = disk_name.len().min(30);
    buf[0x1B0] = name_len as u8;
    buf[0x1B1..0x1B1 + name_len].copy_from_slice(&disk_name[..name_len]);

    // Secondary type = ST_ROOT (1)
    write_i32_be(&mut buf, 508, 1);

    // Set checksum
    set_checksum(&mut buf, 20);

    buf
}

/// Create a valid file header block.
fn create_file_header(
    name: &[u8],
    size: u32,
    parent: u32,
    first_data: u32,
    data_blocks: &[u32],
) -> [u8; 512] {
    let mut buf = [0u8; 512];

    // Block type = T_HEADER (2)
    write_i32_be(&mut buf, 0, 2);

    // High seq (number of data blocks)
    write_i32_be(&mut buf, 8, data_blocks.len() as i32);

    // First data block (OFS)
    write_u32_be(&mut buf, 16, first_data);

    // Data block pointers (stored in reverse order at offset 24)
    for (i, &block) in data_blocks.iter().enumerate() {
        if i < 72 {
            write_u32_be(&mut buf, 24 + (71 - i) * 4, block);
        }
    }

    // File size
    write_u32_be(&mut buf, 0x144, size);

    // Name
    let name_len = name.len().min(30);
    buf[0x1B0] = name_len as u8;
    buf[0x1B1..0x1B1 + name_len].copy_from_slice(&name[..name_len]);

    // Parent
    write_u32_be(&mut buf, 0x1F4, parent);

    // Secondary type = ST_FILE (-3)
    write_i32_be(&mut buf, 0x1FC, -3);

    // Set checksum
    set_checksum(&mut buf, 20);

    buf
}

/// Create a valid directory header block.
fn create_dir_header(name: &[u8], parent: u32, hash_entries: &[(usize, u32)]) -> [u8; 512] {
    let mut buf = [0u8; 512];

    // Block type = T_HEADER (2)
    write_i32_be(&mut buf, 0, 2);

    // Hash table entries
    for &(idx, block) in hash_entries {
        if idx < 72 {
            write_u32_be(&mut buf, 24 + idx * 4, block);
        }
    }

    // Name
    let name_len = name.len().min(30);
    buf[0x1B0] = name_len as u8;
    buf[0x1B1..0x1B1 + name_len].copy_from_slice(&name[..name_len]);

    // Parent
    write_u32_be(&mut buf, 0x1F4, parent);

    // Secondary type = ST_DIR (2)
    write_i32_be(&mut buf, 0x1FC, 2);

    // Set checksum
    set_checksum(&mut buf, 20);

    buf
}

/// Create a valid OFS data block.
fn create_ofs_data_block(header_key: u32, seq_num: u32, data: &[u8], next: u32) -> [u8; 512] {
    let mut buf = [0u8; 512];

    // Block type = T_DATA (8)
    write_i32_be(&mut buf, 0, 8);

    // Header key
    write_u32_be(&mut buf, 4, header_key);

    // Sequence number
    write_u32_be(&mut buf, 8, seq_num);

    // Data size
    let data_size = data.len().min(488);
    write_u32_be(&mut buf, 12, data_size as u32);

    // Next data block
    write_u32_be(&mut buf, 16, next);

    // Data
    buf[24..24 + data_size].copy_from_slice(&data[..data_size]);

    // Set checksum
    set_checksum(&mut buf, 20);

    buf
}

/// Create a file extension block.
fn create_file_ext_block(parent: u32, data_blocks: &[u32], extension: u32) -> [u8; 512] {
    let mut buf = [0u8; 512];

    // Block type = T_LIST (16)
    write_i32_be(&mut buf, 0, 16);

    // High seq
    write_i32_be(&mut buf, 8, data_blocks.len() as i32);

    // Data block pointers (stored in reverse order)
    for (i, &block) in data_blocks.iter().enumerate() {
        if i < 72 {
            write_u32_be(&mut buf, 24 + (71 - i) * 4, block);
        }
    }

    // Parent (file header)
    write_u32_be(&mut buf, 0x1F4, parent);

    // Extension
    write_u32_be(&mut buf, 0x1F8, extension);

    // Secondary type = ST_FILE (-3)
    write_i32_be(&mut buf, 0x1FC, -3);

    // Set checksum
    set_checksum(&mut buf, 20);

    buf
}

/// Create a minimal valid FFS disk with a root directory and one file.
fn create_test_disk() -> MockDevice {
    let mut device = MockDevice::new(1760); // DD floppy

    // Boot block
    let (boot0, boot1) = create_boot_block();
    device.set_block(0, &boot0);
    device.set_block(1, &boot1);

    // Root block at 880
    let mut root = create_root_block(b"TestDisk");
    // Add a file to hash table - hash of "testfile"
    let hash_idx = hash_name(b"testfile", false);
    write_u32_be(&mut root, 24 + hash_idx * 4, 882);
    set_checksum(&mut root, 20);
    device.set_block(880, &root);

    // File header at 882
    let file_header = create_file_header(b"testfile", 100, 880, 883, &[883]);
    device.set_block(882, &file_header);

    // FFS data block at 883 (just raw data, no header)
    let mut data_block = [0u8; 512];
    for (i, byte) in data_block.iter_mut().enumerate().take(100) {
        *byte = (i as u8).wrapping_add(1);
    }
    device.set_block(883, &data_block);

    device
}

/// Create a disk with OFS filesystem.
fn create_ofs_test_disk() -> MockDevice {
    let mut device = MockDevice::new(1760);

    // Boot block - OFS
    let mut block0 = [0u8; 512];
    block0[0] = b'D';
    block0[1] = b'O';
    block0[2] = b'S';
    block0[3] = 0; // OFS
    write_u32_be(&mut block0, 8, 880);
    device.set_block(0, &block0);
    device.set_block(1, &[0u8; 512]);

    // Root block
    let mut root = create_root_block(b"OFSDisk");
    let hash_idx = hash_name(b"ofsfile", false);
    write_u32_be(&mut root, 24 + hash_idx * 4, 882);
    set_checksum(&mut root, 20);
    device.set_block(880, &root);

    // File header
    let file_header = create_file_header(b"ofsfile", 50, 880, 883, &[883]);
    device.set_block(882, &file_header);

    // OFS data block
    let mut data = [0u8; 488];
    for (i, byte) in data.iter_mut().enumerate().take(50) {
        *byte = (i as u8).wrapping_add(10);
    }
    let ofs_block = create_ofs_data_block(882, 1, &data[..50], 0);
    device.set_block(883, &ofs_block);

    device
}

#[test]
fn test_read_ffs_disk() {
    let device = create_test_disk();
    let reader = AffsReader::new(&device).unwrap();

    assert_eq!(reader.fs_type(), FsType::Ffs);
    assert_eq!(reader.disk_name(), b"TestDisk");
    assert_eq!(reader.disk_name_str(), Some("TestDisk"));
    assert_eq!(reader.root_block(), 880);
    assert_eq!(reader.total_blocks(), 1760);
    assert!(reader.bitmap_valid());
    assert!(!reader.is_intl());
}

#[test]
fn test_read_ofs_disk() {
    let device = create_ofs_test_disk();
    let reader = AffsReader::new(&device).unwrap();

    assert_eq!(reader.fs_type(), FsType::Ofs);
    assert_eq!(reader.disk_name(), b"OFSDisk");
}

#[test]
fn test_read_root_dir() {
    let device = create_test_disk();
    let reader = AffsReader::new(&device).unwrap();

    let entries: Vec<_> = reader.read_root_dir().collect();
    assert_eq!(entries.len(), 1);

    let entry = entries[0].as_ref().unwrap();
    assert_eq!(entry.name(), b"testfile");
    assert_eq!(entry.name_str(), Some("testfile"));
    assert!(entry.is_file());
    assert!(!entry.is_dir());
    assert_eq!(entry.size, 100);
}

#[test]
fn test_read_dir() {
    let device = create_test_disk();
    let reader = AffsReader::new(&device).unwrap();

    // Read root dir by block number
    let entries: Vec<_> = reader.read_dir(880).unwrap().collect();
    assert_eq!(entries.len(), 1);
}

#[test]
fn test_find_entry() {
    let device = create_test_disk();
    let reader = AffsReader::new(&device).unwrap();

    let entry = reader.find_entry(880, b"testfile").unwrap();
    assert_eq!(entry.name(), b"testfile");
    assert_eq!(entry.size, 100);

    // Test case insensitivity
    let entry2 = reader.find_entry(880, b"TESTFILE").unwrap();
    assert_eq!(entry2.name(), b"testfile");

    // Test not found
    let result = reader.find_entry(880, b"nonexistent");
    assert!(matches!(result, Err(AffsError::EntryNotFound)));
}

#[test]
fn test_find_path() {
    let device = create_test_disk();
    let reader = AffsReader::new(&device).unwrap();

    let entry = reader.find_path(b"testfile").unwrap();
    assert_eq!(entry.name(), b"testfile");

    // With leading slash
    let entry2 = reader.find_path(b"/testfile").unwrap();
    assert_eq!(entry2.name(), b"testfile");
}

#[test]
fn test_read_file_ffs() {
    let device = create_test_disk();
    let reader = AffsReader::new(&device).unwrap();

    let entry = reader.find_entry(880, b"testfile").unwrap();
    let mut file_reader = reader.read_file(entry.block).unwrap();

    assert_eq!(file_reader.size(), 100);
    assert_eq!(file_reader.remaining(), 100);
    assert_eq!(file_reader.position(), 0);
    assert!(!file_reader.is_eof());

    let mut buf = [0u8; 200];
    let n = file_reader.read(&mut buf).unwrap();
    assert_eq!(n, 100);
    assert!(file_reader.is_eof());
    assert_eq!(file_reader.position(), 100);

    // Verify content
    for (i, byte) in buf.iter().enumerate().take(100) {
        assert_eq!(*byte, (i as u8).wrapping_add(1));
    }
}

#[test]
fn test_read_file_ofs() {
    let device = create_ofs_test_disk();
    let reader = AffsReader::new(&device).unwrap();

    let entry = reader.find_entry(880, b"ofsfile").unwrap();
    let mut file_reader = reader.read_file(entry.block).unwrap();

    assert_eq!(file_reader.size(), 50);

    let mut buf = [0u8; 100];
    let n = file_reader.read(&mut buf).unwrap();
    assert_eq!(n, 50);

    for (i, byte) in buf.iter().enumerate().take(50) {
        assert_eq!(*byte, (i as u8).wrapping_add(10));
    }
}

#[test]
fn test_read_file_all() {
    let device = create_test_disk();
    let reader = AffsReader::new(&device).unwrap();

    let entry = reader.find_entry(880, b"testfile").unwrap();
    let mut file_reader = reader.read_file(entry.block).unwrap();

    let mut buf = [0u8; 200];
    let n = file_reader.read_all(&mut buf).unwrap();
    assert_eq!(n, 100);
}

#[test]
fn test_read_file_buffer_too_small() {
    let device = create_test_disk();
    let reader = AffsReader::new(&device).unwrap();

    let entry = reader.find_entry(880, b"testfile").unwrap();
    let mut file_reader = reader.read_file(entry.block).unwrap();

    let mut buf = [0u8; 50]; // Too small
    let result = file_reader.read_all(&mut buf);
    assert!(matches!(result, Err(AffsError::BufferTooSmall)));
}

#[test]
fn test_read_entry() {
    let device = create_test_disk();
    let reader = AffsReader::new(&device).unwrap();

    let entry = reader.read_entry(882).unwrap();
    assert_eq!(entry.name(), b"testfile");
    assert!(entry.is_file());
    assert!(!entry.is_dir());
    assert_eq!(entry.byte_size, 100);
}

#[test]
fn test_root_entry() {
    let device = create_test_disk();
    let reader = AffsReader::new(&device).unwrap();

    let entry = reader.root_entry();
    assert_eq!(entry.name(), b"TestDisk");
    assert!(entry.is_dir());
    assert_eq!(entry.entry_type, EntryType::Root);
}

#[test]
fn test_fs_flags() {
    // Test INTL mode
    let mut device = MockDevice::new(1760);
    let mut block0 = [0u8; 512];
    block0[0] = b'D';
    block0[1] = b'O';
    block0[2] = b'S';
    block0[3] = 1 | 2; // FFS + INTL
    write_u32_be(&mut block0, 8, 880);
    device.set_block(0, &block0);
    device.set_block(1, &[0u8; 512]);

    let root = create_root_block(b"IntlDisk");
    device.set_block(880, &root);

    let reader = AffsReader::new(&device).unwrap();
    assert!(reader.is_intl());
    assert!(reader.fs_flags().intl);
    assert!(!reader.fs_flags().dircache);
}

#[test]
fn test_hd_floppy() {
    let mut device = MockDevice::new(3520); // HD floppy

    let (boot0, boot1) = create_boot_block();
    device.set_block(0, &boot0);
    device.set_block(1, &boot1);

    // Root at middle of HD disk
    let root = create_root_block(b"HDDisk");
    device.set_block(1760, &root);

    // Modify boot block to point to correct root
    let block0 = device.get_block_mut(0);
    write_u32_be(block0, 8, 1760);

    let reader = AffsReader::new_hd(&device).unwrap();
    assert_eq!(reader.total_blocks(), 3520);
    assert_eq!(reader.root_block(), 1760);
}

#[test]
fn test_invalid_dos_type() {
    let mut device = MockDevice::new(1760);
    let mut block0 = [0u8; 512];
    block0[0] = b'X'; // Invalid
    block0[1] = b'X';
    block0[2] = b'X';
    device.set_block(0, &block0);
    device.set_block(1, &[0u8; 512]);

    let result = AffsReader::new(&device);
    assert!(matches!(result, Err(AffsError::InvalidDosType)));
}

#[test]
fn test_invalid_root_block_type() {
    let mut device = MockDevice::new(1760);
    let (boot0, boot1) = create_boot_block();
    device.set_block(0, &boot0);
    device.set_block(1, &boot1);

    // Invalid root block (wrong type)
    let mut root = [0u8; 512];
    write_i32_be(&mut root, 0, 99); // Invalid type
    device.set_block(880, &root);

    let result = AffsReader::new(&device);
    assert!(matches!(result, Err(AffsError::InvalidBlockType)));
}

#[test]
fn test_invalid_root_sec_type() {
    let mut device = MockDevice::new(1760);
    let (boot0, boot1) = create_boot_block();
    device.set_block(0, &boot0);
    device.set_block(1, &boot1);

    // Root block with wrong secondary type
    let mut root = [0u8; 512];
    write_i32_be(&mut root, 0, 2); // T_HEADER
    write_i32_be(&mut root, 508, 99); // Invalid sec type
    set_checksum(&mut root, 20);
    device.set_block(880, &root);

    let result = AffsReader::new(&device);
    assert!(matches!(result, Err(AffsError::InvalidSecType)));
}

#[test]
fn test_checksum_mismatch() {
    let mut device = MockDevice::new(1760);
    let (boot0, boot1) = create_boot_block();
    device.set_block(0, &boot0);
    device.set_block(1, &boot1);

    // Root block with bad checksum
    let mut root = create_root_block(b"Test");
    root[100] = 0xFF; // Corrupt data without updating checksum
    device.set_block(880, &root);

    let result = AffsReader::new(&device);
    assert!(matches!(result, Err(AffsError::ChecksumMismatch)));
}

#[test]
fn test_not_a_directory() {
    let device = create_test_disk();
    let reader = AffsReader::new(&device).unwrap();

    // Try to read file as directory
    let result = reader.read_dir(882);
    assert!(matches!(result, Err(AffsError::NotADirectory)));
}

#[test]
fn test_not_a_file() {
    let device = create_test_disk();
    let reader = AffsReader::new(&device).unwrap();

    // Try to read root as file
    let result = reader.read_file(880);
    assert!(matches!(result, Err(AffsError::NotAFile)));
}

#[test]
fn test_name_too_long() {
    let device = create_test_disk();
    let reader = AffsReader::new(&device).unwrap();

    let long_name = [b'x'; 50];
    let result = reader.find_entry(880, &long_name);
    assert!(matches!(result, Err(AffsError::NameTooLong)));
}

#[test]
fn test_block_read_error() {
    struct FailingDevice;
    impl BlockDevice for FailingDevice {
        fn read_block(&self, _block: u32, _buf: &mut [u8; 512]) -> Result<(), ()> {
            Err(())
        }
    }

    let device = FailingDevice;
    let result = AffsReader::new(&device);
    assert!(matches!(result, Err(AffsError::BlockReadError)));
}

#[test]
fn test_entry_types() {
    assert!(EntryType::Root.is_dir());
    assert!(EntryType::Dir.is_dir());
    assert!(EntryType::HardLinkDir.is_dir());
    assert!(!EntryType::File.is_dir());

    assert!(EntryType::File.is_file());
    assert!(EntryType::HardLinkFile.is_file());
    assert!(!EntryType::Dir.is_file());

    assert_eq!(EntryType::from_sec_type(1), Some(EntryType::Root));
    assert_eq!(EntryType::from_sec_type(2), Some(EntryType::Dir));
    assert_eq!(EntryType::from_sec_type(-3), Some(EntryType::File));
    assert_eq!(EntryType::from_sec_type(-4), Some(EntryType::HardLinkFile));
    assert_eq!(EntryType::from_sec_type(4), Some(EntryType::HardLinkDir));
    assert_eq!(EntryType::from_sec_type(3), Some(EntryType::SoftLink));
    assert_eq!(EntryType::from_sec_type(999), None);
}

#[test]
fn test_fs_type_data_block_size() {
    assert_eq!(FsType::Ofs.data_block_size(), 488);
    assert_eq!(FsType::Ffs.data_block_size(), 512);
}

#[test]
fn test_access_flags() {
    let access = Access::new(0b11111111);
    assert!(access.is_delete_protected());
    assert!(access.is_execute_protected());
    assert!(access.is_write_protected());
    assert!(access.is_read_protected());
    assert!(access.is_archived());
    assert!(access.is_pure());
    assert!(access.is_script());
    assert!(access.is_hold());

    let access2 = Access::new(0);
    assert!(!access2.is_delete_protected());
    assert!(!access2.is_execute_protected());
    assert!(!access2.is_write_protected());
    assert!(!access2.is_read_protected());
    assert!(!access2.is_archived());
    assert!(!access2.is_pure());
    assert!(!access2.is_script());
    assert!(!access2.is_hold());

    let default_access = Access::default();
    assert_eq!(default_access.0, 0);
}

#[test]
fn test_fs_flags_dircache() {
    let flags = FsFlags::from_dos_type(4); // DIRCACHE
    assert!(!flags.intl);
    assert!(flags.dircache);

    let flags2 = FsFlags::from_dos_type(6); // INTL + DIRCACHE
    assert!(flags2.intl);
    assert!(flags2.dircache);

    let default_flags = FsFlags::default();
    assert!(!default_flags.intl);
    assert!(!default_flags.dircache);
}

#[test]
fn test_amiga_date() {
    let date = AmigaDate::new(0, 0, 0);
    let dt = date.to_date_time();
    assert_eq!(dt.year, 1978);
    assert_eq!(dt.month, 1);
    assert_eq!(dt.day, 1);

    let date2 = AmigaDate::default();
    assert_eq!(date2.days, 0);
    assert_eq!(date2.mins, 0);
    assert_eq!(date2.ticks, 0);
}

#[test]
fn test_error_display() {
    assert_eq!(format!("{}", AffsError::BlockReadError), "block read error");
    assert_eq!(
        format!("{}", AffsError::InvalidDosType),
        "invalid DOS type signature"
    );
    assert_eq!(
        format!("{}", AffsError::InvalidBlockType),
        "invalid block type"
    );
    assert_eq!(
        format!("{}", AffsError::InvalidSecType),
        "invalid secondary type"
    );
    assert_eq!(
        format!("{}", AffsError::ChecksumMismatch),
        "checksum mismatch"
    );
    assert_eq!(
        format!("{}", AffsError::BlockOutOfRange),
        "block out of range"
    );
    assert_eq!(format!("{}", AffsError::EntryNotFound), "entry not found");
    assert_eq!(format!("{}", AffsError::NameTooLong), "name too long");
    assert_eq!(
        format!("{}", AffsError::InvalidState),
        "invalid filesystem state"
    );
    assert_eq!(format!("{}", AffsError::EndOfFile), "end of file");
    assert_eq!(format!("{}", AffsError::NotAFile), "not a file");
    assert_eq!(format!("{}", AffsError::NotADirectory), "not a directory");
    assert_eq!(format!("{}", AffsError::BufferTooSmall), "buffer too small");
    assert_eq!(
        format!("{}", AffsError::InvalidDataSequence),
        "invalid data block sequence"
    );
}

#[test]
fn test_dir_entry_comment() {
    let device = create_test_disk();
    let reader = AffsReader::new(&device).unwrap();

    let entry = reader.find_entry(880, b"testfile").unwrap();
    assert_eq!(entry.comment(), b"");
    assert_eq!(entry.comment_str(), Some(""));
}

#[test]
fn test_hash_chain() {
    // Create disk with multiple files that hash to same bucket
    let mut device = MockDevice::new(1760);
    let (boot0, boot1) = create_boot_block();
    device.set_block(0, &boot0);
    device.set_block(1, &boot1);

    let mut root = create_root_block(b"ChainDisk");

    // First file
    let hash_idx = hash_name(b"file1", false);
    write_u32_be(&mut root, 24 + hash_idx * 4, 882);
    set_checksum(&mut root, 20);
    device.set_block(880, &root);

    // File1 header - points to file2 in same hash chain
    let mut file1 = create_file_header(b"file1", 10, 880, 884, &[884]);
    write_u32_be(&mut file1, 0x1F0, 883); // next_same_hash
    set_checksum(&mut file1, 20);
    device.set_block(882, &file1);

    // File2 header (in same hash chain)
    let file2 = create_file_header(b"file2", 20, 880, 885, &[885]);
    device.set_block(883, &file2);

    // Data blocks
    device.set_block(884, &[1u8; 512]);
    device.set_block(885, &[2u8; 512]);

    let reader = AffsReader::new(&device).unwrap();

    // Should find both files
    let entries: Vec<_> = reader.read_root_dir().filter_map(|e| e.ok()).collect();
    assert_eq!(entries.len(), 2);
}

#[test]
fn test_subdirectory() {
    let mut device = MockDevice::new(1760);
    let (boot0, boot1) = create_boot_block();
    device.set_block(0, &boot0);
    device.set_block(1, &boot1);

    let mut root = create_root_block(b"SubdirDisk");
    let hash_idx = hash_name(b"subdir", false);
    write_u32_be(&mut root, 24 + hash_idx * 4, 882);
    set_checksum(&mut root, 20);
    device.set_block(880, &root);

    // Subdirectory with a file
    let file_hash = hash_name(b"inner", false);
    let subdir = create_dir_header(b"subdir", 880, &[(file_hash, 884)]);
    device.set_block(882, &subdir);

    // File inside subdirectory
    let file = create_file_header(b"inner", 5, 882, 885, &[885]);
    device.set_block(884, &file);
    device.set_block(885, &[0xAB; 512]);

    let reader = AffsReader::new(&device).unwrap();

    // Find via path
    let entry = reader.find_path(b"subdir/inner").unwrap();
    assert_eq!(entry.name(), b"inner");
    assert_eq!(entry.size, 5);

    // Read subdir
    let subdir_entry = reader.find_entry(880, b"subdir").unwrap();
    assert!(subdir_entry.is_dir());

    let inner_entries: Vec<_> = reader.read_dir(subdir_entry.block).unwrap().collect();
    assert_eq!(inner_entries.len(), 1);
}

#[test]
fn test_file_with_extension_blocks() {
    // Create a file larger than 72 data blocks (requires extension)
    let mut device = MockDevice::new(1760);
    let (boot0, boot1) = create_boot_block();
    device.set_block(0, &boot0);
    device.set_block(1, &boot1);

    let mut root = create_root_block(b"ExtDisk");
    let hash_idx = hash_name(b"bigfile", false);
    write_u32_be(&mut root, 24 + hash_idx * 4, 882);
    set_checksum(&mut root, 20);
    device.set_block(880, &root);

    // File with 73 data blocks (1 more than fits in header)
    // First 72 blocks in header, 1 in extension
    let data_blocks: Vec<u32> = (890..962).collect(); // blocks 890-961 (72 blocks)

    let mut file = create_file_header(b"bigfile", 73 * 512, 880, 0, &data_blocks);
    write_u32_be(&mut file, 0x1F8, 883); // extension block
    set_checksum(&mut file, 20);
    device.set_block(882, &file);

    // Extension block with 1 more data block
    let ext = create_file_ext_block(882, &[962], 0);
    device.set_block(883, &ext);

    // Create all data blocks
    for i in 890..=962 {
        let mut block = [0u8; 512];
        block[0] = (i - 890) as u8;
        device.set_block(i, &block);
    }

    let reader = AffsReader::new(&device).unwrap();
    let mut file_reader = reader.read_file(882).unwrap();

    // Read first block
    let mut buf = [0u8; 512];
    let n = file_reader.read(&mut buf).unwrap();
    assert_eq!(n, 512);
    assert_eq!(buf[0], 0);

    // Skip to last block
    let mut big_buf = vec![0u8; 73 * 512];
    file_reader = reader.read_file(882).unwrap();
    let total = file_reader.read_all(&mut big_buf).unwrap();
    assert_eq!(total, 73 * 512);
}

#[test]
fn test_empty_file() {
    let mut device = MockDevice::new(1760);
    let (boot0, boot1) = create_boot_block();
    device.set_block(0, &boot0);
    device.set_block(1, &boot1);

    let mut root = create_root_block(b"EmptyDisk");
    let hash_idx = hash_name(b"empty", false);
    write_u32_be(&mut root, 24 + hash_idx * 4, 882);
    set_checksum(&mut root, 20);
    device.set_block(880, &root);

    // Empty file (size = 0, no data blocks)
    let file = create_file_header(b"empty", 0, 880, 0, &[]);
    device.set_block(882, &file);

    let reader = AffsReader::new(&device).unwrap();
    let mut file_reader = reader.read_file(882).unwrap();

    assert_eq!(file_reader.size(), 0);
    assert!(file_reader.is_eof());

    let mut buf = [0u8; 10];
    let n = file_reader.read(&mut buf).unwrap();
    assert_eq!(n, 0);
}

#[test]
fn test_file_reader_from_entry() {
    let device = create_test_disk();
    let reader = AffsReader::new(&device).unwrap();

    let entry_block = reader.read_entry(882).unwrap();
    let mut file_reader =
        FileReader::from_entry(reader.device(), reader.fs_type(), 882, &entry_block).unwrap();

    assert_eq!(file_reader.size(), 100);

    let mut buf = [0u8; 100];
    file_reader.read_all(&mut buf).unwrap();
}

#[test]
fn test_partial_reads() {
    let device = create_test_disk();
    let reader = AffsReader::new(&device).unwrap();

    let mut file_reader = reader.read_file(882).unwrap();

    // Read in small chunks
    let mut buf = [0u8; 10];
    let mut total = 0;

    while !file_reader.is_eof() {
        let n = file_reader.read(&mut buf).unwrap();
        if n == 0 {
            break;
        }
        total += n;
    }

    assert_eq!(total, 100);
}

#[test]
fn test_intl_hash_and_compare() {
    // Test international character handling
    let hash1 = hash_name(b"cafe", true);
    let hash2 = hash_name(b"CAFE", true);
    assert_eq!(hash1, hash2);

    assert!(names_equal(b"test", b"TEST", false));
    assert!(names_equal(b"test", b"TEST", true));

    // International chars (à = 224, À = 192)
    assert!(names_equal(&[224], &[192], true));
}

#[test]
fn test_boot_block_with_code() {
    // Boot block with executable code should verify checksum
    let mut device = MockDevice::new(1760);

    let mut block0 = [0u8; 512];
    let block1 = [0u8; 512];

    block0[0] = b'D';
    block0[1] = b'O';
    block0[2] = b'S';
    block0[3] = 1;
    write_u32_be(&mut block0, 8, 880);

    // Add some "boot code" (non-zero at offset 12)
    block0[12] = 0x60; // Some code
    block0[13] = 0x00;

    // Calculate boot checksum
    let mut full_boot = [0u8; 1024];
    full_boot[..512].copy_from_slice(&block0);
    full_boot[512..].copy_from_slice(&block1);

    let mut sum: u32 = 0;
    for i in 0..256 {
        if i != 1 {
            let d = u32::from_be_bytes([
                full_boot[i * 4],
                full_boot[i * 4 + 1],
                full_boot[i * 4 + 2],
                full_boot[i * 4 + 3],
            ]);
            let new_sum = sum.wrapping_add(d);
            if new_sum < sum {
                sum = new_sum.wrapping_add(1);
            } else {
                sum = new_sum;
            }
        }
    }
    let checksum = !sum;
    write_u32_be(&mut block0, 4, checksum);

    device.set_block(0, &block0);
    device.set_block(1, &block1);

    let root = create_root_block(b"BootDisk");
    device.set_block(880, &root);

    let reader = AffsReader::new(&device).unwrap();
    assert_eq!(reader.disk_name(), b"BootDisk");
}

#[test]
fn test_root_hash_table() {
    let device = create_test_disk();
    let reader = AffsReader::new(&device).unwrap();

    let hash_table = reader.root_hash_table();
    assert_eq!(hash_table.len(), 72);

    // Should have one entry
    let hash_idx = hash_name(b"testfile", false);
    assert_eq!(hash_table[hash_idx], 882);
}

#[test]
fn test_entry_block_data_block_accessor() {
    let file = create_file_header(b"test", 512, 880, 100, &[100, 101, 102]);
    let entry = EntryBlock::parse(&file).unwrap();

    assert_eq!(entry.data_block(0), 100);
    assert_eq!(entry.data_block(1), 101);
    assert_eq!(entry.data_block(2), 102);
    assert_eq!(entry.data_block(100), 0); // Out of range
}

#[test]
fn test_file_ext_block_data_block_accessor() {
    let ext = create_file_ext_block(882, &[200, 201, 202], 0);
    let ext_block = FileExtBlock::parse(&ext).unwrap();

    assert_eq!(ext_block.data_block(0), 200);
    assert_eq!(ext_block.data_block(1), 201);
    assert_eq!(ext_block.data_block(2), 202);
    assert_eq!(ext_block.data_block(100), 0);
}

#[test]
fn test_ofs_data_block_parsing() {
    let data = [0xAB; 488];
    let block = create_ofs_data_block(882, 1, &data, 884);

    let ofs = OfsDataBlock::parse(&block).unwrap();
    assert_eq!(ofs.header_key, 882);
    assert_eq!(ofs.seq_num, 1);
    assert_eq!(ofs.data_size, 488);
    assert_eq!(ofs.next_data, 884);

    let data_slice = OfsDataBlock::data(&block);
    assert_eq!(data_slice.len(), 488);
    assert_eq!(data_slice[0], 0xAB);
}

#[test]
fn test_invalid_ofs_data_block() {
    let mut block = [0u8; 512];
    write_i32_be(&mut block, 0, 99); // Invalid type
    set_checksum(&mut block, 20);

    let result = OfsDataBlock::parse(&block);
    assert!(matches!(result, Err(AffsError::InvalidBlockType)));
}

#[test]
fn test_invalid_file_ext_block() {
    let mut block = [0u8; 512];
    write_i32_be(&mut block, 0, 2); // T_HEADER instead of T_LIST
    set_checksum(&mut block, 20);

    let result = FileExtBlock::parse(&block);
    assert!(matches!(result, Err(AffsError::InvalidBlockType)));
}

#[test]
fn test_entry_block_entry_type() {
    // File
    let file = create_file_header(b"test", 0, 880, 0, &[]);
    let entry = EntryBlock::parse(&file).unwrap();
    assert_eq!(entry.entry_type(), Some(EntryType::File));

    // Directory
    let dir = create_dir_header(b"dir", 880, &[]);
    let entry = EntryBlock::parse(&dir).unwrap();
    assert_eq!(entry.entry_type(), Some(EntryType::Dir));
}

#[test]
fn test_read_empty_buffer() {
    let device = create_test_disk();
    let reader = AffsReader::new(&device).unwrap();
    let mut file_reader = reader.read_file(882).unwrap();

    let mut buf = [0u8; 0];
    let n = file_reader.read(&mut buf).unwrap();
    assert_eq!(n, 0);
}

#[test]
fn test_ofs_linked_list_reading() {
    // Create OFS file with multiple linked data blocks
    let mut device = MockDevice::new(1760);

    let mut block0 = [0u8; 512];
    block0[0] = b'D';
    block0[1] = b'O';
    block0[2] = b'S';
    block0[3] = 0; // OFS
    write_u32_be(&mut block0, 8, 880);
    device.set_block(0, &block0);
    device.set_block(1, &[0u8; 512]);

    let mut root = create_root_block(b"OFSMulti");
    let hash_idx = hash_name(b"multiofs", false);
    write_u32_be(&mut root, 24 + hash_idx * 4, 882);
    set_checksum(&mut root, 20);
    device.set_block(880, &root);

    // File with 2 OFS data blocks (488 + 12 = 500 bytes)
    let file_header = create_file_header(b"multiofs", 500, 880, 883, &[883, 884]);
    device.set_block(882, &file_header);

    // First OFS data block
    let data1 = [0xAA; 488];
    let ofs1 = create_ofs_data_block(882, 1, &data1, 884);
    device.set_block(883, &ofs1);

    // Second OFS data block
    let data2 = [0xBB; 12];
    let ofs2 = create_ofs_data_block(882, 2, &data2, 0);
    device.set_block(884, &ofs2);

    let reader = AffsReader::new(&device).unwrap();
    let mut file_reader = reader.read_file(882).unwrap();

    let mut buf = vec![0u8; 600];
    let n = file_reader.read_all(&mut buf).unwrap();
    assert_eq!(n, 500);

    // Verify data
    assert!(buf[..488].iter().all(|&b| b == 0xAA));
    assert!(buf[488..500].iter().all(|&b| b == 0xBB));
}

#[test]
fn test_with_size() {
    let mut device = MockDevice::new(2000);

    let (boot0, boot1) = create_boot_block();
    device.set_block(0, &boot0);
    device.set_block(1, &boot1);

    // Root at custom position
    let root = create_root_block(b"CustomSize");
    device.set_block(1000, &root);

    // Update boot block
    let block0 = device.get_block_mut(0);
    write_u32_be(block0, 8, 1000);

    let reader = AffsReader::with_size(&device, 2000).unwrap();
    assert_eq!(reader.total_blocks(), 2000);
    assert_eq!(reader.root_block(), 1000);
}

#[test]
fn test_block_out_of_range() {
    let mut device = MockDevice::new(100); // Very small disk

    let mut block0 = [0u8; 512];
    block0[0] = b'D';
    block0[1] = b'O';
    block0[2] = b'S';
    block0[3] = 1;
    write_u32_be(&mut block0, 8, 200); // Root block out of range
    device.set_block(0, &block0);
    device.set_block(1, &[0u8; 512]);

    let result = AffsReader::with_size(&device, 100);
    assert!(matches!(result, Err(AffsError::BlockOutOfRange)));
}

#[test]
fn test_find_path_empty() {
    let device = create_test_disk();
    let reader = AffsReader::new(&device).unwrap();

    let result = reader.find_path(b"");
    assert!(matches!(result, Err(AffsError::EntryNotFound)));

    let result2 = reader.find_path(b"/");
    assert!(matches!(result2, Err(AffsError::EntryNotFound)));
}

#[test]
fn test_hard_link_types() {
    let mut device = MockDevice::new(1760);
    let (boot0, boot1) = create_boot_block();
    device.set_block(0, &boot0);
    device.set_block(1, &boot1);

    let mut root = create_root_block(b"LinkDisk");
    let hash_idx = hash_name(b"hardlink", false);
    write_u32_be(&mut root, 24 + hash_idx * 4, 882);
    set_checksum(&mut root, 20);
    device.set_block(880, &root);

    // Create a hard link to file (ST_LFILE = -4)
    let mut link = [0u8; 512];
    write_i32_be(&mut link, 0, 2); // T_HEADER
    link[0x1B0] = 8;
    link[0x1B1..0x1B9].copy_from_slice(b"hardlink");
    write_u32_be(&mut link, 0x1F4, 880); // parent
    write_i32_be(&mut link, 0x1FC, -4); // ST_LFILE
    set_checksum(&mut link, 20);
    device.set_block(882, &link);

    let reader = AffsReader::new(&device).unwrap();
    let entry = reader.find_entry(880, b"hardlink").unwrap();
    assert_eq!(entry.entry_type, EntryType::HardLinkFile);
    assert!(entry.is_file());
}

#[test]
fn test_soft_link_type() {
    let mut device = MockDevice::new(1760);
    let (boot0, boot1) = create_boot_block();
    device.set_block(0, &boot0);
    device.set_block(1, &boot1);

    let mut root = create_root_block(b"SoftDisk");
    let hash_idx = hash_name(b"softlink", false);
    write_u32_be(&mut root, 24 + hash_idx * 4, 882);
    set_checksum(&mut root, 20);
    device.set_block(880, &root);

    // Create a soft link (ST_LSOFT = 3)
    let mut link = [0u8; 512];
    write_i32_be(&mut link, 0, 2); // T_HEADER
    link[0x1B0] = 8;
    link[0x1B1..0x1B9].copy_from_slice(b"softlink");
    write_u32_be(&mut link, 0x1F4, 880);
    write_i32_be(&mut link, 0x1FC, 3); // ST_LSOFT
    set_checksum(&mut link, 20);
    device.set_block(882, &link);

    let reader = AffsReader::new(&device).unwrap();
    let entry = reader.find_entry(880, b"softlink").unwrap();
    assert_eq!(entry.entry_type, EntryType::SoftLink);
}

#[test]
fn test_hard_link_dir_type() {
    let mut device = MockDevice::new(1760);
    let (boot0, boot1) = create_boot_block();
    device.set_block(0, &boot0);
    device.set_block(1, &boot1);

    let mut root = create_root_block(b"DirLinkDisk");
    let hash_idx = hash_name(b"dirlink", false);
    write_u32_be(&mut root, 24 + hash_idx * 4, 882);
    set_checksum(&mut root, 20);
    device.set_block(880, &root);

    // Create a hard link to dir (ST_LDIR = 4)
    let mut link = [0u8; 512];
    write_i32_be(&mut link, 0, 2); // T_HEADER
    link[0x1B0] = 7;
    link[0x1B1..0x1B8].copy_from_slice(b"dirlink");
    write_u32_be(&mut link, 0x1F4, 880);
    write_i32_be(&mut link, 0x1FC, 4); // ST_LDIR
    set_checksum(&mut link, 20);
    device.set_block(882, &link);

    let reader = AffsReader::new(&device).unwrap();
    let entry = reader.find_entry(880, b"dirlink").unwrap();
    assert_eq!(entry.entry_type, EntryType::HardLinkDir);
    assert!(entry.is_dir());
}

#[test]
fn test_file_with_comment() {
    let mut device = MockDevice::new(1760);
    let (boot0, boot1) = create_boot_block();
    device.set_block(0, &boot0);
    device.set_block(1, &boot1);

    let mut root = create_root_block(b"CommentDisk");
    let hash_idx = hash_name(b"commented", false);
    write_u32_be(&mut root, 24 + hash_idx * 4, 882);
    set_checksum(&mut root, 20);
    device.set_block(880, &root);

    // File with comment
    let mut file = create_file_header(b"commented", 0, 880, 0, &[]);
    // Add comment at offset 0x148
    let comment = b"This is a test comment";
    file[0x148] = comment.len() as u8;
    file[0x149..0x149 + comment.len()].copy_from_slice(comment);
    set_checksum(&mut file, 20);
    device.set_block(882, &file);

    let reader = AffsReader::new(&device).unwrap();
    let entry = reader.find_entry(880, b"commented").unwrap();
    assert_eq!(entry.comment(), b"This is a test comment");
    assert_eq!(entry.comment_str(), Some("This is a test comment"));
}

#[test]
fn test_default_root_block_calculation() {
    // When boot block has root_block = 0, use middle of disk
    let mut device = MockDevice::new(1760);

    let mut block0 = [0u8; 512];
    block0[0] = b'D';
    block0[1] = b'O';
    block0[2] = b'S';
    block0[3] = 1;
    write_u32_be(&mut block0, 8, 0); // Root block = 0 (use default)
    device.set_block(0, &block0);
    device.set_block(1, &[0u8; 512]);

    let root = create_root_block(b"DefaultRoot");
    device.set_block(880, &root); // 1760/2 = 880

    let reader = AffsReader::new(&device).unwrap();
    assert_eq!(reader.root_block(), 880);
}

#[cfg(feature = "std")]
#[test]
fn test_error_is_std_error() {
    fn assert_error<T: std::error::Error>() {}
    assert_error::<AffsError>();
}

#[test]
fn test_boot_block_checksum_mismatch_with_code() {
    // Boot block with boot code but INVALID checksum should fail
    let mut device = MockDevice::new(1760);

    let mut block0 = [0u8; 512];
    let block1 = [0u8; 512];

    block0[0] = b'D';
    block0[1] = b'O';
    block0[2] = b'S';
    block0[3] = 1;
    write_u32_be(&mut block0, 8, 880);

    // Add "boot code" (non-zero at offset 12) to trigger checksum verification
    block0[12] = 0x60;
    block0[13] = 0x00;

    // Set an INCORRECT checksum (not the correct boot checksum)
    write_u32_be(&mut block0, 4, 0xDEADBEEF);

    device.set_block(0, &block0);
    device.set_block(1, &block1);

    let root = create_root_block(b"BadBoot");
    device.set_block(880, &root);

    let result = AffsReader::new(&device);
    assert!(matches!(result, Err(AffsError::ChecksumMismatch)));
}

#[test]
fn test_entry_block_invalid_block_type() {
    // Create an entry block with wrong block type
    let mut block = [0u8; 512];
    write_i32_be(&mut block, 0, 99); // Invalid type (not T_HEADER=2)
    write_i32_be(&mut block, 0x1FC, -3); // ST_FILE
    set_checksum(&mut block, 20);

    let result = EntryBlock::parse(&block);
    assert!(matches!(result, Err(AffsError::InvalidBlockType)));
}

#[test]
fn test_entry_block_checksum_mismatch() {
    // Create an entry block with bad checksum
    let mut block = create_file_header(b"test", 100, 880, 883, &[883]);
    // Corrupt data without updating checksum
    block[100] = 0xFF;

    let result = EntryBlock::parse(&block);
    assert!(matches!(result, Err(AffsError::ChecksumMismatch)));
}

#[test]
fn test_file_ext_block_checksum_mismatch() {
    // Create file extension block with bad checksum
    let mut block = create_file_ext_block(882, &[200, 201], 0);
    // Corrupt data without updating checksum
    block[100] = 0xFF;

    let result = FileExtBlock::parse(&block);
    assert!(matches!(result, Err(AffsError::ChecksumMismatch)));
}

#[test]
fn test_ofs_data_block_checksum_mismatch() {
    // Create OFS data block with bad checksum
    let mut block = create_ofs_data_block(882, 1, &[0xAB; 100], 0);
    // Corrupt data without updating checksum
    block[100] = 0xFF;

    let result = OfsDataBlock::parse(&block);
    assert!(matches!(result, Err(AffsError::ChecksumMismatch)));
}

#[test]
fn test_names_equal_char_mismatch() {
    // Test names_equal returning false when characters don't match
    assert!(!names_equal(b"abc", b"abd", false));
    assert!(!names_equal(b"abc", b"abd", true));
    assert!(!names_equal(b"ABC", b"ABD", false));

    // Different at various positions
    assert!(!names_equal(b"xbc", b"abc", false));
    assert!(!names_equal(b"axc", b"abc", false));
}

#[test]
fn test_boot_sum_overflow() {
    // Create a boot block that causes overflow during checksum calculation
    let mut boot_buf = [0u8; 1024];

    // Fill with large values to trigger overflow
    // We need to write u32 values at positions that will cause overflow
    for i in 0..256 {
        if i != 1 {
            // Skip checksum position
            let offset = i * 4;
            boot_buf[offset] = 0xFF;
            boot_buf[offset + 1] = 0xFF;
            boot_buf[offset + 2] = 0xFF;
            boot_buf[offset + 3] = 0xFF;
        }
    }

    // DOS signature
    boot_buf[0] = b'D';
    boot_buf[1] = b'O';
    boot_buf[2] = b'S';
    boot_buf[3] = 1;

    // Calculate the correct checksum using the boot_sum algorithm
    use affs_read::boot_sum;
    let checksum = boot_sum(&boot_buf);
    boot_buf[4] = (checksum >> 24) as u8;
    boot_buf[5] = (checksum >> 16) as u8;
    boot_buf[6] = (checksum >> 8) as u8;
    boot_buf[7] = checksum as u8;

    // Verify the boot block can be parsed (checksum should be valid)
    let result = BootBlock::parse(&boot_buf);
    assert!(result.is_ok());
}

#[test]
fn test_bitmap_sum() {
    use affs_read::bitmap_sum;

    let mut block = [0u8; 512];

    // Set some values in the block
    for i in 1..128 {
        write_u32_be(&mut block, i * 4, i as u32);
    }

    let sum = bitmap_sum(&block);

    // Verify the sum is calculated correctly
    // bitmap_sum subtracts values at indices 1..128
    let mut expected: u32 = 0;
    for i in 1..128 {
        expected = expected.wrapping_sub(i as u32);
    }
    assert_eq!(sum, expected);
}

#[test]
fn test_read_u16_be() {
    use affs_read::read_u16_be;

    let mut buf = [0u8; 512];
    buf[0] = 0x12;
    buf[1] = 0x34;
    buf[10] = 0xAB;
    buf[11] = 0xCD;

    assert_eq!(read_u16_be(&buf, 0), 0x1234);
    assert_eq!(read_u16_be(&buf, 10), 0xABCD);
}

#[test]
fn test_file_reader_seek_forward() {
    let device = create_test_disk();
    let reader = AffsReader::new(&device).unwrap();

    let mut file_reader = reader.read_file(882).unwrap();
    assert_eq!(file_reader.position(), 0);

    // Seek forward
    file_reader.seek(50).unwrap();
    assert_eq!(file_reader.position(), 50);

    // Read remaining data
    let mut buf = [0u8; 100];
    let n = file_reader.read(&mut buf).unwrap();
    assert_eq!(n, 50); // 100 - 50 = 50 remaining
}

#[test]
fn test_file_reader_seek_to_same_position() {
    let device = create_test_disk();
    let reader = AffsReader::new(&device).unwrap();

    let mut file_reader = reader.read_file(882).unwrap();

    // Read some data first
    let mut buf = [0u8; 20];
    file_reader.read(&mut buf).unwrap();
    assert_eq!(file_reader.position(), 20);

    // Seek to same position should be no-op
    file_reader.seek(20).unwrap();
    assert_eq!(file_reader.position(), 20);
}

#[test]
fn test_file_reader_seek_past_eof() {
    let device = create_test_disk();
    let reader = AffsReader::new(&device).unwrap();

    let mut file_reader = reader.read_file(882).unwrap();

    // Seek past EOF
    let result = file_reader.seek(200); // File is only 100 bytes
    assert!(matches!(result, Err(AffsError::EndOfFile)));
}

#[test]
fn test_file_reader_seek_backward() {
    let device = create_test_disk();
    let reader = AffsReader::new(&device).unwrap();

    let mut file_reader = reader.read_file(882).unwrap();

    // Read some data first
    let mut buf = [0u8; 50];
    file_reader.read(&mut buf).unwrap();
    assert_eq!(file_reader.position(), 50);

    // Seek backward should now work
    file_reader.seek(20).unwrap();
    assert_eq!(file_reader.position(), 20);
    assert_eq!(file_reader.remaining(), file_reader.size() - 20);

    // Read from the new position
    let mut buf2 = [0u8; 10];
    file_reader.read(&mut buf2).unwrap();
    assert_eq!(file_reader.position(), 30);

    // Verify data matches what we read before at the same offset
    assert_eq!(&buf2[..], &buf[20..30]);
}

#[test]
fn test_file_reader_reset() {
    let device = create_test_disk();
    let reader = AffsReader::new(&device).unwrap();

    let mut file_reader = reader.read_file(882).unwrap();

    // Read entire file
    let mut buf1 = [0u8; 100];
    let n = file_reader.read_all(&mut buf1).unwrap();
    assert_eq!(n, 100);
    assert!(file_reader.is_eof());

    // Reset and read again
    file_reader.reset();
    assert_eq!(file_reader.position(), 0);
    assert_eq!(file_reader.remaining(), file_reader.size());
    assert!(!file_reader.is_eof());

    // Read again and verify same content
    let mut buf2 = [0u8; 100];
    let n = file_reader.read_all(&mut buf2).unwrap();
    assert_eq!(n, 100);
    assert_eq!(buf1, buf2);
}

#[test]
fn test_entry_block_comment_method() {
    // Create a file with a comment and test the EntryBlock::comment() method
    let mut file = create_file_header(b"commented", 0, 880, 0, &[]);
    let comment = b"Test comment here";
    file[0x148] = comment.len() as u8;
    file[0x149..0x149 + comment.len()].copy_from_slice(comment);
    set_checksum(&mut file, 20);

    let entry = EntryBlock::parse(&file).unwrap();
    assert_eq!(entry.comment(), b"Test comment here");
}

// ============================================================================
// New Feature Tests: Symlink reading, Volume label, Modification time
// ============================================================================

/// Create a soft link block.
fn create_softlink(name: &[u8], target: &[u8], parent: u32) -> [u8; 512] {
    let mut buf = [0u8; 512];

    // Block type = T_HEADER (2)
    write_i32_be(&mut buf, 0, 2);

    // Symlink target at offset 24 (GRUB_AFFS_SYMLINK_OFFSET)
    let target_len = target.len().min(288); // Max symlink length for 512 block
    buf[24..24 + target_len].copy_from_slice(&target[..target_len]);

    // Name
    let name_len = name.len().min(30);
    buf[0x1B0] = name_len as u8;
    buf[0x1B1..0x1B1 + name_len].copy_from_slice(&name[..name_len]);

    // Parent
    write_u32_be(&mut buf, 0x1F4, parent);

    // Secondary type = ST_LSOFT (3)
    write_i32_be(&mut buf, 0x1FC, 3);

    // Set checksum
    set_checksum(&mut buf, 20);

    buf
}

#[test]
fn test_symlink_reading() {
    let mut device = MockDevice::new(1760);
    let (boot0, boot1) = create_boot_block();
    device.set_block(0, &boot0);
    device.set_block(1, &boot1);

    let mut root = create_root_block(b"SymlinkDisk");
    let hash_idx = hash_name(b"mylink", false);
    write_u32_be(&mut root, 24 + hash_idx * 4, 882);
    set_checksum(&mut root, 20);
    device.set_block(880, &root);

    // Create a symlink pointing to "path/to/target"
    let symlink = create_softlink(b"mylink", b"path/to/target\0", 880);
    device.set_block(882, &symlink);

    let reader = AffsReader::new(&device).unwrap();

    // Find the symlink entry
    let entry = reader.find_entry(880, b"mylink").unwrap();
    assert!(entry.is_symlink());
    assert_eq!(entry.entry_type, EntryType::SoftLink);

    // Read the symlink target
    let mut target_buf = [0u8; 512];
    let len = reader.read_symlink(entry.block, &mut target_buf).unwrap();
    assert_eq!(&target_buf[..len], b"path/to/target");
}

#[test]
fn test_symlink_colon_replacement() {
    let mut device = MockDevice::new(1760);
    let (boot0, boot1) = create_boot_block();
    device.set_block(0, &boot0);
    device.set_block(1, &boot1);

    let mut root = create_root_block(b"AmigaLink");
    let hash_idx = hash_name(b"bootlink", false);
    write_u32_be(&mut root, 24 + hash_idx * 4, 882);
    set_checksum(&mut root, 20);
    device.set_block(880, &root);

    // Create a symlink with Amiga volume reference (colon at start)
    let symlink = create_softlink(b"bootlink", b":boot/kernel\0", 880);
    device.set_block(882, &symlink);

    let reader = AffsReader::new(&device).unwrap();

    let mut target_buf = [0u8; 512];
    let len = reader.read_symlink(882, &mut target_buf).unwrap();

    // The leading : should be replaced with /
    assert_eq!(&target_buf[..len], b"/boot/kernel");
}

#[test]
fn test_symlink_latin1_to_utf8() {
    let mut device = MockDevice::new(1760);
    let (boot0, boot1) = create_boot_block();
    device.set_block(0, &boot0);
    device.set_block(1, &boot1);

    let mut root = create_root_block(b"Latin1Disk");
    let hash_idx = hash_name(b"intllink", false);
    write_u32_be(&mut root, 24 + hash_idx * 4, 882);
    set_checksum(&mut root, 20);
    device.set_block(880, &root);

    // Create a symlink with Latin1 characters (e-acute = 0xE9)
    let target = [b'c', b'a', b'f', 0xE9, 0]; // "cafe" with accent
    let symlink = create_softlink(b"intllink", &target, 880);
    device.set_block(882, &symlink);

    let reader = AffsReader::new(&device).unwrap();

    let mut target_buf = [0u8; 512];
    let len = reader.read_symlink(882, &mut target_buf).unwrap();

    // Latin1 0xE9 should become UTF-8 0xC3 0xA9
    assert_eq!(&target_buf[..len], &[b'c', b'a', b'f', 0xC3, 0xA9]);
}

#[test]
fn test_symlink_not_a_symlink_error() {
    let device = create_test_disk();
    let reader = AffsReader::new(&device).unwrap();

    // Try to read a regular file as symlink
    let mut target_buf = [0u8; 512];
    let result = reader.read_symlink(882, &mut target_buf);
    assert!(matches!(result, Err(AffsError::NotASymlink)));
}

#[test]
fn test_read_symlink_entry() {
    let mut device = MockDevice::new(1760);
    let (boot0, boot1) = create_boot_block();
    device.set_block(0, &boot0);
    device.set_block(1, &boot1);

    let mut root = create_root_block(b"EntryLink");
    let hash_idx = hash_name(b"link", false);
    write_u32_be(&mut root, 24 + hash_idx * 4, 882);
    set_checksum(&mut root, 20);
    device.set_block(880, &root);

    let symlink = create_softlink(b"link", b"target\0", 880);
    device.set_block(882, &symlink);

    let reader = AffsReader::new(&device).unwrap();
    let entry = reader.find_entry(880, b"link").unwrap();

    let mut target_buf = [0u8; 512];
    let len = reader.read_symlink_entry(&entry, &mut target_buf).unwrap();
    assert_eq!(&target_buf[..len], b"target");
}

#[test]
fn test_volume_label_api() {
    let device = create_test_disk();
    let reader = AffsReader::new(&device).unwrap();

    // label() is an alias for disk_name()
    assert_eq!(reader.label(), b"TestDisk");
    assert_eq!(reader.label_str(), Some("TestDisk"));
    assert_eq!(reader.label(), reader.disk_name());
}

#[test]
fn test_modification_time() {
    let mut device = MockDevice::new(1760);
    let (boot0, boot1) = create_boot_block();
    device.set_block(0, &boot0);
    device.set_block(1, &boot1);

    // Create root block with specific dates
    let mut root = [0u8; 512];
    write_i32_be(&mut root, 0, 2); // T_HEADER
    write_i32_be(&mut root, 12, 72); // hash table size
    write_i32_be(&mut root, 0x138, -1); // bitmap valid

    // Disk name
    root[0x1B0] = 8;
    root[0x1B1..0x1B9].copy_from_slice(b"TimeDisk");

    // Creation date (days=0, mins=0, ticks=0 -> 1978-01-01 00:00:00)
    write_i32_be(&mut root, 0x1A4, 0);
    write_i32_be(&mut root, 0x1A8, 0);
    write_i32_be(&mut root, 0x1AC, 0);

    // Last modified date: 365 days, 60 mins (1 hour), 100 ticks (2 seconds)
    // = 1979-01-01 01:00:02
    write_i32_be(&mut root, 0x1D8, 365);
    write_i32_be(&mut root, 0x1DC, 60);
    write_i32_be(&mut root, 0x1E0, 100);

    write_i32_be(&mut root, 508, 1); // ST_ROOT
    set_checksum(&mut root, 20);
    device.set_block(880, &root);

    let reader = AffsReader::new(&device).unwrap();

    // Check creation date
    let creation = reader.creation_date();
    assert_eq!(creation.days, 0);
    assert_eq!(creation.mins, 0);
    assert_eq!(creation.ticks, 0);
    let dt = creation.to_date_time();
    assert_eq!(dt.year, 1978);
    assert_eq!(dt.month, 1);
    assert_eq!(dt.day, 1);

    // Check last modified date
    let modified = reader.last_modified();
    assert_eq!(modified.days, 365);
    assert_eq!(modified.mins, 60);
    assert_eq!(modified.ticks, 100);
    let dt = modified.to_date_time();
    assert_eq!(dt.year, 1979);
    assert_eq!(dt.month, 1);
    assert_eq!(dt.day, 1);
    assert_eq!(dt.hour, 1);
    assert_eq!(dt.minute, 0);
    assert_eq!(dt.second, 2);

    // Check Unix timestamp (mtime)
    // mtime = days * 86400 + mins * 60 + ticks / 50 + epoch_offset
    // epoch_offset = 2922 * 86400 = 252460800
    // mtime = 365 * 86400 + 60 * 60 + 100 / 50 + 252460800
    //       = 31536000 + 3600 + 2 + 252460800 = 284000402
    let mtime = reader.mtime();
    let expected = 365i64 * 86400 + 60 * 60 + 100 / 50 + 2922 * 86400;
    assert_eq!(mtime, expected);
}

#[test]
fn test_amiga_date_to_unix_timestamp() {
    // Test the AmigaDate::to_unix_timestamp method directly
    let date = AmigaDate::new(0, 0, 0); // 1978-01-01 00:00:00
    let ts = date.to_unix_timestamp();

    // Should be 8 years after Unix epoch
    // 1970 + 8 = 1978
    // Leap years: 1972, 1976 (2 leap days)
    // Days: 8 * 365 + 2 = 2922
    // Seconds: 2922 * 86400 = 252460800
    assert_eq!(ts, 252460800);

    // Test with some days and minutes
    let date2 = AmigaDate::new(1, 60, 50); // 1 day, 1 hour, 1 second later
    let ts2 = date2.to_unix_timestamp();
    // 1 * 86400 + 60 * 60 + 50 / 50 + 252460800 = 86400 + 3600 + 1 + 252460800 = 252550801
    assert_eq!(ts2, 252550801);
}

#[test]
fn test_dir_entry_is_symlink() {
    let mut device = MockDevice::new(1760);
    let (boot0, boot1) = create_boot_block();
    device.set_block(0, &boot0);
    device.set_block(1, &boot1);

    let mut root = create_root_block(b"SymDisk");
    let hash_idx = hash_name(b"sym", false);
    write_u32_be(&mut root, 24 + hash_idx * 4, 882);
    set_checksum(&mut root, 20);
    device.set_block(880, &root);

    let symlink = create_softlink(b"sym", b"target\0", 880);
    device.set_block(882, &symlink);

    let reader = AffsReader::new(&device).unwrap();
    let entry = reader.find_entry(880, b"sym").unwrap();

    assert!(entry.is_symlink());
    assert!(!entry.is_file());
    assert!(!entry.is_dir());
}

#[test]
fn test_symlink_functions() {
    // Test the low-level symlink functions directly
    use affs_read::{MAX_SYMLINK_LEN, max_utf8_len, read_symlink_target};

    // MAX_SYMLINK_LEN should be 512 - 24 - 200 = 288
    assert_eq!(MAX_SYMLINK_LEN, 288);

    // max_utf8_len should double the input (worst case)
    assert_eq!(max_utf8_len(100), 200);
    assert_eq!(max_utf8_len(0), 0);

    // Test read_symlink_target
    let mut buf = [0u8; 512];
    buf[24..30].copy_from_slice(b"hello\0");
    let mut out = [0u8; 100];
    let len = read_symlink_target(&buf, &mut out);
    assert_eq!(len, 5);
    assert_eq!(&out[..len], b"hello");
}
