use crate::constants::{EPSILON, T_EPSILON};
use crate::mass_attenuation::MaterialIndex;
use crate::primitive::{CylinderData, Primitive, PrimitiveTag, RPPData, Ray, SphereData};

pub const SEGMENT_MIN: f32 = EPSILON;
pub const SEGMENT_MAX: f32 = 1.0 - EPSILON;

/// Flatten `CSGNode` into a list of instructions (Reverse Polish Notation)
#[derive(PartialEq, Debug, Clone, Copy)]
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
    #[inline(always)]
    fn evaluate_bitmask(&self, mask: u64) -> bool {
        let mut stack = 0u64;
        let mut top = 0;

        for op in &self.instructions {
            match op {
                Instruction::PushPrimitive(id) => {
                    let bit = (mask >> id) & 1;
                    stack |= bit << top;
                    top += 1;
                }
                Instruction::Union => {
                    let b = (stack >> (top - 1)) & 1;
                    let a = (stack >> (top - 2)) & 1;
                    stack &= !(1 << (top - 1));
                    stack &= !(1 << (top - 2));
                    stack |= (a | b) << (top - 2);
                    top -= 1;
                }
                Instruction::Intersection => {
                    let b = (stack >> (top - 1)) & 1;
                    let a = (stack >> (top - 2)) & 1;
                    stack &= !(1 << (top - 1));
                    stack &= !(1 << (top - 2));
                    stack |= (a & b) << (top - 2);
                    top -= 1;
                }
                Instruction::Difference => {
                    let b = (stack >> (top - 1)) & 1;
                    let a = (stack >> (top - 2)) & 1;
                    stack &= !(1 << (top - 1));
                    stack &= !(1 << (top - 2));
                    stack |= (a & !b) << (top - 2);
                    top -= 1;
                }
                Instruction::Complement => {
                    let a = (stack >> (top - 1)) & 1;
                    stack &= !(1 << (top - 1));
                    stack |= (!a & 1) << (top - 1);
                }
            }
        }
        (stack & 1) != 0
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
                if *id >= 64 {
                    return Err(CSGInstructionValidationError::Other(
                        "Only up to 64 primitives are supported for bitmask evaluation".to_string(),
                    ));
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

#[derive(Default, Debug, Clone)]
pub struct PrimitiveStorage {
    spheres: SphereData,
    rpps: RPPData,
    cylinders: CylinderData,
    tags: Vec<PrimitiveTag>,
    sphere_indices: Vec<usize>,
    rpp_indices: Vec<usize>,
    cylinder_indices: Vec<usize>,
}

impl PrimitiveStorage {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, primitive: Primitive) -> usize {
        let id = self.tags.len();
        match primitive {
            Primitive::Sphere { center, radius2 } => {
                let local_idx = self.spheres.centers.len();
                self.tags.push(PrimitiveTag::Sphere(local_idx));
                self.sphere_indices.push(id);
                self.spheres.push(center, radius2);
            }
            Primitive::RectangularParallelPiped { min, max } => {
                let local_idx = self.rpps.mins.len();
                self.tags.push(PrimitiveTag::RPP(local_idx));
                self.rpp_indices.push(id);
                self.rpps.push(min, max);
            }
            Primitive::FiniteCylinder {
                center,
                direction,
                radius2,
                half_height,
            } => {
                let local_idx = self.cylinders.centers.len();
                self.tags.push(PrimitiveTag::Cylinder(local_idx));
                self.cylinder_indices.push(id);
                self.cylinders.push(center, direction, radius2, half_height);
            }
        }
        id
    }

    pub(crate) fn get_ranges_batched(&self, rays: &[Ray], results: &mut [(f32, f32)]) {
        let n_prims = self.len();
        self.spheres
            .get_ranges_batched(rays, &self.sphere_indices, n_prims, results);
        self.rpps
            .get_ranges_batched(rays, &self.rpp_indices, n_prims, results);
        self.cylinders
            .get_ranges_batched(rays, &self.cylinder_indices, n_prims, results);
    }

    pub fn len(&self) -> usize {
        self.tags.len()
    }
}

pub struct World {
    pub primitives: PrimitiveStorage,
    pub cells: Vec<Cell>,
}

impl World {
    pub(crate) fn get_ray_segments_from_ranges(
        &self,
        ray: &Ray,
        ranges: &[(f32, f32)],
        segments: &mut Vec<(Option<MaterialIndex>, f32)>,
        buf_ts: &mut Vec<f32>,
    ) {
        segments.clear();
        let ray_length = ray.vector.length();
        if ray_length <= EPSILON {
            return;
        }

        buf_ts.clear();
        buf_ts.push(0.0);
        buf_ts.push(1.0);
        for &(t0, t1) in ranges.iter() {
            if t0 > SEGMENT_MIN && t0 < SEGMENT_MAX {
                buf_ts.push(t0);
            }
            if t1 > SEGMENT_MIN && t1 < SEGMENT_MAX {
                buf_ts.push(t1);
            }
        }

        buf_ts.sort_unstable_by(|a, b| a.total_cmp(b));

        // Deduplicate
        if !buf_ts.is_empty() {
            let mut unique_count = 1;
            for i in 1..buf_ts.len() {
                if buf_ts[i] - buf_ts[unique_count - 1] >= T_EPSILON {
                    buf_ts[unique_count] = buf_ts[i];
                    unique_count += 1;
                }
            }
            buf_ts.truncate(unique_count);
        }

        for i in 0..buf_ts.len().saturating_sub(1) {
            let t0 = buf_ts[i];
            let t1 = buf_ts[i + 1];
            let length = (t1 - t0) * ray_length;
            let t_mid = (t0 + t1) * 0.5;

            // Build bitmask for this segment using SIMD
            let mut mask = 0u64;
            let tm = glam::Vec4::splat(t_mid);
            let mut j = 0;
            while j + 4 <= ranges.len() {
                let r0 = ranges[j];
                let r1 = ranges[j + 1];
                let r2 = ranges[j + 2];
                let r3 = ranges[j + 3];

                let mins = glam::Vec4::new(r0.0, r1.0, r2.0, r3.0);
                let maxs = glam::Vec4::new(r0.1, r1.1, r2.1, r3.1);

                let inside = tm.cmpge(mins) & tm.cmple(maxs);
                mask |= (inside.bitmask() as u64) << j;
                j += 4;
            }

            // Remainder
            for k in j..ranges.len() {
                let (rt0, rt1) = ranges[k];
                if t_mid >= rt0 && t_mid <= rt1 {
                    mask |= 1 << k;
                }
            }

            let mut matid = None;
            for cell in &self.cells {
                if cell.csg.evaluate_bitmask(mask) {
                    matid = Some(cell.material_id);
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
    #[error("{0}")]
    Other(String),
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
    use glam::Vec3A;

    #[test]
    fn test_check_primitive_indices() {
        let world = World {
            primitives: PrimitiveStorage::new(),
            cells: vec![],
        };
        assert!(world.validate().is_ok());

        let world = World {
            primitives: PrimitiveStorage::new(),
            cells: vec![Cell {
                csg: FlatCSG {
                    instructions: vec![PushPrimitive(0)],
                },
                material_id: 0,
            }],
        };
        assert!(world.validate().is_err());

        let mut primitives = PrimitiveStorage::new();
        primitives.add(Primitive::Sphere {
            center: Vec3A::ZERO,
            radius2: 1.0,
        });
        let world = World {
            primitives,
            cells: vec![Cell {
                csg: FlatCSG {
                    instructions: vec![PushPrimitive(0)],
                },
                material_id: 0,
            }],
        };
        assert!(world.validate().is_ok());

        let mut primitives = PrimitiveStorage::new();
        primitives.add(Primitive::Sphere {
            center: Vec3A::ZERO,
            radius2: 1.0,
        });
        let world = World {
            primitives,
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
            primitives: PrimitiveStorage::new(),
            cells: vec![],
        };
        let ray = Ray {
            origin: Vec3A::ZERO,
            vector: Vec3A::ZERO,
        };
        let mut segments = Vec::new();
        let mut buf_ts = Vec::new();
        world.get_ray_segments_from_ranges(&ray, &[], &mut segments, &mut buf_ts);
        assert!(segments.is_empty());
        assert!(buf_ts.is_empty());
    }

    #[test]
    fn test_get_ray_segments_one_sphere() {
        let mut primitives = PrimitiveStorage::new();
        primitives.add(Primitive::Sphere {
            center: Vec3A::ZERO,
            radius2: 1.0,
        });
        let world = World {
            primitives,
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
        let mut ranges = vec![(f32::INFINITY, f32::NEG_INFINITY); world.primitives.len()];
        world
            .primitives
            .get_ranges_batched(&[ray.clone()], &mut ranges);

        world.get_ray_segments_from_ranges(&ray, &ranges, &mut segments, &mut buf_ts);
        println!("segments: {:?}", segments);
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
