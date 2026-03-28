use crate::constants::{E_EPSILON, O_EPSILON, T_EPSILON};

/// Error type for buildup factor calculations and data management
#[derive(thiserror::Error, Debug)]
pub enum BuildupError {
    #[error("Material '{0}' not found in buildup data")]
    MaterialNotFound(String),

    #[error("Target energy {target} MeV is too low (minimum {min} MeV) for material '{material}'")]
    EnergyTooLow {
        target: f32,
        min: f32,
        material: String,
    },

    #[error("Target energy {target} MeV is too high (maximum {max} MeV) for material '{material}'")]
    EnergyTooHigh {
        target: f32,
        max: f32,
        material: String,
    },

    #[error("Buildup data for '{0}' is empty")]
    EmptyData(String),

    #[error("Invalid table size: expected {expected}, got {actual}")]
    InvalidTableSize { expected: usize, actual: usize },

    #[error("Material '{0}' missing buildup source")]
    MissingBuildupSource(String),

    #[error("{0}")]
    Other(String),
}

/// Model representing the buildup factor and its required parameters
#[derive(Clone, Copy, Debug)]
pub enum BuildupModel {
    /// Ignores scattering, or useful for testing with a fixed value (usually 1.0)
    Constant(f32),

    /// Taylor form (A, alpha1, alpha2)
    Taylor { a: f32, alpha1: f32, alpha2: f32 },

    /// Berger form (C, D)
    Berger { c: f32, d: f32 },

    /// Geometric Progression (G-P) form.
    /// Has a wide application range (up to 40 mfp) and is the current standard (ANSI/ANS-6.4.3, etc.)
    GeometricProgression {
        a: f32,
        b: f32,
        c: f32,
        d: f32,
        xk: f32,
    },
}

impl BuildupModel {
    /// Calculates the buildup factor given the actual optical thickness (mu * r)
    #[inline(always)]
    pub(crate) fn calculate(&self, optical_thickness: f32) -> f32 {
        match self {
            BuildupModel::Constant(val) => *val,
            BuildupModel::Taylor { a, alpha1, alpha2 } => {
                let x = optical_thickness;
                a * (-alpha1 * x).exp() + (1.0 - a) * (-alpha2 * x).exp()
            }
            BuildupModel::Berger { c, d } => {
                let x = optical_thickness;
                1.0 + c * x * (d * x).exp()
            }
            BuildupModel::GeometricProgression { a, b, c, d, xk } => {
                let x = optical_thickness;
                // Prevent division by zero when optical thickness x is near 0
                if x <= O_EPSILON {
                    return 1.0;
                }

                // Calculate K(x) for the G-P formula
                let k_x = if (*xk - 1.0).abs() < T_EPSILON {
                    // Approximation when K is close to constant 1
                    // Generally calculated using parameters at x=1
                    *c * x.powf(*a) + *d * x.exp() + *b
                } else {
                    // Standard formula for K(X) in G-P method
                    *c * x.powf(*a) + *d * (((x / *xk).tanh() - 1.0) / (1.0 - *xk)) * x.exp() // Example adjusted standard
                };

                // Actual buildup factor B(x)
                if (k_x - 1.0).abs() < T_EPSILON {
                    1.0 + (*b - 1.0) * x
                } else {
                    1.0 + (*b - 1.0) * (k_x.powf(x) - 1.0) / (k_x - 1.0)
                }
            }
        }
    }
}

// ==== G-P Method Provider with Interpolation ====
/// Parameter set for the G-P method at a specific energy and target quantity.
#[derive(Clone, Copy, Debug)]
pub struct GPParams {
    pub energy_mev: f32,
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub xk: f32,
}

/// A provider that holds hardcoded G-P data and interpolates to an arbitrary energy grid.
pub struct GPBuildupProvider {
    /// Mapping of (Material identifier, TargetQuantity) to their energy-dependent G-P parameters.
    /// Data is assumed to be sorted by energy ascending.
    data: std::collections::HashMap<String, Vec<GPParams>>,
}

impl Default for GPBuildupProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl GPBuildupProvider {
    /// Creates a new provider.
    pub fn new() -> Self {
        Self {
            data: std::collections::HashMap::new(),
        }
    }

