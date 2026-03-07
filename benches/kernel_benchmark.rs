use criterion::{Criterion, criterion_group, criterion_main};
use radia_rs::buildup::{GPBuildupProvider, TargetQuantity};
use radia_rs::csg::{CSGNode, Cell, World};
use radia_rs::kernel::calculate_dose_rate_spectrum;
use radia_rs::material::{DummyProvider, MaterialDef, MuTable};
use radia_rs::shape::{Shape, Vec3};
use radia_rs::source::PointSource;
use std::collections::HashMap;
use std::hint::black_box;

fn generate_test_environment() -> (
    World,
    MuTable,
    radia_rs::buildup::BuildupTable,
    Vec<PointSource>,
    Vec<f64>,
) {
    // 1. Setup Materials (Water and Iron)
    let mut water_densities = HashMap::new();
    water_densities.insert(1, 0.111); // Hydrogen
    water_densities.insert(8, 0.889); // Oxygen
    let water = MaterialDef::new(water_densities);

    let mut iron_densities = HashMap::new();
    iron_densities.insert(26, 7.874); // Iron
    let iron = MaterialDef::new(iron_densities);

    let materials = vec![water, iron];
    let energy_groups = vec![0.5, 1.0, 2.0, 4.0, 6.0, 8.0, 10.0];

    // Using DummyProvider for MuTable, normally we'd have real cross-sections
    let provider = DummyProvider;
    let mu_table = MuTable::generate(&materials, &energy_groups, &provider);

    // 2. Setup Buildup
    let mut gp_provider = GPBuildupProvider::new();
    let dummy_params = vec![
        radia_rs::buildup::GPParams {
            energy_mev: 0.5,
            a: 0.1,
            b: 2.0,
            c: 0.5,
            d: 0.05,
            xk: 14.0,
        },
        radia_rs::buildup::GPParams {
            energy_mev: 1.0,
            a: 0.12,
            b: 2.1,
            c: 0.53,
            d: 0.04,
            xk: 14.4,
        },
        radia_rs::buildup::GPParams {
            energy_mev: 10.0,
            a: 0.2,
            b: 1.3,
            c: 0.9,
            d: 0.01,
            xk: 13.5,
        },
    ];
    gp_provider.insert_data(
        "DummyMaterial".to_string(),
        TargetQuantity::AmbientDoseEquivalent,
        dummy_params,
    );

    let buildup_table = gp_provider.generate_table(
        &["DummyMaterial", "DummyMaterial"],
        TargetQuantity::AmbientDoseEquivalent,
        &energy_groups,
    );

    // 3. Setup Geometry (Nested Spheres: Inner Iron core, Outer Water shell)
    let mut world = World {
        shapes: HashMap::new(),
        cells: vec![],
    };

    let inner_sphere = Shape::Sphere {
        center: Vec3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        radius: 10.0,
    };
    let outer_sphere = Shape::Sphere {
        center: Vec3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        radius: 50.0,
    };

    world.shapes.insert(0, inner_sphere);
    world.shapes.insert(1, outer_sphere);

    // Cell 0: Iron core (material index 1)
    world.cells.push(Cell {
        csg: CSGNode::Primitive(0),
        material_id: 1,
    });
    // Cell 1: Water shell (material index 0)
    world.cells.push(Cell {
        material_id: 0,
        csg: CSGNode::Intersection(
            Box::new(CSGNode::Primitive(1)), // inside outer sphere
            Box::new(CSGNode::Difference(
                // outside inner sphere
                Box::new(CSGNode::Primitive(1)),
                Box::new(CSGNode::Primitive(0)),
            )),
        ),
    });

    // 4. Setup Sources (e.g. 1000 points arranged in a grid inside the core)
    let mut sources = Vec::new();
    let grid_size = 10;
    for x in 0..grid_size {
        for y in 0..grid_size {
            for z in 0..grid_size {
                let px = (x as f64 - 4.5) * 1.5;
                let py = (y as f64 - 4.5) * 1.5;
                let pz = (z as f64 - 4.5) * 1.5;

                // Only place if it's strictly inside the inner sphere
                if px * px + py * py + pz * pz < 9.0 * 9.0 {
                    sources.push(PointSource {
                        position: (px, py, pz),
                        intensity: 1.0,
                    });
                }
            }
        }
    }

    // 5. Setup Conversion Factors
    let conversion_factors = vec![1.0; energy_groups.len()];

    (world, mu_table, buildup_table, sources, conversion_factors)
}

fn criterion_benchmark(c: &mut Criterion) {
    let (world, mu_table, buildup_table, sources, conversion_factors) = generate_test_environment();
    let detector_position = (100.0, 0.0, 0.0);

    // We bind the closures outside of the loop to measure inner calculation speed
    let get_mu = mu_table.into_closure();
    let get_buildup = buildup_table.into_closure();

    // Wrap buildup to match Fn(usize, f64) -> f64 expected by kernel.
    // We choose material index 0 (Water) for the buildup factor in this benchmark.
    let get_buildup_wrapped = |grp_idx, ot| get_buildup(0, grp_idx, ot);

    c.bench_function("calculate_dose_rate_spectrum", |b| {
        b.iter(|| {
            calculate_dose_rate_spectrum(
                black_box(&get_mu),
                black_box(&get_buildup_wrapped),
                black_box(&world),
                black_box(&conversion_factors),
                black_box(detector_position),
                black_box(&sources),
            )
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
