use crate::primitive::{Primitive, Ray};
use glam::Vec3A;

pub enum CSGNode {
    Union(Box<CSGNode>, Box<CSGNode>),
    Intersection(Box<CSGNode>, Box<CSGNode>),
    Difference(Box<CSGNode>, Box<CSGNode>),
    Primitive(usize), // primitive_id
}

impl CSGNode {
    fn contains(&self, p: &Vec3A, primitives: &[Primitive]) -> bool {
        match self {
            CSGNode::Union(left, right) => {
                left.contains(p, primitives) || right.contains(p, primitives)
            }
            CSGNode::Intersection(left, right) => {
                left.contains(p, primitives) && right.contains(p, primitives)
            }
            CSGNode::Difference(left, right) => {
                left.contains(p, primitives) && !right.contains(p, primitives)
            }
            CSGNode::Primitive(id) => primitives[*id].contains(p),
        }
    }
    fn check_primitive_indices(&self, primitive_len: usize) -> bool {
        match self {
            CSGNode::Union(left, right) => {
                left.check_primitive_indices(primitive_len)
                    && right.check_primitive_indices(primitive_len)
            }
            CSGNode::Intersection(left, right) => {
                left.check_primitive_indices(primitive_len)
                    && right.check_primitive_indices(primitive_len)
            }
            CSGNode::Difference(left, right) => {
                left.check_primitive_indices(primitive_len)
                    && right.check_primitive_indices(primitive_len)
            }
            CSGNode::Primitive(id) => *id < primitive_len,
        }
    }
}

pub struct Cell {
    pub csg: CSGNode,
    pub material_id: u32,
}

pub struct World {
    pub primitives: Vec<Primitive>,
    pub cells: Vec<Cell>,
}

const EPSILON: f32 = 1e-6;

impl World {
    pub fn get_ray_segments(
        &self,
        ray: &Ray,
        segments: &mut Vec<(u32, f32)>, // result buffer: (material_id, length)
        buf_ts: &mut Vec<f32>,          // intersection points buffer
        buf_merged_ts: &mut Vec<f32>,   // merged intersection points buffer
    ) {
        segments.clear();
        buf_ts.clear();
        buf_merged_ts.clear();
        let dir_len = ray.vector.length();
        if dir_len <= EPSILON {
            return;
        }

        buf_ts.push(0.0);
        buf_ts.push(1.0);

        // Collect intersections for all primitives
        for primitive in &self.primitives {
            let ts = primitive.get_intersections(ray);
            buf_ts.extend_from_slice(&ts.ts[0..ts.count]);
        }

        buf_ts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        for t in buf_ts {
            if buf_merged_ts.is_empty() {
                buf_merged_ts.push(*t);
            } else {
                let last = buf_merged_ts.last().unwrap();
                if *t - *last > EPSILON {
                    buf_merged_ts.push(*t);
                }
            }
        }

        for i in 0..buf_merged_ts.len().saturating_sub(1) {
            let t0 = &buf_merged_ts[i];
            let t1 = buf_merged_ts[i + 1];

            let t_mid = (t0 + t1) * 0.5;
            let p_mid = ray.origin + ray.vector * t_mid;

            for cell in &self.cells {
                if cell.csg.contains(&p_mid, &self.primitives) {
                    let length = (t1 - t0) * dir_len;
                    segments.push((cell.material_id, length));
                    // Stop checking cells once we find the one containing this segment
                    // Assuming cells do not overlap
                    break;
                }
            }
        }
    }

    pub fn check_primitive_indices(&self) -> bool {
        let primitive_len = self.primitives.len();
        for cell in &self.cells {
            if !cell.csg.check_primitive_indices(primitive_len) {
                return false;
            }
        }
        true
    }
}
