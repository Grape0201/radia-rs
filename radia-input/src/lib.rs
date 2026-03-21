pub mod buildup;
pub mod detector;
pub mod material;
pub mod source;
pub mod world;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum InputError {
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
    #[error("Invalid material '{name}': {reason}")]
    InvalidMaterial { name: String, reason: String },
    #[error("Invalid source: {0}")]
    InvalidSource(String),
    #[error("Invalid buildup parameter for material '{name}': {reason}")]
    InvalidBuildup { name: String, reason: String },
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SimulationInput {
    pub world: world::WorldInput,
    #[serde(default)]
    pub materials: Vec<material::MaterialInput>,
    #[serde(default)]
    pub buildup_params: Vec<buildup::BuildupInput>,
    #[serde(default)]
    pub buildup_alias_map: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub detectors: Vec<detector::DetectorInput>,
    #[serde(default)]
    pub sources: Vec<source::SourceInput>,
}
