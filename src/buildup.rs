/// Error type for buildup factor calculations and data management
#[derive(thiserror::Error, Debug)]
pub enum BuildupError {
    #[error("Material '{0}' not found in buildup data")]
    MaterialNotFound(String),

    #[error("Target quantity '{0:?}' not found for material '{1}'")]
    QuantityNotFound(TargetQuantity, String),

    #[error("Target energy {target} MeV is too low (minimum {min} MeV) for material '{material}'")]
    EnergyTooLow {
        target: f64,
        min: f64,
        material: String,
    },

    #[error("Target energy {target} MeV is too high (maximum {max} MeV) for material '{material}'")]
    EnergyTooHigh {
        target: f64,
        max: f64,
        material: String,
    },

    #[error("Buildup data for '{0}' is empty")]
    EmptyData(String),

    #[error("Invalid table size: expected {expected}, got {actual}")]
    InvalidTableSize { expected: usize, actual: usize },
}

/// Model representing the buildup factor and its required parameters
#[derive(Clone, Copy, Debug)]
pub enum BuildupModel {
    /// Ignores scattering, or useful for testing with a fixed value (usually 1.0)
    Constant(f64),

    /// Taylor form (A, alpha1, alpha2)
    Taylor { a: f64, alpha1: f64, alpha2: f64 },

    /// Berger form (C, D)
    Berger { c: f64, d: f64 },

    /// Geometric Progression (G-P) form.
    /// Has a wide application range (up to 40 mfp) and is the current standard (ANSI/ANS-6.4.3, etc.)
    GeometricProgression {
        a: f64,
        b: f64,
        c: f64,
        d: f64,
        xk: f64,
    },
}

impl BuildupModel {
    /// Calculates the buildup factor given the actual optical thickness (mu * r)
    #[inline(always)]
    pub fn calculate(&self, optical_thickness: f64) -> f64 {
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
                if x <= 1e-6 {
                    return 1.0;
                }

                // Calculate K(x) for the G-P formula
                let k_x = if (*xk - 1.0).abs() < 1e-4 {
                    // Approximation when K is close to constant 1
                    // Generally calculated using parameters at x=1
                    *c * x.powf(*a) + *d * x.exp() + *b
                } else {
                    // Standard formula for K(X) in G-P method
                    *c * x.powf(*a) + *d * (((x / *xk).tanh() - 1.0) / (1.0 - *xk)) * x.exp() // Example adjusted standard
                };

                // Actual buildup factor B(x)
                if (k_x - 1.0).abs() < 1e-6 {
                    1.0 + (*b - 1.0) * x
                } else {
                    1.0 + (*b - 1.0) * (k_x.powf(x) - 1.0) / (k_x - 1.0)
                }
            }
        }
    }
}

/// A table for looking up the corresponding buildup factor model
/// from material index and energy group index.
#[derive(Clone, Debug)]
pub struct BuildupTable {
    /// Stored as a 1D array: models[material_index * num_groups + group_index]
    models: Vec<BuildupModel>,
    num_materials: usize,
    num_groups: usize,
}

impl BuildupTable {
    /// Creates a table from pre-calculated models.
    pub fn new(
        models: Vec<BuildupModel>,
        num_materials: usize,
        num_groups: usize,
    ) -> Result<Self, BuildupError> {
        if models.len() != num_materials * num_groups {
            return Err(BuildupError::InvalidTableSize {
                expected: num_materials * num_groups,
                actual: models.len(),
            });
        }
        Ok(Self {
            models,
            num_materials,
            num_groups,
        })
    }

    /// Derives the buildup factor from material ID, group ID, and optical thickness
    #[inline(always)]
    pub fn get_buildup(
        &self,
        material_index: usize,
        group_index: usize,
        optical_thickness: f64,
    ) -> f64 {
        debug_assert!(material_index < self.num_materials);
        debug_assert!(group_index < self.num_groups);
        let model = &self.models[material_index * self.num_groups + group_index];
        model.calculate(optical_thickness)
    }

