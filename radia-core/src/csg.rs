use crate::constants::{EPSILON, T_EPSILON};
use crate::primitive::{Primitive, Ray};
use glam::Vec3A;

#[derive(PartialEq, Debug)]
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
    fn check_primitive_indices(&self, primitive_len: usize) -> Result<(), String> {
        match self {
            CSGNode::Union(left, right) => {
                left.check_primitive_indices(primitive_len)?;
                right.check_primitive_indices(primitive_len)?;
                Ok(())
            }
            CSGNode::Intersection(left, right) => {
                left.check_primitive_indices(primitive_len)?;
                right.check_primitive_indices(primitive_len)?;
                Ok(())
            }
            CSGNode::Difference(left, right) => {
                left.check_primitive_indices(primitive_len)
                    .and_then(|_| right.check_primitive_indices(primitive_len))?;
                Ok(())
            }
            CSGNode::Primitive(id) => {
                if *id >= primitive_len {
                    Err(format!("Primitive index out of bounds: {}", *id))
                } else {
                    Ok(())
                }
            }
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

        // Assume ts are not NaN
        buf_ts.sort_unstable_by(|a, b| a.total_cmp(b));

        for t in buf_ts {
            if buf_merged_ts.is_empty() {
                buf_merged_ts.push(*t);
            } else {
                let last = buf_merged_ts.last().unwrap();
                if *t - *last > T_EPSILON {
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

    pub fn check_primitive_indices(&self) -> Result<(), String> {
        let primitive_len = self.primitives.len();
        for cell in &self.cells {
            cell.csg.check_primitive_indices(primitive_len)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_primitive_indices() {
        let world = World {
            primitives: vec![],
            cells: vec![],
        };
        assert!(world.check_primitive_indices().is_ok());
        let world = World {
            primitives: vec![],
            cells: vec![Cell {
                csg: CSGNode::Primitive(0),
                material_id: 0,
            }],
        };
        assert!(world.check_primitive_indices().is_err());
        let world = World {
            primitives: vec![Primitive::Sphere {
                center: Vec3A::ZERO,
                radius2: 1.0,
            }],
            cells: vec![Cell {
                csg: CSGNode::Primitive(0),
                material_id: 0,
            }],
        };
        assert!(world.check_primitive_indices().is_ok());
        let world = World {
            primitives: vec![Primitive::Sphere {
                center: Vec3A::ZERO,
                radius2: 1.0,
            }],
            cells: vec![Cell {
                csg: CSGNode::Union(
                    Box::new(CSGNode::Primitive(0)),
                    Box::new(CSGNode::Primitive(1)),
                ),
                material_id: 0,
            }],
        };
        assert!(world.check_primitive_indices().is_err());
    }
}
