use garde::Validate;
use radia_core::physics::GPParams;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Validate)]
pub struct GPParamsInput {
    #[garde(range(min = 0.0))]
    pub energy_mev: f32,
    #[garde(skip)]
    pub a: f32,
    #[garde(skip)]
    pub b: f32,
    #[garde(skip)]
    pub c: f32,
    #[garde(skip)]
    pub d: f32,
    #[garde(skip)]
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

#[derive(Serialize, Deserialize, Debug, Validate)]
pub struct BuildupInput {
    #[garde(length(min = 2))]
    pub material_name: String,
    #[garde(length(min = 1), dive)]
    pub params: Vec<GPParamsInput>,
}

impl BuildupInput {
    pub fn build(mut self) -> (String, Vec<GPParams>) {
        // sort by energy
        self.params
            .sort_by(|a, b| a.energy_mev.partial_cmp(&b.energy_mev).unwrap());

        let gp_params = self.params.into_iter().map(|p| p.into()).collect();

        (self.material_name, gp_params)
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
        let (name, params) = input.build();

        assert_eq!(name, "Water");
        assert_eq!(params.len(), 2);
        // It should be sorted by energy, so 0.5 MeV first
        assert_eq!(params[0].energy_mev, 0.5);
        assert_eq!(params[1].energy_mev, 1.0);
    }

    #[test]
    fn test_validate_with_garde() {
        let yaml = r#"material_name: Water
params:
  - energy_mev: 1.0
    a: 0.1
    b: 1.0
    c: 0.5
    d: 0.05
    xk: 10.0
  - energy_mev: 0.5
    a: 0.1
    b: 1.0
    c: 0.5
    d: 0.05
    xk: 10.0
"#;

        let input: Result<BuildupInput, _> = serde_saphyr::from_str_valid(yaml);
        assert!(input.is_ok());

        // empty params
        let yaml = r#"material_name: Water
params:
"#;
        let input: Result<BuildupInput, _> = serde_saphyr::from_str_valid(yaml);
        assert!(input.is_err());

        // negative energy
        let yaml = r#"material_name: Water
params:
  - energy_mev: -1.0
    a: 0.1
    b: 1.0
    c: 0.5
    d: 0.05
    xk: 10.0
  - energy_mev: 0.5
    a: 0.1
    b: 1.0
    c: 0.5
    d: 0.05
    xk: 10.0
"#;

        let input: Result<BuildupInput, _> = serde_saphyr::from_str_valid(yaml);
        assert!(input.is_err());
    }
}