    /// Creates a closure that looks up the model in O(1) time and returns its calculation result.
    /// Signature: (material_index, group_index, optical_thickness) -> buildup_factor
    pub fn into_closure(self) -> impl Fn(usize, usize, f64) -> f64 + Send + Sync {
        let models = self.models;
        let num_groups = self.num_groups;
        move |mat_idx, grp_idx, optical_thickness| {
            models[mat_idx * num_groups + grp_idx].calculate(optical_thickness)
        }
    }
}

// ==== Dummy provider implementation for examples / tests ====

/// Provides default Constant(1.0) for testing
pub struct DummyBuildupProvider;

impl DummyBuildupProvider {
    pub fn generate_constant_table(num_materials: usize, num_groups: usize) -> BuildupTable {
        let models = vec![BuildupModel::Constant(1.0); num_materials * num_groups];
        BuildupTable::new(models, num_materials, num_groups).expect("Dummy table generation failed")
    }
}

// ==== G-P Method Provider with Interpolation ====

/// The physical quantity the buildup factor corresponds to.
/// Different target quantities require different G-P parameters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TargetQuantity {
    /// Exposure (Air Kerma)
    Exposure,
    /// Ambient Dose Equivalent (e.g. 1cm dose)
    AmbientDoseEquivalent,
    /// Effective Dose Equivalent
    EffectiveDoseEquivalent,
    /// Energy Absorption (in the material itself)
    EnergyAbsorption,
}

/// Parameter set for the G-P method at a specific energy and target quantity.
#[derive(Clone, Copy, Debug)]
pub struct GPParams {
    pub energy_mev: f64,
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub xk: f64,
}

/// A provider that holds hardcoded G-P data and interpolates to an arbitrary energy grid.
pub struct GPBuildupProvider {
    /// Mapping of (Material identifier, TargetQuantity) to their energy-dependent G-P parameters.
    /// Data is assumed to be sorted by energy ascending.
    data: std::collections::HashMap<(String, TargetQuantity), Vec<GPParams>>,
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
    pub fn insert_data(
        &mut self,
        material_name: String,
        quantity: TargetQuantity,
        mut params: Vec<GPParams>,
    ) {
        params.sort_by(|a, b| a.energy_mev.partial_cmp(&b.energy_mev).unwrap());
        self.data.insert((material_name, quantity), params);
    }

    /// Returns a list of target quantities supported for the given material.
    pub fn get_available_quantities(&self, material_name: &str) -> Vec<TargetQuantity> {
        self.data
            .keys()
            .filter(|(m, _)| m == material_name)
            .map(|(_, q)| *q)
            .collect()
    }

