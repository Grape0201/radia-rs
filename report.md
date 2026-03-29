# radia-rs Simulation Report

## Metadata
- **Version:** 0.1.0
- **Timestamp:** 1774780631
- **OS:** macos
- **Input File Hash:** examples/1million.yaml

## Physics Summary
- **Cross Section Library:** NIST XCOM (JSON)
- **Buildup Library:** Geometric Progression (GP)
- **Conversion Factors:** Interpolated

## Recognized World Structure
### Primitives
| Index | Type | Parameters |
|-------|------|------------|
| 0 (source_sphere) | Sphere | center: [0.0,0.0,0.0], radius2: 4.0 |
| 1 (shield_box) | RectangularParallelPiped | max: [5.0,5.0,5.0], min: [-5.0,-5.0,-5.0] |

### Cells
| Index | Material | CSG Instructions (RPN) |
|-------|----------|------------------------|
| 0 | Water | PushPrimitive(source_sphere) |
| 1 | Iron | PushPrimitive(shield_box) PushPrimitive(source_sphere) Difference |

## Evaluated Material Properties
### Iron (Density: 7.870 g/cm^3)
**Composition:**
- Z=26 (100.00%)

| Energy (MeV) | $\mu$ (cm$^{-1}$) | Buildup Model |
|--------------|-------------------|---------------|
| 1.000 | 4.7181e-1 | GP(a=1.500e-1, b=1.500e0, c=6.000e-1, d=4.000e-2, xk=1.300e1) |

### Water (Density: 1.000 g/cm^3)
**Composition:**
- Z=1 (11.20%), Z=8 (88.80%)

| Energy (MeV) | $\mu$ (cm$^{-1}$) | Buildup Model |
|--------------|-------------------|---------------|
| 1.000 | 7.0729e-2 | GP(a=1.000e-1, b=2.000e0, c=5.000e-1, d=5.000e-2, xk=1.400e1) |

## Results
### Detector 1 at `[10.000, 0.000, 0.000]`
- **Buildup Material:** Iron (100.0%)
- **Total Dose Rate (Uncollided):** 1.476918e-1
- **Total Dose Rate (with Buildup):** 2.571147e-1

#### Energy Group Details (Aggregated)
| Group | Energy (MeV) | Segments | Uncollided Flux (Avg/Min/Max) | Buildup Factor (Avg/Min/Max) | Dose Rate (Unc.) (Avg/Min/Max) | Dose Rate (Total) (Avg/Min/Max) |
|-------|--------------|----------|-------------------------------|------------------------------|--------------------------------|---------------------------------|
| 0 | 0.000 | 1000000 | 1.48e-7 / 3.10e-13 / 1.40e-6 | 1.73e0 / 1.65e0 / 1.89e0 | 1.48e-7 / 3.10e-13 / 1.40e-6 | 2.57e-7 / 5.27e-13 / 2.32e-6 |

#### Ray Path Summary (Aggregated)
| Material | Proportion (%) | Phys. Thickness (Avg/Min/Max) | Opt. Thickness (Avg/Min/Max) |
|----------|----------------|-------------------------------|------------------------------|
| Water | 17.90% | 1.80e0 / 1.00e-2 / 3.99e0 | 0.00e0 / 0.00e0 / 0.00e0 |
| Iron | 32.10% | 3.23e0 / 3.00e0 / 4.50e0 | 0.00e0 / 0.00e0 / 0.00e0 |
| Vacuum | 50.00% | 5.03e0 / 5.00e0 / 5.10e0 | 0.00e0 / 0.00e0 / 0.00e0 |

### Detector 2 at `[20.000, 0.000, 0.000]`
- **Buildup Material:** Iron (100.0%)
- **Total Dose Rate (Uncollided):** 3.590936e-2
- **Total Dose Rate (with Buildup):** 6.274697e-2

#### Energy Group Details (Aggregated)
| Group | Energy (MeV) | Segments | Uncollided Flux (Avg/Min/Max) | Buildup Factor (Avg/Min/Max) | Dose Rate (Unc.) (Avg/Min/Max) | Dose Rate (Total) (Avg/Min/Max) |
|-------|--------------|----------|-------------------------------|------------------------------|--------------------------------|---------------------------------|
| 0 | 0.000 | 1000000 | 3.59e-8 / 7.76e-14 / 2.77e-7 | 1.73e0 / 1.65e0 / 1.91e0 | 3.59e-8 / 7.76e-14 / 2.77e-7 | 6.27e-8 / 1.32e-13 / 4.58e-7 |

#### Ray Path Summary (Aggregated)
| Material | Proportion (%) | Phys. Thickness (Avg/Min/Max) | Opt. Thickness (Avg/Min/Max) |
|----------|----------------|-------------------------------|------------------------------|
| Water | 8.72% | 1.75e0 / 1.00e-2 / 3.99e0 | 0.00e0 / 0.00e0 / 0.00e0 |
| Iron | 16.28% | 3.26e0 / 3.00e0 / 4.63e0 | 0.00e0 / 0.00e0 / 0.00e0 |
| Vacuum | 75.00% | 1.50e1 / 1.50e1 / 1.51e1 | 0.00e0 / 0.00e0 / 0.00e0 |

