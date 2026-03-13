use crate::shape::{Ray, Shape};
use glam::Vec3A;
use std::collections::HashMap;

pub enum CSGNode {
    Union(Box<CSGNode>, Box<CSGNode>),
    Intersection(Box<CSGNode>, Box<CSGNode>),
    Difference(Box<CSGNode>, Box<CSGNode>),
    Primitive(usize), // shape_id
}

impl CSGNode {
    fn contains(&self, p: &Vec3A, shapes: &HashMap<usize, Shape>) -> bool {
        match self {
            CSGNode::Union(left, right) => left.contains(p, shapes) || right.contains(p, shapes),
            CSGNode::Intersection(left, right) => {
                left.contains(p, shapes) && right.contains(p, shapes)
            }
            CSGNode::Difference(left, right) => {
                left.contains(p, shapes) && !right.contains(p, shapes)
            }
            CSGNode::Primitive(id) => shapes.get(id).is_some_and(|s| s.contains(p)),
        }
    }
}

pub struct Cell {
    pub csg: CSGNode,
    pub material_id: u32,
}

pub struct World {
    pub shapes: HashMap<usize, Shape>,
    pub cells: Vec<Cell>,
}

const EPSILON: f32 = 1e-6;

impl World {
    pub fn get_ray_segments(
        &self,
        ray: &Ray,
        segments: &mut Vec<(u32, f32)>, // result buffer: (material_id, length)
        buffer: &mut Vec<f32>,          // intersection points buffer
    ) {
        segments.clear();
        buffer.clear();
        let dir_len = ray.vector.length();
        if dir_len <= EPSILON {
            return;
        }

        buffer.push(0.0);
        buffer.push(1.0);

        // Collect intersections for all shapes
        for shape in self.shapes.values() {
            let ts = shape.get_intersections(ray);
            buffer.extend_from_slice(&ts.ts[0..ts.count]);
        }

        buffer.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let mut merged_ts = Vec::new();
        for t in buffer {
            if merged_ts.is_empty() {
                merged_ts.push(t);
            } else {
                let last = merged_ts.last().unwrap();
                if *t - **last > EPSILON {
                    merged_ts.push(t);
                }
            }
        }

        for i in 0..merged_ts.len().saturating_sub(1) {
            let t0 = &merged_ts[i];
            let t1 = *merged_ts[i + 1];

            // Only care about points in front of the ray origin
            if t1 <= 0.0 {
                continue;
            }
            let t0 = t0.max(0.0);

            if t1 - t0 <= EPSILON {
                continue;
            }

            let t_mid = (t0 + t1) * 0.5;
            let p_mid = ray.origin + ray.vector * t_mid;

            for cell in &self.cells {
                if cell.csg.contains(&p_mid, &self.shapes) {
                    let length = (t1 - t0) * dir_len;
                    segments.push((cell.material_id, length));
                    // Stop checking cells once we find the one containing this segment
                    // Assuming cells do not overlap
                    break;
                }
            }
        }
    }
}
