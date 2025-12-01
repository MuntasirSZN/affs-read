//! Constants for AFFS filesystem.

/// Logical block size in bytes.
pub const BLOCK_SIZE: usize = 512;

/// Boot block size (2 blocks).
pub const BOOT_BLOCK_SIZE: usize = 1024;

/// Hash table size (entries per directory).
pub const HASH_TABLE_SIZE: usize = 72;

/// Maximum data block pointers per file header or extension block.
pub const MAX_DATABLK: usize = 72;

/// Maximum filename length.
pub const MAX_NAME_LEN: usize = 30;

/// Maximum comment length.
pub const MAX_COMMENT_LEN: usize = 79;

/// Bitmap pages in root block.
pub const BM_PAGES_ROOT_SIZE: usize = 25;

/// Bitmap pages in extension block.
pub const BM_PAGES_EXT_SIZE: usize = 127;

/// Bitmap map entries.
pub const BM_MAP_SIZE: usize = 127;

/// Standard floppy disk sector count (DD: 880KB).
pub const FLOPPY_DD_SECTORS: u32 = 1760;

/// Standard floppy disk sector count (HD: 1.76MB).
pub const FLOPPY_HD_SECTORS: u32 = 3520;

/// Sectors per track (DD).
pub const SECTORS_PER_TRACK_DD: u32 = 11;

/// Sectors per track (HD).
pub const SECTORS_PER_TRACK_HD: u32 = 22;

/// Number of heads.
pub const HEADS: u32 = 2;

/// Number of cylinders (tracks).
pub const CYLINDERS: u32 = 80;

// Filesystem type flags (in dosType[3])
/// Original File System.
pub const DOSFS_OFS: u8 = 0;
/// Fast File System.
pub const DOSFS_FFS: u8 = 1;
/// International mode (case-insensitive for international characters).
pub const DOSFS_INTL: u8 = 2;
/// Directory cache mode.
pub const DOSFS_DIRCACHE: u8 = 4;

// Block types
/// Header block type.
pub const T_HEADER: i32 = 2;
/// Data block type (OFS only).
pub const T_DATA: i32 = 8;
/// List/extension block type.
pub const T_LIST: i32 = 16;
/// Directory cache block type.
pub const T_DIRC: i32 = 33;

// Secondary types
/// Root block secondary type.
pub const ST_ROOT: i32 = 1;
/// Directory secondary type.
pub const ST_DIR: i32 = 2;
/// Soft link secondary type.
pub const ST_LSOFT: i32 = 3;
/// Hard link to directory secondary type.
pub const ST_LDIR: i32 = 4;
/// File secondary type.
pub const ST_FILE: i32 = -3;
/// Hard link to file secondary type.
pub const ST_LFILE: i32 = -4;

// Access flags
/// Delete protected.
pub const ACC_DELETE: u32 = 1 << 0;
/// Execute protected.
pub const ACC_EXECUTE: u32 = 1 << 1;
/// Write protected.
pub const ACC_WRITE: u32 = 1 << 2;
/// Read protected.
pub const ACC_READ: u32 = 1 << 3;
/// Archived.
pub const ACC_ARCHIVE: u32 = 1 << 4;
/// Pure (re-entrant).
pub const ACC_PURE: u32 = 1 << 5;
/// Script.
pub const ACC_SCRIPT: u32 = 1 << 6;
/// Hidden.
pub const ACC_HOLD: u32 = 1 << 7;

/// Valid bitmap flag value.
pub const BM_VALID: i32 = -1;

/// OFS data block payload size.
pub const OFS_DATA_SIZE: usize = 488;

/// FFS data block payload size (full block).
pub const FFS_DATA_SIZE: usize = 512;

// Variable block size constants (GRUB parity)
/// Maximum log2 block size (512 << 4 = 8192 bytes).
pub const MAX_LOG_BLOCK_SIZE: u8 = 4;

/// Maximum boot block location to probe (sector 0 and 1).
pub const MAX_BOOT_BLOCK: u32 = 1;

/// Symlink target offset in block (same as hash table offset).
pub const SYMLINK_OFFSET: usize = 24;

/// File header structure offset from end of block.
pub const FILE_LOCATION: usize = 200;

/// Amiga epoch: January 1, 1978 00:00:00 UTC.
/// Offset from Unix epoch (January 1, 1970) in seconds.
/// 8 years = 2922 days (including leap years 1972, 1976).
pub const AMIGA_EPOCH_OFFSET: i64 = 252288000;

/// Supported block sizes for probing.
pub const BLOCK_SIZES: [usize; 5] = [512, 1024, 2048, 4096, 8192];
