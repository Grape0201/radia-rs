use crate::buildup::{BuildupModel, BuildupProvider, BuildupProviderError};
use crate::mass_attenuation::{
    GroupIndex, MassAttenuationProvider, MassAttenuationProviderError, MaterialIndex,
    MaterialRegistry,
};

use std::collections::HashMap;

/// Error type for MaterialPhysicsTable operations
#[derive(thiserror::Error, Debug)]
pub enum MaterialPhysicsError {
    #[error("Material '{0}' not found in registry")]
    MaterialDataNotInRegistry(String),

    #[error(transparent)]
    MassAttenuation(#[from] MassAttenuationProviderError),

    #[error(transparent)]
    Buildup(#[from] BuildupProviderError),
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
        buildup_alias_map: &HashMap<String, String>,
        registry: &MaterialRegistry,
        energy_groups: &[f32],
        mu_provider: &impl MassAttenuationProvider,
        buildup_provider: &impl BuildupProvider<BuildupModel>,
    ) -> Result<Self, MaterialPhysicsError> {
        let num_materials = material_names.len();
        let num_groups = energy_groups.len();

        let mut mu_data = Vec::with_capacity(num_materials * num_groups);
        let mut buildup_models = Vec::with_capacity(num_materials * num_groups);

        for name in material_names {
            let buildup_source = buildup_alias_map
                .get(name)
                .ok_or_else(|| BuildupProviderError::MaterialNotFound(name.clone()))?;

            let mat = registry
                .get_material(name)
                .ok_or_else(|| MaterialPhysicsError::MaterialDataNotInRegistry(name.into()))?;

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
            for &energy in energy_groups {
                let model = if buildup_source.eq_ignore_ascii_case("none") {
                    BuildupModel::Constant(1.0)
                } else {
                    buildup_provider.get_model(buildup_source, energy)?
                };
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

    /// Returns the raw linear attenuation coefficient data.
    pub fn get_mu_data(&self) -> &[f32] {
        &self.mu_data
    }

    #[cfg(test)]
    pub(crate) fn generate_for_test(
        mu_data: Vec<f32>,
        buildup_models: Vec<BuildupModel>,
        num_materials: usize,
        num_groups: usize,
    ) -> Self {
        Self {
            mu_data,
            buildup_models,
            num_materials,
            num_groups,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buildup::{BuildupModel, GPBuildupProvider, GPParams};
    use crate::mass_attenuation::MaterialDef;

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
        provider.insert_data("DummyMaterial".to_string(), dummy_data.clone());
        provider.insert_data("DummyMaterial".to_string(), dummy_data);

        // Exact match
        let model_1mev = provider.interpolate("DummyMaterial", 1.0).unwrap();
        if let BuildupModel::GeometricProgression { a, .. } = model_1mev {
            assert!((a - 0.12).abs() < 1e-5);
        } else {
            panic!("Expected GP model");
        }

        // Interpolated (e.g. at 1.414 MeV)
        let model_interp = provider.interpolate("DummyMaterial", 1.414).unwrap();
        if let BuildupModel::GeometricProgression { a, .. } = model_interp {
            // Should be between 1.0 MeV (a=0.12) and 10.0 MeV (a=0.2)
            assert!(a > 0.12 && a < 0.2);
        } else {
            panic!("Expected GP model");
        }

        // Out of range (Extrapolation) should now be an error
        let err_low = provider.interpolate("DummyMaterial", 0.1);
        assert!(matches!(
            err_low,
            Err(BuildupProviderError::EnergyTooLow { .. })
        ));

        let err_high = provider.interpolate("DummyMaterial", 20.0);
        assert!(matches!(
            err_high,
            Err(BuildupProviderError::EnergyTooHigh { .. })
        ));

        let mut registry = MaterialRegistry::new();
        let mut dummy_composition = std::collections::HashMap::new();
        dummy_composition.insert(1, 1.0);
        let dummy_mat = MaterialDef::new(dummy_composition, 1.0);
        registry.insert("DummyMaterial".to_string(), dummy_mat);

        let mat_names = vec!["DummyMaterial".to_string()];
        let buildup_alias_map =
            HashMap::from([("DummyMaterial".to_string(), "DummyMaterial".to_string())]);

        let mu_provider = crate::mass_attenuation::DummyProvider;
        let table = MaterialPhysicsTable::generate(
            &mat_names,
            &buildup_alias_map,
            &registry,
            &[1.0, 2.0],
            &mu_provider,
            &provider,
        )
        .unwrap();

        // Check mu
        // DummyMaterial (Z=1) density 1.0. DummyProvider gives 0.05 for Z=1.
        // mu = 0.05 * 1.0 = 0.05
        assert!((table.get_mu_data()[0] - 0.05).abs() < 1e-6);

        // Check buildup: DummyMaterial (material 0), 2.0 MeV (energy index 1)
        let b = table.get_buildup(0, 1, 5.0); // B(x) at x=5.0
        assert!(b > 1.0); // Should be >> 1.0 due to buildup
    }
}
