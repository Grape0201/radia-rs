pub mod buildup;
pub mod detector;
pub mod material;
pub mod source;
pub mod world;

use std::path::Path;

use miette::{Diagnostic, NamedSource, SourceSpan};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Diagnostic, Debug)]
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
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("YAML error: {message}")]
    #[diagnostic(code(radia_input::yaml_error))]
    Yaml {
        message: String,
        #[source_code]
        src: NamedSource<String>,
        #[label("here")]
        span: Option<SourceSpan>,
    },
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
    pub conversion_factors: Vec<f32>,
    #[serde(default)]
    pub sources: Vec<source::SourceInput>,
}

impl SimulationInput {
    pub fn from_yaml_file<P: AsRef<Path>>(path: P) -> Result<Self, InputError> {
        let path_ref = path.as_ref();
        let yaml_str = std::fs::read_to_string(path_ref).map_err(InputError::Io)?;
        let filename = path_ref.to_string_lossy().into_owned();

        let input: SimulationInput = serde_yaml::from_str(&yaml_str).map_err(|e| {
            let span = e
                .location()
                .map(|loc| SourceSpan::new(loc.index().into(), 0));
            InputError::Yaml {
                message: e.to_string(),
                src: NamedSource::new(filename, yaml_str),
                span,
            }
        })?;
        Ok(input)
    }
}
