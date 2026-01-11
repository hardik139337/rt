# Contributing to Rust Torrent Downloader

Thank you for your interest in contributing to Rust Torrent Downloader! This document provides guidelines and instructions for contributing to the project.

## Code of Conduct

This project adheres to a code of conduct. By participating, you are expected to uphold this code. Please report unacceptable behavior to [your.email@example.com].

## Getting Started

### Prerequisites

- Rust 1.70 or later
- Git

### Setting Up the Development Environment

1. Fork the repository on GitHub
2. Clone your fork locally:

```bash
git clone https://github.com/yourusername/rust-torrent-downloader.git
cd rust-torrent-downloader
```

3. Create a new branch for your feature or bugfix:

```bash
git checkout -b feature/your-feature-name
# or
git checkout -b fix/your-bugfix-name
```

4. Build the project:

```bash
cargo build
```

5. Run the tests:

```bash
cargo test
```

## Development Workflow

### Code Style

- Follow the standard Rust style guidelines
- Use `cargo fmt` to format your code before committing
- Run `cargo clippy` to catch common mistakes and improve code quality

```bash
cargo fmt
cargo clippy -- -D warnings
```

### Testing

- Write unit tests for new functionality
- Ensure all existing tests pass before submitting
- Consider adding integration tests for complex features
- Test on multiple platforms if possible (Linux, macOS, Windows)

```bash
cargo test
cargo test --release
```

### Documentation

- Add documentation comments (`///`) to all public APIs
- Update the README.md if you change user-facing behavior
- Add examples to the `examples/` directory for new features
- Document any breaking changes in the commit message

### Commit Messages

Follow the [Conventional Commits](https://www.conventionalcommits.org/) specification:

- `feat:` for new features
- `fix:` for bug fixes
- `docs:` for documentation changes
- `style:` for code style changes (formatting, etc.)
- `refactor:` for code refactoring
- `test:` for adding or updating tests
- `chore:` for maintenance tasks

Examples:
```
feat: add support for magnet links
fix: resolve memory leak in peer connection
docs: update README with new usage examples
```

## Pull Request Process

1. Ensure your code passes all tests and linting:

```bash
cargo test
cargo clippy -- -D warnings
cargo fmt -- --check
```

2. Update the documentation as needed

3. Push your branch to your fork:

```bash
git push origin feature/your-feature-name
```

4. Create a pull request on GitHub with:
   - A clear description of the changes
   - Reference any related issues
   - Screenshots for UI changes (if applicable)
   - Testing instructions

5. Wait for code review and address any feedback

## Project Structure

```
rust-torrent-downloader/
├── src/
│   ├── cli/          # Command-line interface
│   ├── dht/          # Distributed Hash Table
│   ├── peer/         # Peer connection management
│   ├── protocol/     # BitTorrent wire protocol
│   ├── storage/      # File storage and piece management
│   ├── torrent/      # Torrent file parsing
│   ├── error.rs      # Error types
│   ├── lib.rs        # Library entry point
│   └── main.rs       # CLI entry point
├── examples/         # Usage examples
├── tests/            # Integration tests
├── Cargo.toml        # Project metadata
├── README.md         # Project documentation
└── CONTRIBUTING.md   # This file
```

## Coding Guidelines

### Error Handling

- Use the `anyhow` crate for application errors
- Use the `thiserror` crate for library errors (if needed)
- Provide helpful error messages with context
- Handle errors gracefully and log appropriately

### Async/Await

- Use `tokio` for async runtime
- Prefer async functions over blocking operations
- Use `tokio::spawn` for concurrent tasks when appropriate
- Be mindful of cancellation and resource cleanup

### Performance

- Avoid unnecessary allocations
- Use efficient data structures (e.g., `Vec`, `HashMap`)
- Profile performance-critical code
- Consider using `#[inline]` for small, frequently called functions

### Security

- Validate all external input
- Use safe Rust practices
- Be careful with buffer operations
- Follow security best practices for network applications

## Testing Guidelines

### Unit Tests

- Test individual functions and methods
- Use `#[cfg(test)]` for test-only code
- Mock external dependencies when necessary
- Test both success and error paths

### Integration Tests

- Place in the `tests/` directory
- Test module interactions
- Use real torrent files for testing (include in repository)
- Test network operations carefully

### Example Test Structure

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_name() {
        // Arrange
        let input = ...;
        
        // Act
        let result = function_to_test(input);
        
        // Assert
        assert_eq!(result, expected);
    }
}
```

## Documentation Guidelines

### Public API Documentation

- Use `///` for documentation comments
- Include examples for complex functions
- Document all parameters and return values
- Note any panics that may occur

Example:
```rust
/// Parses a torrent file and returns the metadata.
///
/// # Arguments
///
/// * `path` - Path to the torrent file to parse
///
/// # Returns
///
/// Returns a `Result` containing the `Torrent` metadata or an error.
///
/// # Errors
///
/// Returns an error if:
/// - The file cannot be read
/// - The file is not valid bencode format
/// - Required fields are missing
///
/// # Examples
///
/// ```no_run
/// use rust_torrent_downloader::torrent::Torrent;
///
/// let torrent = Torrent::from_file("example.torrent")?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn from_file(path: &Path) -> Result<Torrent> {
    // ...
}
```

### README Documentation

- Keep the README up-to-date
- Include installation instructions
- Provide usage examples
- Document configuration options
- Note any breaking changes in releases

## Issue Reporting

When reporting bugs, please include:

- A clear description of the problem
- Steps to reproduce the issue
- Expected behavior
- Actual behavior
- Environment details (OS, Rust version)
- Relevant logs or error messages
- A minimal reproducible example if possible

## Feature Requests

When requesting features, please include:

- A clear description of the feature
- Why the feature would be useful
- Potential implementation approaches
- Examples of how the feature would be used

## Release Process

Releases are versioned according to [Semantic Versioning](https://semver.org/):

- **MAJOR**: Incompatible API changes
- **MINOR**: Backwards-compatible functionality additions
- **PATCH**: Backwards-compatible bug fixes

Release checklist:

1. Update version in `Cargo.toml`
2. Update CHANGELOG.md
3. Tag the release in Git
4. Create a GitHub release
5. Publish to crates.io

## Questions?

Feel free to open an issue for questions about contributing or using the project.

## License

By contributing to Rust Torrent Downloader, you agree that your contributions will be licensed under the same license as the project (MIT OR Apache-2.0).
