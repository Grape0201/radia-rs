use glam::Vec3A;
use radia_core::config::WorldConfig;
use radia_core::csg::World;
use radia_core::primitive::Primitive;
use wasm_bindgen::prelude::*;

/// Point cloud for each cell
/// Passed to the JS side as an array of objects: { cell_index, material_id, positions: Float32Array }
#[wasm_bindgen]
pub struct PointCloudResult {
    // wasm_bindgen cannot pass Vec<struct> directly, so it is passed as a flat array
    // layout: [cell_index(f32), material_id(f32), x, y, z, x, y, z, ...]
    // Cell boundary whenever cell_index changes
}

#[derive(serde::Serialize)]
struct CellPoints {
    cell_index: usize,
    material_id: u32,
    /// flat [x,y,z, x,y,z, ...]
    positions: Vec<f32>,
}

#[derive(serde::Serialize)]
struct PointCloudOutput {
    cells: Vec<CellPoints>,
    total_pts: usize,
    aabb_min: [f32; 3],
    aabb_max: [f32; 3],
}

#[wasm_bindgen]
pub fn generate_point_cloud(world_json: &str, resolution: f32) -> Result<JsValue, JsValue> {
    let world_config: WorldConfig =
        serde_json::from_str(world_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let world = world_config
        .build()
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    let result = generate_surface_points(&world, resolution);

    serde_wasm_bindgen::to_value(&result).map_err(|e| JsValue::from_str(&e.to_string()))
}

fn generate_surface_points(world: &World, resolution: f32) -> PointCloudOutput {
    let aabb = world_aabb(world);
    let threshold = resolution * 1.5;

    let mut cells_out: Vec<CellPoints> = world
        .cells
        .iter()
        .enumerate()
        .map(|(i, c)| CellPoints {
            cell_index: i,
            material_id: c.material_id,
            positions: Vec::new(),
        })
        .collect();

    let mut total_pts = 0usize;

    // grid sampling
    let mut x = aabb.0[0];
    while x <= aabb.1[0] {
        let mut y = aabb.0[1];
        while y <= aabb.1[1] {
            let mut z = aabb.0[2];
            while z <= aabb.1[2] {
                let p = Vec3A::new(x, y, z);

                // evaluate sdf of all cells → assign to the nearest cell
                // ※ assume cells do not overlap (same assumption as kernel.rs)
                let mut best_cell: Option<usize> = None;
                let mut best_dist = threshold; // only consider within threshold

                for (ci, cell) in world.cells.iter().enumerate() {
                    let d = cell.csg.sdf(&p, &world.primitives).abs();
                    if d < best_dist {
                        best_dist = d;
                        best_cell = Some(ci);
                    }
                }

                if let Some(ci) = best_cell {
                    cells_out[ci].positions.extend_from_slice(&[x, y, z]);
                    total_pts += 1;
                }

                z += resolution;
            }
            y += resolution;
        }
        x += resolution;
    }

    PointCloudOutput {
        cells: cells_out,
        total_pts,
        aabb_min: aabb.0,
        aabb_max: aabb.1,
    }
}

/// Calculate AABB from primitives
fn world_aabb(world: &World) -> ([f32; 3], [f32; 3]) {
    let mut mn = [f32::INFINITY; 3];
    let mut mx = [f32::NEG_INFINITY; 3];

    let expand = |mn: &mut [f32; 3], mx: &mut [f32; 3], c: Vec3A, r: f32| {
        for i in 0..3 {
            mn[i] = mn[i].min(c[i] - r);
            mx[i] = mx[i].max(c[i] + r);
        }
    };

    for prim in &world.primitives {
        match prim {
            Primitive::Sphere { center, radius2 } => {
                expand(&mut mn, &mut mx, *center, radius2.sqrt() + 0.1);
            }
            Primitive::RectangularParallelPiped { min, max } => {
                for i in 0..3 {
                    mn[i] = mn[i].min(min[i]);
                    mx[i] = mx[i].max(max[i]);
                }
            }
            Primitive::FiniteCylinder {
                center,
                radius2,
                half_height,
                ..
            } => {
                let r = radius2.sqrt();
                expand(&mut mn, &mut mx, *center, r.max(*half_height) + 0.1);
            }
        }
    }

    // Fallback
    if !mn[0].is_finite() {
        return ([-2.0; 3], [2.0; 3]);
    }
    (mn, mx)
}
