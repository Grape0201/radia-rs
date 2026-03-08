use serde::Deserialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

pub type AtomicNumber = u32;

#[derive(thiserror::Error, Debug)]
pub enum MaterialError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Element with atomic number {0} not found")]
    ElementNotFound(AtomicNumber),

    #[error("Target energy {target} MeV is too low (minimum {min} MeV)")]
    EnergyTooLow { target: f64, min: f64 },

    #[error("Target energy {target} MeV is too high (maximum {max} MeV)")]
    EnergyTooHigh { target: f64, max: f64 },

    #[error("{0}")]
    Other(String),
}

/// Trait abstracting a provider of mass attenuation coefficients data.
/// It is responsible for returning the mass attenuation coefficient for a given atomic number and energy.
pub trait MassAttenuationProvider {
    /// Retrieves the mass attenuation coefficient [cm^2/g] for a specific atomic number and energy (MeV).
    fn get_mass_attenuation(&self, z: AtomicNumber, energy_mev: f64) -> Result<f64, MaterialError>;
}

/// Element data structure for JSON deserialization.
#[derive(Debug, Deserialize)]
struct ElementData {
    #[serde(alias = "name")]
    _name: String,
    energies: Vec<f64>,
    mu_over_rho: Vec<f64>,
}

/// JSON-based provider for mass attenuation data.
pub struct JsonMassAttenuationProvider {
    elements: HashMap<AtomicNumber, ElementData>,
}

