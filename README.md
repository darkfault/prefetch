# prefetch

**Intelligent file prefetching with format-aware byte range ordering.**

`prefetch` pre-warms files into the OS page cache by advising the kernel which byte ranges to read ahead. Unlike a simple sequential read, it understands file formats and loads segments in the order they'll actually be accessed.

Built-in support for **GGUF** (LLM model files). Extensible to any format via the **provider system** or user-defined **manifest files**.

## The Problem

Operating systems use lazy loading. When a program opens a large file via `mmap()`, nothing is actually read from disk. Data only arrives in RAM when the program touches each page, triggering a **page fault**. This means:

- **LLM inference**: First prompt after loading a model is slow (30-45s on HDD, 3-8s on SATA SSD)
- **Database cold start**: First queries after restart hit cold storage
- **Game level loading**: Assets stutter in as the player moves through the world
- **Scientific data**: Analysis pipelines stall waiting for instrument data

`prefetch` tells the kernel "read this data now" before the application needs it, eliminating the cold-start penalty.

## Quick Start

```bash
git clone https://github.com/darkfault/prefetch.git
cd prefetch
./scripts/install.sh
./scripts/test.sh
```

## Usage

```bash
# Discover available Ollama models
prefetch discover

# Analyze a file's internal structure
prefetch analyze model.gguf

# Warm a file into page cache (format auto-detected)
prefetch warm model.gguf --force

# Warm an Ollama model by name
prefetch warm llama3:latest --force

# Check what percentage of a file is in RAM
prefetch status model.gguf

# Check all discovered models
prefetch status

# Warm with specific strategy
prefetch warm model.gguf --strategy first-n-layers --layers 8 --force
prefetch warm data.bin --strategy sequential --force
```

## How It Works

```
1. DETECT    Auto-detect the file format via registered providers
2. PARSE     Read file headers to build a segment map (never reads bulk data)
3. ORDER     Sort segments by access priority (e.g., embedding -> layers -> output)
4. ADVISE    Issue kernel advisories for each byte range in priority order
5. REPORT    Query mincore() to show what's actually resident in RAM
```

On Linux, prefetching uses `posix_fadvise(FADV_WILLNEED)`. On macOS, `fcntl(F_RDADVISE)` with fallback to `madvise(MADV_WILLNEED)`. Both are non-blocking and asynchronous.

## Format Providers

### Built-in: GGUF (LLM Models)

Auto-detects GGUF files by magic bytes. Parses the tensor layout and maps tensors to logical layers (embedding, transformer blocks, output head). Warms in inference execution order so the first layers needed are cached first.

```
$ prefetch analyze deepseek-r1:1.5b
Format:  GGUF (qwen2)
Segments (31):
  token_embedding    125.2 MB    priority 0
  block.0             28.6 MB    priority 100
  block.1             28.6 MB    priority 101
  ...
  output_head        182.6 MB    priority 10001
```

### Custom: Manifest Files

For any file format, describe its structure in a `.prefetch.toml` file:

```toml
# mydata.db.prefetch.toml
format = "sqlite"

[[segments]]
name = "btree-index"
offset = 0
length = 4096
priority = 0

[[segments]]
name = "users-table"
offset = 8192
length = 52428800
priority = 1

[[segments]]
name = "logs-table"
offset = 60000000
length = 200000000
priority = 2
```

Place the manifest alongside the target file. `prefetch` auto-discovers it:

```bash
prefetch warm mydata.db --force     # reads mydata.db.prefetch.toml automatically
prefetch analyze mydata.db          # shows the segment layout
```

### Writing Your Own Provider

Implement the `FileProvider` trait in Rust:

```rust
use prefetch_core::providers::{FileProvider, FileLayout, Segment};

pub struct MyFormatProvider;

impl FileProvider for MyFormatProvider {
    fn name(&self) -> &str { "my-format" }

    fn can_handle(&self, path: &Path) -> bool {
        // Check magic bytes or extension
    }

    fn analyze(&self, path: &Path) -> Result<FileLayout> {
        // Parse headers, return segments with priorities
    }
}
```

Register it with the engine:

```rust
let mut engine = PrefetchEngine::new();
engine.register_provider(Box::new(MyFormatProvider));
engine.prefetch_file(path, &strategy, |progress| { ... })?;
```

## Use Cases

