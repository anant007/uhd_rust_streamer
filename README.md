# RFNoC Tool - High-Performance Rust Implementation

A feature-rich Rust implementation of the RFNoC streaming tool for USRP SDR devices. This project provides enhanced performance, safety, and functionality compared to the original C++ implementation.

## Features

### Core Capabilities
- **Multi-stream capture** with independent ring buffers per stream
- **PPS synchronization** for precise timing alignment
- **Lock-free ring buffers** using `rtrb` for high-performance data handling
- **Zero-copy packet processing** with `zerocopy` and `rkyv`
- **Async coordination** using `smol` runtime (optimized for real-time)
- **Comprehensive YAML configuration** with validation

### Export Formats
- CSV for analysis
- MATLAB (.mat) files via HDF5
- Raw binary data
- JSON for debugging
- Native HDF5 format

### Interactive Features
- **REPL interface** with syntax highlighting and command history
- **Real-time monitoring** of streams and buffers
- **Dynamic graph manipulation** during runtime
- **Performance profiling** and telemetry

### Architecture Highlights
- **Layered architecture** with clear separation of concerns
- **Actor-based coordination** for loose coupling
- **Safe FFI** with UHD using CXX
- **Comprehensive error handling** with structured types
- **Property-based testing** for reliability

## Prerequisites

- Rust 1.70+ (for stable async traits)
- UHD 4.0+ installed
- C++ compiler supporting C++14
- Boost libraries
- CMake (for building UHD if needed)

### Platform-specific

**Linux:**
```bash
sudo apt-get install libuhd-dev libboost-all-dev
```

**macOS:**
```bash
brew install uhd boost
```

**Windows:**
- Install UHD from NI website
- Set `UHD_DIR` environment variable

## Building

1. Clone the repository:
```bash
git clone https://github.com/tsianant/rust_uhd_streamer
cd rfnoc-tool
```

2. Build the project:
```bash
cargo build --release
```

3. Run tests:
```bash
cargo test
```

4. Run benchmarks:
```bash
cargo bench
```

## Usage

### Basic Capture
```bash
# Capture 10,000 packets at 10 Msps
rfnoc-tool capture --rate 10e6 --num-packets 10000

# Continuous capture with PPS sync
rfnoc-tool capture --pps-reset --num-packets 0
```

### Multi-Stream Capture
```bash
# Capture from all DDC outputs
rfnoc-tool capture --multi-stream --rate 25e6 --pps-reset

# With custom YAML configuration
rfnoc-tool capture --config advanced_config.yaml --multi-stream
```

### Interactive REPL
```bash
# Start REPL with API server
rfnoc-tool repl --with-api --api-addr 0.0.0.0:8080

# In REPL:
rfnoc> devices
rfnoc> connect usrp_0
rfnoc> graph usrp_0
rfnoc> start usrp_0 0/DDC#0:0 0/DDC#0:1
rfnoc> streamstats
rfnoc> export csv output.csv
```

### Analysis and Export
```bash
# Analyze captured file
rfnoc-tool analyze capture_0.dat --output analysis.csv

# Export to MATLAB format
rfnoc-tool export capture_*.dat --format matlab --output data.mat

# Export specific time range
rfnoc-tool export capture_0.dat --format csv --output subset.csv \
    --start-time 1.0 --end-time 5.0
```

### Performance Testing
```bash
# Run benchmark at 200 Msps with 4 streams
rfnoc-tool benchmark --rate 200e6 --streams 4 --duration 30
```

## Configuration

### YAML Configuration Example

```yaml
# Device configuration
device:
  args: "addr=192.168.10.2"
  clock_source: "internal"
  time_source: "internal"
  pps_reset:
    enable: true
    wait_time_sec: 1.5
    verify_reset: true

# Multi-stream configuration
stream:
  multi_stream:
    enable: true
    sync_streams: true
    separate_files: true
    buffer_config:
      ring_buffer_size: 268435456  # 256 MB per stream
      batch_write_size: 1000

# Graph configuration
graph:
  connections:
    - src_block: "0/Radio#0"
      src_port: 0
      dst_block: "0/DDC#0"
      dst_port: 0
      
  block_properties:
    "0/Radio#0":
      freq: "1e9"
      gain: "30"
      rate: "200e6"
    "0/DDC#0":
      output_rate/0: "25e6"
      output_rate/1: "25e6"
```

## Performance Optimization

### Real-time Configuration
```bash
# Set CPU affinity and real-time priority
sudo rfnoc-tool capture --config rt_config.yaml --cpu-affinity 2,3,4,5

# Enable huge pages
echo 1024 | sudo tee /proc/sys/vm/nr_hugepages
```

### Buffer Tuning
- Ring buffer size: Set based on capture rate and disk speed
- Batch write size: Balance between latency and throughput
- High water mark: Monitor and adjust based on overflow statistics

## Development

### Running the REPL for Development
```bash
# With debug logging
RUST_LOG=debug cargo run -- repl

# With tracing spans
RUST_LOG=rfnoc_tool[span{device_io}]=trace cargo run
```

### Adding New Export Formats
1. Add format variant to `ExportFormat` enum
2. Implement export function in `export.rs`
3. Add format parsing in `FromStr` implementation
4. Update CLI and REPL commands

### Extending the REPL
1. Add command variant to `ReplCommand` enum
2. Implement parsing logic in `ReplCommand::parse`
3. Add execution logic in `execute_command`
4. Update help text

## Troubleshooting

### Common Issues

**UHD not found during build:**
```bash
export UHD_DIR=/usr/local
export BOOST_ROOT=/usr/local
cargo clean
cargo build
```

**Ring buffer overflows:**
- Increase buffer size in configuration
- Reduce sample rate
- Use separate files for each stream
- Check disk write speed

**PPS sync failures:**
- Verify PPS signal is connected
- Increase wait time in configuration
- Check GPS lock status if using GPS

## Performance Benchmarks

On a typical system (Intel i7, NVMe SSD):
- Single stream at 200 Msps: <5% CPU, no overflows
- 4 streams at 50 Msps each: <20% CPU, no overflows
- Ring buffer latency: <20µs average
- Export throughput: >1 GB/s to SSD

## Contributing

1. Fork the repository
2. Create a feature branch
3. Add tests for new functionality
4. Ensure all tests pass
5. Submit a pull request

### Code Style
- Run `cargo fmt` before committing
- Run `cargo clippy` and address warnings
- Add documentation for public APIs
- Include benchmarks for performance-critical code

## License

This project is licensed under GPL 3.0.

## Acknowledgments

- Original C++ implementation authors
- UHD/USRP team at Ettus Research/NI
- Rust embedded and DSP communities
- Contributors to the async ecosystem

## References

- [UHD Documentation](https://files.ettus.com/manual/)
- [RFNoC Specification](https://www.ettus.com/sdr-software/rfnoc/)
- [Rust Async Book](https://rust-lang.github.io/async-book/)
- [CXX Documentation](https://cxx.rs/)
