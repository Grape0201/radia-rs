use glam::Vec3A;
use radia_core::csg::{CSGNode, Cell, World};
use radia_core::primitive::Primitive;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::InputError;

const EPSILON: f32 = 1e-6; // length [cm]

#[derive(Serialize, Deserialize, Debug)]
pub struct WorldInput {
    pub primitives: Vec<PrimitiveInput>,
    pub cells: Vec<CellInput>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
pub enum PrimitiveInput {
    #[serde(alias = "SPH")]
    Sphere {
        name: String,
        center: [f32; 3],
        radius: f32,
    },
    #[serde(alias = "RPP", alias = "Aabb", alias = "AABB")]
    RectangularParallelPiped {
        name: String,
        min: [f32; 3],
        max: [f32; 3],
    },
    #[serde(alias = "CYL")]
    FiniteCylinder {
        name: String,
        center: [f32; 3],
        vector: [f32; 3],
        radius: f32,
    },
}

impl PrimitiveInput {
    pub fn name(&self) -> &str {
        match self {
            Self::Sphere { name, .. } => name,
            Self::RectangularParallelPiped { name, .. } => name,
            Self::FiniteCylinder { name, .. } => name,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CellInput {
    pub material_name: String,
    pub csg: CSGInput,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum CSGInput {
    Operation { op: String, prs: Vec<CSGInput> },
    Name(String),
}

impl WorldInput {
    pub fn build(self, material_map: &HashMap<String, u32>) -> Result<World, InputError> {
        let mut used_primitive_names = std::collections::HashSet::new();
        for c_conf in &self.cells {
            collect_used_primitives(&c_conf.csg, &mut used_primitive_names);
        }

        let mut primitive_map = HashMap::new();
        let mut primitives = Vec::new();

        for p_conf in self.primitives {
            let name = p_conf.name();
            if !used_primitive_names.contains(name) {
                eprintln!("Warning: Primitive '{}' is defined but never used.", name);
                continue;
            }

            let id = primitives.len();
            primitive_map.insert(name.to_string(), id);

            let p = match p_conf {
                PrimitiveInput::Sphere { center, radius, .. } => {
                    if radius < 0.0 {
                        return Err(InputError::InvalidPrimitive {
                            name: name.to_string(),
                            reason: format!("radius must be non-negative, got {}", radius),
                        });
                    }
                    Primitive::Sphere {
                        center: Vec3A::from_array(center),
                        radius2: radius * radius,
                    }
                }
                PrimitiveInput::RectangularParallelPiped { min, max, .. } => {
                    if min[0] > max[0] || min[1] > max[1] || min[2] > max[2] {
                        return Err(InputError::InvalidPrimitive {
                            name: name.to_string(),
                            reason: format!(
                                "min elements must be <= max elements, got min: {:?}, max: {:?}",
                                min, max
                            ),
                        });
                    }
                    Primitive::RectangularParallelPiped {
                        min: Vec3A::from_array(min),
                        max: Vec3A::from_array(max),
                    }
                }
                PrimitiveInput::FiniteCylinder {
                    center,
                    vector,
                    radius,
                    ..
                } => {
                    if radius < 0.0 {
                        return Err(InputError::InvalidPrimitive {
                            name: name.to_string(),
                            reason: format!("radius must be non-negative, got {}", radius),
                        });
                    }
                    let v = Vec3A::from_array(vector);
                    let length = v.length();
                    if length <= EPSILON {
                        return Err(InputError::InvalidPrimitive {
                            name: name.to_string(),
                            reason: format!("Invalid cylinder length: {}", length),
                        });
                    }
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
            "primitives": [
                {"name": "unused", "type": "Sphere", "center": [1.0, 1.0, 1.0], "radius": 1.0},
                {"name": "source", "type": "Sphere", "center": [0.0, 0.0, 0.0], "radius": 2.0},
                {"name": "whole_world", "type": "RectangularParallelPiped", "min": [0.0, 0.0, 0.0], "max": [1.0, 1.0, 1.0]}
            ],
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
        assert_eq!(world.cells[0].csg, CSGNode::Primitive(0));
        assert_eq!(world.cells[0].material_id, 0); // Water
        assert_eq!(world.cells[1].material_id, 1); // Air
        assert_eq!(
            world.cells[1].csg,
            CSGNode::Difference(
                Box::new(CSGNode::Primitive(1)),
                Box::new(CSGNode::Primitive(0)),
            )
        );
    }

    #[test]
    fn test_deserialize_primitive_aliases() {
        let json = r#"{
            "primitives": [
                {"name": "s", "type": "SPH", "center": [0.0, 0.0, 0.0], "radius": 1.0},
                {"name": "r1", "type": "RPP", "min": [0.0, 0.0, 0.0], "max": [1.0, 1.0, 1.0]},
                {"name": "r2", "type": "Aabb", "min": [0.0, 0.0, 0.0], "max": [1.0, 1.0, 1.0]},
                {"name": "r3", "type": "AABB", "min": [0.0, 0.0, 0.0], "max": [1.0, 1.0, 1.0]},
                {"name": "c", "type": "CYL", "center": [0.0, 0.0, 0.0], "vector": [0.0, 0.0, 1.0], "radius": 0.5}
            ],
            "cells": [
                {"material_name": "Water", "csg": "s"}
            ]
        }"#;

        let input: WorldInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.primitives.len(), 5);

        match &input.primitives[0] {
            PrimitiveInput::Sphere { name, .. } => assert_eq!(name, "s"),
            _ => panic!("Expected Sphere"),
        }
        match &input.primitives[1] {
            PrimitiveInput::RectangularParallelPiped { name, .. } => assert_eq!(name, "r1"),
            _ => panic!("Expected RectangularParallelPiped"),
        }
        match &input.primitives[2] {
            PrimitiveInput::RectangularParallelPiped { name, .. } => assert_eq!(name, "r2"),
            _ => panic!("Expected RectangularParallelPiped"),
        }
        match &input.primitives[3] {
            PrimitiveInput::RectangularParallelPiped { name, .. } => assert_eq!(name, "r3"),
            _ => panic!("Expected RectangularParallelPiped"),
        }
        match &input.primitives[4] {
            PrimitiveInput::FiniteCylinder { name, .. } => assert_eq!(name, "c"),
            _ => panic!("Expected FiniteCylinder"),
        }
    }
}
