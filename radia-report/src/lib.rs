use serde::Serialize;

pub mod collector;
pub use collector::DetailedCollector;

use radia_core::mass_attenuation::MaterialIndex;

/// Metadata for a simulation run.
#[derive(Debug, Clone, Serialize, Default)]
pub struct RunMetadata {
    pub radia_rs_version: String,
    pub timestamp: String,
    pub os: String,
    pub input_file_hash: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct EvaluatedMaterial {
    pub name: String,
    pub density: f32,
    pub composition: std::collections::HashMap<usize, f32>,
    pub mu_by_group: Vec<f32>,
    pub buildup_by_group: Vec<String>,
}

/// Summary of physics data used in the calculation.
#[derive(Debug, Clone, Serialize, Default)]
pub struct PhysicsSummary {
    pub cross_section_library: String,
    pub buildup_library: String,
    pub conversion_factors: String,
    pub energy_groups: Vec<f32>,
    pub evaluated_materials: Vec<EvaluatedMaterial>,
}

/// Information about a ray path segment.
#[derive(Debug, Clone, Serialize)]
pub struct PathSegmentSummary {
    pub material_id: Option<MaterialIndex>,
    pub material_name: String,
    pub segments: usize,
    pub physical_thickness_sum: f32,
    pub physical_thickness_min: f32,
    pub physical_thickness_max: f32,
    pub optical_thickness_sum: f32,
    pub optical_thickness_min: f32,
    pub optical_thickness_max: f32,
}

/// Detailed results for a specific energy group.
#[derive(Debug, Clone, Serialize)]
pub struct EnergyGroupResult {
    pub group_index: usize,
    pub energy_mev: f32,
    pub count: usize,
    pub uncollided_flux_sum: f32,
    pub uncollided_flux_min: f32,
    pub uncollided_flux_max: f32,
    pub buildup_factor_sum: f32,
    pub buildup_factor_min: f32,
    pub buildup_factor_max: f32,
    pub uncollided_dose_rate_sum: f32,
    pub uncollided_dose_rate_min: f32,
    pub uncollided_dose_rate_max: f32,
    pub dose_rate_with_buildup_sum: f32,
    pub dose_rate_with_buildup_min: f32,
    pub dose_rate_with_buildup_max: f32,
}

/// Simulation result at a specific detector location.
#[derive(Debug, Clone, Serialize)]
pub struct DetectorResult {
    /// 3D position [x, y, z] of the detector.
    pub position: [f32; 3],

    pub total_dose_rate_uncollided: f32,
    pub total_dose_rate_with_buildup: f32,

    pub buildup_material_frequencies: std::collections::HashMap<Option<usize>, usize>,
    pub energy_group_details: Vec<EnergyGroupResult>,
    pub ray_path_summary: Vec<PathSegmentSummary>,
}

/// The root report structure summarizing the entire simulation.
#[derive(Debug, Clone, Serialize, Default)]
pub struct SimulationReport {
    pub metadata: RunMetadata,

    /// Echo of the parsed input as JSON or specific structures.
    pub input_echo: serde_json::Value,

    pub physics_data: PhysicsSummary,

    pub recognized_world: Option<serde_json::Value>,
    pub primitive_names: Vec<String>,

    pub results: Vec<DetectorResult>,

    pub warnings: Vec<String>,
}
