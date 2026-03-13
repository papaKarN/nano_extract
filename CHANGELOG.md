## [2.1.0] - 2026-03-13

### Added
- Output files are now compressed with pigz (.txt.gz)
- pigz threads controlled by -t option
- Error message at startup if pigz is missing and .gz input files are provided

---

## [2.0.0] - 2025-03-10

### Added
- Auto-detection of input format (.fastq, .fastq.gz, .bam)
- BAM support via samtools subprocess
- Options --include_unmapped, --skip_secondary, --skip_supplementary
- Mixed input files in a single command (fastq + bam together)

### Changed
- Version bump 1.0.0 → 2.0.0
- Unified tool replaces both nano_extract and bam_extract


# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [1.0.0] - 2025-01-01

### Added
- Initial Rust release — port of `Nano_Extract.V3.8.py`
- Native gzip decompression via `flate2` + `zlib-ng` (no external `pigz` dependency)
- Parallel quality computation via `rayon`
- Identical CLI interface to the Python version (`-i`, `-o`, `-t`, `--chunk_size`)
- Identical TSV output format (`read_id`, `length`, `mean_quality`)
- Progress bar via `indicatif`
- Unit tests for quality score calculation
- Support for `.fastq` and `.fastq.gz` input files
- Support for multiple input files in a single run
- Dynamic chunk size (50k reads < 1 GB, 200k reads ≥ 1 GB)

---

## Python reference — Nano_Extract.V3.8

### V3.8
- Dynamic thread allocation, skips pigz if all files uncompressed

### V3.7 and earlier
- Previous Python versions (see `Nano_Extract.V3.8.py`)