    /// Programmatically inserts G-P parameters for a material and quantity.
    /// Data is automatically sorted by energy.
    pub fn insert_data(&mut self, material_name: String, mut params: Vec<GPParams>) {
        params.sort_by(|a, b| a.energy_mev.partial_cmp(&b.energy_mev).unwrap());
        self.data.insert(material_name, params);
    }

    /// Linear interpolation with respect to log(E).
    /// This is common practice for buildup factor parameters.
    pub(crate) fn interpolate(
        &self,
        material_name: &str,
        target_energy: f32,
    ) -> Result<BuildupModel, BuildupError> {
        let params_list = self
            .data
            .get(material_name)
            .ok_or_else(|| BuildupError::MaterialNotFound(material_name.to_string()))?;

        if params_list.is_empty() {
            return Err(BuildupError::EmptyData(material_name.to_string()));
        }

        // Extrapolation is not allowed to ensure accuracy
        let min_e = params_list.first().unwrap().energy_mev;
        if target_energy < min_e {
            return Err(BuildupError::EnergyTooLow {
                target: target_energy,
                min: min_e,
                material: material_name.to_string(),
            });
        }

        let max_e = params_list.last().unwrap().energy_mev;
        if target_energy > max_e {
            return Err(BuildupError::EnergyTooHigh {
                target: target_energy,
                max: max_e,
                material: material_name.to_string(),
            });
        }

        // Exact match for boundaries (or very close)
        if (target_energy - min_e).abs() < E_EPSILON {
            let p = params_list.first().unwrap();
            return Ok(BuildupModel::GeometricProgression {
                a: p.a,
                b: p.b,
                c: p.c,
                d: p.d,
                xk: p.xk,
            });
        }
        if (target_energy - max_e).abs() < E_EPSILON {
            let p = params_list.last().unwrap();
            return Ok(BuildupModel::GeometricProgression {
                a: p.a,
                b: p.b,
                c: p.c,
                d: p.d,
                xk: p.xk,
            });
        }

        // Interpolation
        for i in 0..(params_list.len() - 1) {
            let p1 = &params_list[i];
            let p2 = &params_list[i + 1];

            if target_energy >= p1.energy_mev && target_energy <= p2.energy_mev {
                // Log-linear interpolation weight
                let log_e1 = p1.energy_mev.ln();
                let log_e2 = p2.energy_mev.ln();
                let log_e = target_energy.ln();

                let weight = (log_e - log_e1) / (log_e2 - log_e1);

                // Helper for linear interpolation
                let lerp = |v1: f32, v2: f32| v1 + weight * (v2 - v1);

                return Ok(BuildupModel::GeometricProgression {
                    a: lerp(p1.a, p2.a),
                    b: lerp(p1.b, p2.b),
                    c: lerp(p1.c, p2.c),
                    d: lerp(p1.d, p2.d),
                    xk: lerp(p1.xk, p2.xk),
                });
            }
        }

        // This path should technically not be reached if range checks pass
        Err(BuildupError::EmptyData(material_name.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buildup_constant() {
        let model = BuildupModel::Constant(1.0);
        assert_eq!(model.calculate(5.0), 1.0);
    }
    #[test]
    fn test_gp_unsorted_insertion() {
        let mut provider = GPBuildupProvider::new();
        // Provide data in reverse order of energy
        let unsorted_data = vec![
            GPParams {
                energy_mev: 10.0,
                a: 0.2,
                b: 1.3,
                c: 0.9,
                d: 0.01,
                xk: 13.5,
            },
            GPParams {
                energy_mev: 1.0,
                a: 0.12,
                b: 2.1,
                c: 0.53,
                d: 0.04,
                xk: 14.4,
            },
        ];
        provider.insert_data("UnsortedMaterial".to_string(), unsorted_data);

        // Interpolation at 2.0 MeV should work if it was sorted correctly
        let result = provider.interpolate("UnsortedMaterial", 2.0);
        assert!(result.is_ok());
        if let Ok(BuildupModel::GeometricProgression { a, .. }) = result {
            // a should be between 0.12 (at 1MeV) and 0.2 (at 10MeV)
            assert!(a > 0.12 && a < 0.2);
        } else {
            panic!("Expected GP model, got {:?}", result);
        }
    }
}
