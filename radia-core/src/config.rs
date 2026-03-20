use crate::constants::EPSILON;
use crate::csg::{CSGNode, Cell, World};
use crate::primitive::Primitive;
use glam::Vec3A;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum WorldConfigError {
    #[error("Material '{0}' not found")]
    MaterialNotFound(String),
    #[error("Primitive '{0}' not found")]
    PrimitiveNotFound(String),
    #[error("CSG operation has no primitives")]
    EmptyCsgOperation,
    #[error("Unknown CSG operation: {0}")]
    UnknownCsgOperation(String),
    #[error("Invalid primitive '{name}': {reason}")]
    InvalidPrimitive { name: String, reason: String },
}

#[derive(Serialize, Deserialize, Debug)]
pub struct WorldConfig {
    pub primitives: Vec<PrimitiveConfig>,
    pub materials: Vec<String>,
    pub cells: Vec<CellConfig>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
pub enum PrimitiveConfig {
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

impl PrimitiveConfig {
    pub fn name(&self) -> &str {
        match self {
            Self::Sphere { name, .. } => name,
            Self::RectangularParallelPiped { name, .. } => name,
            Self::FiniteCylinder { name, .. } => name,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CellConfig {
    pub material_name: String,
    pub csg: CSGConfig,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum CSGConfig {
    Operation { op: String, prs: Vec<CSGConfig> },
    Name(String),
}

impl WorldConfig {
    pub fn build(self) -> Result<World, WorldConfigError> {
        let mut used_primitive_names = std::collections::HashSet::new();
        for c_conf in &self.cells {
            collect_used_primitives(&c_conf.csg, &mut used_primitive_names);
        }

        let material_map: HashMap<String, u32> = self
            .materials
            .iter()
            .enumerate()
            .map(|(i, name)| (name.clone(), i as u32))
            .collect();

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
                PrimitiveConfig::Sphere { center, radius, .. } => {
                    if radius < 0.0 {
                        return Err(WorldConfigError::InvalidPrimitive {
                            name: name.to_string(),
                            reason: format!("radius must be non-negative, got {}", radius),
                        });
                    }
                    Primitive::Sphere {
                        center: Vec3A::from_array(center),
                        radius2: radius * radius,
                    }
                }
                PrimitiveConfig::RectangularParallelPiped { min, max, .. } => {
                    if min[0] > max[0] || min[1] > max[1] || min[2] > max[2] {
                        return Err(WorldConfigError::InvalidPrimitive {
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
                PrimitiveConfig::FiniteCylinder {
                    center,
                    vector,
                    radius,
                    ..
                } => {
                    if radius < 0.0 {
                        return Err(WorldConfigError::InvalidPrimitive {
                            name: name.to_string(),
                            reason: format!("radius must be non-negative, got {}", radius),
                        });
                    }
                    let v = Vec3A::from_array(vector);
                    let length = v.length();
                    if length <= EPSILON {
                        return Err(WorldConfigError::InvalidPrimitive {
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
                .ok_or_else(|| WorldConfigError::MaterialNotFound(c_conf.material_name.clone()))?;

            let csg = build_csg_node(&c_conf.csg, &primitive_map)?;
            cells.push(Cell { csg, material_id });
        }

        Ok(World { primitives, cells })
    }
}

fn collect_used_primitives(config: &CSGConfig, used: &mut std::collections::HashSet<String>) {
    match config {
        CSGConfig::Name(name) => {
            used.insert(name.clone());
        }
        CSGConfig::Operation { prs, .. } => {
            for p in prs {
                collect_used_primitives(p, used);
            }
        }
    }
}

fn build_csg_node(
    config: &CSGConfig,
    primitive_map: &HashMap<String, usize>,
) -> Result<CSGNode, WorldConfigError> {
    match config {
        CSGConfig::Name(name) => {
            let id = *primitive_map
                .get(name)
                .ok_or_else(|| WorldConfigError::PrimitiveNotFound(name.clone()))?;
            Ok(CSGNode::Primitive(id))
        }
        CSGConfig::Operation { op, prs } => {
            if prs.is_empty() {
                return Err(WorldConfigError::EmptyCsgOperation);
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
                _ => Err(WorldConfigError::UnknownCsgOperation(op.clone())),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_world_config() {
        let json = r#"{
            "primitives": [
                {"name": "unused", "type": "Sphere", "center": [1.0, 1.0, 1.0], "radius": 1.0},
                {"name": "source", "type": "Sphere", "center": [0.0, 0.0, 0.0], "radius": 2.0},
                {"name": "whole_world", "type": "RectangularParallelPiped", "min": [0.0, 0.0, 0.0], "max": [1.0, 1.0, 1.0]}
            ],
            "materials": ["Water", "Air"],
            "cells": [
                {"material_name": "Water", "csg": {"op": "inner", "prs": ["source"]}},
                {"material_name": "Air", "csg": {"op": "difference", "prs": ["whole_world", "source"]}}
            ]
        }"#;

        let config: WorldConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.primitives.len(), 3);
        assert_eq!(config.materials.len(), 2);
        assert_eq!(config.cells.len(), 2);

        let world = config.build().unwrap();
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
            "materials": ["Water"],
            "cells": [
                {"material_name": "Water", "csg": "s"}
            ]
        }"#;

        let config: WorldConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.primitives.len(), 5);
        
        match &config.primitives[0] {
            PrimitiveConfig::Sphere { name, .. } => assert_eq!(name, "s"),
            _ => panic!("Expected Sphere"),
        }
        match &config.primitives[1] {
            PrimitiveConfig::RectangularParallelPiped { name, .. } => assert_eq!(name, "r1"),
            _ => panic!("Expected RectangularParallelPiped"),
        }
        match &config.primitives[2] {
            PrimitiveConfig::RectangularParallelPiped { name, .. } => assert_eq!(name, "r2"),
            _ => panic!("Expected RectangularParallelPiped"),
        }
        match &config.primitives[3] {
            PrimitiveConfig::RectangularParallelPiped { name, .. } => assert_eq!(name, "r3"),
            _ => panic!("Expected RectangularParallelPiped"),
        }
        match &config.primitives[4] {
            PrimitiveConfig::FiniteCylinder { name, .. } => assert_eq!(name, "c"),
            _ => panic!("Expected FiniteCylinder"),
        }
    }
}