impl JsonMassAttenuationProvider {
    /// Loads the data from a JSON file.
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, MaterialError> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut elements: HashMap<AtomicNumber, ElementData> = serde_json::from_reader(reader)?;

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
        energies: &[f64],
        values: &[f64],
        target_energy: f64,
    ) -> Result<f64, MaterialError> {
        // do not **extrapolate**
        if target_energy <= energies[0] {
            return Err(MaterialError::EnergyTooLow {
                target: target_energy,
                min: energies[0],
            });
        }
        if target_energy >= *energies.last().unwrap() {
            return Err(MaterialError::EnergyTooHigh {
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
    fn get_mass_attenuation(&self, z: AtomicNumber, energy_mev: f64) -> Result<f64, MaterialError> {
        if let Some(element) = self.elements.get(&z) {
            Self::interpolate(&element.energies, &element.mu_over_rho, energy_mev)
        } else {
            Err(MaterialError::ElementNotFound(z))
        }
    }
}

/// Composition data structure for JSON deserialization.
#[derive(Debug, Deserialize)]
struct CompositionData {
    density: f64,
    composition: HashMap<AtomicNumber, f64>,
}

/// Registry for standard material compositions.
pub struct MaterialRegistry {
    compositions: HashMap<String, CompositionData>,
}

impl MaterialRegistry {
    /// Loads common compositions from a JSON file.
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, MaterialError> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let compositions: HashMap<String, CompositionData> = serde_json::from_reader(reader)?;
        Ok(Self { compositions })
    }

    /// Returns a MaterialDef and density for a given material name.
    pub fn get_material(&self, name: &str) -> Option<(MaterialDef, f64)> {
        self.compositions.get(name).map(|data| {
            let partial_densities = data
                .composition
                .iter()
                .map(|(&z, &fraction)| (z, fraction * data.density))
                .collect();
            (MaterialDef::new(partial_densities), data.density)
        })
    }

    /// List available materials.
    pub fn list_available(&self) -> Vec<String> {
        self.compositions.keys().cloned().collect()
    }
}

/// Material definition provided by the user.
#[derive(Clone, Debug)]
pub struct MaterialDef {
    /// Partial density (g/cm^3) of each element composing the material, mapped by its atomic number.
    pub partial_densities: HashMap<AtomicNumber, f64>,
}

impl MaterialDef {
    /// Creates a new material definition.
    pub fn new(partial_densities: HashMap<AtomicNumber, f64>) -> Self {
        Self { partial_densities }
    }
}

/// A table of pre-calculated macroscopic cross sections (linear attenuation coefficients).
#[derive(Clone, Debug)]
pub struct MuTable {
    /// Linear attenuation coefficients [cm^-1] stored in (material_index, group_index) order.
    /// Flattened into a 1D array for performance: data[material_index * num_groups + group_index].
    data: Vec<f64>,
    num_materials: usize,
    num_groups: usize,
}

impl MuTable {
    /// Generates a table by pre-calculating the linear attenuation coefficients
    /// given the user's materials, energy groups, and a data provider.
    pub fn generate(
        materials: &[MaterialDef],
        energy_groups: &[f64],
        provider: &impl MassAttenuationProvider,
    ) -> Result<Self, MaterialError> {
        let num_materials = materials.len();
        let num_groups = energy_groups.len();
        let mut data = Vec::with_capacity(num_materials * num_groups);

        for material in materials {
            for &energy in energy_groups {
                let mut mu = 0.0;
                for (&z, &density) in &material.partial_densities {
                    let mass_att = provider.get_mass_attenuation(z, energy)?;
                    mu += mass_att * density;
                }
                data.push(mu);
            }
        }

        Ok(Self {
            data,
            num_materials,
            num_groups,
        })
    }

    /// Gets the linear attenuation coefficient mu [cm^-1] in O(1) time.
    /// Due to contiguous memory access, this is highly cache-efficient and fast.
    #[inline(always)]
    pub fn get_mu(&self, material_index: usize, group_index: usize) -> f64 {
        debug_assert!(
            material_index < self.num_materials,
            "Invalid material index"
        );
        debug_assert!(group_index < self.num_groups, "Invalid energy group index");
        self.data[material_index * self.num_groups + group_index]
    }

    /// Returns a closure that captures the table and provides O(1) access.
    /// You can move this closure without worrying about lifetimes.
    pub fn into_closure(self) -> impl Fn(usize, usize) -> f64 + Send + Sync {
        let data = self.data;
        let num_groups = self.num_groups;
        move |mat_idx, grp_idx| data[mat_idx * num_groups + grp_idx]
    }
}

// ==== Dummy provider implementation for examples / tests ====
pub struct DummyProvider;
impl MassAttenuationProvider for DummyProvider {
    fn get_mass_attenuation(
        &self,
        z: AtomicNumber,
        _energy_mev: f64,
    ) -> Result<f64, MaterialError> {
        match z {
            1 => Ok(0.05),
            8 => Ok(0.06),
            82 => Ok(0.01),
            _ => Err(MaterialError::ElementNotFound(z)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mu_table_generation_and_access() {
        // Create water-like material
        let mut water_densities = HashMap::new();
        water_densities.insert(1, 0.111); // Hydrogen
        water_densities.insert(8, 0.889); // Oxygen
        let water = MaterialDef::new(water_densities);

        // Create lead-like material
        let mut lead_densities = HashMap::new();
        lead_densities.insert(82, 11.35); // Lead
        let lead = MaterialDef::new(lead_densities);

        let materials = vec![water, lead];
        let energy_groups = vec![0.5, 1.0, 2.0];
        let provider = DummyProvider;

        let mu_table = MuTable::generate(&materials, &energy_groups, &provider).unwrap();

        // Hydrogen mass att at DummyProvider: 0.05
        // Oxygen mass att at DummyProvider: 0.06
        // Water linear att: 0.111 * 0.05 + 0.889 * 0.06 = 0.00555 + 0.05334 = 0.05889
        let water_mu = mu_table.get_mu(0, 0);
        assert!((water_mu - 0.05889).abs() < 1e-6);

        // Lead mass att at DummyProvider: 0.01 (default)
        // Lead linear att: 11.35 * 0.01 = 0.1135
        let lead_mu = mu_table.get_mu(1, 2);
        assert!((lead_mu - 0.1135).abs() < 1e-6);
    }

    #[test]
    fn test_mu_table_closure() {
        let mut densities = HashMap::new();
        densities.insert(1, 1.0);
        let mat = MaterialDef::new(densities);

        let materials = vec![mat];
        let energy_groups = vec![1.0];
        let provider = DummyProvider;

        let mu_table = MuTable::generate(&materials, &energy_groups, &provider).unwrap();
        let mu_func = mu_table.into_closure();

        let mu = mu_func(0, 0);
        // Hydrogen mass att: 0.05
        // Total density: 1.0
        // Expected mu: 0.05 * 1.0 = 0.05
        assert!((mu - 0.05).abs() < 1e-6);
    }

    #[test]
    fn test_nist_json_provider() {
        let provider = JsonMassAttenuationProvider::from_file("data/elements.json").unwrap();

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
        let registry = MaterialRegistry::from_file("data/compositions.json").unwrap();
        let (water, density) = registry.get_material("Water, Liquid").unwrap();

        assert!((density - 1.0).abs() < 1e-3);
        // Water is H2O. Z=1 (fraction ~0.111), Z=8 (fraction ~0.888)
        assert!(water.partial_densities.contains_key(&1));
        assert!(water.partial_densities.contains_key(&8));

        // Calculate mu for Water at 1.0 MeV
        let provider = JsonMassAttenuationProvider::from_file("data/elements.json").unwrap();
        let mu_table = MuTable::generate(&[water], &[1.0], &provider).unwrap();

        // NIST mu/rho for Water at 1.0 MeV: 7.072E-02 cm2/g
        // Density = 1.0, so mu should be ~0.07072 cm-1
        let water_mu = mu_table.get_mu(0, 0);
        assert!((water_mu - 0.07072).abs() < 1e-4);
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
}
