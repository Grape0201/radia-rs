use std::collections::HashMap;

pub type AtomicNumber = usize;

pub type MaterialIndex = usize;
pub type GroupIndex = usize;

#[derive(thiserror::Error, Debug)]
pub enum MaterialError {
    #[error("Element with atomic number {0} not found")]
    ElementNotFound(AtomicNumber),

    #[error("Target energy {target} MeV is too low (minimum {min} MeV)")]
    EnergyTooLow { target: f32, min: f32 },

    #[error("Target energy {target} MeV is too high (maximum {max} MeV)")]
    EnergyTooHigh { target: f32, max: f32 },

    #[error("{0}")]
    Other(String),
}

/// Trait abstracting a provider of mass attenuation coefficients data.
/// It is responsible for returning the mass attenuation coefficient for a given atomic number and energy.
pub trait MassAttenuationProvider {
    /// Retrieves the mass attenuation coefficient [cm^2/g] for a specific atomic number and energy (MeV).
    fn get_mass_attenuation(&self, z: AtomicNumber, energy_mev: f32) -> Result<f32, MaterialError>;
}

/// Registry for standard material compositions.
pub struct MaterialRegistry {
    compositions: HashMap<String, MaterialDef>,
}

impl Default for MaterialRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl MaterialRegistry {
    /// Creates a new empty material registry.
    pub fn new() -> Self {
        Self {
            compositions: HashMap::new(),
        }
    }

    /// Returns a MaterialDef for a given material name.
    pub fn get_material(&self, name: &str) -> Option<MaterialDef> {
        self.compositions.get(name).cloned()
    }

    /// Adds a material to the registry manually.
    pub fn insert(&mut self, name: String, material: MaterialDef) {
        self.compositions.insert(name, material);
    }

    /// List available materials.
    pub fn list_available(&self) -> Vec<String> {
        self.compositions.keys().cloned().collect()
    }
}

/// Material definition provided by the user.
#[derive(Clone, Debug)]
pub struct MaterialDef {
    /// Density (g/cm^3) of the material.
    pub(crate) density: f32,
    /// Element weight fractions of the material, mapped by its atomic number.
    pub(crate) composition: HashMap<AtomicNumber, f32>,
}

impl MaterialDef {
    /// Creates a new material definition.
    pub fn new(composition: HashMap<AtomicNumber, f32>, density: f32) -> Self {
        Self {
            density,
            composition,
        }
    }

    /// Returns the partial densities (g/cm^3) of each element composing the material.
    pub fn partial_densities(&self) -> HashMap<AtomicNumber, f32> {
        self.composition
            .iter()
            .map(|(&z, &fraction)| (z, fraction * self.density))
            .collect()
    }

    /// getter for density
    pub fn density(&self) -> f32 {
        self.density
    }

    /// getter for composition
    pub fn composition(&self) -> &HashMap<AtomicNumber, f32> {
        &self.composition
    }
}

// ==== Dummy provider implementation for examples / tests ====
pub struct DummyProvider;
impl MassAttenuationProvider for DummyProvider {
    fn get_mass_attenuation(
        &self,
        z: AtomicNumber,
        _energy_mev: f32,
    ) -> Result<f32, MaterialError> {
        match z {
            1 => Ok(0.05),
            8 => Ok(0.06),
            26 => Ok(0.01),
            82 => Ok(0.01),
            _ => Err(MaterialError::ElementNotFound(z)),
        }
    }
}
