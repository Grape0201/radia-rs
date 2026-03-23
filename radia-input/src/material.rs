use garde::Validate;
use radia_core::material::MaterialDef;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::atomic_number::{deserialize_composition, validate_composition};

#[derive(Serialize, Deserialize, Debug, Validate)]
pub struct UserDefinedMaterialInput {
    #[garde(range(min = 0.0))]
    pub density: f32,
    #[serde(deserialize_with = "deserialize_composition")]
    #[garde(custom(validate_composition))]
    pub composition: HashMap<usize, f32>,
}

impl UserDefinedMaterialInput {
    pub fn build(self) -> MaterialDef {
        MaterialDef::new(self.composition, self.density)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_material() {
        let yaml = r#"density: 1.0
composition:
  1: 0.111
  8: 0.889
        "#;

        let input: UserDefinedMaterialInput = serde_saphyr::from_str_valid(yaml).unwrap();
        let def = input.build();

        let pd = def.partial_densities();
        assert_eq!(pd[&1], 0.111);
        assert_eq!(pd[&8], 0.889);
    }

    #[test]
    fn test_invalid_density() {
        let yaml = r#"density: -1.0
composition:
  1: 1.0
        "#;
        let input: Result<UserDefinedMaterialInput, _> = serde_saphyr::from_str_valid(yaml);
        assert!(input.is_err());
    }

    #[test]
    fn test_invalid_z() {
        let yaml = r#"density: 1.0
composition:
  0: 1.0
        "#;
        let input: Result<UserDefinedMaterialInput, _> = serde_saphyr::from_str_valid(yaml);
        assert!(input.is_err());
    }
}
