# radia-core

The core library for the radia-rs photon shielding simulation engine.

## Module Overview

- `physics.rs`: Unified physics data management. Provides `MaterialPhysicsTable` which handles both macroscopic cross sections (attenuation) and buildup factor models.
- `material.rs`: Material and composition definitions. Handles loading from NIST database and managing partial densities.
- `csg.rs`: Constructive Solid Geometry (CSG) implementation. Manages the 3D world, cells, and ray-segment intersections.
- `primitive.rs`: Basic geometric primitives (Sphere, RPP, Cylinder) and their intersection logic.
- `source.rs`: Radiation source definitions, primarily point sources and spectrum data.
- `config.rs`: Serialization and deserialization logic for world and simulation configurations.
- `constants.rs`: Physical constants and numerical tolerances used across the engine.
