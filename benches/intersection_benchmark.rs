use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use glam::Vec3A;
use radia_rs::primitive::{Ray, Primitive};

fn benchmark_intersection(c: &mut Criterion) {
    let mut group = c.benchmark_group("Intersection");

    let primitives = [
        Primitive::Sphere {
            center: Vec3A::ZERO,
            radius2: 1.0,
        },
        Primitive::RectangularPrallelPiped {
            min: Vec3A::new(-1.0, -1.0, -1.0),
            max: Vec3A::new(1.0, 1.0, 1.0),
        },
        Primitive::FiniteCylinder {
            center: Vec3A::ZERO,
            direction: Vec3A::Y,
            radius2: 1.0,
            half_height: 1.0,
        },
    ];

    let ray = Ray {
        origin: Vec3A::new(0.0, 0.0, -2.0),
        vector: Vec3A::new(0.0, 0.0, 4.0),
    };

    for primitive in &primitives {
        group.bench_with_input(BenchmarkId::from_parameter(primitive), primitive, |b, s| {
            b.iter(|| {
                s.get_intersections(&ray);
            });
        });
    }
    group.finish();
}

criterion_group!(benches, benchmark_intersection);
criterion_main!(benches);
