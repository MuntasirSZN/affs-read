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
//! - Support for OFS and FFS filesystems
//! - Support for INTL and DIRCACHE modes
//! - Streaming file reading
//! - Directory traversal
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
mod types;

pub use block::*;
pub use checksum::{bitmap_sum, boot_sum, normal_sum, read_u16_be};
pub use constants::*;
pub use date::AmigaDate;
pub use dir::{DirEntry, DirIter};
pub use error::AffsError;
pub use file::FileReader;
pub use reader::AffsReader;
pub use types::*;
