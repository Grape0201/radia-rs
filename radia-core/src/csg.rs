use crate::constants::{EPSILON, T_EPSILON};
use crate::material::MaterialIndex;
use crate::primitive::{Primitive, Ray};
use glam::Vec3A;

/// Flatten `CSGNode` into a list of instructions (Reverse Polish Notation)
#[derive(PartialEq, Debug)]
pub enum Instruction {
    Union,
    Intersection,
    Difference,
    PushPrimitive(usize),
}

pub struct FlatCSG {
    pub instructions: Vec<Instruction>,
}

impl FlatCSG {
    pub fn contains(&self, p: &Vec3A, primitives: &[Primitive]) -> bool {
        let mut stack = [false; 16];
        let mut top = 0;

        for op in &self.instructions {
            match op {
                Instruction::PushPrimitive(id) => {
                    stack[top] = primitives[*id].contains(p);
                    top += 1;
                }
                Instruction::Union => {
                    stack[top - 2] = stack[top - 2] || stack[top - 1];
                    top -= 1;
                }
                Instruction::Intersection => {
                    stack[top - 2] = stack[top - 2] && stack[top - 1];
                    top -= 1;
                }
                Instruction::Difference => {
                    stack[top - 2] = stack[top - 2] && !stack[top - 1];
                    top -= 1;
                }
            }
        }
        stack[0]
    }

    fn check_primitive_indices(&self, primitive_len: usize) -> Result<(), String> {
        for op in &self.instructions {
            if let Instruction::PushPrimitive(id) = op {
                if *id >= primitive_len {
                    return Err(format!("Primitive index out of bounds: {}", *id));
                }
            }
        }
        Ok(())
    }
}

pub struct Cell {
    pub csg: FlatCSG,
    pub material_id: MaterialIndex,
}

pub struct World {
    pub primitives: Vec<Primitive>,
    pub cells: Vec<Cell>,
}

impl World {
    pub fn get_ray_segments(
        &self,
        ray: &Ray,
        segments: &mut Vec<(Option<MaterialIndex>, f32)>, // result buffer: (material_id, length)
        buf_ts: &mut Vec<f32>,                            // intersection points buffer
        buf_merged_ts: &mut Vec<f32>,                     // merged intersection points buffer
    ) {
        segments.clear();
        buf_ts.clear();
        buf_merged_ts.clear();
        let ray_length = ray.vector.length();
        if ray_length <= EPSILON {
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
            let length = (t1 - t0) * ray_length;

            let t_mid = (t0 + t1) * 0.5;
            let p_mid = ray.origin + ray.vector * t_mid;

            let mut matid = None;
            for cell in &self.cells {
                if cell.csg.contains(&p_mid, &self.primitives) {
                    matid = Some(cell.material_id);
                    // Stop checking cells once we find the one containing this segment
                    // Assuming cells do not overlap
                    break;
                }
            }
            segments.push((matid, length));
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
                csg: FlatCSG {
                    instructions: vec![Instruction::PushPrimitive(0)],
                },
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
                csg: FlatCSG {
                    instructions: vec![Instruction::PushPrimitive(0)],
                },
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
                csg: FlatCSG {
                    instructions: vec![
                        Instruction::PushPrimitive(0),
                        Instruction::PushPrimitive(1),
                        Instruction::Union,
                    ],
                },
                material_id: 0,
            }],
        };
        assert!(world.check_primitive_indices().is_err());
    }

    #[test]
    fn test_get_ray_segments_empty() {
        let world = World {
            primitives: vec![],
            cells: vec![],
        };
        let ray = Ray {
            origin: Vec3A::ZERO,
            vector: Vec3A::ZERO,
        };
        let mut segments = Vec::new();
        let mut buf_ts = Vec::new();
        let mut buf_merged_ts = Vec::new();
        world.get_ray_segments(&ray, &mut segments, &mut buf_ts, &mut buf_merged_ts);
        assert!(segments.is_empty());
        assert!(buf_ts.is_empty());
        assert!(buf_merged_ts.is_empty());
    }

    #[test]
    fn test_get_ray_segments_one_sphere() {
        let world = World {
            primitives: vec![Primitive::Sphere {
                center: Vec3A::ZERO,
                radius2: 1.0,
            }],
            cells: vec![Cell {
                csg: FlatCSG {
                    instructions: vec![Instruction::PushPrimitive(0)],
                },
                material_id: 0,
            }],
        };
        let ray = Ray {
            origin: Vec3A::ZERO,
            vector: Vec3A::from([3.0, 0.0, 0.0]),
        };
        let mut segments = Vec::new();
        let mut buf_ts = Vec::new();
        let mut buf_merged_ts = Vec::new();
        world.get_ray_segments(&ray, &mut segments, &mut buf_ts, &mut buf_merged_ts);
        assert_eq!(segments.len(), 2);
        // in sphere, material_id == 0
        assert_eq!(segments[0].0, Some(0));
        assert!((segments[0].1 - 1.0).abs() < EPSILON);
        // out of sphere, material_id == None
        assert_eq!(segments[1].0, None);
        assert!((segments[1].1 - 2.0).abs() < EPSILON);
    }
}
