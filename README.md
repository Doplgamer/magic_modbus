# Magic Modbus 🚌⚡

> A powerful Terminal User Interface (TUI) tool for working with Modbus devices, making Modbus protocols more accessible and easy to use.

![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=for-the-badge&logo=rust&logoColor=white)
![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue?style=for-the-badge)

## Overview

Magic Modbus is a CLI tool built with Rust that provides an intuitive terminal interface for interacting with Modbus devices over TCP connections. It features a tabbed interface for viewing and editing different Modbus data types with real-time updates and efficient memory management.

## Features

- 🖥️ **Beautiful TUI** - Built with `ratatui` for a responsive terminal interface
- 📊 **Multi-Table Support** - Separate tabs for Coils, Discrete Inputs, Input Registers, and Holding Registers
- 🔄 **Real-time Updates** - Live data refresh and monitoring capabilities
- 📝 **Interactive Editing** - Edit values directly in the interface with queued operations
- 🚀 **Async Architecture** - Non-blocking operations with `tokio` for smooth performance
- 💾 **Memory Efficient** - Sparse data storage using HashMaps instead of pre-allocated arrays
- 🎨 **Dynamic Styling** - Color-coded interface that changes based on selected data type
- ❓ **Comprehensive Help Menu** - Help menu which shows how to properly use every included feature

## Installation

### From Source

```bash
git clone https://github.com/yourusername/magic_modbus.git
cd magic_modbus
cargo build --release
```

The binary will be available at `target/release/magic_modbus`.

## Usage

### Basic Usage

```bash
# Run the application
cargo run -- --help
# or if you built the release binary:
./target/release/magic_modbus --help

# There are two distinct modes which can be used
# TUI Mode
cargo run --
# Macro Parser Mode
cargo run parse-macro
```


### Macro Mode
- You have the ability to save queued commands as macro files which then can be parsed by the application
- To use this feature, do the following:
1. Connect to a server in TUI mode
2. Queue/Toggle different operations without applying
3. In the `Queue` Tab, save the queued operations to a macro file by pressing `M`
4. Enter a name for your file - your file will appear in the current working directory with the extension `.magmod`
5. Run in Macro Parser mode, providing the `.magmod` file from before.

### TUI Controls

#### Main Navigation
- `Esc` - Quit application
- `Q` - Previous tab
- `E` - Next tab  
- `Tab` - Change focus between areas
- `?` - Help menu

#### Table Navigation
- `W A S D` or `↑ ↓ ← →` - Navigate cells
- `Space` - Queue/Toggle cell values
- `Enter` - Apply changes

#### Connection
- Navigate to connection tab to set up TCP connection to your Modbus device
- Enter IP address and port
- Connect to start reading/writing data

## Supported Modbus Functions

- **Coils (0x)** - Read/Write single and multiple coils
- **Discrete Inputs (1x)** - Read-only discrete input status  
- **Input Registers (3x)** - Read-only input register values
- **Holding Registers (4x)** - Read/Write holding register values

## Architecture

Magic Modbus uses an async event-driven architecture:

- **Main UI Loop** - Handles keyboard input and rendering at 60fps
- **Modbus Task** - Manages TCP connections and protocol communication
- **MPSC Channels** - Coordinate between UI and networking threads
- **Sparse Storage** - Efficient memory usage with HashMap-based cell storage

## Development

### Building

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Check syntax
cargo check
```

### Code Quality

```bash
# Run clippy linter
cargo clippy

# Format code
cargo fmt

# Check formatting
cargo fmt -- --check
```

### Testing

```bash
# Run tests
cargo test
```

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Acknowledgments

- Built with [`ratatui`](https://github.com/ratatui-org/ratatui) for the terminal UI
- Built with [`clap`](https://github.com/clap-rs/clap) for the CLI interface
- Uses [`tokio-modbus`](https://github.com/slowtec/tokio-modbus) for async Modbus protocol support
- Inspired by the need to make Modbus more accessible to developers and engineers
