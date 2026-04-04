use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use miette::IntoDiagnostic;
use radia_core::buildup::{GPBuildupProvider, GPParams};
use radia_core::mass_attenuation::{
    AtomicNumber, MassAttenuationProvider, MassAttenuationProviderError, MaterialDef,
    MaterialRegistry,
};
use serde::Deserialize;

/// Element data structure for JSON deserialization.
#[derive(Debug, Deserialize)]
struct ElementData {
    #[serde(alias = "name")]
    _name: String,
    energies: Vec<f32>,
    mu_over_rho: Vec<f32>,
}

#[derive(Deserialize)]
struct _MaterialDef {
    density: f32,
    composition: HashMap<AtomicNumber, f32>,
}

impl From<_MaterialDef> for MaterialDef {
    fn from(value: _MaterialDef) -> Self {
        MaterialDef::new(value.composition, value.density)
    }
}

/// G-P parameters for JSON deserialization.
#[derive(Debug, Deserialize)]
struct GpParamsData {
    energy_mev: f32,
    a: f32,
    b: f32,
    c: f32,
    d: f32,
    xk: f32,
}

impl From<GpParamsData> for GPParams {
    fn from(value: GpParamsData) -> Self {
        GPParams {
            energy_mev: value.energy_mev,
            a: value.a,
            b: value.b,
            c: value.c,
            d: value.d,
            xk: value.xk,
        }
    }
}

/// JSON-based provider for mass attenuation data.
pub struct JsonMassAttenuationProvider {
    elements: HashMap<AtomicNumber, ElementData>,
}

impl JsonMassAttenuationProvider {
    /// Loads the data from a JSON file.
    pub fn from_file<P: AsRef<Path>>(path: P) -> miette::Result<Self> {
        let file = File::open(path).into_diagnostic()?;
        let reader = BufReader::new(file);
        let mut elements: HashMap<AtomicNumber, ElementData> =
            serde_json::from_reader(reader).into_diagnostic()?;

        // Ensure each element's energy data is sorted
        for element in elements.values_mut() {
            // Check if sorted
            let is_sorted = element.energies.windows(2).all(|w| w[0] <= w[1]);

            if !is_sorted {
                // Create indices and sort them based on energy
                let mut indices: Vec<usize> = (0..element.energies.len()).collect();
                indices.sort_by(|&a, &b| {
                    element.energies[a]
                        .partial_cmp(&element.energies[b])
                        .unwrap()
                });

                // Reorder energies and mu_over_rho
                let mut sorted_energies = Vec::with_capacity(element.energies.len());
                let mut sorted_mu = Vec::with_capacity(element.mu_over_rho.len());
                for &idx in &indices {
                    sorted_energies.push(element.energies[idx]);
                    sorted_mu.push(element.mu_over_rho[idx]);
                }
                element.energies = sorted_energies;
                element.mu_over_rho = sorted_mu;
            }
        }

        Ok(Self { elements })
    }

    /// Performs log-log linear interpolation for the mass attenuation coefficient.
    fn interpolate(
        energies: &[f32],
        values: &[f32],
        target_energy: f32,
    ) -> Result<f32, MassAttenuationProviderError> {
        // do not **extrapolate**
        if target_energy <= energies[0] {
            return Err(MassAttenuationProviderError::EnergyTooLow {
                target: target_energy,
                min: energies[0],
            });
        }
        if target_energy >= *energies.last().unwrap() {
            return Err(MassAttenuationProviderError::EnergyTooHigh {
                target: target_energy,
                max: *energies.last().unwrap(),
            });
        }

        // Find the interval
        let idx = match energies.binary_search_by(|e| e.partial_cmp(&target_energy).unwrap()) {
            Ok(i) => return Ok(values[i]),
            Err(i) => i,
        };

        let x1 = energies[idx - 1];
        let x2 = energies[idx];
        let y1 = values[idx - 1];
        let y2 = values[idx];

        // Linear interpolation in log-log scale:
        // ln(y) = ln(y1) + (ln(y2) - ln(y1)) / (ln(x2) - ln(x1)) * (ln(x) - ln(x1))
        let log_x = target_energy.ln();
        let log_x1 = x1.ln();
        let log_x2 = x2.ln();
        let log_y1 = y1.ln();
        let log_y2 = y2.ln();

        let log_y = log_y1 + (log_y2 - log_y1) / (log_x2 - log_x1) * (log_x - log_x1);
        Ok(log_y.exp())
    }
}

impl MassAttenuationProvider for JsonMassAttenuationProvider {
    fn get_mass_attenuation(
        &self,
        z: AtomicNumber,
        energy_mev: f32,
    ) -> Result<f32, MassAttenuationProviderError> {
        if let Some(element) = self.elements.get(&z) {
            Self::interpolate(&element.energies, &element.mu_over_rho, energy_mev)
        } else {
            Err(MassAttenuationProviderError::ElementNotFound(z))
        }
    }
}

