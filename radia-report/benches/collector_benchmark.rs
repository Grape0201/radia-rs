use criterion::{Criterion, black_box, criterion_group, criterion_main};
use glam::Vec3A;
use pprof::criterion::{Output, PProfProfiler};
use radia_core::buildup::GPBuildupProvider;
use radia_core::csg::{Cell, FlatCSG, Instruction, PrimitiveStorage, World};
use radia_core::kernel::{FastCollector, calculate_dose_rate};
use radia_core::mass_attenuation::{DummyProvider, MaterialDef, MaterialRegistry};
use radia_core::primitive::Primitive;
use radia_core::source::{PointSource, generate_sphere_source};
use std::collections::HashMap;

fn config_with_profiler() -> Criterion {
    Criterion::default().with_profiler(PProfProfiler::new(100, Output::Flamegraph(None)))
}

fn generate_test_environment() -> (
    World,
    radia_core::physics::MaterialPhysicsTable,
    Vec<PointSource>,
    Vec<f32>,
    Vec<f32>,
) {
    let mut water_composition = HashMap::new();
    water_composition.insert(1, 0.111);
    water_composition.insert(8, 0.889);
    let water = MaterialDef::new(water_composition, 1.0);

    let mut iron_composition = HashMap::new();
    iron_composition.insert(26, 1.0);
    let iron = MaterialDef::new(iron_composition, 7.874);

    let mut registry = MaterialRegistry::new();
    registry.insert("Water".to_string(), water);
    registry.insert("Iron".to_string(), iron);

    let material_names = Vec::from(["Water".to_string(), "Iron".to_string()]);
    let buildup_alias_map = HashMap::from([
        ("Water".to_string(), "DummyMaterial".to_string()),
        ("Iron".to_string(), "DummyMaterial".to_string()),
    ]);
    let energy_groups = vec![0.5, 1.0, 2.0, 4.0, 6.0, 8.0, 10.0];

    let mut gp_provider = GPBuildupProvider::new();
    let dummy_params = vec![
        radia_core::buildup::GPParams {
            energy_mev: 0.5,
            a: 0.1,
            b: 2.0,
            c: 0.5,
            d: 0.05,
            xk: 14.0,
        },
        radia_core::buildup::GPParams {
            energy_mev: 1.0,
            a: 0.12,
            b: 2.1,
            c: 0.53,
            d: 0.04,
            xk: 14.4,
        },
        radia_core::buildup::GPParams {
            energy_mev: 10.0,
            a: 0.2,
            b: 1.3,
            c: 0.9,
            d: 0.01,
            xk: 13.5,
        },
    ];
    gp_provider.insert_data("DummyMaterial".to_string(), dummy_params);

    let physics_table = radia_core::physics::MaterialPhysicsTable::generate(
        &material_names,
        &buildup_alias_map,
        &registry,
        &energy_groups,
        &DummyProvider,
        &gp_provider,
    )
    .expect("Failed to generate physics table");

    let mut world = World {
        primitives: PrimitiveStorage::new(),
        cells: vec![],
    };
    world.primitives.add(Primitive::Sphere {
        center: Vec3A::ZERO,
        radius2: 100.0,
    });
    world.primitives.add(Primitive::Sphere {
        center: Vec3A::ZERO,
        radius2: 2500.0,
    });
    world.cells.push(Cell {
        csg: FlatCSG {
            instructions: vec![Instruction::PushPrimitive(0)],
        },
        material_id: 1,
    });
    world.cells.push(Cell {
        material_id: 0,
        csg: FlatCSG {
            instructions: vec![
                Instruction::PushPrimitive(0),
                Instruction::PushPrimitive(1),
                Instruction::Difference,
            ],
        },
    });

    let sources = generate_sphere_source(Vec3A::ZERO, 9.0, 10, 10, 10, 1.0);
    let conversion_factors = vec![1.0; energy_groups.len()];
    let intensity_by_group = vec![1.0 / energy_groups.len() as f32; energy_groups.len()];

    (
        world,
        physics_table,
        sources,
        conversion_factors,
        intensity_by_group,
    )
}

fn benchmark_collector_comparison(c: &mut Criterion) {
    let (world, physics_table, sources, conversion_factors, intensity_by_group) =
        generate_test_environment();
    let detector_position = Vec3A::new(100.0, 0.0, 0.0);

    let mut group = c.benchmark_group("Dose_Collector_Comparisons");

    group.bench_function("1_fast_collector_abstraction", |b| {
        b.iter(|| {
            let mut collector = FastCollector::default();
            calculate_dose_rate(
                black_box(&physics_table),
                black_box(&world),
                black_box(&conversion_factors),
                black_box(&intensity_by_group),
                black_box(detector_position),
                black_box(&sources),
                &mut collector,
            )
        })
    });

    // add more collector cases

    group.finish();
}

criterion_group!(
    name = benches;
    config = config_with_profiler();
    targets = benchmark_collector_comparison
);
criterion_main!(benches);
