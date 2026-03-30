use crate::constants::EPSILON;
use glam::Vec3A;

#[derive(Debug, Clone, Default)]
pub struct SphereData {
    pub centers: Vec<Vec3A>,
    pub radius2s: Vec<f32>,
}

impl SphereData {
    pub fn push(&mut self, center: Vec3A, radius2: f32) {
        self.centers.push(center);
        self.radius2s.push(radius2);
    }

    /// Batched version of get_ranges to optimize for many rays.
    /// By inverting the loop (Outer: Primitives, Inner: Rays), primitive data like centers
    /// and radii stay in the CPU cache while processing multiple rays.
    pub fn get_ranges_batched(
        &self,
        rays: &[Ray],
        global_indices: &[usize],
        total_prims: usize,
        results: &mut [(f32, f32)],
    ) {
        for (local_idx, &global_idx) in global_indices.iter().enumerate() {
            let center = self.centers[local_idx];
            let radius2 = self.radius2s[local_idx];

            for (ray_idx, ray) in rays.iter().enumerate() {
                let oc = ray.origin - center;
                let a = ray.vector.length_squared();
                let b = oc.dot(ray.vector);
                let c = oc.length_squared() - radius2;
                let discriminant = b * b - a * c;

                if discriminant > 0.0 {
                    let sqrt_d = discriminant.sqrt();
                    results[ray_idx * total_prims + global_idx] =
                        ((-b - sqrt_d) / a, (-b + sqrt_d) / a);
                } else {
                    results[ray_idx * total_prims + global_idx] =
                        (f32::INFINITY, f32::NEG_INFINITY);
                }
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct RPPData {
    pub mins: Vec<Vec3A>,
    pub maxs: Vec<Vec3A>,
}

impl RPPData {
    pub fn push(&mut self, min: Vec3A, max: Vec3A) {
        self.mins.push(min);
        self.maxs.push(max);
    }

    pub fn get_ranges_batched(
        &self,
        rays: &[Ray],
        global_indices: &[usize],
        total_prims: usize,
        results: &mut [(f32, f32)],
    ) {
        for (local_idx, &global_idx) in global_indices.iter().enumerate() {
            let min = self.mins[local_idx];
            let max = self.maxs[local_idx];

            for (ray_idx, ray) in rays.iter().enumerate() {
                let inv_dir = 1.0 / ray.vector;
                let t0 = (min - ray.origin) * inv_dir;
                let t1 = (max - ray.origin) * inv_dir;
                let tmin_v = t0.min(t1);
                let tmax_v = t0.max(t1);
                let tmin = tmin_v.max_element();
                let tmax = tmax_v.min_element();
                if tmin <= tmax + EPSILON {
                    results[ray_idx * total_prims + global_idx] = (tmin, tmax);
                } else {
                    results[ray_idx * total_prims + global_idx] =
                        (f32::INFINITY, f32::NEG_INFINITY);
                }
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CylinderData {
    pub centers: Vec<Vec3A>,
    pub directions: Vec<Vec3A>,
    pub radius2s: Vec<f32>,
    pub half_heights: Vec<f32>,
}

impl CylinderData {
    pub fn push(&mut self, center: Vec3A, direction: Vec3A, radius2: f32, half_height: f32) {
        self.centers.push(center);
        self.directions.push(direction);
        self.radius2s.push(radius2);
        self.half_heights.push(half_height);
    }

    pub fn get_ranges_batched(
        &self,
        rays: &[Ray],
        global_indices: &[usize],
        total_prims: usize,
        results: &mut [(f32, f32)],
    ) {
        for (i, (&center, &direction)) in self.centers.iter().zip(&self.directions).enumerate() {
            let radius2 = self.radius2s[i];
            let half_height = self.half_heights[i];
            let global_idx = global_indices[i];

            if half_height <= EPSILON {
                for ray_idx in 0..rays.len() {
                    results[ray_idx * total_prims + global_idx] =
                        (f32::INFINITY, f32::NEG_INFINITY);
                }
                continue;
            }
            let axis = direction;

            for (ray_idx, ray) in rays.iter().enumerate() {
                let v = ray.vector;
                let w = ray.origin - center;

                let v_cross_axis = v.cross(axis);
                let w_cross_axis = w.cross(axis);

                let a = v_cross_axis.length_squared();
                let b = w_cross_axis.dot(v_cross_axis);
                let c = w_cross_axis.length_squared() - radius2;

                let mut t_min = f32::INFINITY;
                let mut t_max = f32::NEG_INFINITY;

                if a.abs() > EPSILON {
                    let discriminant = b * b - a * c;
                    if discriminant > 0.0 {
                        let sqrt_d = discriminant.sqrt();
                        for &t in &[(-b - sqrt_d) / a, (-b + sqrt_d) / a] {
                            let point = ray.origin + v * t;
                            let axial = (point - center).dot(axis);
                            if axial.abs() <= half_height + EPSILON {
                                if t < t_min {
                                    t_min = t;
                                }
                                if t > t_max {
                                    t_max = t;
                                }
                            }
                        }
                    }
                }

                let axis_dot_dir = v.dot(axis);
                if axis_dot_dir.abs() > EPSILON {
                    for &sign in &[1.0f32, -1.0f32] {
                        let cap_center = center + axis * (sign * half_height);
                        let t = (cap_center - ray.origin).dot(axis) / axis_dot_dir;
                        let point = ray.origin + v * t;
                        if (point - cap_center).length_squared() <= radius2 + EPSILON {
                            if t < t_min {
                                t_min = t;
                            }
                            if t > t_max {
                                t_max = t;
                            }
                        }
                    }
                }

                if t_min <= t_max {
                    results[ray_idx * total_prims + global_idx] = (t_min, t_max);
                } else {
                    results[ray_idx * total_prims + global_idx] =
                        (f32::INFINITY, f32::NEG_INFINITY);
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum PrimitiveTag {
    Sphere(usize),
    RPP(usize),
    Cylinder(usize),
}

/// Only convex primitives are supported.
#[derive(Debug, Clone)]
pub enum Primitive {
    Sphere {
        center: Vec3A,
        /// radius^2
        radius2: f32,
    },
    RectangularParallelPiped {
        min: Vec3A,
        max: Vec3A,
    },
    FiniteCylinder {
        center: Vec3A,
        /// unit vector
        direction: Vec3A,
        /// radius^2
        radius2: f32,
        half_height: f32,
    },
}

impl std::fmt::Display for Primitive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Primitive::Sphere { center, radius2 } => {
                write!(f, "Sphere {{ center: {:?}, radius2: {} }}", center, radius2)
            }
            Primitive::RectangularParallelPiped { min, max } => write!(
                f,
                "RectangularParallelPiped {{ min: {:?}, max: {:?} }}",
                min, max
            ),
            Primitive::FiniteCylinder {
                center,
                direction,
                radius2,
                half_height,
            } => write!(
                f,
                "FiniteCylinder {{ center: {:?}, direction: {:?}, radius2: {}, half_height: {} }}",
                center, direction, radius2, half_height
            ),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Ray {
    pub origin: Vec3A,
    /// from origin to the detector point
    pub vector: Vec3A,
}
