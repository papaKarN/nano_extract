# Contributing to nano_extract

Thank you for your interest in contributing!

## Requirements

- Rust >= 1.70
- `cmake` (for `zlib-ng` compilation)
- A conda environment is recommended (see README)

## Setup

```bash
git clone https://github.com/YOURUSERNAME/nano_extract
cd nano_extract
cargo build
```

## Running tests

```bash
cargo test
```

## Submitting changes

1. Fork the repository
2. Create a branch: `git checkout -b feature/my-feature`
3. Make your changes
4. Run tests: `cargo test`
5. Verify formatting: `cargo fmt --check`
6. Submit a Pull Request

## Code style

This project uses standard Rust formatting. Before submitting, run:

```bash
cargo fmt
cargo clippy
```

## Reporting bugs

Please open an issue on GitHub with:
- Your OS and architecture
- Rust version (`rustc --version`)
- The command you ran
- The full error message
