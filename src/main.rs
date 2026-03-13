// ---------------------------------------------------------
// nano_extract v2.1.0
// Unified Nanopore read length/quality extractor
// Auto-detects input format: .fastq / .fastq.gz / .bam
//
// FASTQ/FASTQ.GZ : native Rust (flate2 + rayon)
// BAM            : samtools view subprocess + rayon
//
// Output format: TSV gzipped (read_id / length / mean_quality)
// Output compression: pigz (uses -t threads)
// ---------------------------------------------------------

use clap::Parser;
use flate2::read::MultiGzDecoder;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Instant;

// ================================================================
// CLI
// ================================================================
#[derive(Parser, Debug)]
#[command(
    name = "nano_extract",
    version = "2.1.0",
    about = "Fast Nanopore read length/quality extractor\n\
             Auto-detects: .fastq  .fastq.gz  .bam\n\
             BAM support requires samtools in PATH"
)]
struct Args {
    /// Input files (.fastq, .fastq.gz or .bam — can be mixed)
    #[arg(short, long, required = true, num_args = 1..)]
    input: Vec<String>,

    /// Suffix for output filenames (_ added automatically, no .txt)
    #[arg(short, long, default_value = "length_quality")]
    output: String,

    /// Number of threads
    #[arg(short, long, default_value_t = num_cpus())]
    threads: usize,

    /// [BAM only] Include unmapped reads
    #[arg(long, default_value_t = true)]
    include_unmapped: bool,

    /// [BAM only] Skip secondary alignments
    #[arg(long, default_value_t = true)]
    skip_secondary: bool,

    /// [BAM only] Skip supplementary alignments
    #[arg(long, default_value_t = true)]
    skip_supplementary: bool,

    /// Reads per chunk for parallel processing (0 = auto)
    #[arg(long, default_value = "0")]
    chunk_size: usize,
}

fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

// ================================================================
// Input format detection
// ================================================================
#[derive(Debug, PartialEq)]
enum InputFormat {
    Fastq,
    FastqGz,
    Bam,
}

fn detect_format(path: &str) -> Result<InputFormat, String> {
    if path == "-" {
        return Ok(InputFormat::Fastq); // stdin assumed FASTQ
    }
    if path.ends_with(".fastq.gz") || path.ends_with(".fq.gz") {
        Ok(InputFormat::FastqGz)
    } else if path.ends_with(".fastq") || path.ends_with(".fq") {
        Ok(InputFormat::Fastq)
    } else if path.ends_with(".bam") {
        Ok(InputFormat::Bam)
    } else {
        Err(format!(
            "Unrecognised file extension for '{}'. Expected: .fastq, .fastq.gz, .fq, .fq.gz, .bam",
            path
        ))
    }
}

// ================================================================
// Quality calculation (identical for FASTQ and SAM — both ASCII+33)
// ================================================================
#[inline(always)]
fn mean_quality(qual: &[u8]) -> f64 {
    if qual.is_empty() {
        return 0.0;
    }
    let sum_prob: f64 = qual.iter().map(|&b| {
        let q = (b as f64) - 33.0;
        10_f64.powf(-q / 10.0)
    }).sum();
    let mean_err = sum_prob / qual.len() as f64;
    if mean_err == 0.0 { 0.0 } else { -10.0 * mean_err.log10() }
}

// ================================================================
// Shared result type
// ================================================================
struct ReadResult {
    id:     String,
    length: usize,
    mean_q: f64,
}

