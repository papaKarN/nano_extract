// ---------------------------------------------------------
// nano_extract v1.0.0
// Rust port of Nano_Extract.V3.8.py
// Fast Nanopore FASTQ length/quality extractor
// ---------------------------------------------------------

use clap::Parser;
use flate2::read::MultiGzDecoder;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::path::Path;
use std::time::Instant;

// -----------------------------
// CLI Arguments (mirrors Python script)
// -----------------------------
#[derive(Parser, Debug)]
#[command(
    name = "nano_extract",
    version = "1.0.0",
    about = "Fast Nanopore FASTQ length/quality extractor (Rust port of Nano_Extract V3.8)"
)]
struct Args {
    /// Input FASTQ files (.fastq or .fastq.gz)
    #[arg(short, long, required = true, num_args = 1..)]
    input: Vec<String>,

    /// Suffix for output filenames (_ added automatically, no .txt)
    #[arg(short, long, default_value = "length_quality")]
    output: String,

    /// Number of threads for parallel processing
    #[arg(short, long, default_value_t = num_cpus())]
    threads: usize,

    /// Reads per chunk for processing
    #[arg(long, default_value = "0")]
    chunk_size: usize,
}

fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

// -----------------------------
// Quality calculation
// Identical logic to mean_read_quality_np_safe in Python
// -----------------------------
#[inline(always)]
fn mean_read_quality(qual: &[u8]) -> f64 {
    if qual.is_empty() {
        return 0.0;
    }
    // Sum of error probabilities: 10^(-(Q-33)/10)
    let sum_prob: f64 = qual.iter().map(|&b| {
        let q = (b as f64) - 33.0;
        10_f64.powf(-q / 10.0)
    }).sum();

    let mean_err = sum_prob / qual.len() as f64;
    if mean_err == 0.0 {
        0.0
    } else {
        -10.0 * mean_err.log10()
    }
}

// -----------------------------
// A single parsed read
// -----------------------------
struct Read {
    id:     String,
    length: usize,
    qual:   Vec<u8>,
}

// -----------------------------
// Result after quality calc
// -----------------------------
struct ReadResult {
    id:      String,
    length:  usize,
    mean_q:  f64,
}

// -----------------------------
// Open a FASTQ file (plain or gzipped)
// Returns a boxed BufRead trait object
// -----------------------------
fn open_fastq(path: &str) -> io::Result<Box<dyn BufRead + Send>> {
    if path == "-" {
        return Ok(Box::new(BufReader::new(io::stdin())));
    }

    let file = File::open(path)?;

    if path.ends_with(".gz") {
        Ok(Box::new(BufReader::with_capacity(
            2 * 1024 * 1024, // 2 MB read buffer
            MultiGzDecoder::new(file),
        )))
    } else {
        Ok(Box::new(BufReader::with_capacity(
            4 * 1024 * 1024, // 4 MB read buffer
            file,
        )))
    }
}

// -----------------------------
// Parse FASTQ into chunks of reads
// Yields Vec<Read> of `chunk_size` reads at a time
// -----------------------------
fn parse_fastq_chunks(
    reader: &mut dyn BufRead,
    chunk_size: usize,
) -> Vec<Vec<Read>> {
    let mut all_chunks: Vec<Vec<Read>> = Vec::new();
    let mut current_chunk: Vec<Read> = Vec::with_capacity(chunk_size);

    let mut header = String::new();
    let mut seq    = String::new();
    let mut plus   = String::new();
    let mut qual   = String::new();

    loop {
        header.clear();
        if reader.read_line(&mut header).unwrap_or(0) == 0 { break; }
        let header = header.trim_end();
        if header.is_empty() { continue; }

        seq.clear();
        reader.read_line(&mut seq).unwrap_or(0);
        let seq = seq.trim_end();

        plus.clear();
        reader.read_line(&mut plus).unwrap_or(0); // skip '+'

        qual.clear();
        reader.read_line(&mut qual).unwrap_or(0);
        let qual = qual.trim_end();

        // Extract read ID (first token after '@')
        let read_id = header
            .trim_start_matches('@')
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_string();

        current_chunk.push(Read {
            id:     read_id,
            length: seq.len(),
            qual:   qual.as_bytes().to_vec(),
        });

        if current_chunk.len() >= chunk_size {
            all_chunks.push(current_chunk);
            current_chunk = Vec::with_capacity(chunk_size);
        }
    }

    if !current_chunk.is_empty() {
        all_chunks.push(current_chunk);
    }

    all_chunks
}

