use crate::constants::{E_EPSILON, O_EPSILON, T_EPSILON};
use crate::material::{GroupIndex, MassAttenuationProvider, MaterialIndex, MaterialRegistry};

/// Error type for buildup factor calculations and data management
#[derive(thiserror::Error, Debug)]
pub enum BuildupError {
    #[error("Material '{0}' not found in buildup data")]
    MaterialNotFound(String),

    #[error("Target quantity '{0:?}' not found for material '{1}'")]
    QuantityNotFound(TargetQuantity, String),

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

/// Error type for MaterialPhysicsTable operations
#[derive(thiserror::Error, Debug)]
pub enum MaterialPhysicsError {
    #[error("Material '{0}' not found in registry")]
    MaterialDataNotInRegistry(String),

    #[error(transparent)]
    Material(#[from] crate::material::MaterialError),

    #[error(transparent)]
    Buildup(#[from] BuildupError),
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
    pub fn calculate(&self, optical_thickness: f32) -> f32 {
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

/// A unified table for looking up both macroscopic cross sections (mu) and buildup factor models.
/// This ensures consistent indexing for both physical properties.
#[derive(Clone, Debug)]
pub struct MaterialPhysicsTable {
    /// Macroscopic cross sections [cm^-1] stored in (material_index, group_index) order.
    mu_data: Vec<f32>,
    /// Buildup factor models stored in (material_index, group_index) order.
    buildup_models: Vec<BuildupModel>,
    num_materials: usize,
    num_groups: usize,
}

impl MaterialPhysicsTable {
    /// Generates a physics table by pre-calculating linear attenuation coefficients
    /// and buildup models for the given materials and energy groups.
    pub fn generate(
        material_names: &[String],
        registry: &MaterialRegistry,
        energy_groups: &[f32],
        mu_provider: &impl MassAttenuationProvider,
        buildup_provider: &GPBuildupProvider,
        quantity: TargetQuantity,
    ) -> Result<Self, MaterialPhysicsError> {
        let num_materials = material_names.len();
        let num_groups = energy_groups.len();

        let mut mu_data = Vec::with_capacity(num_materials * num_groups);
        let mut buildup_models = Vec::with_capacity(num_materials * num_groups);

        for name in material_names {
            let mat = registry
                .get_material(name)
                .ok_or_else(|| MaterialPhysicsError::MaterialDataNotInRegistry(name.clone()))?;

            // 1. Calculate macroscopic cross sections (mu)
            for &energy in energy_groups {
                let mut mu = 0.0;
                for (&z, &fraction) in &mat.composition {
                    let mass_att = mu_provider.get_mass_attenuation(z, energy)?;
                    mu += mass_att * fraction * mat.density;
                }
                mu_data.push(mu);
            }

            // 2. Interpolate buildup models
            let buildup_source = mat.buildup_source.as_deref().unwrap_or(name);

            for &energy in energy_groups {
                let model = buildup_provider
                    .interpolate(buildup_source, quantity, energy)?;
                buildup_models.push(model);
            }
        }

        Ok(Self {
            mu_data,
            buildup_models,
            num_materials,
            num_groups,
        })
    }

    /// Gets the linear attenuation coefficient mu [cm^-1] in O(1) time.
    #[inline(always)]
    pub fn get_mu(&self, material_index: MaterialIndex, group_index: GroupIndex) -> f32 {
        debug_assert!(material_index < self.num_materials);
        debug_assert!(group_index < self.num_groups);
        self.mu_data[material_index * self.num_groups + group_index]
    }

    /// Calculates the buildup factor in O(1) time.
    #[inline(always)]
    pub fn get_buildup(
        &self,
        material_index: MaterialIndex,
        group_index: GroupIndex,
        optical_thickness: f32,
    ) -> f32 {
        debug_assert!(material_index < self.num_materials);
        debug_assert!(group_index < self.num_groups);
        let model = &self.buildup_models[material_index * self.num_groups + group_index];
        model.calculate(optical_thickness)
    }

    /// Returns index-based closures for attenuation and buildup calculations.
    pub fn into_closures(
        self,
    ) -> (
        impl Fn(MaterialIndex, GroupIndex) -> f32 + Send + Sync,
        impl Fn(MaterialIndex, GroupIndex, f32) -> f32 + Send + Sync,
    ) {
        let mu_data = self.mu_data;
        let buildup_models = self.buildup_models;
        let num_groups = self.num_groups;

        let mu_closure = {
            let mu_data = mu_data.clone();
            move |mat_idx: MaterialIndex, grp_idx: GroupIndex| mu_data[mat_idx * num_groups + grp_idx]
        };

        let buildup_closure = move |mat_idx: MaterialIndex, grp_idx: GroupIndex, ot: f32| {
            buildup_models[mat_idx * num_groups + grp_idx].calculate(ot)
        };

        (mu_closure, buildup_closure)
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
        target_energy: f32,
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
    use crate::material::MaterialDef;

    #[test]
    fn test_buildup_constant() {
        let model = BuildupModel::Constant(1.0);
        assert_eq!(model.calculate(5.0), 1.0);
    }

    #[test]
    fn test_physics_table_closures() {
        let mut composition = std::collections::HashMap::new();
        composition.insert(1, 1.0);
        let _mat = MaterialDef::new(composition, 1.0, Some("Dummy".into()));

        let _provider = GPBuildupProvider::new(); // empty, but we'll use Constant for this test
        // Actually, let's just manually construct the table for this logic test
        let table = MaterialPhysicsTable {
            mu_data: vec![0.05],
            buildup_models: vec![BuildupModel::Constant(1.0)],
            num_materials: 1,
            num_groups: 1,
        };
        
        let (mu_f, b_f) = table.into_closures();
        assert_eq!(mu_f(0, 0), 0.05);
        assert_eq!(b_f(0, 0, 2.5), 1.0);
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

        let mut registry = MaterialRegistry::new();
        let mut dummy_composition = std::collections::HashMap::new();
        dummy_composition.insert(1, 1.0);
        let dummy_mat = MaterialDef::new(dummy_composition, 1.0, None);
        registry.insert("DummyMaterial".to_string(), dummy_mat);

        let mat_name = "DummyMaterial".to_string();

        let mu_provider = crate::material::DummyProvider;
        let table = MaterialPhysicsTable::generate(
            &[mat_name], 
            &registry,
            &[1.0, 2.0], 
            &mu_provider, 
            &provider, 
            TargetQuantity::Exposure
        ).unwrap();

        // Check mu
        // DummyMaterial (Z=1) density 1.0. DummyProvider gives 0.05 for Z=1.
        // mu = 0.05 * 1.0 = 0.05
        assert!((table.get_mu(0, 0) - 0.05).abs() < 1e-6);

        // Check buildup: DummyMaterial (material 0), 2.0 MeV (energy index 1)
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
