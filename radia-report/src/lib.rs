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

/// Summary of physics data used in the calculation.
#[derive(Debug, Clone, Serialize, Default)]
pub struct PhysicsSummary {
    pub cross_section_library: String,
    pub buildup_library: String,
    pub conversion_factors: String,
}

/// Information about a ray path segment.
#[derive(Debug, Clone, Serialize)]
pub struct PathSegmentSummary {
    pub material_id: MaterialIndex,
    pub material_name: String,
    pub physical_thickness: f32,
    pub optical_thickness: f32,
}

/// Detailed results for a specific energy group.
#[derive(Debug, Clone, Serialize)]
pub struct EnergyGroupResult {
    pub group_index: usize,
    pub energy_mev: f32,
    pub uncollided_flux: f32,
    pub buildup_factor: f32,
    pub uncollided_dose_rate: f32,
    pub dose_rate_with_buildup: f32,
}

/// Simulation result at a specific detector location.
#[derive(Debug, Clone, Serialize)]
pub struct DetectorResult {
    /// 3D position [x, y, z] of the detector.
    pub position: [f32; 3],

    pub total_dose_rate_uncollided: f32,
    pub total_dose_rate_with_buildup: f32,

    pub buildup_material_name: String,
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

    pub results: Vec<DetectorResult>,

    pub warnings: Vec<String>,
}
