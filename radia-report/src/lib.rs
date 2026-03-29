use glam::Vec3A;
use radia_core::kernel::DoseCollector;
use radia_core::mass_attenuation::MaterialIndex;
use serde::Serialize;

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

#[derive(Default)]
pub struct DetailedCollector {
    pub report: SimulationReport,
}

impl DetailedCollector {
    pub fn new(
        metadata: RunMetadata,
        physics_data: PhysicsSummary,
        input_echo: serde_json::Value,
    ) -> Self {
        Self {
            report: SimulationReport {
                metadata,
                input_echo,
                physics_data,
                results: Vec::new(),
                warnings: Vec::new(),
            },
        }
    }
}

impl DoseCollector for DetailedCollector {
    fn begin_detector(&mut self, position: Vec3A) {
        self.report.results.push(DetectorResult {
            position: position.into(),
            total_dose_rate_uncollided: 0.0,
            total_dose_rate_with_buildup: 0.0,
            buildup_material_name: String::new(),
            energy_group_details: Vec::new(),
            ray_path_summary: Vec::new(),
        });
    }

    fn begin_source(&mut self, _position: Vec3A, _intensity: f32) {}

    fn record_ray_segment(
        &mut self,
        material_id: Option<MaterialIndex>,
        physical_thickness: f32,
        optical_thickness: f32,
    ) {
        if let Some(res) = self.report.results.last_mut() {
            // To avoid duplicating per energy group, we just clear and keep the last energy group's segments for now.
            // A more robust implementation might map this by energy group.
            res.ray_path_summary.push(PathSegmentSummary {
                material_id: material_id.unwrap_or(MaterialIndex::MAX),
                material_name: format!("Material_{}", material_id.unwrap_or(MaterialIndex::MAX)),
                physical_thickness,
                optical_thickness,
            });
        }
    }

    fn record_buildup_material(&mut self, material_id: Option<usize>) {
        if let Some(res) = self.report.results.last_mut() {
            res.buildup_material_name = format!("Material_{}", material_id.unwrap_or(usize::MAX));
        }
    }

    fn record_energy_group(
        &mut self,
        group_index: usize,
        uncollided_flux: f32,
        buildup: f32,
        uncollided_dose: f32,
        total_dose: f32,
    ) {
        if let Some(res) = self.report.results.last_mut() {
            res.energy_group_details.push(EnergyGroupResult {
                group_index,
                energy_mev: 0.0, // To be mapped from external data context
                uncollided_flux,
                buildup_factor: buildup,
                uncollided_dose_rate: uncollided_dose,
                dose_rate_with_buildup: total_dose,
            });
            res.total_dose_rate_uncollided += uncollided_dose;
            res.total_dose_rate_with_buildup += total_dose;
        }
    }

    fn merge(&mut self, mut other: Self) {
        self.report.results.append(&mut other.report.results);
        self.report.warnings.append(&mut other.report.warnings);
    }
}
