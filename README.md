# nano_extract v2.0.0

Fast Nanopore read length/quality extractor.  
**Auto-detects input format: `.fastq` `.fastq.gz` `.bam`**

Replaces both `nano_extract v1.0.0` and `bam_extract`.

---

## Supported formats

| Format | Backend | Dependency |
|--------|---------|------------|
| `.fastq` | Native Rust | none |
| `.fastq.gz` | flate2/zlib-ng | none |
| `.bam` | samtools subprocess | samtools |

Files can be **mixed** in a single command:
```bash
nano_extract -i reads.fastq.gz mapping.bam other.fastq -o results
```

---

## Installation

```bash
conda activate your_env

# Only required if you use BAM files
conda install -c bioconda samtools

# Compile
conda install -c conda-forge rust cmake   # if not already installed
git clone https://github.com/papaKarN/nano_extract
cd nano_extract
cargo build --release
cp target/release/nano_extract $CONDA_PREFIX/bin/
```

---

## Usage

```bash
# FASTQ (identical to v1.0.0)
nano_extract -i sample.fastq.gz -o results -t 8

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
| `-t / --threads` | all CPUs | Threads |
| `--chunk_size` | auto | Reads per chunk (50k < 1 GB, 200k otherwise) |
| `--include_unmapped` | `true` | [BAM] Include unmapped reads |
| `--skip_secondary` | `true` | [BAM] Skip secondary alignments |
| `--skip_supplementary` | `true` | [BAM] Skip supplementary alignments |

---

## Output format

Identical for all input formats:

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
| `nano_extract` v2.0.0 (Rust) | ~40 sec | flate2/zlib-ng + rayon |

*Benchmarked on a 16-core Linux workstation.*

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

- Original Python script developed at [your lab / institution]
- Rust port assisted by [Claude](https://claude.ai) (Anthropic)
- Built with [rayon](https://github.com/rayon-rs/rayon), [flate2](https://github.com/rust-lang/flate2-rs), [clap](https://github.com/clap-rs/clap), [indicatif](https://github.com/console-rs/indicatif)

---

## License

MIT — see [LICENSE](LICENSE)
