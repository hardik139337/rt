# Rust Torrent Downloader

[![Crates.io](https://img.shields.io/crates/v/rust-torrent-downloader)](https://crates.io/crates/rust-torrent-downloader)
[![Documentation](https://img.shields.io/docsrs/rust-torrent-downloader)](https://docs.rs/rust-torrent-downloader)
[![License](https://img.shields.io/crates/l/rust-torrent-downloader)](https://github.com/yourusername/rust-torrent-downloader#license)
[![Build Status](https://img.shields.io/github/actions/workflow/status/yourusername/rust-torrent-downloader/ci.yml)](https://github.com/yourusername/rust-torrent-downloader/actions)

A full-featured BitTorrent CLI downloader written in Rust with DHT, seeding, and resume support.

## Features

- **BitTorrent Protocol Implementation**: Complete implementation of the BitTorrent protocol
- **DHT Support**: Distributed Hash Table for peer discovery without trackers
- **Seeding Support**: Continue seeding after download completion
- **Resume Support**: Resume interrupted downloads from saved state
- **Multiple Peer Connections**: Efficiently manage multiple peer connections simultaneously
- **Piece Verification**: Automatic verification of downloaded pieces using SHA-1 hashes
- **Configurable Download**: Fine-grained control over download parameters
- **Progress Tracking**: Real-time download progress display
- **CLI Interface**: User-friendly command-line interface with rich options
- **Error Handling**: Comprehensive error handling and reporting
- **Logging Support**: Built-in tracing and logging for debugging

## Installation

### From Crates.io

```bash
cargo install rust-torrent-downloader
```

### From Source

```bash
git clone https://github.com/yourusername/rust-torrent-downloader.git
cd rust-torrent-downloader
cargo install --path .
```

### Build from Source

```bash
git clone https://github.com/yourusername/rust-torrent-downloader.git
cd rust-torrent-downloader
cargo build --release
```

The binary will be available at `target/release/rust-torrent-downloader`.

## Usage

### Basic Usage

Download a torrent file to the current directory:

```bash
rust-torrent-downloader download example.torrent
```

Download to a specific directory:

```bash
rust-torrent-downloader download example.torrent --output /path/to/downloads
```

### Advanced Usage

Download with custom port and peer connections:

```bash
rust-torrent-downloader download example.torrent --port 6881 --max-peers 50
```

Download and seed after completion:

```bash
rust-torrent-downloader download example.torrent --seed
```

Download with DHT disabled:

```bash
rust-torrent-downloader download example.torrent --no-dht
```

Resume a download:

```bash
rust-torrent-downloader download example.torrent --resume
```

### Command-Line Options

```
rust-torrent-downloader <COMMAND>

Commands:
  download    Download a torrent file
  seed        Seed a torrent file
  help        Print this message or the help of the given subcommand

Options:
  -h, --help     Print help
  -V, --version  Print version
```

#### Download Command Options

```
rust-torrent-downloader download <TORRENT_FILE>

Arguments:
  <TORRENT_FILE>  Path to the torrent file

Options:
  -o, --output <OUTPUT>          Output directory for downloaded files [default: .]
  -p, --port <PORT>              Port to listen on [default: 6881]
  -m, --max-peers <MAX_PEERS>    Maximum number of peer connections [default: 50]
  -s, --seed                     Continue seeding after download completes
  -d, --no-dht                   Disable DHT peer discovery
  -r, --resume                   Resume from saved state
  -v, --verbose                  Enable verbose logging
  -h, --help                     Print help
```

#### Seed Command Options

```
rust-torrent-downloader seed <TORRENT_FILE>

Arguments:
  <TORRENT_FILE>  Path to the torrent file

Options:
  -o, --output <OUTPUT>          Directory containing downloaded files [default: .]
  -p, --port <PORT>              Port to listen on [default: 6881]
  -m, --max-peers <MAX_PEERS>    Maximum number of peer connections [default: 50]
  -d, --no-dht                   Disable DHT peer discovery
  -v, --verbose                  Enable verbose logging
  -h, --help                     Print help
```

## Configuration Options

The application supports configuration through command-line arguments:

- **Port**: The port to listen for incoming peer connections (default: 6881)
- **Max Peers**: Maximum number of simultaneous peer connections (default: 50)
- **Output Directory**: Directory where downloaded files are saved (default: current directory)
- **DHT**: Enable or disable Distributed Hash Table for peer discovery (default: enabled)
- **Seeding**: Continue seeding after download completion (default: disabled)
- **Resume**: Resume from saved state (default: disabled)
- **Verbose**: Enable detailed logging for debugging (default: disabled)

## Examples

### Example 1: Download a Linux ISO

```bash
rust-torrent-downloader download ubuntu-22.04.torrent --output ~/Downloads
```

### Example 2: Download and seed

```bash
rust-torrent-downloader download movie.torrent --seed --max-peers 100
```

### Example 3: Resume interrupted download

```bash
rust-torrent-downloader download large-file.torrent --resume
```

### Example 4: Seed an existing download

```bash
rust-torrent-downloader seed ubuntu-22.04.torrent --output ~/Downloads
```

### Example 5: Download with custom settings

```bash
rust-torrent-downloader download example.torrent \
  --output ~/Torrents \
  --port 6882 \
  --max-peers 75 \
  --verbose
```

## Library Usage

You can also use `rust-torrent-downloader` as a library in your own Rust projects:

```toml
[dependencies]
rust-torrent-downloader = "0.1.0"
```

See the [examples](examples/) directory for more detailed usage examples.

## Architecture

The application is organized into several modules:

- **`torrent`**: Torrent file parsing and metadata handling
- **`protocol`**: BitTorrent wire protocol implementation
- **`peer`**: Peer connection management and state tracking
- **`storage`**: File storage, piece management, and resume state
- **`dht`**: Distributed Hash Table implementation for peer discovery
- **`cli`**: Command-line interface and configuration
- **`error`**: Error types and handling

## Development

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines on contributing to this project.

## Testing

Run the test suite:

```bash
cargo test
```

Run with verbose output:

```bash
cargo test -- --nocapture
```

## License

This project is licensed under either of:

- MIT License ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)

at your option.

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for details.

## Disclaimer

This software is intended for educational purposes and for downloading legal content only. The authors are not responsible for any misuse of this software.

## Acknowledgments

- Built with [Tokio](https://tokio.rs/) for async I/O
- Uses [Clap](https://github.com/clap-rs/clap) for CLI parsing
- Implements the [BitTorrent Protocol](http://www.bittorrent.org/beps/bep_0003.html)
- Implements [DHT Protocol](http://www.bittorrent.org/beps/bep_0005.html)
# rt
