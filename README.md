# radia-rs

A photon shielding calculation implementation using the Point Kernel Method in Rust.

## Project Structure

This repository is a Cargo workspace consisting of the following crates:

- **`radia-core`**: Core calculation logic, geometry (CSG), and material handling.
  - No dependencies on `rayon`.
  - Suitable for use in environments where thread-based parallelism is not desired or needed.
- **`radia-cli`**: CLI-specific logic and high-performance parallel kernel.
  - Extends `radia-core` with `rayon`-based parallelization.
  - Contains benchmarks and the main executable.
- **`radia-wasm`**: WebAssembly bindings for `radia-core`.
  - Enables point cloud generation and visualization in web browsers.

## Development

### Build

```bash
cargo build
```

### Test

```bash
cargo test
```

### Run Benchmarks

```bash
# Run all benchmarks
cargo bench

# Run a specific benchmark in radia-cli
cargo bench --package radia-cli --bench kernel_benchmark
```

## License

MIT License
