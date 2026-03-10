# nano_extract v2.0.0

Fast Nanopore read length/quality extractor.  
**Détection automatique du format d'entrée : `.fastq` `.fastq.gz` `.bam`**

Remplace à la fois `nano_extract v1.0.0` et `bam_extract`.

---

## Formats supportés

| Format | Backend | Dépendance |
|--------|---------|------------|
| `.fastq` | Rust natif | aucune |
| `.fastq.gz` | flate2/zlib-ng | aucune |
| `.bam` | samtools subprocess | samtools |

Les fichiers peuvent être **mélangés** dans la même commande :
```bash
nano_extract -i reads.fastq.gz mapping.bam autre.fastq -o results
```

---

## Installation

```bash
conda activate nanostats

# Uniquement si tu utilises des fichiers BAM
conda install -c bioconda samtools

# Compiler
conda install -c conda-forge rust cmake   # si pas déjà installé
git clone https://github.com/papaKarN/nano_extract
cd nano_extract
cargo build --release
cp target/release/nano_extract $CONDA_PREFIX/bin/
```

---

## Utilisation

```bash
# FASTQ (identique à v1.0.0)
nano_extract -i sample.fastq.gz -o results -t 8

# BAM
nano_extract -i sample.bam -o results -t 8

# Fichiers mixtes
nano_extract -i *.fastq.gz *.bam -o results -t 8

# BAM — exclure les reads non mappés
nano_extract -i sample.bam -o results --include_unmapped false
```

### Options

| Option | Défaut | Description |
|--------|--------|-------------|
| `-i / --input` | requis | Fichiers `.fastq`, `.fastq.gz`, `.fq`, `.fq.gz`, `.bam` |
| `-o / --output` | `length_quality` | Suffixe du fichier de sortie |
| `-t / --threads` | nb CPUs | Threads |
| `--chunk_size` | auto | Reads par chunk (50k < 1 GB, 200k sinon) |
| `--include_unmapped` | `true` | [BAM] Inclure les reads non mappés |
| `--skip_secondary` | `true` | [BAM] Ignorer les alignements secondaires |
| `--skip_supplementary` | `true` | [BAM] Ignorer les alignements supplémentaires |

---

## Format de sortie

Identique pour tous les formats d'entrée :

```
read_id	length	mean_quality
read1	15234	12.45
read2	8901	14.20
```

---

## Tests

```bash
cargo test
```

---

## License

MIT
