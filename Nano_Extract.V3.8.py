#!/usr/bin/env python3
# ---------------------------------------------------------
# Script: Nano_Extract.V3.8.py
# Version: 3.8
# Description: Dynamic thread allocation, skips pigz if all files uncompressed
# ---------------------------------------------------------

import argparse
import os
import sys
import numpy as np
from multiprocessing import Pool, cpu_count, Manager, Process
from tqdm import tqdm
import subprocess

# -----------------------------
# FASTQ Handling
# -----------------------------
def mean_read_quality_np_safe(qual):
    if not qual:
        return 0.0
    phred_scores = np.array([ord(c)-33 for c in qual], dtype=np.float32)
    if len(phred_scores) == 0:
        return 0.0
    probs = 10**(-phred_scores/10)
    mean_err = probs.mean()
    return 0.0 if mean_err==0.0 else -10*np.log10(mean_err)

def process_read(read):
    read_id, seq, qual = read
    return (read_id, len(seq), mean_read_quality_np_safe(qual))

def produce_chunks(fastq_file, chunk_size, queue, pigz_threads=1):
    """Producer process: reads FASTQ and fills queue with chunks"""
    chunk = []
    use_pigz = fastq_file.endswith(".gz") and pigz_threads > 0
    if fastq_file == "-":
        fh = sys.stdin
    elif use_pigz:
        proc = subprocess.Popen(
            ["pigz", "-dc", "-p", str(pigz_threads), fastq_file],
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            universal_newlines=True,
            bufsize=1
        )
        fh = proc.stdout
    else:
        fh = open(fastq_file, "r")

    with fh:
        while True:
            header = fh.readline()
            if not header:
                if chunk:
                    queue.put(chunk)
                break
            seq = fh.readline().rstrip()
            fh.readline()  # '+'
            qual = fh.readline().rstrip()
            read_id = header.rstrip()[1:].split()[0]
            chunk.append((read_id, seq, qual))
            if len(chunk) >= chunk_size:
                queue.put(chunk)
                chunk = []
    queue.put(None)  # sentinel to indicate end

# -----------------------------
# Argument Parsing
# -----------------------------
def parse_args():
    parser = argparse.ArgumentParser(
        description="Nano_Extract.V3.8: Async Nanopore FASTQ extraction, skip pigz if all uncompressed"
    )
    parser.add_argument("-i","--input",required=True,nargs="+",help="Input FASTQ files (.fastq or .fastq.gz)")
    parser.add_argument("-o","--output",default="length_quality",help="Suffix for output filenames (_ added automatically, no .txt)")
    parser.add_argument("-t","--threads",type=int,default=cpu_count(),help="Maximum total threads for pigz + Python Pool")
    parser.add_argument("--chunk_size",type=int,default=None,help="Optional base reads per chunk for processing")
    return parser.parse_args()

# -----------------------------
# Main
# -----------------------------
def main():
    args = parse_args()
    total_cpus = min(args.threads, cpu_count())

    # Check if any input is gzipped
    any_gz = any(f.endswith(".gz") for f in args.input)

    suffix = args.output
    if suffix.endswith(".txt"):
        suffix = suffix[:-4]
    if not suffix.startswith("_"):
        suffix = "_" + suffix

    for fastq_file in args.input:
        base_name = os.path.basename(fastq_file)
        if base_name.endswith(".fastq.gz"):
            base_name = base_name[:-9]
        elif base_name.endswith(".fastq"):
            base_name = base_name[:-6]
        output_file = f"{base_name}{suffix}.txt"

        file_size = os.path.getsize(fastq_file) if fastq_file != "-" else 0
        chunk_size = args.chunk_size if args.chunk_size else (50000 if file_size < 1e9 else 200000)

        # Dynamic thread allocation
        if any_gz:
            # Allocate threads for pigz and Pool
            pigz_threads = max(1, min(total_cpus-1, int(total_cpus * (0.2 if file_size>1e9 else 0.4))))
            pool_threads = max(1, total_cpus - pigz_threads)
        else:
            # All uncompressed: use all threads for Pool
            pigz_threads = 0
            pool_threads = total_cpus

        print(f"File: {fastq_file} ({file_size/1e6:.1f} MB) -> pigz_threads={pigz_threads}, pool_threads={pool_threads}, chunk_size={chunk_size}")

        manager = Manager()
        queue = manager.Queue(maxsize=10)
        producer = Process(target=produce_chunks, args=(fastq_file, chunk_size, queue, pigz_threads))
        producer.start()

        total_reads = 0
        buffer = []

        with open(output_file,"w") as out_handle:
            out_handle.write("read_id\tlength\tmean_quality\n")
            pbar = tqdm(desc=f"Processing {fastq_file}", ncols=100, unit='reads')

            with Pool(processes=pool_threads) as pool:
                while True:
                    chunk = queue.get()
                    if chunk is None:
                        break
                    for read_id, length, mean_q in pool.imap_unordered(process_read, chunk):
                        buffer.append(f"{read_id}\t{length}\t{mean_q:.2f}\n")
                        total_reads += 1
                        if len(buffer) >= 10000:
                            out_handle.writelines(buffer)
                            buffer = []
                        if total_reads % 1000 == 0:
                            pbar.update(1000)

            if buffer:
                out_handle.writelines(buffer)
            pbar.update(total_reads % 1000)
            pbar.close()

        producer.join()
        print(f"{total_reads} reads processed in {fastq_file}")
        print(f"Results written to: {output_file}\n")

if __name__ == "__main__":
    main()