# affs-read

A `no_std` compatible Rust crate for reading Amiga Fast File System (AFFS) disk images.

[![CI](https://github.com/MuntasirSZN/affs-read/actions/workflows/ci.yml/badge.svg)](https://github.com/USERNAME/affs-read/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/affs-read.svg)](https://crates.io/crates/affs-read)
[![Documentation](https://docs.rs/affs-read/badge.svg)](https://docs.rs/affs-read)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

## Features

- **`no_std` compatible** - Works in embedded and bare-metal environments
- **Zero heap allocations** - Core functionality uses only stack memory
- **Optimized for performance** - Byte-level operations, branchless algorithms, and careful memory access patterns
- **OFS and FFS support** - Handles both Original File System and Fast File System
- **INTL and DIRCACHE modes** - Full support for international character handling and directory caching
- **Streaming file reading** - Memory-efficient sequential file access
- **Directory traversal** - Iterate through directory contents with lazy loading
- **Checksum validation** - Ensures data integrity on all block reads
- **Extensively tested** - Fuzz-tested and benchmarked for correctness and performance

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
affs-read = "0.1"
```

### Feature Flags

| Feature | Default | Description |
|---------|---------|-------------|
| `std` | Yes | Enables `std::error::Error` implementation |
| `alloc` | No | Enables features requiring heap allocation |
| `perf-asm` | No | Enables assembly-level optimizations (experimental) |
| `perf-simd` | No | Reserved for future SIMD optimizations |

For `no_std` environments:

```toml
[dependencies]
affs-read = { version = "0.1", default-features = false }
```

## Quick Start

```rust
use affs_read::{AffsReader, BlockDevice};

// Implement BlockDevice for your storage medium
struct DiskImage {
    data: Vec<u8>,
}

impl BlockDevice for DiskImage {
    fn read_block(&self, block: u32, buf: &mut [u8; 512]) -> Result<(), ()> {
        let offset = block as usize * 512;
        if offset + 512 <= self.data.len() {
            buf.copy_from_slice(&self.data[offset..offset + 512]);
            Ok(())
        } else {
            Err(())
        }
    }
}

fn main() -> Result<(), affs_read::AffsError> {
    let adf_data = std::fs::read("disk.adf").unwrap();
    let device = DiskImage { data: adf_data };
    
    // Create reader for standard DD floppy (880KB)
    let reader = AffsReader::new(&device)?;
    
    // Print disk information
    println!("Disk: {:?}", reader.disk_name_str());
    println!("Type: {:?}", reader.fs_type());
    
    // List root directory
    for entry in reader.read_root_dir() {
        let entry = entry?;
        if entry.is_file() {
            println!("File: {} ({} bytes)", 
                entry.name_str().unwrap_or("?"), 
                entry.size);
        } else {
            println!("Dir:  {}/", entry.name_str().unwrap_or("?"));
        }
    }
    
    Ok(())
}
```

## Reading Files

```rust
// Find and read a file
let entry = reader.find_path(b"s/startup-sequence")?;
let mut file = reader.read_file(entry.block)?;

// Read into buffer
let mut buffer = vec![0u8; entry.size as usize];
file.read_all(&mut buffer)?;

// Or read in chunks
let mut chunk = [0u8; 512];
while !file.is_eof() {
    let n = file.read(&mut chunk)?;
    // Process chunk[..n]
}
```

## Navigating Directories

```rust
// Find a subdirectory
let subdir = reader.find_entry(reader.root_block(), b"devs")?;

// List its contents
for entry in reader.read_dir(subdir.block)? {
    let entry = entry?;
    println!("{}", entry.name_str().unwrap_or("?"));
}

// Or use path-based lookup
let deep_file = reader.find_path(b"libs/icon.library")?;
```

## Disk Sizes

```rust
// Standard DD floppy (880KB, 1760 blocks)
let reader = AffsReader::new(&device)?;

// HD floppy (1.76MB, 3520 blocks)
let reader = AffsReader::new_hd(&device)?;

// Custom size (e.g., hard disk partition)
let reader = AffsReader::with_size(&device, num_blocks)?;
```

## Supported Formats

| Format | DOS Type | Description |
|--------|----------|-------------|
| OFS | DOS\\0 | Original File System |
| FFS | DOS\\1 | Fast File System |
| OFS+INTL | DOS\\2 | OFS with international mode |
| FFS+INTL | DOS\\3 | FFS with international mode |
| OFS+DC | DOS\\4 | OFS with directory cache |
| FFS+DC | DOS\\5 | FFS with directory cache |

## Error Handling

All operations return `Result<T, AffsError>`:

```rust
use affs_read::AffsError;

match reader.find_path(b"nonexistent") {
    Ok(entry) => println!("Found: {}", entry.name_str().unwrap()),
    Err(AffsError::EntryNotFound) => println!("File not found"),
    Err(AffsError::ChecksumMismatch) => println!("Disk corruption detected"),
    Err(e) => println!("Error: {}", e),
}
```

## Performance

The crate is optimized for performance with:

- **Byte-level operations** - Direct memory access for checksums and hashing
- **Branchless algorithms** - Reduced branch mispredictions in hot paths
- **Cache-friendly access** - Sequential memory access patterns
- **Zero allocations** - No heap allocations in core functionality
- **Comprehensive benchmarks** - See `PERFORMANCE.md` for detailed metrics

Typical performance on modern hardware:
- Checksum calculation: ~150-200ns per block
- Name hashing: ~5-30ns depending on length
- Name comparison: ~1.5-7ns with early exit optimization

Run benchmarks with `cargo bench` to see performance on your system.

## Safety

This crate:

- Uses `#![deny(unsafe_op_in_unsafe_fn)]` for strict unsafe handling
- Has no external runtime dependencies
- Validates checksums on all block reads
- Is fuzz-tested for robustness against malformed inputs

## Minimum Supported Rust Version

Rust 2024 edition (1.85+)

## License

Licensed under the MIT License. See [LICENSE](LICENSE) for details.

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## Reference

- [adflib](https://github.com/adflib/adflib)
