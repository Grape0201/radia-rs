pub mod buildup;
mod common;
pub mod detector;
pub mod material;
pub mod source;
pub mod world;

use std::path::Path;

use garde::Validate;
use miette::Diagnostic;
use serde::Deserialize;
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
    Yaml { message: String },
}

#[derive(Deserialize, Debug, Validate)]
pub struct SimulationInput {
    #[garde(dive)]
    pub world: world::WorldInput,
    #[garde(dive)]
    pub materials: Vec<material::MaterialInput>,
    #[garde(dive)]
    pub buildup_params: Vec<buildup::BuildupInput>,
    #[garde(length(min = 1))]
    pub buildup_alias_map: std::collections::HashMap<String, String>,
    #[garde(dive)]
    pub detectors: Vec<detector::DetectorInput>,
    #[garde(skip)]
    pub conversion_factors: Vec<f32>,
    #[garde(dive)]
    pub source: source::SourceInput,
}

impl SimulationInput {
    pub fn from_yaml_file<P: AsRef<Path>>(path: P) -> Result<Self, InputError> {
        let path_ref = path.as_ref();
        let yaml_str = std::fs::read_to_string(path_ref).map_err(InputError::Io)?;

        let input: SimulationInput =
            serde_saphyr::from_str_valid(&yaml_str).map_err(|e| InputError::Yaml {
                message: e.to_string(),
            })?;
        input.validate()?;

        Ok(input)
    }

    pub fn validate(&self) -> Result<(), InputError> {
        if self.detectors.is_empty() {
            return Err(InputError::ValidationError(
                "At least one detector must be defined".to_string(),
            ));
        }

        let mut mat_names = std::collections::HashSet::new();
        for mat in &self.materials {
            if !mat_names.insert(&mat.name) {
                return Err(InputError::ValidationError(format!(
                    "Duplicate material definition: '{}'",
                    mat.name
                )));
            }
        }

        let mut buildup_names = std::collections::HashSet::new();
        for bp in &self.buildup_params {
            if !buildup_names.insert(&bp.material_name) {
                return Err(InputError::ValidationError(format!(
                    "Duplicate buildup parameters definition for material: '{}'",
                    bp.material_name
                )));
            }
        }

        let mut det_names = std::collections::HashSet::new();
        for det in &self.detectors {
            let name = &det.name;
            if !det_names.insert(name.clone()) {
                return Err(InputError::ValidationError(format!(
                    "Duplicate detector definition: '{}'",
                    name
                )));
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

        if self.source.energy_groups.is_empty() {
            return Err(InputError::InvalidSource(
                "energy_groups cannot be empty".to_string(),
            ));
        }
        if self.source.energy_groups.len() != self.source.intensity_by_group.len() {
            return Err(InputError::InvalidSource(
                "energy_groups length and intensity_by_group length must match".to_string(),
            ));
        }
        if self.source.intensity_by_group.iter().any(|&i| i < 0.0) {
            return Err(InputError::InvalidSource(
                "intensity_by_group elements cannot be negative".to_string(),
            ));
        }
        if !self.conversion_factors.is_empty()
            && self.conversion_factors.len() != self.source.energy_groups.len()
        {
            return Err(InputError::ValidationError(
                "conversion_factors length must match sources energy_groups length".to_string(),
            ));
        }

        Ok(())
    }
}
