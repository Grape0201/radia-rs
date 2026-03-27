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
    Complement,
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
                Instruction::Complement => {
                    stack[top - 1] = !stack[top - 1];
                }
            }
        }
        stack[0]
    }

    fn check_primitive_indices(
        &self,
        primitive_len: usize,
    ) -> Result<(), CSGInstructionValidationError> {
        for op in &self.instructions {
            if let Instruction::PushPrimitive(id) = op {
                if *id >= primitive_len {
                    return Err(CSGInstructionValidationError::PrimitiveIndexOutOfBounds {
                        index: *id,
                    });
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

    /// - Primitive indices are valid ?
    /// - CSG instructions are valid ?
    pub fn validate(&self) -> Result<(), CSGInstructionValidationError> {
        let primitive_len = self.primitives.len();
        for cell in &self.cells {
            cell.csg.check_primitive_indices(primitive_len)?;
            validate_csg_instructions(&cell.csg.instructions)?;
        }
        Ok(())
    }
}

#[derive(thiserror::Error, Debug, PartialEq)]
pub enum CSGInstructionValidationError {
    #[error("Primitive index out of bounds: {index}")]
    PrimitiveIndexOutOfBounds { index: usize },
    #[error("Empty instructions")]
    EmptyInstructions,
    #[error("Stack underflow at index {index}: {instruction}")]
    StackUnderflow { index: usize, instruction: String },
    #[error("Stack not exhausted: {remaining} items left")]
    StackNotExhausted { remaining: usize },
}

pub fn validate_csg_instructions(
    instructions: &[Instruction],
) -> Result<(), CSGInstructionValidationError> {
    if instructions.is_empty() {
        return Err(CSGInstructionValidationError::EmptyInstructions);
    }

    let mut depth: isize = 0;

    for (i, inst) in instructions.iter().enumerate() {
        match inst {
            Instruction::PushPrimitive(_) => depth += 1,
            // 2 pop -> 1 push = net -1
            Instruction::Union | Instruction::Intersection | Instruction::Difference => {
                if depth < 2 {
                    return Err(CSGInstructionValidationError::StackUnderflow {
                        index: i,
                        instruction: format!("{:?}", inst),
                    });
                }
                depth -= 1;
            }
            // net 0
            Instruction::Complement => {
                if depth < 1 {
                    return Err(CSGInstructionValidationError::StackUnderflow {
                        index: i,
                        instruction: format!("{:?}", inst),
                    });
                }
            }
        }
    }

    if depth != 1 {
        return Err(CSGInstructionValidationError::StackNotExhausted {
            remaining: depth as usize,
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Instruction::*;
    use super::*;

    #[test]
    fn test_check_primitive_indices() {
        let world = World {
            primitives: vec![],
            cells: vec![],
        };
        assert!(world.validate().is_ok());
        let world = World {
            primitives: vec![],
            cells: vec![Cell {
                csg: FlatCSG {
                    instructions: vec![PushPrimitive(0)],
                },
                material_id: 0,
            }],
        };
        assert!(world.validate().is_err());
        let world = World {
            primitives: vec![Primitive::Sphere {
                center: Vec3A::ZERO,
                radius2: 1.0,
            }],
            cells: vec![Cell {
                csg: FlatCSG {
                    instructions: vec![PushPrimitive(0)],
                },
                material_id: 0,
            }],
        };
        assert!(world.validate().is_ok());
        let world = World {
            primitives: vec![Primitive::Sphere {
                center: Vec3A::ZERO,
                radius2: 1.0,
            }],
            cells: vec![Cell {
                csg: FlatCSG {
                    instructions: vec![PushPrimitive(0), PushPrimitive(1), Union],
                },
                material_id: 0,
            }],
        };
        assert!(world.validate().is_err());
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
                    instructions: vec![PushPrimitive(0)],
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

    #[test]
    fn test_validate_single_primitive() {
        let instructions = vec![PushPrimitive(0)];
        assert_eq!(validate_csg_instructions(&instructions), Ok(()));
    }

    #[test]
    fn test_validate_two_primitives_no_op() {
        let instructions = vec![PushPrimitive(0), PushPrimitive(1)];
        assert!(matches!(
            validate_csg_instructions(&instructions),
            Err(CSGInstructionValidationError::StackNotExhausted { remaining: 2 })
        ));
    }

    #[test]
    fn test_validate_union() {
        let instructions = vec![PushPrimitive(0), PushPrimitive(1), Union];
        assert_eq!(validate_csg_instructions(&instructions), Ok(()));
    }

    #[test]
    fn test_validate_complement() {
        let instructions = vec![PushPrimitive(0), Complement];
        assert_eq!(validate_csg_instructions(&instructions), Ok(()));
    }

    #[test]
    fn test_validate_underflow() {
        let instructions = vec![PushPrimitive(0), Union];
        assert!(matches!(
            validate_csg_instructions(&instructions),
            Err(CSGInstructionValidationError::StackUnderflow { .. })
        ));
    }

    #[test]
    fn test_validate_complex() {
        // (A union B) difference (complement C)
        let instructions = vec![
            PushPrimitive(0),
            PushPrimitive(1),
            Union,
            PushPrimitive(2),
            Complement,
            Difference,
        ];
        assert_eq!(validate_csg_instructions(&instructions), Ok(()));
    }
}