// ================================================================
// Output filename builder
// ================================================================
fn build_output_path(input_path: &str, suffix: &str) -> String {
    let base = Path::new(input_path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();

    let base = if base.ends_with(".fastq.gz") { &base[..base.len()-9] }
    else if base.ends_with(".fq.gz")    { &base[..base.len()-6] }
    else if base.ends_with(".fastq")    { &base[..base.len()-6] }
    else if base.ends_with(".fq")       { &base[..base.len()-3] }
    else if base.ends_with(".bam")      { &base[..base.len()-4] }
    else                                { &base };

    format!("{}{}.txt", base, suffix)
}

// ================================================================
// Chunk writer — shared by FASTQ and BAM paths
// ================================================================
fn flush_chunk(
    chunk: &mut Vec<(String, usize, Vec<u8>)>,
    writer: &mut BufWriter<File>,
) -> io::Result<()> {
    let results: Vec<ReadResult> = chunk
        .drain(..)
        .collect::<Vec<_>>()
        .into_par_iter()
        .map(|(id, length, qual)| ReadResult {
            mean_q: mean_quality(&qual),
            id,
            length,
        })
        .collect();

    for r in results {
        writeln!(writer, "{}\t{}\t{:.2}", r.id, r.length, r.mean_q)?;
    }
    Ok(())
}

// ================================================================
// FASTQ processor (plain or gzipped)
// ================================================================
fn open_fastq(path: &str) -> io::Result<Box<dyn BufRead + Send>> {
    if path == "-" {
        return Ok(Box::new(BufReader::new(io::stdin())));
    }
    let file = File::open(path)?;
    if path.ends_with(".gz") {
        Ok(Box::new(BufReader::with_capacity(2 * 1024 * 1024, MultiGzDecoder::new(file))))
    } else {
        Ok(Box::new(BufReader::with_capacity(4 * 1024 * 1024, file)))
    }
}

fn process_fastq(
    path: &str,
    output_path: &str,
    threads: usize,
    chunk_size: usize,
    pb: &ProgressBar,
) -> io::Result<u64> {

    rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build_global()
        .ok();

    let mut reader = open_fastq(path)?;
    let out_file = File::create(output_path)?;
    let mut writer = BufWriter::with_capacity(4 * 1024 * 1024, out_file);
    writeln!(writer, "read_id\tlength\tmean_quality")?;

    let mut total: u64 = 0;
    let mut chunk: Vec<(String, usize, Vec<u8>)> = Vec::with_capacity(chunk_size);

    let mut header = String::new();
    let mut seq    = String::new();
    let mut plus   = String::new();
    let mut qual   = String::new();

    loop {
        header.clear();
        if reader.read_line(&mut header).unwrap_or(0) == 0 { break; }
        let header = header.trim_end();
        if header.is_empty() { continue; }

        seq.clear();  reader.read_line(&mut seq).unwrap_or(0);
        plus.clear(); reader.read_line(&mut plus).unwrap_or(0);
        qual.clear(); reader.read_line(&mut qual).unwrap_or(0);

        let read_id = header.trim_start_matches('@')
            .split_whitespace().next().unwrap_or("").to_string();
        let seq_len = seq.trim_end().len();
        let qual_bytes = qual.trim_end().as_bytes().to_vec();

        chunk.push((read_id, seq_len, qual_bytes));

        if chunk.len() >= chunk_size {
            flush_chunk(&mut chunk, &mut writer)?;
            total += chunk_size as u64;
            pb.set_position(total);
        }
    }

    if !chunk.is_empty() {
        let rem = chunk.len() as u64;
        flush_chunk(&mut chunk, &mut writer)?;
        total += rem;
    }

    writer.flush()?;
    Ok(total)
}

// ================================================================
// ================================================================
// pigz check and output compression
// ================================================================
fn check_pigz() -> Result<String, String> {
    Command::new("pigz")
        .arg("--version")
        .output()
        .map_err(|_| "pigz not found in PATH. Please install pigz (conda install -c conda-forge pigz).".to_string())
        .map(|o| {
            // pigz prints version to stderr
            let v = String::from_utf8_lossy(&o.stderr);
            v.lines().next().unwrap_or("pigz").to_string()
        })
}

fn compress_output(output_path: &str, threads: usize) -> Result<String, String> {
    // pigz -f : force overwrite, -p : threads
    // compresses file.txt -> file.txt.gz and removes original
    let status = Command::new("pigz")
        .arg("-f")
        .arg("-p").arg(threads.to_string())
        .arg(output_path)
        .status()
        .map_err(|e| format!("Failed to run pigz: {}", e))?;

    if !status.success() {
        return Err(format!("pigz failed on {}", output_path));
    }
    Ok(format!("{}.gz", output_path))
}

// BAM processor (via samtools view)
// ================================================================
fn check_samtools() -> Result<String, String> {
    Command::new("samtools")
        .arg("--version")
        .output()
        .map_err(|_| "samtools not found in PATH. Please install samtools (conda install -c bioconda samtools).".to_string())
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines().next().unwrap_or("samtools").to_string()
        })
}

