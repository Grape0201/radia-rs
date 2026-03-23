mod atomic_number;
pub mod buildup;
mod common;
pub mod material;
pub mod source;
pub mod world;

use std::collections::HashMap;
use std::path::Path;

use garde::Validate;
use miette::Diagnostic;
use serde::Deserialize;
use thiserror::Error;

use crate::common::is_all_zero_or_more;

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
    #[error("Invalid energy group length: {0}")]
    InvalidEnergyGroupLength(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Validation(#[from] serde_saphyr::Error),
}

#[derive(Deserialize, Debug, Validate)]
pub struct DoseQuantityInput {
    #[garde(dive)]
    pub buildup_params: HashMap<String, Vec<buildup::GPParamsInput>>,
    #[garde(length(min = 1))]
    pub buildup_alias_map: std::collections::HashMap<String, String>,
    #[garde(custom(is_all_zero_or_more), length(min = 1))]
    pub energy_groups: Vec<f32>,
    #[garde(custom(is_all_zero_or_more), length(min = 1))]
    pub conversion_factors: Vec<f32>,
}

#[derive(Deserialize, Debug, Validate)]
pub struct SimulationInput {
    #[garde(dive)]
    pub world: world::WorldInput,
    #[garde(dive)]
    pub user_defined_materials: HashMap<String, material::UserDefinedMaterialInput>,
    #[garde(dive)]
    pub dose_quantity: DoseQuantityInput,
    #[garde(length(min = 1))]
    pub detectors: HashMap<String, [f32; 3]>,
    #[garde(dive)]
    pub source: source::SourceInput,
}

impl SimulationInput {
    pub fn from_yaml_file<P: AsRef<Path>>(path: P) -> Result<Self, InputError> {
        let path_ref = path.as_ref();
        let yaml_str = std::fs::read_to_string(path_ref).map_err(InputError::Io)?;

        let input: SimulationInput = serde_saphyr::from_str_valid(&yaml_str)?;
        input.validate()?;

        Ok(input)
    }

    pub fn validate(&self) -> Result<(), InputError> {
        if self.source.energy_groups.len() != self.source.intensity_by_group.len() {
            return Err(InputError::InvalidEnergyGroupLength(
                "energy_groups length and intensity_by_group length must match".to_string(),
            ));
        }
        if self.dose_quantity.energy_groups.len() != self.dose_quantity.conversion_factors.len() {
            return Err(InputError::InvalidEnergyGroupLength(
                "dose_quantity.energy_groups length and dose_quantity.conversion_factors length must match".to_string(),
            ));
        }

        Ok(())
    }

    /// Interpolates conversion factors onto the source's energy groups
    pub fn get_interpolated_conversion_factors(&self) -> Result<Vec<f32>, InputError> {
        let qs_e = &self.dose_quantity.energy_groups;
        let qs_cf = &self.dose_quantity.conversion_factors;
        let src_e = &self.source.energy_groups;

        Ok(src_e.iter().map(|&e| {
            // Find the proper interval for interpolation
            if e <= qs_e[0] {
                qs_cf[0]
            } else if e >= *qs_e.last().unwrap() {
                *qs_cf.last().unwrap()
            } else {
                let mut idx = 0;
                while idx < qs_e.len() - 1 && qs_e[idx + 1] < e {
                    idx += 1;
                }
                
                let e1 = qs_e[idx];
                let e2 = qs_e[idx + 1];
                let f1 = qs_cf[idx];
                let f2 = qs_cf[idx + 1];

                // Linear interpolation: f(e) = f1 + (e - e1) * (f2 - f1) / (e2 - e1)
                f1 + (e - e1) * (f2 - f1) / (e2 - e1)
            }
        }).collect())
    }
}
