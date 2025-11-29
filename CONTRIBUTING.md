# Contributing to affs-read

Thank you for your interest in contributing to affs-read! This document provides guidelines and information for contributors.

## Code of Conduct

This project follows the [Contributor Covenant Code of Conduct](CODE_OF_CONDUCT.md). By participating, you are expected to uphold this code.

## Getting Started

### Prerequisites

- Rust 2024 edition (1.85+)
- Git

### Setup

```bash
# Clone the repository
git clone https://github.com/USERNAME/affs-read.git
cd affs-read

# Build the project
cargo build

# Run tests
cargo test

# Run clippy
cargo clippy --all-features --all-targets
```

## Development Workflow

### Running Tests

```bash
# Run all tests
cargo test

# Run tests with nextest (recommended)
cargo nextest run --all-features

# Run a specific test
cargo test test_read_ffs_disk

# Run tests with output
cargo test -- --nocapture
```

### Code Quality

Before submitting a PR, ensure your code passes all checks:

```bash
# Format code
cargo fmt

# Run lints
cargo clippy --all-features --all-targets -- -D warnings

# Run tests
cargo nextest run --all-features

# Check all feature combinations
cargo check --no-default-features
cargo check --features std
cargo check --features alloc
cargo check --all-features
```

### Fuzz Testing

The project includes fuzz tests for robustness:

```bash
# Install cargo-fuzz (requires nightly)
rustup install nightly
cargo +nightly install cargo-fuzz

# Run a fuzz target
cargo +nightly fuzz run fuzz_target_1

# Run for a specific duration
cargo +nightly fuzz run fuzz_blocks -- -max_total_time=60
```

Available fuzz targets:

- `fuzz_read` - General filesystem reading
- `fuzz_blocks` - Block parsing
- `fuzz_checksum` - Checksum calculations
- `fuzz_names` - Filename hashing and comparison

## Making Changes

### Branching

1. Create a branch from `main`:

   ```bash
   git checkout -b feature/your-feature-name
   ```

1. Make your changes with clear, atomic commits

1. Push and open a pull request

### Commit Messages

Follow conventional commit format:

```
type(scope): description

[optional body]

[optional footer]
```

Types:

- `feat` - New feature
- `fix` - Bug fix
- `docs` - Documentation changes
- `refactor` - Code refactoring
- `test` - Adding or updating tests
- `chore` - Maintenance tasks

Examples:

```
feat(reader): add support for HD floppy disks

fix(checksum): handle overflow in boot_sum calculation

docs(readme): add usage examples for file reading

test(file): add tests for extension block handling
```

### Pull Request Guidelines

1. **Title**: Use a clear, descriptive title following commit message conventions

1. **Description**: Include:

   - What the change does
   - Why the change is needed
   - Any breaking changes
   - Related issues (use "Fixes #123" or "Closes #123")

1. **Size**: Keep PRs focused and reasonably sized. Split large changes into multiple PRs.

1. **Tests**: Add tests for new functionality. Maintain or improve coverage.

1. **Documentation**: Update docs for API changes.

## Design Principles

### `no_std` Compatibility

All core functionality must work without `std`:

```rust
// Good - uses core types
use core::fmt;

// Avoid in core code - requires std
use std::io::Read;
```

### Zero Allocation

Core operations should not allocate heap memory:

```rust
// Good - stack-allocated buffer
let mut buf = [0u8; 512];

// Avoid in core code - heap allocation
let mut buf = vec![0u8; 512];
```

### Checksum Validation

Always validate checksums when parsing blocks:

```rust
pub fn parse(buf: &[u8; BLOCK_SIZE]) -> Result<Self> {
    let checksum = read_u32_be(buf, 20);
    let calculated = normal_sum(buf, 20);
    if checksum != calculated {
        return Err(AffsError::ChecksumMismatch);
    }
    // ... continue parsing
}
```

### Error Handling

Use the `AffsError` enum for all fallible operations:

```rust
pub fn read_block(&self, block: u32) -> Result<[u8; 512]> {
    if block >= self.total_blocks {
        return Err(AffsError::BlockOutOfRange);
    }
    // ...
}
```

## Adding Features

### New Block Types

1. Add constants to `src/constants.rs`
1. Create a struct in `src/block.rs`
1. Implement `parse()` with checksum validation
1. Add tests in `tests/integration_tests.rs`

### New Entry Types

1. Add variant to `EntryType` in `src/types.rs`
1. Update `from_sec_type()` mapping
1. Update `is_dir()` / `is_file()` if needed
1. Add tests

## Testing Guidelines

### Unit Tests

Place unit tests in the same file as the code:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_name() {
        assert!(hash_name(b"test", false) < HASH_TABLE_SIZE);
    }
}
```

### Integration Tests

Use `MockDevice` for integration tests:

```rust
#[test]
fn test_read_file() {
    let device = create_test_disk();
    let reader = AffsReader::new(&device).unwrap();
    // ...
}
```

### Test Coverage

Aim for comprehensive coverage:

- Happy path scenarios
- Error conditions
- Edge cases (empty files, max-length names, etc.)
- Both OFS and FFS formats

## Documentation

### Code Documentation

Document all public items:

```rust
/// Read a file's contents.
///
/// # Arguments
/// * `block` - Block number of the file header
///
/// # Returns
/// A `FileReader` for streaming the file contents.
///
/// # Errors
/// Returns `AffsError::NotAFile` if the block is not a file header.
pub fn read_file(&self, block: u32) -> Result<FileReader<'_, D>> {
    // ...
}
```

### Examples

Include examples in documentation:

````rust
/// # Example
///
/// ```ignore
/// let mut reader = FileReader::new(&device, FsType::Ffs, block)?;
/// let mut buf = [0u8; 1024];
/// let n = reader.read(&mut buf)?;
/// ```
````

## Release Process

Releases are automated via GitHub Actions when tags are pushed:

1. Update version in `Cargo.toml`
1. Update CHANGELOG (if present)
1. Create and push a tag:
   ```bash
   git tag v0.2.0
   git push origin v0.2.0
   ```

## Getting Help

- Open an issue for bugs or feature requests
- Start a discussion for questions
- Check existing issues before creating new ones

## Recognition

Contributors will be acknowledged in release notes and the project's contributor list.

Thank you for contributing!
