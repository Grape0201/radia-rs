# radia-rs

A photon shielding calculation implementation using the Point Kernel Method in Rust.

## Project Structure

This repository is a Cargo workspace consisting of the following crates:

- **`radia-core`**: Core calculation logic, geometry (CSG), and material handling.
  - MINIMAL dependencies.
  - Optimized with SIMD-accelerated batched intersections.
- **`radia-cli`**: CLI-specific logic and high-performance parallel kernel.
  - Contains the main executable.
  - Integrates all workspace crates into a production-ready tool.
- **`radia-input`**: High-level input handling and validation.
  - Handles YAML parsing into strongly-typed simulation configurations.
  - Provides robust structural and self-contained validation via `garde`.
  - see `examples/` for example usage.
- **`radia-report`**: Formatting and output logic.
  - Generates detailed reports (JSON/CSV) for regulatory compliance.
  - Implements zero-cost abstractions for data collection.
- **`radia-gui`**: Desktop application for editing and visualizing simulations.
  - Built with Tauri v2, React, and Three.js.
  - Provides a 3D preview of the geometry alongside a Monaco-powered YAML editor.
  - Leverages `radia-input` for real-time validation of simulation configurations.

## Key Features

- **Point Kernel Engine**: Accurate photon shielding calculation using the Point Kernel Method.
- **SIMD Acceleration**: Geometric primitive intersections (Sphere, RPP, Cylinder) are optimized using `glam`'s SIMD-backed types and batched algorithms.
- **Parallel Execution**: Leverages `rayon` for massive parallelization of dose-rate calculations across CPU cores.
- **Interactive 3D Visualization**: Real-time rendering of complex CSG geometries within the desktop application.
- **Comprehensive Physics**: Supports multiple buildup factor models (Constant, Taylor, Berger, G-P, Table) and composition-based mass attenuation calculations.
- **Robust Validation**: Two-stage validation policy ensuring both structural integrity and physical consistency.

### Input Validation Policy

Input validation is deliberately split into two distinct stages to maintain separation of concerns:

1. **Structural & Self-Contained Validation (`radia-input`)**
   - **Scope:** Validates syntax, structure, and internal consistency of configurations without requiring heavy data loading.
   - **Examples:** Ensuring arrays match in length (e.g., `energy_groups` vs `intensity_by_group`), ensuring numeric bounds, enforcing non-empty arrays, and verifying internal cross-references.
   - **Implementation:** Executed via `SimulationInput::validate()` directly after YAML deserialization.

2. **Semantic & Context-Dependent Validation (`radia-cli` / `radia-core`)**
   - **Scope:** Validates that structurally sound data actually maps to available physical and environmental models.
   - **Examples:** Checking if specified elements exist in `elements.json` or verifying that attenuation tables can be generated for the given energies.
   - **Implementation:** Executed during the application run (e.g., within `MaterialPhysicsTable::generate()`).

## Development

### Build

```bash
cargo build --release
```

### Test

```bash
cargo test
```

### Run Benchmarks

```bash
# Run all benchmarks
cargo bench

# Run specific kernel benchmarks in radia-core
cargo bench --package radia-core --bench kernel_benchmark
```

### GUI Application

The GUI requires [Bun](https://bun.sh/) for frontend dependencies.

```bash
cd radia-gui
bun install
bun tauri dev
```

## License

MIT License
