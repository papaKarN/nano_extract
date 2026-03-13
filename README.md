# nano_extract v2.1.0

Fast Nanopore read length/quality extractor.  
**Auto-detects input format: `.fastq` `.fastq.gz` `.bam`**  
**Output files are automatically compressed with pigz (.txt.gz)**

---

## Supported formats

| Format | Backend | Dependency |
|--------|---------|------------|
| `.fastq` | Native Rust | none |
| `.fastq.gz` | flate2/zlib-ng | pigz |
| `.bam` | samtools subprocess | samtools |

Files can be **mixed** in a single command:
```bash
nano_extract -i reads.fastq.gz mapping.bam other.fastq -o results
```

---

## Dependencies

| Tool | Required when | Install |
|------|--------------|---------|
| `pigz` | `.fastq.gz` input | `conda install -c conda-forge pigz` |
| `samtools` | `.bam` input | `conda install -c bioconda samtools` |
| `cmake` | compilation only | `conda install -c conda-forge cmake` |

Both `pigz` and `samtools` are checked at startup — a clear error message is displayed if they are missing.

---

## Installation

```bash
conda activate your_env

# Install dependencies according to your input formats
conda install -c conda-forge pigz cmake
conda install -c bioconda samtools        # only for BAM files

# Compile
conda install -c conda-forge rust         # if not already installed
git clone https://github.com/papaKarN/nano_extract
cd nano_extract
cargo build --release
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
# FASTQ gzipped
nano_extract -i sample.fastq.gz -o results -t 8

# FASTQ plain
nano_extract -i sample.fastq -o results -t 8

# BAM
nano_extract -i sample.bam -o results -t 8

# Mixed files
nano_extract -i *.fastq.gz *.bam -o results -t 8

# BAM — exclude unmapped reads
nano_extract -i sample.bam -o results --include_unmapped false
```

### Options

| Option | Default | Description |
|--------|---------|-------------|
| `-i / --input` | required | `.fastq`, `.fastq.gz`, `.fq`, `.fq.gz`, `.bam` |
| `-o / --output` | `length_quality` | Output filename suffix |
| `-t / --threads` | all CPUs | Threads (used for processing AND pigz compression) |
| `--chunk_size` | auto | Reads per chunk (50k < 1 GB, 200k otherwise) |
| `--include_unmapped` | `true` | [BAM] Include unmapped reads |
| `--skip_secondary` | `true` | [BAM] Skip secondary alignments |
| `--skip_supplementary` | `true` | [BAM] Skip supplementary alignments |

---

## Output format

Compressed TSV (`.txt.gz`), identical for all input formats:

```
read_id	length	mean_quality
read1	15234	12.45
read2	8901	14.20
```

---

## Performance

| Tool | 10 GB `.fastq.gz` | Notes |
|------|-------------------|-------|
| `Nano_Extract.V3.8.py` | ~8 min | pigz + multiprocessing |
| `nano_extract` v2.1.0 (Rust) | ~40 sec | flate2/zlib-ng + rayon + pigz output |

*Benchmarked on a 16-core Linux workstation.*

---

## Changelog

### v2.1.0
- Output files are now compressed with pigz (`.txt.gz`)
- pigz thread count controlled by `-t` option
- Error message at startup if pigz is missing and `.gz` input files are provided

### v2.0.0
- Auto-detection of input format (`.fastq`, `.fastq.gz`, `.bam`)
- BAM support via samtools subprocess
- Mixed input files in a single command

### v1.0.0
- Initial Rust release — port of `Nano_Extract.V3.8.py`
- Native gzip decompression via flate2/zlib-ng
- Parallel quality computation via rayon

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

## Acknowledgements

- Original Python script developed at [PGTB- UMR 1202 BioGeCo / INRAe]
- Rust port assisted by [Claude](https://claude.ai) (Anthropic)
- Built with [rayon](https://github.com/rayon-rs/rayon), [flate2](https://github.com/rust-lang/flate2-rs), [clap](https://github.com/clap-rs/clap), [indicatif](https://github.com/console-rs/indicatif)
- Thnak you to all my great PGTB colleagues

---

## License

MIT — see [LICENSE](LICENSE)