fn build_exclude_flag(include_unmapped: bool, skip_secondary: bool, skip_supplementary: bool) -> u32 {
    let mut flag: u32 = 0;
    if !include_unmapped  { flag |= 4; }
    if skip_secondary     { flag |= 256; }
    if skip_supplementary { flag |= 2048; }
    flag
}

fn process_bam(
    path: &str,
    output_path: &str,
    threads: usize,
    chunk_size: usize,
    include_unmapped: bool,
    skip_secondary: bool,
    skip_supplementary: bool,
    pb: &ProgressBar,
) -> Result<u64, Box<dyn std::error::Error>> {

    rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build_global()
        .ok();

    let exclude_flag = build_exclude_flag(include_unmapped, skip_secondary, skip_supplementary);

    let mut cmd = Command::new("samtools");
    cmd.arg("view").arg("-@").arg(threads.to_string());
    if exclude_flag > 0 {
        cmd.arg("-F").arg(exclude_flag.to_string());
    }
    cmd.arg(path).stdout(Stdio::piped()).stderr(Stdio::null());

    let mut child = cmd.spawn()
        .map_err(|e| format!("Failed to spawn samtools: {}", e))?;

    let stdout = child.stdout.take()
        .ok_or("Could not capture samtools stdout")?;

    let reader = BufReader::with_capacity(4 * 1024 * 1024, stdout);

    let out_file = File::create(output_path)?;
    let mut writer = BufWriter::with_capacity(4 * 1024 * 1024, out_file);
    writeln!(writer, "read_id\tlength\tmean_quality")?;

    let mut total: u64 = 0;
    let mut chunk: Vec<(String, usize, Vec<u8>)> = Vec::with_capacity(chunk_size);

    for line in reader.lines() {
        let line = line?;
        if line.starts_with('@') { continue; }

        let fields: Vec<&str> = line.splitn(12, '\t').collect();
        if fields.len() < 11 { continue; }

        let read_id = fields[0].to_string();
        let seq_len = fields[9].len();
        let qual_str = fields[10];
        if qual_str == "*" { continue; }

        // SAM qual is ASCII+33 — same encoding as FASTQ
        chunk.push((read_id, seq_len, qual_str.as_bytes().to_vec()));

        if chunk.len() >= chunk_size {
            flush_chunk(&mut chunk, &mut writer)?;
            total += chunk_size as u64;
            pb.set_position(total);
        }
    }

    if !chunk.is_empty() {
        let rem = chunk.len() as u64;
        flush_chunk(&mut chunk, &mut writer)?;
        total += rem;
    }

    writer.flush()?;
    child.wait()?;
    Ok(total)
}

