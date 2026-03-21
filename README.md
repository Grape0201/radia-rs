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
- **`radia-input`**: Input handling for the simulation.
  - Handles YAML input files.
  - Produces robust output ready for verification.
  - Provides structural validation for the input files.

### Input Validation Policy

Input validation is deliberately split into two distinct stages to maintain separation of concerns:

1. **Structural & Self-Contained Validation (`radia-input`)**
   - **Scope:** Validates syntax, structure, and internal consistency of configurations without requiring heavy data loading.
   - **Examples:** Ensuring arrays match in length (e.g., `energy_groups` vs `intensity_by_group`), ensuring numeric bounds, enforcing non-empty arrays, and verifying internal cross-references (e.g., cell material names exist in `buildup_alias_map`).
   - **Implementation:** Executed via `SimulationInput::validate()` and component `build()` methods directly after YAML deserialization.

2. **Semantic & Context-Dependent Validation (`radia-cli` / `radia-core`)**
   - **Scope:** Validates that structurally sound data actually maps to available physical and environmental models.
   - **Examples:** Checking if specified elements exist in the loaded physical properties database (`elements.json`), or verifying that attenuation tables can be successfully generated for given energies.
   - **Implementation:** Executed during the application run (in `radia-cli` or `radia-core`, e.g. within `MaterialPhysicsTable::generate()`).

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
