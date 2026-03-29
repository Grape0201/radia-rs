# radia-core

The core library for the radia-rs photon shielding simulation engine.

## Module Overview

- `kernel.rs`: Implementation of the Point Kernel calculation. Supports both serial and **parallel execution** (via `rayon`) for multi-detector dose rate assessments.
- `physics.rs`: Unified physics data management. `MaterialPhysicsTable` consolidates macroscopic cross-sections and buildup factors, optimized for high-throughput lookups during simulation.
- `buildup.rs`: Extensible buildup factor framework. Supports multiple semi-empirical models:
    - **Constant**: Fixed value (primarily for testing/vacuum).
    - **Taylor**: Two-exponential approximation.
    - **Berger**: Linear-exponential form.
    - **Geometric Progression (G-P)**: Standard ANS-6.4.3 compliant method with log-linear interpolation.
    - **Table**: Direct lookup with linear interpolation.
- `mass_attenuation.rs`: Material composition and attenuation logic. Handles NIST-based data and custom mixture calculations.
- `csg.rs`: Constructive Solid Geometry (CSG) engine. Uses **Reverse Polish Notation (RPN)** based bitmask evaluation for complex cell geometry intersections.
- `primitive.rs`: Highly optimized geometric primitives (Sphere, RPP, Cylinder). Features **SIMD-accelerated batched intersection** algorithms to maximize data-level parallelism.
- `source.rs`: Energy-dependent radiation source definitions, supporting spectrum weighting and coordinate transformations.
- `constants.rs`: Physical constants and precision-related epsilon values used throughout the engine.

## Performance Optimizations

`radia-core` is engineered for high performance in radiation shielding scenarios:

### 1. Batched Intersection Engine
Traditional ray-tracing handles one ray at a time. `radia-core` implements a **Primitive-on-Rays** batching strategy. By processing multiple rays against a single primitive in a tight loop, primitive parameters (like center and radii) stay pinned in the CPU L1 cache, significantly reducing memory latency.

### 2. SIMD-Accelerated Math
The geometry engine leverages `glam`, which uses **SSE2/AVX** (on x86) or **NEON** (on ARM/Mac) instructions. Primitives like RPPs use vectorized min/max operations to calculate intersection intervals in a fraction of the time required by scalar code.

### 3. Bitmask CSG Evaluation
Complex geometry is evaluated using a fast bitmask approach. All primitive intersection ranges are pre-calculated for a ray, and the CSG tree is evaluated using bitwise logic, avoiding expensive branching and recursive function calls.

### 4. Zero-Cost Parallelism
The calculation kernel is designed to be thread-safe and embarrassingly parallel. Using `rayon`, work is automatically balanced across all available CPU threads with minimal synchronization overhead.
