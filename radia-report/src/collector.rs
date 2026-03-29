use glam::Vec3A;
use radia_core::kernel::DoseCollector;
use radia_core::mass_attenuation::MaterialIndex;

use crate::{
    DetectorResult, EnergyGroupResult, PathSegmentSummary, PhysicsSummary, RunMetadata,
    SimulationReport,
};

#[derive(Default)]
pub struct DetailedCollector {
    pub report: SimulationReport,
}

impl DetailedCollector {
    pub fn new(
        physics_data: PhysicsSummary,
        input_echo: serde_json::Value,
        input_file_hash: String,
    ) -> Self {
        let timestamp =
            if let Ok(dur) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
                dur.as_secs().to_string()
            } else {
                "unknown".to_string()
            };

        let metadata = RunMetadata {
            radia_rs_version: radia_core::VERSION.to_string(),
            timestamp,
            os: std::env::consts::OS.to_string(),
            input_file_hash,
        };

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

    pub fn to_markdown(&self) -> String {
        use std::fmt::Write;

        let mut md = String::new();
        let _ = writeln!(md, "# radia-rs Simulation Report\n");

        let _ = writeln!(md, "## Metadata");
        let _ = writeln!(
            md,
            "- **Version:** {}",
            self.report.metadata.radia_rs_version
        );
        let _ = writeln!(md, "- **Timestamp:** {}", self.report.metadata.timestamp);
        let _ = writeln!(md, "- **OS:** {}", self.report.metadata.os);
        let _ = writeln!(
            md,
            "- **Input File Hash:** {}\n",
            self.report.metadata.input_file_hash
        );

        let _ = writeln!(md, "## Physics Summary");
        let _ = writeln!(
            md,
            "- **Cross Section Library:** {}",
            self.report.physics_data.cross_section_library
        );
        let _ = writeln!(
            md,
            "- **Buildup Library:** {}",
            self.report.physics_data.buildup_library
        );
        let _ = writeln!(
            md,
            "- **Conversion Factors:** {}\n",
            self.report.physics_data.conversion_factors
        );

        if !self.report.warnings.is_empty() {
            let _ = writeln!(md, "## Warnings");
            for w in &self.report.warnings {
                let _ = writeln!(md, "- {}", w);
            }
            let _ = writeln!(md);
        }

        let _ = writeln!(md, "## Results");
        if self.report.results.is_empty() {
            let _ = writeln!(md, "No results collected.");
        } else {
            for (i, res) in self.report.results.iter().enumerate() {
                let _ = writeln!(
                    md,
                    "### Detector {} at `[{:.3}, {:.3}, {:.3}]`",
                    i + 1,
                    res.position[0],
                    res.position[1],
                    res.position[2]
                );
                let _ = writeln!(md, "- **Buildup Material:** {}", res.buildup_material_name);
                let _ = writeln!(
                    md,
                    "- **Total Dose Rate (Uncollided):** {:.6e}",
                    res.total_dose_rate_uncollided
                );
                let _ = writeln!(
                    md,
                    "- **Total Dose Rate (with Buildup):** {:.6e}\n",
                    res.total_dose_rate_with_buildup
                );

                if !res.energy_group_details.is_empty() {
                    let _ = writeln!(md, "#### Energy Group Details");
                    let _ = writeln!(
                        md,
                        "| Group | Energy (MeV) | Uncollided Flux | Buildup Factor | Dose Rate (Unc.) | Dose Rate (Total) |"
                    );
                    let _ = writeln!(
                        md,
                        "|-------|--------------|-----------------|----------------|------------------|-------------------|"
                    );
                    for eg in &res.energy_group_details {
                        let _ = writeln!(
                            md,
                            "| {} | {:.3} | {:.6e} | {:.6e} | {:.6e} | {:.6e} |",
                            eg.group_index,
                            eg.energy_mev,
                            eg.uncollided_flux,
                            eg.buildup_factor,
                            eg.uncollided_dose_rate,
                            eg.dose_rate_with_buildup
                        );
                    }
                    let _ = writeln!(md);
                }

                if !res.ray_path_summary.is_empty() {
                    let _ = writeln!(md, "#### Ray Path Summary");
                    let _ = writeln!(md, "| Material | Physical Thickness | Optical Thickness |");
                    let _ = writeln!(md, "|----------|--------------------|-------------------|");
                    for seg in &res.ray_path_summary {
                        let _ = writeln!(
                            md,
                            "| {} | {:.6e} | {:.6e} |",
                            seg.material_name, seg.physical_thickness, seg.optical_thickness
                        );
                    }
                    let _ = writeln!(md);
                }
            }
        }

        md
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
