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
    #[error("Validation error: {0}")]
    ValidationError(String),
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
        
        input.validate()?;
        
        Ok(input)
    }

    pub fn validate(&self) -> Result<(), InputError> {
        if self.sources.is_empty() {
            return Err(InputError::ValidationError("At least one source must be defined".to_string()));
        }
        if self.detectors.is_empty() {
            return Err(InputError::ValidationError("At least one detector must be defined".to_string()));
        }

        let mut mat_names = std::collections::HashSet::new();
        for mat in &self.materials {
            if !mat_names.insert(&mat.name) {
                return Err(InputError::ValidationError(format!("Duplicate material definition: '{}'", mat.name)));
            }
        }

        let mut buildup_names = std::collections::HashSet::new();
        for bp in &self.buildup_params {
            if !buildup_names.insert(&bp.material_name) {
                return Err(InputError::ValidationError(format!("Duplicate buildup parameters definition for material: '{}'", bp.material_name)));
            }
        }

        let mut det_names = std::collections::HashSet::new();
        for det in &self.detectors {
            let name = &det.name;
            if !det_names.insert(name.clone()) {
                return Err(InputError::ValidationError(format!("Duplicate detector definition: '{}'", name)));
            }
        }

        for cell in &self.world.cells {
            if !self.buildup_alias_map.contains_key(&cell.material_name) {
                return Err(InputError::InvalidMaterial {
                    name: cell.material_name.clone(),
                    reason: "Missing from buildup_alias_map. Must map used materials to a valid buildup parameter name.".to_string(),
                });
            }
        }

        for src in &self.sources {
            if src.energy_groups.is_empty() {
                return Err(InputError::InvalidSource("energy_groups cannot be empty".to_string()));
            }
            if src.energy_groups.len() != src.intensity_by_group.len() {
                return Err(InputError::InvalidSource(
                    "energy_groups length and intensity_by_group length must match".to_string()
                ));
            }
            if src.intensity_by_group.iter().any(|&i| i < 0.0) {
                return Err(InputError::InvalidSource(
                    "intensity_by_group elements cannot be negative".to_string()
                ));
            }
            if !self.conversion_factors.is_empty() && self.conversion_factors.len() != src.energy_groups.len() {
                return Err(InputError::ValidationError(
                    "conversion_factors length must match sources energy_groups length".to_string()
                ));
            }
        }

        Ok(())
    }
}