    /// Linear interpolation with respect to log(E).
    /// This is common practice for buildup factor parameters.
    pub fn interpolate(
        &self,
        material_name: &str,
        quantity: TargetQuantity,
        target_energy: f64,
    ) -> Result<BuildupModel, BuildupError> {
        let params_list = self
            .data
            .get(&(material_name.to_string(), quantity))
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
        if (target_energy - min_e).abs() < 1e-9 {
            let p = params_list.first().unwrap();
            return Ok(BuildupModel::GeometricProgression {
                a: p.a,
                b: p.b,
                c: p.c,
                d: p.d,
                xk: p.xk,
            });
        }
        if (target_energy - max_e).abs() < 1e-9 {
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
                let lerp = |v1: f64, v2: f64| v1 + weight * (v2 - v1);

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

    /// Generates a BuildupTable for the specified material names, target quantity, and energy grid.
    pub fn generate_table(
        &self,
        material_names: &[&str],
        quantity: TargetQuantity,
        energy_groups: &[f64],
    ) -> Result<BuildupTable, BuildupError> {
        let num_materials = material_names.len();
        let num_groups = energy_groups.len();
        let mut models = Vec::with_capacity(num_materials * num_groups);

        for &mat in material_names {
            for &energy in energy_groups {
                let model = self.interpolate(mat, quantity, energy)?;
                models.push(model);
            }
        }

        BuildupTable::new(models, num_materials, num_groups)
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
    fn test_buildup_closure() {
        let table = DummyBuildupProvider::generate_constant_table(2, 3);
        let bpf = table.into_closure();
        // material 0, group 1, optical thickness 2.5
        assert_eq!(bpf(0, 1, 2.5), 1.0);
    }

    #[test]
    fn test_gp_interpolation() {
        let mut provider = GPBuildupProvider::new();
        let dummy_data = vec![
            GPParams {
                energy_mev: 0.5,
                a: 0.1,
                b: 2.0,
                c: 0.5,
                d: 0.05,
                xk: 14.0,
            },
            GPParams {
                energy_mev: 1.0,
                a: 0.12,
                b: 2.1,
                c: 0.53,
                d: 0.04,
                xk: 14.4,
            },
            GPParams {
                energy_mev: 10.0,
                a: 0.2,
                b: 1.3,
                c: 0.9,
                d: 0.01,
                xk: 13.5,
            },
        ];
        provider.insert_data(
            "DummyMaterial".to_string(),
            TargetQuantity::Exposure,
            dummy_data.clone(),
        );
        provider.insert_data(
            "DummyMaterial".to_string(),
            TargetQuantity::AmbientDoseEquivalent,
            dummy_data,
        );

        let quantities = provider.get_available_quantities("DummyMaterial");
        assert!(quantities.contains(&TargetQuantity::Exposure));
        assert!(quantities.contains(&TargetQuantity::AmbientDoseEquivalent));

        // Exact match
        let model_1mev = provider
            .interpolate("DummyMaterial", TargetQuantity::Exposure, 1.0)
            .unwrap();
        if let BuildupModel::GeometricProgression { a, .. } = model_1mev {
            assert!((a - 0.12).abs() < 1e-5);
        } else {
            panic!("Expected GP model");
        }

        // Interpolated (e.g. at 1.414 MeV)
        let model_interp = provider
            .interpolate("DummyMaterial", TargetQuantity::Exposure, 1.414)
            .unwrap();
        if let BuildupModel::GeometricProgression { a, .. } = model_interp {
            // Should be between 1.0 MeV (a=0.12) and 10.0 MeV (a=0.2)
            assert!(a > 0.12 && a < 0.2);
        } else {
            panic!("Expected GP model");
        }

        // Out of range (Extrapolation) should now be an error
        let err_low = provider.interpolate("DummyMaterial", TargetQuantity::Exposure, 0.1);
        assert!(matches!(err_low, Err(BuildupError::EnergyTooLow { .. })));

        let err_high = provider.interpolate("DummyMaterial", TargetQuantity::Exposure, 20.0);
        assert!(matches!(err_high, Err(BuildupError::EnergyTooHigh { .. })));

        // Test Table Generation
        let table = provider
            .generate_table(&["DummyMaterial"], TargetQuantity::Exposure, &[1.0, 2.0])
            .unwrap();
        // DummyMaterial (material 0), 2.0 MeV (group 1)
        let b = table.get_buildup(0, 1, 5.0); // B(x) at x=5.0
        assert!(b > 1.0); // Should be >> 1.0 due to buildup
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
        provider.insert_data(
            "UnsortedMaterial".to_string(),
            TargetQuantity::Exposure,
            unsorted_data,
        );

        // Interpolation at 2.0 MeV should work if it was sorted correctly
        let result = provider.interpolate("UnsortedMaterial", TargetQuantity::Exposure, 2.0);
        assert!(result.is_ok());
        if let Ok(BuildupModel::GeometricProgression { a, .. }) = result {
            // a should be between 0.12 (at 1MeV) and 0.2 (at 10MeV)
            assert!(a > 0.12 && a < 0.2);
        } else {
            panic!("Expected GP model, got {:?}", result);
        }
    }
}
