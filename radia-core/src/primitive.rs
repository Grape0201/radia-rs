use crate::constants::EPSILON;
use glam::{Vec3A, Vec4};

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

    pub fn get_intersections(&self, ray: &Ray, results: &mut Vec<f32>) {
        let a = ray.vector.length_squared();
        let inv_a = 1.0 / a;
        let origin = ray.origin;
        let vector = ray.vector;

        let mut i = 0;
        // SIMD loop for 4 spheres at once
        while i + 4 <= self.centers.len() {
            let c0 = self.centers[i];
            let c1 = self.centers[i+1];
            let c2 = self.centers[i+2];
            let c3 = self.centers[i+3];
            
            let cx = Vec4::new(c0.x, c1.x, c2.x, c3.x);
            let cy = Vec4::new(c0.y, c1.y, c2.y, c3.y);
            let cz = Vec4::new(c0.z, c1.z, c2.z, c3.z);
            let r2 = Vec4::new(self.radius2s[i], self.radius2s[i+1], self.radius2s[i+2], self.radius2s[i+3]);
            
            let ocx = origin.x - cx;
            let ocy = origin.y - cy;
            let ocz = origin.z - cz;
            
            let b = ocx * vector.x + ocy * vector.y + ocz * vector.z;
            let c = ocx*ocx + ocy*ocy + ocz*ocz - r2;
            let discriminant = b*b - a*c;
            
            let mask = discriminant.cmpgt(Vec4::splat(EPSILON));
            if mask.any() {
                let sqrt_d = discriminant.abs().sqrt();
                let t0 = (-b - sqrt_d) * inv_a;
                let t1 = (-b + sqrt_d) * inv_a;
                
                let m = mask.bitmask();
                for j in 0..4 {
                    if (m & (1 << j)) != 0 {
                        let val0 = t0.to_array()[j];
                        let val1 = t1.to_array()[j];
                        if val0 > SEGMENT_MIN && val0 < SEGMENT_MAX { results.push(val0); }
                        if val1 > SEGMENT_MIN && val1 < SEGMENT_MAX { results.push(val1); }
                    }
                }
            }
            i += 4;
        }

        // Remainder loop
        for (center, radius2) in self.centers.iter().zip(&self.radius2s).skip(i) {
            let oc = ray.origin - *center;
            let a = ray.vector.length_squared();
            let b = oc.dot(ray.vector);
            let c = oc.length_squared() - radius2;
            let discriminant = b * b - a * c;
            if discriminant > EPSILON {
                let sqrt_d = discriminant.sqrt();
                let t0 = (-b - sqrt_d) / a;
                let t1 = (-b + sqrt_d) / a;
                if t0 > SEGMENT_MIN && t0 < SEGMENT_MAX { results.push(t0); }
                if t1 > SEGMENT_MIN && t1 < SEGMENT_MAX { results.push(t1); }
            } else if discriminant.abs() <= EPSILON {
                let t = -b / a;
                if t > SEGMENT_MIN && t < SEGMENT_MAX { results.push(t); }
            }
        }
    }

    pub fn get_ranges(&self, ray: &Ray, global_indices: &[usize], results: &mut [(f32, f32)]) {
        let a = ray.vector.length_squared();
        let inv_a = 1.0 / a;
        let origin = ray.origin;
        let vector = ray.vector;

        let mut i = 0;
        // SIMD loop for 4 spheres at once
        while i + 4 <= self.centers.len() {
            let c0 = self.centers[i];
            let c1 = self.centers[i+1];
            let c2 = self.centers[i+2];
            let c3 = self.centers[i+3];
            
            let cx = Vec4::new(c0.x, c1.x, c2.x, c3.x);
            let cy = Vec4::new(c0.y, c1.y, c2.y, c3.y);
            let cz = Vec4::new(c0.z, c1.z, c2.z, c3.z);
            let r2 = Vec4::new(self.radius2s[i], self.radius2s[i+1], self.radius2s[i+2], self.radius2s[i+3]);
            
            let ocx = origin.x - cx;
            let ocy = origin.y - cy;
            let ocz = origin.z - cz;
            
            let b = ocx * vector.x + ocy * vector.y + ocz * vector.z;
            let c = ocx*ocx + ocy*ocy + ocz*ocz - r2;
            let discriminant = b*b - a*c;
            
            let mask = discriminant.cmpgt(Vec4::ZERO);
            let m = mask.bitmask();
            let sqrt_d = discriminant.abs().sqrt();
            let t0 = (-b - sqrt_d) * inv_a;
            let t1 = (-b + sqrt_d) * inv_a;

            for j in 0..4 {
                let global_idx = global_indices[i+j];
                if (m & (1 << j)) != 0 {
                    results[global_idx] = (t0.to_array()[j], t1.to_array()[j]);
                } else {
                    results[global_idx] = (f32::INFINITY, f32::NEG_INFINITY);
                }
            }
            i += 4;
        }

        // Remainder loop
        for j in i..self.centers.len() {
            let oc = origin - self.centers[j];
            let b = oc.dot(vector);
            let c = oc.length_squared() - self.radius2s[j];
            let discriminant = b * b - a * c;
            let global_idx = global_indices[j];
            if discriminant > 0.0 {
                let sqrt_d = discriminant.sqrt();
                results[global_idx] = ((-b - sqrt_d) * inv_a, (-b + sqrt_d) * inv_a);
            } else {
                results[global_idx] = (f32::INFINITY, f32::NEG_INFINITY);
            }
        }
    }

    /// Batched version of get_ranges to optimize for many rays.
    /// By inverting the loop (Outer: Primitives, Inner: Rays), primitive data like centers
    /// and radii stay in the CPU cache while processing multiple rays.
    /// This also enables SIMD vectorization over rays.
    pub fn get_ranges_batched(&self, rays: &[Ray], global_indices: &[usize], total_prims: usize, results: &mut [(f32, f32)]) {
        for (local_idx, &global_idx) in global_indices.iter().enumerate() {
            let center = self.centers[local_idx];
            let radius2 = self.radius2s[local_idx];

            let cx = Vec4::splat(center.x);
            let cy = Vec4::splat(center.y);
            let cz = Vec4::splat(center.z);
            let r2 = Vec4::splat(radius2);

            let mut i = 0;
            // SIMD loop: Vectorize intersection tests for 4 rays against 1 sphere.
            while i + 4 <= rays.len() {
                let r0 = &rays[i];
                let r1 = &rays[i+1];
                let r2_ray = &rays[i+2];
                let r3 = &rays[i+3];

                let ox = Vec4::new(r0.origin.x, r1.origin.x, r2_ray.origin.x, r3.origin.x);
                let oy = Vec4::new(r0.origin.y, r1.origin.y, r2_ray.origin.y, r3.origin.y);
                let oz = Vec4::new(r0.origin.z, r1.origin.z, r2_ray.origin.z, r3.origin.z);

                let vx = Vec4::new(r0.vector.x, r1.vector.x, r2_ray.vector.x, r3.vector.x);
                let vy = Vec4::new(r0.vector.y, r1.vector.y, r2_ray.vector.y, r3.vector.y);
                let vz = Vec4::new(r0.vector.z, r1.vector.z, r2_ray.vector.z, r3.vector.z);

                let a = vx*vx + vy*vy + vz*vz;
                let inv_a = 1.0 / a;
                let ocx = ox - cx;
                let ocy = oy - cy;
                let ocz = oz - cz;

                let b = ocx*vx + ocy*vy + ocz*vz;
                let c = ocx*ocx + ocy*ocy + ocz*ocz - r2;
                let discriminant = b*b - a*c;
                
                let mask = discriminant.cmpgt(Vec4::ZERO);
                let m = mask.bitmask();
                let sqrt_d = discriminant.abs().sqrt();
                let t0 = (-b - sqrt_d) * inv_a;
                let t1 = (-b + sqrt_d) * inv_a;

                let t0_arr = t0.to_array();
                let t1_arr = t1.to_array();

                for j in 0..4 {
                    if (m & (1 << j)) != 0 {
                        results[(i+j) * total_prims + global_idx] = (t0_arr[j], t1_arr[j]);
                    } else {
                        results[(i+j) * total_prims + global_idx] = (f32::INFINITY, f32::NEG_INFINITY);
                    }
                }
                i += 4;
            }

            for j in i..rays.len() {
                let ray = &rays[j];
                let oc = ray.origin - center;
                let a = ray.vector.length_squared();
                let b = oc.dot(ray.vector);
                let c = oc.length_squared() - radius2;
                let discriminant = b * b - a * c;
                if discriminant > 0.0 {
                    let sqrt_d = discriminant.sqrt();
                    results[j * total_prims + global_idx] = ((-b - sqrt_d) / a, (-b + sqrt_d) / a);
                } else {
                    results[j * total_prims + global_idx] = (f32::INFINITY, f32::NEG_INFINITY);
                }
            }
        }
    }

    #[inline(always)]
    pub fn contains(&self, index: usize, p: &Vec3A) -> bool {
        let center = self.centers[index];
        let radius2 = self.radius2s[index];
        let dist_sq = (*p - center).length_squared();
        dist_sq <= radius2 + EPSILON
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

    pub fn get_intersections(&self, ray: &Ray, results: &mut Vec<f32>) {
        let inv_dir = 1.0 / ray.vector;
        let origin = ray.origin;

        let mut i = 0;
        while i + 4 <= self.mins.len() {
            let min0 = self.mins[i];
            let min1 = self.mins[i+1];
            let min2 = self.mins[i+2];
            let min3 = self.mins[i+3];
            let max0 = self.maxs[i];
            let max1 = self.maxs[i+1];
            let max2 = self.maxs[i+2];
            let max3 = self.maxs[i+3];

            let t0_0 = (min0 - origin) * inv_dir;
            let t0_1 = (min1 - origin) * inv_dir;
            let t0_2 = (min2 - origin) * inv_dir;
            let t0_3 = (min3 - origin) * inv_dir;
            let t1_0 = (max0 - origin) * inv_dir;
            let t1_1 = (max1 - origin) * inv_dir;
            let t1_2 = (max2 - origin) * inv_dir;
            let t1_3 = (max3 - origin) * inv_dir;

            let tmin_0 = t0_0.min(t1_0);
            let tmin_1 = t0_1.min(t1_1);
            let tmin_2 = t0_2.min(t1_2);
            let tmin_3 = t0_3.min(t1_3);
            let tmax_0 = t0_0.max(t1_0);
            let tmax_1 = t0_1.max(t1_1);
            let tmax_2 = t0_2.max(t1_2);
            let tmax_3 = t0_3.max(t1_3);

            let tmin = Vec4::new(tmin_0.max_element(), tmin_1.max_element(), tmin_2.max_element(), tmin_3.max_element());
            let tmax = Vec4::new(tmax_0.min_element(), tmax_1.min_element(), tmax_2.min_element(), tmax_3.min_element());

            let mask = tmin.cmple(tmax + Vec4::splat(EPSILON));
            let m = mask.bitmask();
            for j in 0..4 {
                if (m & (1 << j)) != 0 {
                    let val0 = tmin.to_array()[j];
                    let val1 = tmax.to_array()[j];
                    if val0 > SEGMENT_MIN && val0 < SEGMENT_MAX { results.push(val0); }
                    if val1 > SEGMENT_MIN && val1 < SEGMENT_MAX { results.push(val1); }
                }
            }
            i += 4;
        }

        // Remainder loop
        for (min, max) in self.mins.iter().zip(&self.maxs).skip(i) {
            let t0 = (*min - ray.origin) * inv_dir;
            let t1 = (*max - ray.origin) * inv_dir;
            let tmin_v = t0.min(t1);
            let tmax_v = t0.max(t1);
            let tmin = tmin_v.max_element();
            let tmax = tmax_v.min_element();
            if tmin <= tmax + EPSILON {
                if tmin > SEGMENT_MIN && tmin < SEGMENT_MAX { results.push(tmin); }
                if tmax > SEGMENT_MIN && tmax < SEGMENT_MAX { results.push(tmax); }
            }
        }
    }

    pub fn get_ranges(&self, ray: &Ray, global_indices: &[usize], results: &mut [(f32, f32)]) {
        let inv_dir = 1.0 / ray.vector;
        let origin = ray.origin;

        let mut i = 0;
        while i + 4 <= self.mins.len() {
            let t0_0 = (self.mins[i] - origin) * inv_dir;
            let t0_1 = (self.mins[i+1] - origin) * inv_dir;
            let t0_2 = (self.mins[i+2] - origin) * inv_dir;
            let t0_3 = (self.mins[i+3] - origin) * inv_dir;
            let t1_0 = (self.maxs[i] - origin) * inv_dir;
            let t1_1 = (self.maxs[i+1] - origin) * inv_dir;
            let t1_2 = (self.maxs[i+2] - origin) * inv_dir;
            let t1_3 = (self.maxs[i+3] - origin) * inv_dir;

            let tmin_0 = t0_0.min(t1_0);
            let tmin_1 = t0_1.min(t1_1);
            let tmin_2 = t0_2.min(t1_2);
            let tmin_3 = t0_3.min(t1_3);
            let tmax_0 = t0_0.max(t1_0);
            let tmax_1 = t0_1.max(t1_1);
            let tmax_2 = t0_2.max(t1_2);
            let tmax_3 = t0_3.max(t1_3);

            let tmin = glam::Vec4::new(tmin_0.max_element(), tmin_1.max_element(), tmin_2.max_element(), tmin_3.max_element());
            let tmax = glam::Vec4::new(tmax_0.min_element(), tmax_1.min_element(), tmax_2.min_element(), tmax_3.min_element());

            let mask = tmin.cmple(tmax + glam::Vec4::splat(EPSILON));
            let m = mask.bitmask();
            for j in 0..4 {
                let global_idx = global_indices[i+j];
                if (m & (1 << j)) != 0 {
                    results[global_idx] = (tmin.to_array()[j], tmax.to_array()[j]);
                } else {
                    results[global_idx] = (f32::INFINITY, f32::NEG_INFINITY);
                }
            }
            i += 4;
        }

        for j in i..self.mins.len() {
            let t0 = (self.mins[j] - origin) * inv_dir;
            let t1 = (self.maxs[j] - origin) * inv_dir;
            let tmin_v = t0.min(t1);
            let tmax_v = t0.max(t1);
            let tmin = tmin_v.max_element();
            let tmax = tmax_v.min_element();
            let global_idx = global_indices[j];
            if tmin <= tmax + EPSILON {
                results[global_idx] = (tmin, tmax);
            } else {
                results[global_idx] = (f32::INFINITY, f32::NEG_INFINITY);
            }
        }
    }

    pub fn get_ranges_batched(&self, rays: &[Ray], global_indices: &[usize], total_prims: usize, results: &mut [(f32, f32)]) {
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
                    results[ray_idx * total_prims + global_idx] = (f32::INFINITY, f32::NEG_INFINITY);
                }
            }
        }
    }

    #[inline(always)]
    pub fn contains(&self, index: usize, p: &Vec3A) -> bool {
        let min = self.mins[index];
        let max = self.maxs[index];
        p.cmpge(min - EPSILON).all() && p.cmple(max + EPSILON).all()
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

    pub fn get_ranges(&self, ray: &Ray, global_indices: &[usize], results: &mut [(f32, f32)]) {
        for (i, (&center, &direction)) in self.centers.iter().zip(&self.directions).enumerate() {
            let radius2 = self.radius2s[i];
            let half_height = self.half_heights[i];
            let global_idx = global_indices[i];

            if half_height <= EPSILON {
                results[global_idx] = (f32::INFINITY, f32::NEG_INFINITY);
                continue;
            }
            let axis = direction;
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
                            if t < t_min { t_min = t; }
                            if t > t_max { t_max = t; }
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
                        if t < t_min { t_min = t; }
                        if t > t_max { t_max = t; }
                    }
                }
            }

            if t_min <= t_max {
                results[global_idx] = (t_min, t_max);
            } else {
                results[global_idx] = (f32::INFINITY, f32::NEG_INFINITY);
            }
        }
    }

    pub fn get_ranges_batched(&self, rays: &[Ray], global_indices: &[usize], total_prims: usize, results: &mut [(f32, f32)]) {
        for (i, (&center, &direction)) in self.centers.iter().zip(&self.directions).enumerate() {
            let radius2 = self.radius2s[i];
            let half_height = self.half_heights[i];
            let global_idx = global_indices[i];

            if half_height <= EPSILON {
                for ray_idx in 0..rays.len() {
                    results[ray_idx * total_prims + global_idx] = (f32::INFINITY, f32::NEG_INFINITY);
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
                                if t < t_min { t_min = t; }
                                if t > t_max { t_max = t; }
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
                            if t < t_min { t_min = t; }
                            if t > t_max { t_max = t; }
                        }
                    }
                }

                if t_min <= t_max {
                    results[ray_idx * total_prims + global_idx] = (t_min, t_max);
                } else {
                    results[ray_idx * total_prims + global_idx] = (f32::INFINITY, f32::NEG_INFINITY);
                }
            }
        }
    }

    pub fn get_intersections(&self, ray: &Ray, results: &mut Vec<f32>) {
        for i in 0..self.centers.len() {
            let center = self.centers[i];
            let direction = self.directions[i];
            let radius2 = self.radius2s[i];
            let half_height = self.half_heights[i];

            if half_height <= EPSILON { continue; }
            let axis = direction;
            let v = ray.vector;
            let w = ray.origin - center;

            let v_cross_axis = v.cross(axis);
            let w_cross_axis = w.cross(axis);

            let a = v_cross_axis.length_squared();
            let b = w_cross_axis.dot(v_cross_axis);
            let c = w_cross_axis.length_squared() - radius2;

            if a.abs() > EPSILON {
                let discriminant = b * b - a * c;
                if discriminant > EPSILON {
                    let sqrt_d = discriminant.sqrt();
                    for &t in &[(-b - sqrt_d) / a, (-b + sqrt_d) / a] {
                        let point = ray.origin + ray.vector * t;
                        let axial = (point - center).dot(axis);
                        if axial.abs() <= half_height + EPSILON {
                            if t > SEGMENT_MIN && t < SEGMENT_MAX { results.push(t); }
                        }
                    }
                } else if discriminant.abs() <= EPSILON {
                    let t = -b / a;
                    let point = ray.origin + ray.vector * t;
                    let axial = (point - center).dot(axis);
                    if axial.abs() <= half_height + EPSILON {
                        if t > SEGMENT_MIN && t < SEGMENT_MAX { results.push(t); }
                    }
                }
            }

            let axis_dot_dir = ray.vector.dot(axis);
            if axis_dot_dir.abs() > EPSILON {
                for &sign in &[1.0f32, -1.0f32] {
                    let cap_center = center + axis * (sign * half_height);
                    let t = (cap_center - ray.origin).dot(axis) / axis_dot_dir;
                    let point = ray.origin + ray.vector * t;
                    let radial = point - cap_center;
                    let radial_proj = radial.dot(axis);
                    let radial_vec = radial - axis * radial_proj;
                    if radial_vec.length_squared() <= radius2 + EPSILON {
                        if t > SEGMENT_MIN && t < SEGMENT_MAX { results.push(t); }
                    }
                }
            }
        }
    }

    #[inline(always)]
    pub fn contains(&self, index: usize, p: &Vec3A) -> bool {
        let center = self.centers[index];
        let direction = self.directions[index];
        let radius2 = self.radius2s[index];
        let half_height = self.half_heights[index];

        if half_height <= EPSILON { return false; }
        let axis = direction;
        let w = *p - center;
        let axial = w.dot(axis);
        if axial.abs() > half_height + EPSILON { return false; }
        let radial = w - axis * axial;
        radial.length_squared() <= radius2 + EPSILON
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

pub const SEGMENT_MIN: f32 = EPSILON;
pub const SEGMENT_MAX: f32 = 1.0 - EPSILON;

#[doc(hidden)]
#[derive(Debug, Clone)]
pub struct IntersectionTs {
    pub(crate) count: usize,
    /// 0 < ts[0],ts[1] < 1
    pub(crate) ts: [f32; 2],
}

impl IntersectionTs {
    fn empty() -> Self {
        Self {
            count: 0,
            ts: [0.0, 0.0],
        }
    }

    fn push_candidate(&mut self, value: f32) {
        if value <= SEGMENT_MIN || value >= SEGMENT_MAX {
            return;
        }

        match self.count {
            0 => {
                self.ts[0] = value;
                self.count = 1;
            }
            1 => {
                self.ts[1] = value;
                self.count = 2;
            }
            _ => unreachable!(),
        }
    }
}

impl Primitive {
    #[inline(always)]
    #[doc(hidden)]
    pub fn get_intersections(&self, ray: &Ray) -> IntersectionTs {
        let mut hits = IntersectionTs::empty();
        match self {
            Primitive::Sphere { center, radius2 } => {
                let oc = ray.origin - *center;
                let a = ray.vector.length_squared();
                let b = oc.dot(ray.vector);
                let c = oc.length_squared() - radius2;
                let discriminant = b * b - a * c;
                if discriminant > EPSILON {
                    let sqrt_d = discriminant.sqrt();
                    hits.push_candidate((-b - sqrt_d) / a);
                    hits.push_candidate((-b + sqrt_d) / a);
                } else if discriminant.abs() <= EPSILON {
                    hits.push_candidate(-b / a);
                }
                hits
            }
            Primitive::RectangularParallelPiped { min, max } => {
                let inv_dir = 1.0 / ray.vector;
                let t0 = (*min - ray.origin) * inv_dir;
                let t1 = (*max - ray.origin) * inv_dir;

                let tmin_v = t0.min(t1);
                let tmax_v = t0.max(t1);

                let tmin = tmin_v.max_element();
                let tmax = tmax_v.min_element();

                if tmin > tmax + EPSILON {
                    hits
                } else {
                    hits.push_candidate(tmin);
                    hits.push_candidate(tmax);
                    hits
                }
            }
            Primitive::FiniteCylinder {
                center,
                direction,
                radius2,
                half_height,
            } => {
                if *half_height <= EPSILON {
                    return hits;
                }
                let axis = *direction;

                let v = ray.vector;
                let w = ray.origin - *center;

                let v_cross_axis = v.cross(axis);
                let w_cross_axis = w.cross(axis);

                let a = v_cross_axis.length_squared();
                let b = w_cross_axis.dot(v_cross_axis);
                let c = w_cross_axis.length_squared() - *radius2;

                if a.abs() > EPSILON {
                    let discriminant = b * b - a * c;
                    if discriminant > EPSILON {
                        let sqrt_d = discriminant.sqrt();
                        for &t in &[(-b - sqrt_d) / a, (-b + sqrt_d) / a] {
                            let point = ray.origin + ray.vector * t;
                            let axial = (point - *center).dot(axis);
                            if axial.abs() <= *half_height + EPSILON {
                                hits.push_candidate(t);
                            }
                        }
                    } else if discriminant.abs() <= EPSILON {
                        let t = -b / a;
                        let point = ray.origin + ray.vector * t;
                        let axial = (point - *center).dot(axis);
                        if axial.abs() <= *half_height + EPSILON {
                            hits.push_candidate(t);
                        }
                    }
                }

                let axis_dot_dir = ray.vector.dot(axis);
                if axis_dot_dir.abs() > EPSILON {
                    for &sign in &[1.0f32, -1.0f32] {
                        let cap_center = *center + axis * (sign * *half_height);
                        let t = (cap_center - ray.origin).dot(axis) / axis_dot_dir;
                        let point = ray.origin + ray.vector * t;
                        let radial = point - cap_center;
                        let radial_proj = radial.dot(axis);
                        let radial_vec = radial - axis * radial_proj;
                        if radial_vec.length_squared() <= *radius2 + EPSILON {
                            hits.push_candidate(t);
                        }
                    }
                }

                hits
            }
        }
    }

    #[inline(always)]
    pub fn sdf(&self, p: &Vec3A) -> f32 {
        match self {
            Primitive::Sphere { center, radius2 } => {
                let radius = radius2.sqrt();
                (*p - *center).length() - radius
            }
            Primitive::RectangularParallelPiped { min, max } => {
                let center = (*min + *max) * 0.5;
                let extents = (*max - *min) * 0.5;
                let q = (*p - center).abs() - extents;
                let outside = q.max(Vec3A::ZERO).length();
                let inside = q.max_element().min(0.0);
                outside + inside
            }
            Primitive::FiniteCylinder {
                center,
                direction,
                radius2,
                half_height,
            } => {
                if *half_height <= EPSILON {
                    return f32::INFINITY;
                }
                let axis = *direction;
                let w = *p - *center;
                let axial = w.dot(axis);
                let radial = (w - axis * axial).length();
                let radius = radius2.sqrt();
                let dx = radial - radius;
                let dy = axial.abs() - *half_height;
                let outside = (dx.max(0.0).powi(2) + dy.max(0.0).powi(2)).sqrt();
                let inside = dx.max(dy).min(0.0);
                outside + inside
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sphere_intersections() {
        let sphere = Primitive::Sphere {
            center: Vec3A::ZERO,
            radius2: 1.0,
        };
        let ray = Ray {
            origin: Vec3A::new(0.0, 0.0, -2.0),
            vector: Vec3A::new(0.0, 0.0, 4.0),
        };
        let ts = sphere.get_intersections(&ray);
        assert_eq!(ts.count, 2);
        assert!((ts.ts[0] - 0.25).abs() < EPSILON);
        assert!((ts.ts[1] - 0.75).abs() < EPSILON);
    }

    #[test]
    fn test_sphere_tangent() {
        let sphere = Primitive::Sphere {
            center: Vec3A::ZERO,
            radius2: 1.0,
        };
        let ray = Ray {
            origin: Vec3A::new(1.0, 0.0, -2.0),
            vector: Vec3A::new(0.0, 0.0, 4.0),
        };
        let ts = sphere.get_intersections(&ray);
        assert_eq!(ts.count, 1);
        assert!((ts.ts[0] - 0.5).abs() < EPSILON);
    }

    #[test]
    fn test_rect_intersections() {
        let rect = Primitive::RectangularParallelPiped {
            min: Vec3A::new(-1.0, -1.0, -1.0),
            max: Vec3A::new(1.0, 1.0, 1.0),
        };
        let ray = Ray {
            origin: Vec3A::new(0.0, 0.0, -2.0),
            vector: Vec3A::new(0.0, 0.0, 4.0),
        };
        let ts = rect.get_intersections(&ray);
        assert_eq!(ts.count, 2);
        assert!((ts.ts[0] - 0.25).abs() < EPSILON);
        assert!((ts.ts[1] - 0.75).abs() < EPSILON);
    }

    #[test]
    fn test_cylinder_intersections() {
        let cylinder = Primitive::FiniteCylinder {
            center: Vec3A::ZERO,
            direction: Vec3A::Y,
            radius2: 1.0,
            half_height: 1.0,
        };
        // from -z to z
        let ray = Ray {
            origin: Vec3A::new(0.0, 0.0, -2.0),
            vector: Vec3A::new(0.0, 0.0, 4.0),
        };
        let ts = cylinder.get_intersections(&ray);
        assert_eq!(ts.count, 2);
        assert!((ts.ts[0] - 0.25).abs() < EPSILON);
        assert!((ts.ts[1] - 0.75).abs() < EPSILON);
        // from top to bottom
        let ray = Ray {
            origin: Vec3A::new(0.0, 2.0, 0.0),
            vector: Vec3A::new(0.0, -4.0, 0.0),
        };
        let ts = cylinder.get_intersections(&ray);
        assert_eq!(ts.count, 2);
        assert!((ts.ts[0] - 0.25).abs() < EPSILON);
        assert!((ts.ts[1] - 0.75).abs() < EPSILON);
        // from top to center
        let ray = Ray {
            origin: Vec3A::new(0.0, 2.0, 0.0),
            vector: Vec3A::new(0.0, -2.0, 0.0),
        };
        let ts = cylinder.get_intersections(&ray);
        assert_eq!(ts.count, 1);
        assert!((ts.ts[0] - 0.5).abs() < EPSILON);
    }

    #[test]
    fn test_primitive_sdf() {
        let sphere = Primitive::Sphere {
            center: Vec3A::ZERO,
            radius2: 4.0,
        };
        assert!((sphere.sdf(&Vec3A::ZERO) + 2.0).abs() < EPSILON);
        assert!((sphere.sdf(&Vec3A::new(3.0, 0.0, 0.0)) - 1.0).abs() < EPSILON);

        let rect = Primitive::RectangularParallelPiped {
            min: Vec3A::new(-1.0, -1.0, -1.0),
            max: Vec3A::new(1.0, 1.0, 1.0),
        };
        assert!(rect.sdf(&Vec3A::ZERO).is_sign_negative());
        assert!((rect.sdf(&Vec3A::new(2.0, 0.0, 0.0)) - 1.0).abs() < EPSILON);

        let cylinder = Primitive::FiniteCylinder {
            center: Vec3A::ZERO,
            direction: Vec3A::Y,
            radius2: 1.0,
            half_height: 1.0,
        };
        assert!((cylinder.sdf(&Vec3A::ZERO) + 1.0).abs() < EPSILON);
        assert!((cylinder.sdf(&Vec3A::new(2.0, 0.0, 0.0)) - 1.0).abs() < EPSILON);
    }
}