// -----------------------------
// Process a single file
// -----------------------------
fn process_file(
    fastq_path: &str,
    output_suffix: &str,
    threads: usize,
    chunk_size: usize,
) -> io::Result<()> {
    // Build output filename
    let base = Path::new(fastq_path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();

    let base = if base.ends_with(".fastq.gz") {
        &base[..base.len() - 9]
    } else if base.ends_with(".fastq") {
        &base[..base.len() - 6]
    } else {
        &base
    };

    let suffix = {
        let s = output_suffix.trim_end_matches(".txt");
        if s.starts_with('_') {
            s.to_string()
        } else {
            format!("_{}", s)
        }
    };

    let output_path = format!("{}{}.txt", base, suffix);

    let file_size = std::fs::metadata(fastq_path)
        .map(|m| m.len())
        .unwrap_or(0);

    let effective_chunk = if chunk_size > 0 {
        chunk_size
    } else if file_size < 1_000_000_000 {
        50_000
    } else {
        200_000
    };

    println!(
        "File: {} ({:.1} MB) -> threads={}, chunk_size={}",
        fastq_path,
        file_size as f64 / 1e6,
        threads,
        effective_chunk
    );

    // --- Configure Rayon thread pool ---
    rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build_global()
        .ok(); // ignore error if already initialized

    let start = Instant::now();

    // --- Open & parse ---
    let mut reader = open_fastq(fastq_path)?;
    let chunks = parse_fastq_chunks(&mut *reader, effective_chunk);

    // --- Progress bar ---
    let total_reads_approx: u64 = chunks.iter().map(|c| c.len() as u64).sum();
    let pb = ProgressBar::new(total_reads_approx);
    pb.set_style(
        ProgressStyle::with_template(
            "Processing {msg} [{bar:40.cyan/blue}] {pos}/{len} reads ({eta})"
        )
        .unwrap()
        .progress_chars("=>-"),
    );
    pb.set_message(fastq_path.to_string());

    // --- Open output ---
    let out_file = File::create(&output_path)?;
    let mut writer = BufWriter::with_capacity(4 * 1024 * 1024, out_file);
    writeln!(writer, "read_id\tlength\tmean_quality")?;

    // --- Process chunks in parallel with Rayon ---
    let mut total_reads: u64 = 0;

    for chunk in chunks {
        let chunk_len = chunk.len() as u64;

        // Parallel quality calculation over the chunk
        let results: Vec<ReadResult> = chunk
            .into_par_iter()
            .map(|read| ReadResult {
                mean_q: mean_read_quality(&read.qual),
                id:     read.id,
                length: read.length,
            })
            .collect();

        // Write results sequentially (I/O is not parallelisable here)
        for r in results {
            writeln!(writer, "{}\t{}\t{:.2}", r.id, r.length, r.mean_q)?;
        }

        total_reads += chunk_len;
        pb.set_position(total_reads);
    }

    pb.finish_with_message(format!("done ({})", fastq_path));
    writer.flush()?;

    let elapsed = start.elapsed();
    println!(
        "{} reads processed in {:.2}s -> {}",
        total_reads,
        elapsed.as_secs_f64(),
        output_path
    );

    Ok(())
}

// -----------------------------
// Entry point
// -----------------------------
fn main() {
    let args = Args::parse();

    let threads = args.threads.max(1);
    let chunk_size = args.chunk_size;

    let mut suffix = args.output.clone();
    if suffix.ends_with(".txt") {
        suffix = suffix[..suffix.len() - 4].to_string();
    }
    if !suffix.starts_with('_') {
        suffix = format!("_{}", suffix);
    }

    for fastq_path in &args.input {
        if let Err(e) = process_file(fastq_path, &suffix, threads, chunk_size) {
            eprintln!("Error processing {}: {}", fastq_path, e);
            std::process::exit(1);
        }
    }
}

// -----------------------------
// Tests
// -----------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mean_quality_simple() {
        // All phred 30 => Q30 => error prob 0.001
        // mean_err = 0.001 => -10*log10(0.001) = 30.0
        let qual: Vec<u8> = vec![30 + 33; 100]; // ASCII '?' = 63 = 30+33
        let q = mean_read_quality(&qual);
        assert!((q - 30.0).abs() < 0.01, "Expected Q30, got {}", q);
    }

    #[test]
    fn test_mean_quality_empty() {
        assert_eq!(mean_read_quality(&[]), 0.0);
    }

    #[test]
    fn test_mean_quality_mixed() {
        // Mix of Q10 and Q30
        let mut qual = vec![10 + 33; 50];
        qual.extend(vec![30 + 33; 50]);
        let q = mean_read_quality(&qual);
        // mean_err = (0.1 + 0.001) / 2 = 0.0505
        // expected = -10*log10(0.0505) ≈ 12.97
        assert!(q > 12.0 && q < 14.0, "Got {}", q);
    }
}
