//! Core types for AFFS.

/// Block device trait for reading blocks from storage.
///
/// Implement this trait for your storage medium (file, memory, hardware, etc.).
pub trait BlockDevice {
    /// Read a single 512-byte block.
    ///
    /// # Arguments
    /// * `block` - Block number to read
    /// * `buf` - Buffer to read into (must be exactly 512 bytes)
    ///
    /// # Returns
    /// `Ok(())` on success, `Err(())` on failure.
    #[allow(clippy::result_unit_err)]
    fn read_block(&self, block: u32, buf: &mut [u8; 512]) -> Result<(), ()>;
}

/// Filesystem type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsType {
    /// Original File System.
    Ofs,
    /// Fast File System.
    Ffs,
}

impl FsType {
    /// Returns the data payload size per block.
    #[inline]
    pub const fn data_block_size(self) -> usize {
        match self {
            Self::Ofs => crate::OFS_DATA_SIZE,
            Self::Ffs => crate::FFS_DATA_SIZE,
        }
    }
}

/// Entry type in the filesystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryType {
    /// Root directory.
    Root,
    /// Directory.
    Dir,
    /// File.
    File,
    /// Hard link to file.
    HardLinkFile,
    /// Hard link to directory.
    HardLinkDir,
    /// Soft link.
    SoftLink,
}

impl EntryType {
    /// Create from secondary type value.
    pub const fn from_sec_type(sec_type: i32) -> Option<Self> {
        match sec_type {
            crate::ST_ROOT => Some(Self::Root),
            crate::ST_DIR => Some(Self::Dir),
            crate::ST_FILE => Some(Self::File),
            crate::ST_LFILE => Some(Self::HardLinkFile),
            crate::ST_LDIR => Some(Self::HardLinkDir),
            crate::ST_LSOFT => Some(Self::SoftLink),
            _ => None,
        }
    }

    /// Returns true if this is a directory type.
    #[inline]
    pub const fn is_dir(self) -> bool {
        matches!(self, Self::Root | Self::Dir | Self::HardLinkDir)
    }

    /// Returns true if this is a file type.
    #[inline]
    pub const fn is_file(self) -> bool {
        matches!(self, Self::File | Self::HardLinkFile)
    }
}

/// Filesystem flags.
#[derive(Debug, Clone, Copy, Default)]
pub struct FsFlags {
    /// International mode enabled.
    pub intl: bool,
    /// Directory cache enabled.
    pub dircache: bool,
}

impl FsFlags {
    /// Create flags from DOS type byte.
    #[inline]
    pub const fn from_dos_type(dos_type: u8) -> Self {
        Self {
            intl: (dos_type & crate::DOSFS_INTL) != 0,
            dircache: (dos_type & crate::DOSFS_DIRCACHE) != 0,
        }
    }
}

/// Access permissions.
#[derive(Debug, Clone, Copy, Default)]
pub struct Access(pub u32);

impl Access {
    /// Create from raw access value.
    #[inline]
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    /// Check if delete is protected (inverted in AFFS).
    #[inline]
    pub const fn is_delete_protected(self) -> bool {
        (self.0 & crate::ACC_DELETE) != 0
    }

    /// Check if execute is protected.
    #[inline]
    pub const fn is_execute_protected(self) -> bool {
        (self.0 & crate::ACC_EXECUTE) != 0
    }

    /// Check if write is protected.
    #[inline]
    pub const fn is_write_protected(self) -> bool {
        (self.0 & crate::ACC_WRITE) != 0
    }

    /// Check if read is protected.
    #[inline]
    pub const fn is_read_protected(self) -> bool {
        (self.0 & crate::ACC_READ) != 0
    }

    /// Check if archived flag is set.
    #[inline]
    pub const fn is_archived(self) -> bool {
        (self.0 & crate::ACC_ARCHIVE) != 0
    }

    /// Check if pure (re-entrant) flag is set.
    #[inline]
    pub const fn is_pure(self) -> bool {
        (self.0 & crate::ACC_PURE) != 0
    }

    /// Check if script flag is set.
    #[inline]
    pub const fn is_script(self) -> bool {
        (self.0 & crate::ACC_SCRIPT) != 0
    }

    /// Check if hold flag is set.
    #[inline]
    pub const fn is_hold(self) -> bool {
        (self.0 & crate::ACC_HOLD) != 0
    }
}