pub fn load_material_registry_from_file<P: AsRef<Path>>(
    path: P,
) -> miette::Result<MaterialRegistry> {
    let file = File::open(path).into_diagnostic()?;
    let reader = BufReader::new(file);
    let compositions: HashMap<String, _MaterialDef> =
        serde_json::from_reader(reader).into_diagnostic()?;
    let mut registry = MaterialRegistry::new();
    for (name, def) in compositions {
        registry.insert(name, def.into());
    }
    Ok(registry)
}

pub fn load_buildup_registry_from_file<P: AsRef<Path>>(
    path: P,
) -> miette::Result<GPBuildupProvider> {
    let file = File::open(path).into_diagnostic()?;
    let reader = BufReader::new(file);
    let data: HashMap<String, Vec<GpParamsData>> =
        serde_json::from_reader(reader).into_diagnostic()?;
    let mut provider = GPBuildupProvider::new();
    for (name, params) in data {
        let params: Vec<GPParams> = params.into_iter().map(|p| p.into()).collect();
        provider.insert_data(name, params);
    }
    Ok(provider)
}

#[cfg(test)]
mod tests {
    use super::*;
    use radia_core::buildup::BuildupProvider;

    #[test]
    fn test_nist_json_provider() {
        let provider = JsonMassAttenuationProvider::from_file("../data/elements.json").unwrap();

        // Hydrogen (Z=1) at 1.0 MeV
        // NIST value: 1.263E-01 cm2/g
        let h_1mev = provider.get_mass_attenuation(1, 1.0).unwrap();
        assert!((h_1mev - 0.1263).abs() < 1e-4);

        // Lead (Z=82) at 1.0 MeV
        // NIST value: 7.102E-02 cm2/g
        let pb_1mev = provider.get_mass_attenuation(82, 1.0).unwrap();
        assert!((pb_1mev - 0.07102).abs() < 1e-5);

        // Interpolation test: Hydrogen between 1.0 and 1.5 MeV
        // 1.0 MeV: 0.1263
        // 1.5 MeV: 0.1032
        let h_1_25mev = provider.get_mass_attenuation(1, 1.25).unwrap();
        assert!(h_1_25mev < 0.1263 && h_1_25mev > 0.1032);
    }

    #[test]
    fn test_material_registry() {
        let registry = load_material_registry_from_file("../data/compositions.json").unwrap();
        let water = registry.get_material("Water, Liquid").unwrap();

        assert!((water.density() - 1.0).abs() < 1e-3);
        // Water is H2O. Z=1 (fraction ~0.111), Z=8 (fraction ~0.888)
        let composition = water.composition();
        assert!(composition.contains_key(&1));
        assert!(composition.contains_key(&8));
    }

    #[test]
    fn test_json_provider_auto_sort() {
        use std::io::Write;
        let temp_path = "/tmp/unsorted_elements.json";
        let json_content = r#"{
            "1": {
                "name": "Hydrogen",
                "energies": [2.0, 0.5, 1.0],
                "mu_over_rho": [0.1032, 0.1263, 0.1111]
            }
        }"#;
        let mut file = File::create(temp_path).unwrap();
        file.write_all(json_content.as_bytes()).unwrap();

        let provider = JsonMassAttenuationProvider::from_file(temp_path).unwrap();

        // If sorted, it should be: [0.5, 1.0, 2.0] with [0.1263, 0.1111, 0.1032]
        // 0.75 MeV should interpolate between 0.5 and 1.0
        let val = provider.get_mass_attenuation(1, 0.75).unwrap();
        assert!(val > 0.1111 && val < 0.1263);

        std::fs::remove_file(temp_path).unwrap();
    }

    #[test]
    fn test_buildup_registry() {
        use std::io::Write;
        let temp_path = "/tmp/buildup_data.json";
        let json_content = r#"{
            "Water": [
                {
                    "energy_mev": 1.0,
                    "a": 0.1,
                    "b": 2.0,
                    "c": 0.5,
                    "d": 0.01,
                    "xk": 10.0
                },
                {
                    "energy_mev": 2.0,
                    "a": 0.2,
                    "b": 1.5,
                    "c": 0.6,
                    "d": 0.02,
                    "xk": 12.0
                }
            ]
        }"#;
        let mut file = File::create(temp_path).unwrap();
        file.write_all(json_content.as_bytes()).unwrap();

        let provider = load_buildup_registry_from_file(temp_path).unwrap();
        let model = provider
            .get_model("Water", 1.5)
            .expect("Should find and interpolate Water at 1.5 MeV");

        use radia_core::buildup::BuildupModel;
        if let BuildupModel::GeometricProgression { a, b, .. } = model {
            // a should be (0.1 + 0.2) / 2 = 0.15 (approx, due to log-linear interp)
            // b should be between 2.0 and 1.5
            assert!(a > 0.1 && a < 0.2);
            assert!(b > 1.5 && b < 2.0);
        } else {
            panic!("Expected GP model");
        }

        std::fs::remove_file(temp_path).unwrap();
    }
}
