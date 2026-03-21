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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_buildup() {
        let json = r#"[
                {"energy_mev": 1.0, "a": 0.1, "b": 1.0, "c": 0.5, "d": 0.05, "xk": 10.0},
                {"energy_mev": 0.5, "a": 0.1, "b": 1.0, "c": 0.5, "d": 0.05, "xk": 10.0}
            ]"#;

        let input: Vec<GPParamsInput> = serde_json::from_str(json).unwrap();
        assert_eq!(input.len(), 2);
    }

    #[test]
    fn test_validate_with_garde() {
        let yaml = r#"
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

        let input: Result<Vec<GPParamsInput>, _> = serde_saphyr::from_str_valid(yaml);
        assert!(input.is_ok());

        // negative energy
        let yaml = r#"
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

        let input: Result<Vec<GPParamsInput>, _> = serde_saphyr::from_str_valid(yaml);
        assert!(input.is_err());
    }
}