| Domain | File Format | What Gets Prefetched |
|--------|-------------|---------------------|
| **LLM Inference** | GGUF | Embedding, transformer blocks, output head in execution order |
| **Databases** | SQLite, PostgreSQL | Index pages, hot tables, WAL |
| **Game Assets** | Custom packs | Textures and meshes for the next area the player is approaching |
| **Genomics** | BAM/CRAM | Chromosome regions for the current analysis window |
| **Video Editing** | MP4/ProRes | Next 30 seconds of timeline data |
| **Scientific Data** | HDF5/NetCDF | Variables and dimensions for the current computation |
| **Satellite Imagery** | GeoTIFF | Tiles about to enter the viewport |
| **ML Training** | Datasets | Next N batches in the shuffle order |

## Prefetch Strategies

| Strategy | Description |
|----------|-------------|
| `inference-order` | Load segments by priority (lowest first). Default. |
| `first-n-layers` | Load only the first N segments. For memory-constrained systems. |
| `sequential` | Read file start to end. No format awareness needed. |

## Installation

### From Source

```bash
git clone https://github.com/darkfault/prefetch.git
cd prefetch
./scripts/install.sh
```

The install script handles Rust installation (if needed), builds the release binary, and copies it to `~/.cargo/bin/`.

### Manual Build

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
cargo build --release
# Binary: ./target/release/prefetch
```

### System Service

**Linux (systemd):**
```bash
sudo cp target/release/prefetch /usr/local/bin/
cp service/prefetch.service ~/.config/systemd/user/
systemctl --user enable --now prefetch
```

**macOS (launchd):**
```bash
sudo cp target/release/prefetch /usr/local/bin/
cp service/com.prefetch.plist ~/Library/LaunchAgents/
launchctl load ~/Library/LaunchAgents/com.prefetch.plist
```

## Configuration

```bash
prefetch config example > "$(prefetch config path)"
```

Key settings in `config/prefetch.example.toml`:

```toml
[prefetch]
strategy = "inference-order"
chunk_size_mb = 64

[memory]
max_cache_percent = 50
min_free_ram_gb = 2

[watch]
directories = ["~/.ollama/models"]
```

## Architecture

```
crates/
  prefetch-cli/        CLI binary: warm, status, analyze, discover, config
  prefetch-core/       Prefetch engine, platform backends, provider system
  prefetch-gguf/       GGUF file format parser
  prefetch-config/     Configuration and Ollama model discovery
  prefetch-daemon/     Background daemon
```

## Supported Platforms

| Platform | Prefetch | Cache Query | IO Priority |
|----------|----------|-------------|-------------|
| Linux (x86_64, aarch64) | `posix_fadvise` | `mincore` | `ioprio_set(IDLE)` |
| macOS (Apple Silicon, Intel) | `fcntl(F_RDADVISE)` | `mincore` | `setiopolicy_np` |

## Security

- **Input validation**: GGUF parser caps all sizes (tensor count, array length, string length, dimensions) to prevent memory exhaustion from malicious files
- **Integer overflow protection**: All tensor byte size calculations use checked arithmetic
- **Path traversal prevention**: Ollama blob digests are validated and canonicalized
- **File type validation**: Rejects non-regular files (devices, pipes, sockets) before mmap
- **Symlink safety**: Warns on symlinks, rejects those pointing to non-regular files
- **Truncation detection**: Verifies file size before and after mmap to avoid SIGBUS
- **No network access**: The tool never makes network calls
- **No data collection**: No telemetry, no usage tracking, no files written during operation
- **Supply chain**: `deny.toml` config for `cargo-deny` dependency auditing
- **CI**: GitHub Actions with build, test, clippy, and `cargo audit`

## Verification

Run the included test suite:

```bash
./scripts/test.sh
```

Tests cover: build, unit tests, CLI smoke tests, model discovery, cache status, prefetch warming, and GGUF parsing.

## Roadmap

- [x] Format-aware prefetching with pluggable provider system
- [x] GGUF provider (LLM models)
- [x] Custom manifest provider (any file format)
- [x] Cache residency reporting via mincore
- [x] Ollama model discovery
- [x] Security hardening
- [ ] Background daemon with filesystem watching
- [ ] Predictive pre-warming based on usage patterns
- [ ] Community format providers (SQLite, HDF5, BAM, GeoTIFF)
- [ ] Homebrew / apt / AUR packaging

## Contributing

Contributions welcome. Run the test suite before submitting:

```bash
./scripts/test.sh
```

To add a new format provider:
1. Implement `FileProvider` trait
2. Register it in `crates/prefetch-cli/src/main.rs`
3. Add tests
4. Submit a PR

## License

MIT
