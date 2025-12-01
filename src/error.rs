//! Error types for AFFS operations.

use core::fmt;

/// Error type for AFFS operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AffsError {
    /// Block read failed.
    BlockReadError,
    /// Invalid DOS type signature.
    InvalidDosType,
    /// Invalid block type.
    InvalidBlockType,
    /// Invalid secondary type.
    InvalidSecType,
    /// Checksum verification failed.
    ChecksumMismatch,
    /// Block number out of valid range.
    BlockOutOfRange,
    /// Entry not found.
    EntryNotFound,
    /// Name too long (max 30 characters).
    NameTooLong,
    /// Invalid filesystem state.
    InvalidState,
    /// End of file reached.
    EndOfFile,
    /// Not a file entry.
    NotAFile,
    /// Not a directory entry.
    NotADirectory,
    /// Buffer too small.
    BufferTooSmall,
    /// Invalid data block sequence.
    InvalidDataSequence,
    /// Not a symlink entry.
    NotASymlink,
    /// Symlink target too long.
    SymlinkTooLong,
}

impl fmt::Display for AffsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BlockReadError => write!(f, "block read error"),
            Self::InvalidDosType => write!(f, "invalid DOS type signature"),
            Self::InvalidBlockType => write!(f, "invalid block type"),
            Self::InvalidSecType => write!(f, "invalid secondary type"),
            Self::ChecksumMismatch => write!(f, "checksum mismatch"),
            Self::BlockOutOfRange => write!(f, "block out of range"),
            Self::EntryNotFound => write!(f, "entry not found"),
            Self::NameTooLong => write!(f, "name too long"),
            Self::InvalidState => write!(f, "invalid filesystem state"),
            Self::EndOfFile => write!(f, "end of file"),
            Self::NotAFile => write!(f, "not a file"),
            Self::NotADirectory => write!(f, "not a directory"),
            Self::BufferTooSmall => write!(f, "buffer too small"),
            Self::InvalidDataSequence => write!(f, "invalid data block sequence"),
            Self::NotASymlink => write!(f, "not a symlink"),
            Self::SymlinkTooLong => write!(f, "symlink target too long"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for AffsError {}

/// Result type for AFFS operations.
pub type Result<T> = core::result::Result<T, AffsError>;
