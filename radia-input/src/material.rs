use garde::Validate;
use radia_core::material::MaterialDef;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::InputError;

#[derive(Serialize, Deserialize, Debug, Validate)]
pub struct MaterialInput {
    #[garde(length(min = 2))]
    pub name: String,
    #[garde(range(min = 0.0))]
    pub density: f32,
    #[garde(skip)]
    pub composition: HashMap<u32, f32>,
}

impl MaterialInput {
    pub fn build(self) -> Result<(String, MaterialDef), InputError> {
        let mut sum = 0.0;
        for (&z, &f) in &self.composition {
            if z == 0 || z > 118 {
                return Err(InputError::InvalidMaterial {
                    name: self.name.clone(),
                    reason: format!("Invalid atomic number {}", z),
                });
            }
            sum += f;
        }

        if (sum - 1.0).abs() > 0.05 {
            eprintln!(
                "Warning: Weight fractions for material '{}' sum to {}, expected close to 1.0.",
                self.name, sum
            );
        }

        Ok((self.name, MaterialDef::new(self.composition, self.density)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_material() {
        let json = r#"{
            "name": "Water",
            "density": 1.0,
            "composition": {
                "1": 0.111,
                "8": 0.889
            }
        }"#;

        let input: MaterialInput = serde_json::from_str(json).unwrap();
        let (name, def) = input.build().unwrap();

        assert_eq!(name, "Water");
        let pd = def.partial_densities();
        assert_eq!(pd[&1], 0.111);
        assert_eq!(pd[&8], 0.889);
    }

    #[test]
    fn test_invalid_density() {
        let yaml = r#"name: Bad
density: -1.0
composition:
  1: 1.0
        "#;
        let input: Result<MaterialInput, _> = serde_saphyr::from_str_valid(yaml);
        assert!(input.is_err());
    }

    #[test]
    fn test_invalid_z() {
        let json = r#"{
            "name": "BadZ",
            "density": 1.0,
            "composition": {"0": 1.0}
        }"#;
        let input: MaterialInput = serde_json::from_str(json).unwrap();
        assert!(input.build().is_err());
    }
}