// ================================================================
// Main
// ================================================================
fn main() {
    let args = Args::parse();
    let threads = args.threads.max(1);

    // Build output suffix
    let mut suffix = args.output.clone();
    if suffix.ends_with(".txt") { suffix = suffix[..suffix.len()-4].to_string(); }
    if !suffix.starts_with('_') { suffix = format!("_{}", suffix); }

    // Check pigz if any .gz input file is present
    let has_gz = args.input.iter().any(|f| f.ends_with(".gz"));
    if has_gz {
        match check_pigz() {
            Ok(v)  => println!("Compression support: {}", v),
            Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
        }
    }

    // Check samtools if any .bam file is present
    let has_bam = args.input.iter().any(|f| f.ends_with(".bam"));
    if has_bam {
        match check_samtools() {
            Ok(v)  => println!("BAM support: {}", v),
            Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
        }
    }

    for path in &args.input {
        // Detect format
        let fmt = match detect_format(path) {
            Ok(f)  => f,
            Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
        };

        let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        let chunk_size = if args.chunk_size > 0 { args.chunk_size }
                         else if file_size < 1_000_000_000 { 50_000 }
                         else { 200_000 };

        let output_path = build_output_path(path, &suffix);

        let fmt_label = match fmt {
            InputFormat::Fastq   => "FASTQ",
            InputFormat::FastqGz => "FASTQ.GZ",
            InputFormat::Bam     => "BAM",
        };

        println!(
            "File: {} [{}] ({:.1} MB) -> threads={}, chunk_size={}",
            path, fmt_label, file_size as f64 / 1e6, threads, chunk_size
        );

        // Progress bar
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::with_template(
                "Processing {msg} {spinner:.cyan} {pos} reads ({elapsed})"
            ).unwrap()
        );
        pb.set_message(path.clone());

        let start = Instant::now();

        let result = match fmt {
            InputFormat::Fastq | InputFormat::FastqGz => {
                process_fastq(path, &output_path, threads, chunk_size, &pb)
                    .map_err(|e| e.to_string())
            }
            InputFormat::Bam => {
                process_bam(
                    path, &output_path, threads, chunk_size,
                    args.include_unmapped, args.skip_secondary, args.skip_supplementary,
                    &pb,
                ).map_err(|e| e.to_string())
            }
        };

        match result {
            Ok(total) => {
                pb.finish_with_message(format!("done ({})", path));

                // Compress output with pigz
                print!("Compressing {} ... ", output_path);
                match compress_output(&output_path, threads) {
                    Ok(gz_path) => {
                        println!("done");
                        println!(
                            "{} reads processed in {:.2}s -> {}\n",
                            total, start.elapsed().as_secs_f64(), gz_path
                        );
                    }
                    Err(e) => {
                        eprintln!("\nWarning: compression failed ({}). Uncompressed file kept: {}", e, output_path);
                        println!(
                            "{} reads processed in {:.2}s -> {}\n",
                            total, start.elapsed().as_secs_f64(), output_path
                        );
                    }
                }
            }
            Err(e) => {
                eprintln!("Error processing {}: {}", path, e);
                std::process::exit(1);
            }
        }
    }
}

// ================================================================
// Tests
// ================================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quality_q30() {
        let qual = vec![b'?' ; 100]; // ASCII 63 = Phred 30
        let q = mean_quality(&qual);
        assert!((q - 30.0).abs() < 0.01, "Expected Q30, got {}", q);
    }

    #[test]
    fn test_quality_empty() {
        assert_eq!(mean_quality(&[]), 0.0);
    }

    #[test]
    fn test_quality_mixed() {
        let mut qual = vec![b'+'; 50]; // Q10
        qual.extend(vec![b'?'; 50]);   // Q30
        let q = mean_quality(&qual);
        assert!(q > 12.0 && q < 14.0, "Got {}", q);
    }

    #[test]
    fn test_detect_format() {
        assert_eq!(detect_format("sample.fastq").unwrap(),    InputFormat::Fastq);
        assert_eq!(detect_format("sample.fq").unwrap(),       InputFormat::Fastq);
        assert_eq!(detect_format("sample.fastq.gz").unwrap(), InputFormat::FastqGz);
        assert_eq!(detect_format("sample.fq.gz").unwrap(),    InputFormat::FastqGz);
        assert_eq!(detect_format("sample.bam").unwrap(),      InputFormat::Bam);
        assert!(detect_format("sample.txt").is_err());
    }

    #[test]
    fn test_output_path() {
        assert_eq!(build_output_path("sample.fastq",    "_lq"), "sample_lq.txt");
        assert_eq!(build_output_path("sample.fastq.gz", "_lq"), "sample_lq.txt");
        assert_eq!(build_output_path("sample.bam",      "_lq"), "sample_lq.txt");
    }

    #[test]
    fn test_exclude_flag() {
        // All defaults: skip secondary (256) + supplementary (2048) = 2304
        assert_eq!(build_exclude_flag(true, true, true), 2304);
        // Include unmapped OFF adds 4 -> 2308
        assert_eq!(build_exclude_flag(false, true, true), 2308);
        // Nothing excluded
        assert_eq!(build_exclude_flag(true, false, false), 0);
    }
}
