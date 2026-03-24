use garde::Validate;
use glam::Vec3A;
use radia_core::csg::{CSGNode, Cell, World};
use radia_core::material::MaterialIndex;
use radia_core::primitive::Primitive;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::InputError;
use crate::common::{MinMaxBounds, is_vector_longer_than_epsilon};

#[derive(Serialize, Deserialize, Debug, Validate)]
pub struct WorldInput {
    #[garde(dive)]
    pub primitives: HashMap<String, PrimitiveInput>,
    #[garde(dive)]
    pub cells: Vec<CellInput>,
}

#[derive(Serialize, Deserialize, Debug, Validate)]
#[serde(tag = "type")]
pub enum PrimitiveInput {
    #[serde(alias = "SPH")]
    Sphere {
        #[garde(skip)]
        center: [f32; 3],
        #[garde(range(min = 0.0))]
        radius: f32,
    },
    #[serde(alias = "RPP", alias = "Aabb", alias = "AABB")]
    RectangularParallelPiped {
        #[garde(dive)]
        #[serde(flatten)]
        bounds: MinMaxBounds,
    },
    #[serde(alias = "CYL")]
    FiniteCylinder {
        #[garde(skip)]
        center: [f32; 3],
        #[garde(custom(is_vector_longer_than_epsilon))]
        vector: [f32; 3],
        #[garde(range(min = 0.0))]
        radius: f32,
    },
}

#[derive(Serialize, Deserialize, Debug, Validate)]
pub struct CellInput {
    #[garde(skip)]
    pub material_name: String,
    #[garde(dive)]
    pub csg: CSGInput,
}

#[derive(Serialize, Deserialize, Debug, Validate)]
#[serde(untagged)]
pub enum CSGInput {
    #[garde(skip)]
    Operation {
        #[garde(skip)]
        op: String,
        #[garde(dive)]
        prs: Vec<CSGInput>,
    },
    #[garde(skip)]
    Name(#[garde(skip)] String),
}

impl WorldInput {
    pub fn build(self, material_map: &HashMap<String, MaterialIndex>) -> Result<World, InputError> {
        let mut used_primitive_names = std::collections::HashSet::new();
        for c_conf in &self.cells {
            collect_used_primitives(&c_conf.csg, &mut used_primitive_names);
        }

        let mut primitive_map = HashMap::new();
        let mut primitives = Vec::new();

        for (name, p_conf) in self.primitives {
            if !used_primitive_names.contains(&name) {
                tracing::warn!("Primitive '{}' is defined but never used.", name);
                continue;
            }

            let id = primitives.len();
            primitive_map.insert(name.to_string(), id);

            let p = match p_conf {
                PrimitiveInput::Sphere { center, radius, .. } => Primitive::Sphere {
                    center: Vec3A::from_array(center),
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
                    let v = Vec3A::from_array(vector);
                    let length = v.length();
                    Primitive::FiniteCylinder {
                        center: Vec3A::from_array(center),
                        direction: v / length,
                        radius2: radius * radius,
                        half_height: length / 2.0,
                    }
                }
            };
            primitives.push(p);
        }

        let mut cells = Vec::new();
        for c_conf in self.cells {
            let material_id = *material_map
                .get(&c_conf.material_name)
                .ok_or_else(|| InputError::MaterialNotFound(c_conf.material_name.clone()))?;

            let csg = build_csg_node(&c_conf.csg, &primitive_map)?;
            cells.push(Cell { csg, material_id });
        }

        Ok(World { primitives, cells })
    }
}

fn collect_used_primitives(config: &CSGInput, used: &mut std::collections::HashSet<String>) {
    match config {
        CSGInput::Name(name) => {
            used.insert(name.clone());
        }
        CSGInput::Operation { prs, .. } => {
            for p in prs {
                collect_used_primitives(p, used);
            }
        }
    }
}

fn build_csg_node(
    config: &CSGInput,
    primitive_map: &HashMap<String, usize>,
) -> Result<CSGNode, InputError> {
    match config {
        CSGInput::Name(name) => {
            let id = *primitive_map
                .get(name)
                .ok_or_else(|| InputError::PrimitiveNotFound(name.clone()))?;
            Ok(CSGNode::Primitive(id))
        }
        CSGInput::Operation { op, prs } => {
            if prs.is_empty() {
                return Err(InputError::EmptyCsgOperation);
            }
            match op.as_str() {
                "union" | "outer" => {
                    let mut node = build_csg_node(&prs[0], primitive_map)?;
                    for p in &prs[1..] {
                        node = CSGNode::Union(
                            Box::new(node),
                            Box::new(build_csg_node(p, primitive_map)?),
                        );
                    }
                    Ok(node)
                }
                "intersection" | "inner" => {
                    let mut node = build_csg_node(&prs[0], primitive_map)?;
                    for p in &prs[1..] {
                        node = CSGNode::Intersection(
                            Box::new(node),
                            Box::new(build_csg_node(p, primitive_map)?),
                        );
                    }
                    Ok(node)
                }
                "difference" => {
                    let mut node = build_csg_node(&prs[0], primitive_map)?;
                    for p in &prs[1..] {
                        node = CSGNode::Difference(
                            Box::new(node),
                            Box::new(build_csg_node(p, primitive_map)?),
                        );
                    }
                    Ok(node)
                }
                _ => Err(InputError::UnknownCsgOperation(op.clone())),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_world_input() {
        let json = r#"{
            "primitives": {
                "unused": {"type": "Sphere", "center": [1.0, 1.0, 1.0], "radius": 1.0},
                "source": {"type": "Sphere", "center": [0.0, 0.0, 0.0], "radius": 2.0},
                "whole_world": {"type": "RectangularParallelPiped", "min": [0.0, 0.0, 0.0], "max": [1.0, 1.0, 1.0]}
            },
            "cells": [
                {"material_name": "Water", "csg": {"op": "inner", "prs": ["source"]}},
                {"material_name": "Air", "csg": {"op": "difference", "prs": ["whole_world", "source"]}}
            ]
        }"#;

        let input: WorldInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.primitives.len(), 3);
        assert_eq!(input.cells.len(), 2);

        let mut material_map = HashMap::new();
        material_map.insert("Water".to_string(), 0);
        material_map.insert("Air".to_string(), 1);

        let world = input.build(&material_map).unwrap();
        assert_eq!(world.primitives.len(), 2);
        assert_eq!(world.cells.len(), 2);
        assert_eq!(world.cells[0].material_id, 0); // Water
        assert_eq!(world.cells[1].material_id, 1); // Air
    }

    #[test]
    fn test_deserialize_primitive_aliases() {
        let yaml = r#"primitives:
  s:
    type: SPH
    center: [0.0, 0.0, 0.0]
    radius: 1.0
  r1:
    type: RPP
    min: [0.0, 0.0, 0.0]
    max: [1.0, 1.0, 1.0]
  r2:
    type: Aabb
    min: [0.0, 0.0, 0.0]
    max: [1.0, 1.0, 1.0]
  r3:
    type: AABB
    min: [0.0, 0.0, 0.0]
    max: [1.0, 1.0, 1.0]
  c:
    type: CYL
    center: [0.0, 0.0, 0.0]
    vector: [0.0, 0.0, 1.0]
    radius: 0.5
cells:
- material_name: Water
  csg: s
        "#;

        let input: Result<WorldInput, _> = serde_saphyr::from_str_valid(yaml);
        assert!(input.is_ok());
    }
}
