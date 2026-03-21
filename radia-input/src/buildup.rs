use radia_core::physics::GPParams;
use serde::{Deserialize, Serialize};

use crate::InputError;

#[derive(Serialize, Deserialize, Debug)]
pub struct GPParamsInput {
    pub energy_mev: f32,
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub xk: f32,
}

impl From<GPParamsInput> for GPParams {
    fn from(val: GPParamsInput) -> Self {
        GPParams {
            energy_mev: val.energy_mev,
            a: val.a,
            b: val.b,
            c: val.c,
            d: val.d,
            xk: val.xk,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BuildupInput {
    pub material_name: String,
    pub params: Vec<GPParamsInput>,
}

impl BuildupInput {
    pub fn build(mut self) -> Result<(String, Vec<GPParams>), InputError> {
        if self.params.is_empty() {
            return Err(InputError::InvalidBuildup {
                name: self.material_name,
                reason: "Buildup parameters list cannot be empty".to_string(),
            });
        }

        // Validate energies are positive and sort them
        for p in &self.params {
            if p.energy_mev <= 0.0 {
                return Err(InputError::InvalidBuildup {
                    name: self.material_name.clone(),
                    reason: format!("Energy must be positive, got {}", p.energy_mev),
                });
            }
        }

        self.params
            .sort_by(|a, b| a.energy_mev.partial_cmp(&b.energy_mev).unwrap());

        let gp_params = self.params.into_iter().map(|p| p.into()).collect();

        Ok((self.material_name, gp_params))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_buildup() {
        let json = r#"{
            "material_name": "Water",
            "params": [
                {"energy_mev": 1.0, "a": 0.1, "b": 1.0, "c": 0.5, "d": 0.05, "xk": 10.0},
                {"energy_mev": 0.5, "a": 0.1, "b": 1.0, "c": 0.5, "d": 0.05, "xk": 10.0}
            ]
        }"#;

        let input: BuildupInput = serde_json::from_str(json).unwrap();
        let (name, params) = input.build().unwrap();

        assert_eq!(name, "Water");
        assert_eq!(params.len(), 2);
        // It should be sorted by energy, so 0.5 MeV first
        assert_eq!(params[0].energy_mev, 0.5);
        assert_eq!(params[1].energy_mev, 1.0);
    }
}
