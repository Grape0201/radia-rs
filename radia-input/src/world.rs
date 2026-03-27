use crate::csg_parser::{parse_csg, validate_csg_syntax};
use garde::Validate;
use glam::Vec3A;
use radia_core::csg::{Cell, Instruction, World};
use radia_core::material::MaterialIndex;
use radia_core::primitive::Primitive;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::InputError;
use crate::common::{MinMaxBounds, is_vector_longer_than_epsilon};

#[derive(Serialize, Deserialize, Debug, Validate, Default)]
pub struct WorldInput {
    #[garde(dive)]
    #[serde(default)]
    pub primitives: Vec<PrimitiveInput>,
    #[garde(dive)]
    #[serde(default)]
    pub cells: Vec<CellInput>,
}

#[derive(Serialize, Deserialize, Debug, Validate)]
#[serde(tag = "type")]
pub enum PrimitiveInput {
    #[serde(alias = "SPH")]
    Sphere {
        #[garde(length(min = 1))]
        name: String,
        #[garde(skip)]
        center: [f32; 3],
        #[garde(range(min = 0.0))]
        radius: f32,
    },
    #[serde(alias = "RPP", alias = "Aabb", alias = "AABB")]
    RectangularParallelPiped {
        #[garde(length(min = 1))]
        name: String,
        #[garde(dive)]
        #[serde(flatten)]
        bounds: MinMaxBounds,
    },
    #[serde(alias = "CYL")]
    FiniteCylinder {
        #[garde(length(min = 1))]
        name: String,
        #[garde(skip)]
        center: [f32; 3],
        #[garde(custom(is_vector_longer_than_epsilon))]
        vector: [f32; 3],
        #[garde(range(min = 0.0))]
        radius: f32,
    },
}

