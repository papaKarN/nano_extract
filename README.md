# nano_extract

[![CI](https://github.com/papaKarN/nano_extract/actions/workflows/ci.yml/badge.svg)](https://github.com/papaKarN/nano_extract/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org)

Fast Nanopore FASTQ read length and mean quality extractor. Outputs are planned to be used with https://github.com/papaKarN/nano_stats.py
Rust port of `Nano_Extract.V3.8.py` — **5 to 20× faster**, drop-in replacement, identical output format.

---

## Features

- Supports `.fastq` and `.fastq.gz` input files
- Native gzip decompression — no external `pigz` dependency
- Multi-threaded quality computation via `rayon`
- Identical CLI interface and TSV output to the original Python script
- Progress bar with ETA
- Processes multiple files in a single command

---

## Output format

Tab-separated file with header:

```
read_id	length	mean_quality
read1	15234	12.45
read2	8901	14.20
...
```

---

## Installation

### Option 1 — Compile from source (recommended)

**Prerequisites**

```bash
# In your conda environment
conda activate your_env
conda install -c conda-forge rust cmake
```

**Compile**

```bash
git clone https://github.com/papaKarN/nano_extract
cd nano_extract
cargo build --release
```

**Install into conda environment**

```bash
cp target/release/nano_extract $CONDA_PREFIX/bin/
```

### Option 2 — Download prebuilt binary (Linux x86_64)

Download the latest binary from the [Releases](https://github.com/papaKarN/nano_extract/releases) page.

```bash
chmod +x nano_extract
cp nano_extract $CONDA_PREFIX/bin/
```

---

## Usage

```bash
# Single file
nano_extract -i sample.fastq.gz -o length_quality

# Multiple files
nano_extract -i *.fastq.gz -o results

# Control number of threads
nano_extract -i sample.fastq.gz -o results -t 8

# Uncompressed input
nano_extract -i sample.fastq -o results

# Custom chunk size
nano_extract -i sample.fastq.gz -o results --chunk_size 100000
```

### Options

| Option | Default | Description |
|--------|---------|-------------|
| `-i / --input` | required | Input FASTQ files (`.fastq` or `.fastq.gz`) |
| `-o / --output` | `length_quality` | Output filename suffix |
| `-t / --threads` | all CPUs | Threads for parallel processing |
| `--chunk_size` | auto | Reads per chunk (50k < 1 GB, 200k ≥ 1 GB) |

---

## Performance comparison

| Tool | 10 GB `.fastq.gz` | Notes |
|------|-------------------|-------|
| `Nano_Extract.V3.8.py` | ~8 min | pigz + multiprocessing |
| `nano_extract` (Rust) | ~40 sec | flate2/zlib-ng + rayon |

*Benchmarked on 16-core Linux workstation.*

---

## Why faster?

| Aspect | Python V3.8 | Rust v1.0 |
|--------|-------------|-----------|
| Gzip decompression | `pigz` (external process + IPC) | `flate2` native (zlib-ng) |
| Parallelism | `multiprocessing.Pool` + pickle serialization | `rayon` (zero-copy threads) |
| I/O | Python `open()` | `BufReader` with 4 MB buffer |
| Quality computation | NumPy (Python overhead) | LLVM auto-vectorized SIMD |

---

## Development

```bash
# Run tests
cargo test

# Check formatting
cargo fmt --check

# Lint
cargo clippy
```

---

## Python original

The original Python script `Nano_Extract.V3.8.py` is included in this repository for reference.

---

## License

MIT — see [LICENSE](LICENSE)
