//! # affs-read
//!
//! A `no_std` compatible crate for reading Amiga Fast File System (AFFS) disk images.
//!
//! This crate provides zero-allocation reading of ADF (Amiga Disk File) images,
//! supporting both OFS (Original File System) and FFS (Fast File System) variants.
//!
//! ## Features
//!
//! - `no_std` compatible by default
//! - Zero heap allocations in core functionality
//! - Optimized for performance with byte-level operations and branchless algorithms
//! - Support for OFS and FFS filesystems
//! - Support for INTL and DIRCACHE modes
//! - Streaming file reading
//! - Directory traversal
//! - Extensively fuzz-tested for safety and correctness
//!
//! ## Performance
//!
//! The crate is optimized for performance while maintaining safety:
//! - Checksum calculations: ~150-200ns per block
//! - Name hashing: ~5-30ns depending on length
//! - No unsafe code in critical paths
//! - Cache-friendly sequential memory access
//!
//! See `PERFORMANCE.md` for detailed benchmarks and optimization documentation.
//!
//! ## Example
//!
//! ```ignore
//! use affs_read::{AffsReader, BlockDevice};
//!
//! // Implement BlockDevice for your storage
//! struct MyDevice { /* ... */ }
//!
//! impl BlockDevice for MyDevice {
//!     fn read_block(&self, block: u32, buf: &mut [u8; 512]) -> Result<(), ()> {
//!         // Read block from storage
//!         Ok(())
//!     }
//! }
//!
//! let device = MyDevice { /* ... */ };
//! let reader = AffsReader::new(&device)?;
//!
//! // List root directory
//! for entry in reader.read_dir(reader.root_block())? {
//!     println!("{}", entry.name());
//! }
//! ```

#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]
#![warn(missing_docs)]
#![warn(clippy::all)]

#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "alloc")]
extern crate alloc;

mod block;
mod checksum;
mod constants;
mod date;
mod dir;
mod error;
mod file;
mod reader;
mod symlink;
mod types;
mod utf8;
mod varblock;

pub use block::*;
pub use checksum::{bitmap_sum, boot_sum, normal_sum, normal_sum_slice, read_u16_be};
pub use constants::*;
pub use date::AmigaDate;
pub use dir::{DirEntry, DirIter};
pub use error::AffsError;
pub use file::FileReader;
pub use reader::AffsReader;
pub use symlink::{
    MAX_SYMLINK_LEN, max_utf8_len, read_symlink_target, read_symlink_target_with_block_size,
};
pub use types::*;
pub use varblock::{AffsReaderVar, MAX_BLOCK_SIZE, VarDirEntry, VarDirIter};