impl PrimitiveInput {
    pub fn name(&self) -> &str {
        match self {
            PrimitiveInput::Sphere { name, .. } => name,
            PrimitiveInput::RectangularParallelPiped { name, .. } => name,
            PrimitiveInput::FiniteCylinder { name, .. } => name,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Validate)]
pub struct CellInput {
    #[garde(skip)]
    pub material_name: String,
    #[garde(custom(validate_csg_syntax_garde))]
    pub csg: String,
}

fn validate_csg_syntax_garde(value: &str, _context: &()) -> garde::Result {
    validate_csg_syntax(value).map_err(|e| garde::Error::new(e))
}

impl WorldInput {
    pub fn build(self, material_map: &HashMap<String, MaterialIndex>) -> Result<World, InputError> {
        let mut prim_map = HashMap::<String, usize>::new();
        for (index, p) in self.primitives.iter().enumerate() {
            let name = p.name();
            if prim_map.contains_key(name) {
                return Err(InputError::InvalidPrimitive {
                    name: name.to_string(),
                    reason: "Duplicate primitive name".to_string(),
                });
            }
            prim_map.insert(name.to_string(), index);
        }

        let primitives: Vec<Primitive> = self
            .primitives
            .into_iter()
            .map(|p| convert_primitive(&p))
            .collect();

        let cells: Vec<Cell> = self
            .cells
            .into_iter()
            .map(|cell_input| {
                let csg = parse_csg(&cell_input.csg, &prim_map)?;
                let material_id =
                    *material_map.get(&cell_input.material_name).ok_or_else(|| {
                        InputError::MaterialNotFound(cell_input.material_name.clone())
                    })?;
                Ok(Cell { csg, material_id })
            })
            .collect::<Result<_, InputError>>()?;

        let used_ids: std::collections::HashSet<usize> = cells
            .iter()
            .flat_map(|cell| &cell.csg.instructions)
            .filter_map(|inst| match inst {
                Instruction::PushPrimitive(id) => Some(*id),
                _ => None,
            })
            .collect();

        for (name, &idx) in &prim_map {
            if !used_ids.contains(&idx) {
                tracing::warn!(primitive = %name, "Primitive is defined but never used in any cell");
            }
        }

        Ok(World { primitives, cells })
    }
}

fn convert_primitive(p: &PrimitiveInput) -> Primitive {
    match p {
        PrimitiveInput::Sphere { center, radius, .. } => Primitive::Sphere {
            center: Vec3A::from_array(*center),
            radius2: radius * radius,
        },
        PrimitiveInput::RectangularParallelPiped { bounds, .. } => {
            Primitive::RectangularParallelPiped {
                min: Vec3A::from_array(bounds.min),
                max: Vec3A::from_array(bounds.max),
            }
        }
        PrimitiveInput::FiniteCylinder {
            center,
            vector,
            radius,
            ..
        } => {
            let v = Vec3A::from_array(*vector);
            let length = v.length();
            Primitive::FiniteCylinder {
                center: Vec3A::from_array(*center),
                direction: v / length,
                radius2: radius * radius,
                half_height: length / 2.0,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_world_input() {
        let yaml = r#"
primitives: 
  - name: unused
    type: Sphere
    center: [1.0, 1.0, 1.0]
    radius: 1.0
  - name: source
    type: Sphere
    center: [0.0, 0.0, 0.0]
    radius: 2.0
  - name: whole_world
    type: RectangularParallelPiped
    min: [0.0, 0.0, 0.0]
    max: [1.0, 1.0, 1.0]
cells: 
  - material_name: Water
    csg: source
  - material_name: Air
    csg: whole_world - source
"#;

        let input: WorldInput = serde_saphyr::from_str_valid(yaml).unwrap();
        assert_eq!(input.primitives.len(), 3);
        assert_eq!(input.cells.len(), 2);

        let mut material_map = HashMap::new();
        material_map.insert("Water".to_string(), 0);
        material_map.insert("Air".to_string(), 1);

        let world = input.build(&material_map).unwrap();
        assert_eq!(world.primitives.len(), 3);
        assert_eq!(world.cells.len(), 2);
        assert_eq!(world.cells[0].material_id, 0); // Water
        assert_eq!(world.cells[1].material_id, 1); // Air
        assert_eq!(
            world.cells[0].csg.instructions,
            vec![Instruction::PushPrimitive(1)]
        );
        assert_eq!(
            world.cells[1].csg.instructions,
            vec![
                Instruction::PushPrimitive(2),
                Instruction::PushPrimitive(1),
                Instruction::Difference
            ]
        );
    }

    #[test]
    fn test_deserialize_primitive_aliases() {
        let yaml = r#"primitives:
  - name: s
    type: SPH
    center: [0.0, 0.0, 0.0]
    radius: 1.0
  - name: r1
    type: RPP
    min: [0.0, 0.0, 0.0]
    max: [1.0, 1.0, 1.0]
  - name: r2
    type: Aabb
    min: [0.0, 0.0, 0.0]
    max: [1.0, 1.0, 1.0]
  - name: r3
    type: AABB
    min: [0.0, 0.0, 0.0]
    max: [1.0, 1.0, 1.0]
  - name: c
    type: CYL
    center: [0.0, 0.0, 0.0]
    vector: [0.0, 0.0, 1.0]
    radius: 0.5
cells:
- material_name: Water
  csg: s
        "#;

        let input: WorldInput = serde_saphyr::from_str_valid(yaml).unwrap();
        assert!(matches!(input.primitives[0], PrimitiveInput::Sphere { .. }));
        assert!(matches!(
            input.primitives[1],
            PrimitiveInput::RectangularParallelPiped { .. }
        ));
        assert!(matches!(
            input.primitives[2],
            PrimitiveInput::RectangularParallelPiped { .. }
        ));
        assert!(matches!(
            input.primitives[3],
            PrimitiveInput::RectangularParallelPiped { .. }
        ));
        assert!(matches!(
            input.primitives[4],
            PrimitiveInput::FiniteCylinder { .. }
        ));
        assert!(input.validate().is_ok());
    }

    #[test]
    fn test_complicated_csg() {
        let yaml = r#"
primitives: 
  - {name: s1, type: Sphere, center: [0.0, 0.0, 0.0], radius: 1.0 }
  - {name: s2, type: Sphere, center: [1.0, 0.0, 0.0], radius: 1.0 }
  - {name: s3, type: Sphere, center: [2.0, 0.0, 0.0], radius: 1.0 }
  - {name: s4, type: Sphere, center: [3.0, 0.0, 0.0], radius: 1.0 }
  - {name: s5, type: Sphere, center: [4.0, 0.0, 0.0], radius: 1.0 }
  - {name: s6, type: Sphere, center: [5.0, 0.0, 0.0], radius: 1.0 }
  - {name: s7, type: Sphere, center: [6.0, 0.0, 0.0], radius: 1.0 }
cells:
  - material_name: Water
    csg: s1 + s2 + s3 + s4 + s5 + s6 + s7
  - material_name: Air
    csg: s1 * s2 * s3 * s4 * s5 * s6 * s7
  - material_name: Air
    csg: (s1 + s2) * s3
"#;

        let input: WorldInput = serde_saphyr::from_str_valid(yaml).unwrap();
        let mut material_map = HashMap::new();
        material_map.insert("Water".to_string(), 0);
        material_map.insert("Air".to_string(), 1);
        let world = input.build(&material_map).unwrap();
        assert_eq!(world.cells[0].csg.instructions.len(), 13);
        println!("{:?}", world.cells[0].csg.instructions);
        assert_eq!(
            world.cells[0].csg.instructions.last(),
            Some(&Instruction::Union)
        );
        assert_eq!(world.cells[1].csg.instructions.len(), 13);
        println!("{:?}", world.cells[1].csg.instructions);
        assert_eq!(
            world.cells[1].csg.instructions.last(),
            Some(&Instruction::Intersection)
        );
        assert_eq!(world.cells[2].csg.instructions.len(), 5);
        println!("{:?}", world.cells[2].csg.instructions);
        assert_eq!(
            world.cells[2].csg.instructions.last(),
            Some(&Instruction::Intersection)
        );
    }
    #[test]
    fn test_validate_csg_syntax() {
        let yaml = "material_name: mat1\ncsg: p1 | p2";
        assert!(serde_saphyr::from_str_valid::<CellInput>(yaml).is_err());
        let yaml = "material_name: mat1\ncsg: p1 p2";
        assert!(serde_saphyr::from_str_valid::<CellInput>(yaml).is_err());
    }
}
