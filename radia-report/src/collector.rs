use glam::Vec3A;
use radia_core::kernel::DoseCollector;
use radia_core::mass_attenuation::{MaterialIndex, MaterialRegistry};
use radia_core::physics::MaterialPhysicsTable;
use radia_core::csg::{World, Instruction};
use radia_input::SimulationInput;

use crate::{
    DetectorResult, EnergyGroupResult, EvaluatedMaterial, PathSegmentSummary, PhysicsSummary,
    RunMetadata, SimulationReport,
};

#[derive(Default)]
pub struct DetailedCollector {
    pub report: SimulationReport,
}

impl DetailedCollector {
    pub fn new(
        sim_input: &SimulationInput,
        physics_table: &MaterialPhysicsTable,
        registry: &MaterialRegistry,
        world: &World,
        input_file_path: String,
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
            input_file_hash: input_file_path,
        };

        // Extract input echo
        let input_echo = std::fs::read_to_string(&metadata.input_file_hash)
            .ok()
            .map(serde_json::Value::String)
            .unwrap_or(serde_json::Value::Null);

        // Build Physics Summary
        let mut used_materials: Vec<String> = sim_input.world.cells.iter()
            .map(|c| c.material_name.clone())
            .collect();
        used_materials.sort();
        used_materials.dedup();

        let energy_groups = &sim_input.source.energy_groups;
        let mut evaluated_materials = Vec::new();
        for (i, name) in used_materials.iter().enumerate() {
            if let Some(mat_def) = registry.get_material(name) {
                let mut mu_by_group = Vec::new();
                let mut buildup_by_group = Vec::new();
                for ig in 0..energy_groups.len() {
                    let mu = physics_table.get_mu_data()[i * energy_groups.len() + ig];
                    let buildup = physics_table.get_buildup_model(i, ig).to_string();
                    mu_by_group.push(mu);
                    buildup_by_group.push(buildup);
                }
                evaluated_materials.push(EvaluatedMaterial {
                    name: name.clone(),
                    density: mat_def.density(),
                    composition: mat_def.composition().clone(),
                    mu_by_group,
                    buildup_by_group,
                });
            }
        }

        let physics_data = PhysicsSummary {
            cross_section_library: "NIST XCOM (JSON)".to_string(),
            buildup_library: "Geometric Progression (GP)".to_string(),
            conversion_factors: "Interpolated".to_string(),
            energy_groups: energy_groups.clone(),
            evaluated_materials,
        };

        // Build Recognized World JSON manually
        let mut cell_json = Vec::new();
        for cell in &world.cells {
            let mut insts = Vec::new();
            for inst in &cell.csg.instructions {
                match inst {
                    Instruction::Union => insts.push(serde_json::json!("Union")),
                    Instruction::Intersection => insts.push(serde_json::json!("Intersection")),
                    Instruction::Difference => insts.push(serde_json::json!("Difference")),
                    Instruction::Complement => insts.push(serde_json::json!("Complement")),
                    Instruction::PushPrimitive(id) => insts.push(serde_json::json!({"PushPrimitive": id})),
                }
            }
            cell_json.push(serde_json::json!({
                "material_id": cell.material_id,
                "csg": { "instructions": insts }
            }));
        }

        let prim_json: Vec<_> = world.primitives.get_primitives().iter().map(|p| {
            match p {
                radia_core::primitive::Primitive::Sphere { center, radius2 } => {
                    serde_json::json!({
                        "type": "Sphere",
                        "center": [center.x, center.y, center.z],
                        "radius2": radius2
                    })
                }
                radia_core::primitive::Primitive::RectangularParallelPiped { min, max } => {
                    serde_json::json!({
                        "type": "RectangularParallelPiped",
                        "min": [min.x, min.y, min.z],
                        "max": [max.x, max.y, max.z]
                    })
                }
                radia_core::primitive::Primitive::FiniteCylinder { center, direction, radius2, half_height } => {
                    serde_json::json!({
                        "type": "FiniteCylinder",
                        "center": [center.x, center.y, center.z],
                        "direction": [direction.x, direction.y, direction.z],
                        "radius2": radius2,
                        "half_height": half_height
                    })
                }
            }
        }).collect();

        let recognized_world = serde_json::json!({
            "primitives": prim_json,
            "cells": cell_json
        });

        let primitive_names: Vec<String> = sim_input.world.primitives.iter().map(|p| p.name().to_string()).collect();

        Self {
            report: SimulationReport {
                metadata,
                input_echo,
                physics_data,
                recognized_world: Some(recognized_world),
                primitive_names,
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

        if let Some(world) = &self.report.recognized_world {
            let _ = writeln!(md, "## Recognized World Structure");
            
            // Render Primitives
            if let Some(primitives) = world.get("primitives").and_then(|p| p.as_array()) {
                let _ = writeln!(md, "### Primitives");
                let _ = writeln!(md, "| Index | Type | Parameters |");
                let _ = writeln!(md, "|-------|------|------------|");
                for (i, p) in primitives.iter().enumerate() {
                    let p_type = p.get("type").and_then(|t| t.as_str()).unwrap_or("Unknown");
                    let p_name = if i < self.report.primitive_names.len() {
                        self.report.primitive_names[i].clone()
                    } else {
                        format!("Primitive_{}", i)
                    };
                    let mut params = Vec::new();
                    if let Some(obj) = p.as_object() {
                        for (k, v) in obj {
                            if k != "type" {
                                params.push(format!("{}: {}", k, v));
                            }
                        }
                    }
                    let _ = writeln!(md, "| {} ({}) | {} | {} |", i, p_name, p_type, params.join(", "));
                }
                let _ = writeln!(md);
            }

            // Render Cells
            if let Some(cells) = world.get("cells").and_then(|c| c.as_array()) {
                let _ = writeln!(md, "### Cells");
                let _ = writeln!(md, "| Index | Material | CSG Instructions (RPN) |");
                let _ = writeln!(md, "|-------|----------|------------------------|");
                for (i, c) in cells.iter().enumerate() {
                    let mat_id = c.get("material_id").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                    let mat_name = if mat_id < self.report.physics_data.evaluated_materials.len() {
                        self.report.physics_data.evaluated_materials[mat_id].name.clone()
                    } else {
                        format!("Material_{}", mat_id)
                    };
                    
                    let mut inst_strs = Vec::new();
                    if let Some(instructions) = c.get("csg").and_then(|csg| csg.get("instructions")).and_then(|ins| ins.as_array()) {
                        for inst in instructions {
                            if let Some(s) = inst.as_str() {
                                inst_strs.push(s.to_string());
                            } else if let Some(obj) = inst.as_object() {
                                for (k, v) in obj {
                                    if k == "PushPrimitive" {
                                        if let Some(id) = v.as_u64() {
                                            let id = id as usize;
                                            let name = if id < self.report.primitive_names.len() {
                                                self.report.primitive_names[id].clone()
                                            } else {
                                                format!("{}", id)
                                            };
                                            inst_strs.push(format!("{}({})", k, name));
                                        } else {
                                            inst_strs.push(format!("{}({:?})", k, v));
                                        }
                                    } else {
                                        inst_strs.push(format!("{}({})", k, v));
                                    }
                                }
                            }
                        }
                    }
                    let _ = writeln!(md, "| {} | {} | {} |", i, mat_name, inst_strs.join(" "));
                }
                let _ = writeln!(md);
            }
        }

        if !self.report.physics_data.evaluated_materials.is_empty() {
            let _ = writeln!(md, "## Evaluated Material Properties");
            for mat in &self.report.physics_data.evaluated_materials {
                let _ = writeln!(md, "### {} (Density: {:.3} g/cm^3)", mat.name, mat.density);
                let _ = writeln!(md, "**Composition:**");
                let mut comp_strs: Vec<_> = mat.composition.iter().collect();
                comp_strs.sort_by_key(|tuple| *tuple.0);
                let formatted_comps: Vec<String> = comp_strs
                    .into_iter()
                    .map(|(z, frac)| format!("Z={} ({:.2}%)", z, frac * 100.0))
                    .collect();
                let _ = writeln!(md, "- {}\n", formatted_comps.join(", "));

                let _ = writeln!(
                    md,
                    "| Energy (MeV) | $\\mu$ (cm$^{{-1}}$) | Buildup Model |"
                );
                let _ = writeln!(md, "|--------------|-------------------|---------------|");
                for (ig, &energy) in self.report.physics_data.energy_groups.iter().enumerate() {
                    let _ = writeln!(
                        md,
                        "| {:.3} | {:.4e} | {} |",
                        energy, mat.mu_by_group[ig], mat.buildup_by_group[ig]
                    );
                }
                let _ = writeln!(md);
            }
        }

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
                let _ = writeln!(md, "### Detector {} at `[{:.3}, {:.3}, {:.3}]`", i + 1, res.position[0], res.position[1], res.position[2]);
                
                let mut buildup_parts = Vec::new();
                let mut total_freq = 0;
                for &count in res.buildup_material_frequencies.values() {
                    total_freq += count;
                }
                
                let mut sorted_freqs: Vec<_> = res.buildup_material_frequencies.iter().collect();
                sorted_freqs.sort_by_key(|&(_, count)| std::cmp::Reverse(count));
                
                for (&mid, count) in sorted_freqs {
                    let freq = (*count as f32 / total_freq as f32) * 100.0;
                    let m_name = match mid {
                        Some(id) if id < self.report.physics_data.evaluated_materials.len() => {
                            self.report.physics_data.evaluated_materials[id].name.clone()
                        }
                        Some(id) => format!("Material_{}", id),
                        None => "Vacuum".to_string(),
                    };
                    buildup_parts.push(format!("{} ({:.1}%)", m_name, freq));
                }

                let _ = writeln!(md, "- **Buildup Material:** {}", buildup_parts.join(", "));
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
                    let _ = writeln!(md, "#### Energy Group Details (Aggregated)");
                    let _ = writeln!(
                        md,
                        "| Group | Energy (MeV) | Segments | Uncollided Flux (Avg/Min/Max) | Buildup Factor (Avg/Min/Max) | Dose Rate (Unc.) (Avg/Min/Max) | Dose Rate (Total) (Avg/Min/Max) |"
                    );
                    let _ = writeln!(
                        md,
                        "|-------|--------------|----------|-------------------------------|------------------------------|--------------------------------|---------------------------------|"
                    );

                    for eg in &res.energy_group_details {
                        let count = eg.count as f32;
                        let _ = writeln!(
                            md,
                            "| {} | {:.3} | {} | {:.2e} / {:.2e} / {:.2e} | {:.2e} / {:.2e} / {:.2e} | {:.2e} / {:.2e} / {:.2e} | {:.2e} / {:.2e} / {:.2e} |",
                            eg.group_index,
                            eg.energy_mev,
                            eg.count,
                            eg.uncollided_flux_sum / count,
                            eg.uncollided_flux_min,
                            eg.uncollided_flux_max,
                            eg.buildup_factor_sum / count,
                            eg.buildup_factor_min,
                            eg.buildup_factor_max,
                            eg.uncollided_dose_rate_sum / count,
                            eg.uncollided_dose_rate_min,
                            eg.uncollided_dose_rate_max,
                            eg.dose_rate_with_buildup_sum / count,
                            eg.dose_rate_with_buildup_min,
                            eg.dose_rate_with_buildup_max
                        );
                    }
                    let _ = writeln!(md);
                }

                if !res.ray_path_summary.is_empty() {
                    let mut total_phys = 0.0;
                    for seg in &res.ray_path_summary {
                        total_phys += seg.physical_thickness_sum;
                    }

                    let _ = writeln!(md, "#### Ray Path Summary (Aggregated)");
                    let _ = writeln!(
                        md,
                        "| Material | Proportion (%) | Phys. Thickness (Avg/Min/Max) | Opt. Thickness (Avg/Min/Max) |"
                    );
                    let _ = writeln!(
                        md,
                        "|----------|----------------|-------------------------------|------------------------------|"
                    );

                    for seg in &res.ray_path_summary {
                        let count = seg.segments as f32;
                        let proportion = if total_phys > 0.0 {
                            (seg.physical_thickness_sum / total_phys) * 100.0
                        } else {
                            0.0
                        };

                        let avg_phys = seg.physical_thickness_sum / count;
                        let avg_opt = seg.optical_thickness_sum / count;

                        let display_name = match seg.material_id {
                            Some(id) if id < self.report.physics_data.evaluated_materials.len() => {
                                self.report.physics_data.evaluated_materials[id]
                                    .name
                                    .clone()
                            }
                            Some(_) => seg.material_name.clone(),
                            None => "Vacuum".to_string(),
                        };

                        let _ = writeln!(
                            md,
                            "| {} | {:.2}% | {:.2e} / {:.2e} / {:.2e} | {:.2e} / {:.2e} / {:.2e} |",
                            display_name,
                            proportion,
                            avg_phys,
                            seg.physical_thickness_min,
                            seg.physical_thickness_max,
                            avg_opt,
                            seg.optical_thickness_min,
                            seg.optical_thickness_max
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
            buildup_material_frequencies: std::collections::HashMap::new(),
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
            if let Some(seg) = res
                .ray_path_summary
                .iter_mut()
                .find(|s| s.material_id == material_id)
            {
                seg.segments += 1;
                seg.physical_thickness_sum += physical_thickness;
                seg.physical_thickness_min = seg.physical_thickness_min.min(physical_thickness);
                seg.physical_thickness_max = seg.physical_thickness_max.max(physical_thickness);
                seg.optical_thickness_sum += optical_thickness;
                seg.optical_thickness_min = seg.optical_thickness_min.min(optical_thickness);
                seg.optical_thickness_max = seg.optical_thickness_max.max(optical_thickness);
            } else {
                let material_name = material_id
                    .map(|id| format!("Material_{}", id))
                    .unwrap_or_else(|| "Vacuum".to_string());

                res.ray_path_summary.push(PathSegmentSummary {
                    material_id,
                    material_name,
                    segments: 1,
                    physical_thickness_sum: physical_thickness,
                    physical_thickness_min: physical_thickness,
                    physical_thickness_max: physical_thickness,
                    optical_thickness_sum: optical_thickness,
                    optical_thickness_min: optical_thickness,
                    optical_thickness_max: optical_thickness,
                });
            }
        }
    }

    fn record_buildup_material(&mut self, material_id: Option<usize>) {
        if let Some(res) = self.report.results.last_mut() {
            *res.buildup_material_frequencies.entry(material_id).or_insert(0) += 1;
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
            if let Some(eg) = res
                .energy_group_details
                .iter_mut()
                .find(|e| e.group_index == group_index)
            {
                eg.count += 1;
                eg.uncollided_flux_sum += uncollided_flux;
                eg.uncollided_flux_min = eg.uncollided_flux_min.min(uncollided_flux);
                eg.uncollided_flux_max = eg.uncollided_flux_max.max(uncollided_flux);
                eg.buildup_factor_sum += buildup;
                eg.buildup_factor_min = eg.buildup_factor_min.min(buildup);
                eg.buildup_factor_max = eg.buildup_factor_max.max(buildup);
                eg.uncollided_dose_rate_sum += uncollided_dose;
                eg.uncollided_dose_rate_min = eg.uncollided_dose_rate_min.min(uncollided_dose);
                eg.uncollided_dose_rate_max = eg.uncollided_dose_rate_max.max(uncollided_dose);
                eg.dose_rate_with_buildup_sum += total_dose;
                eg.dose_rate_with_buildup_min = eg.dose_rate_with_buildup_min.min(total_dose);
                eg.dose_rate_with_buildup_max = eg.dose_rate_with_buildup_max.max(total_dose);
            } else {
                res.energy_group_details.push(EnergyGroupResult {
                    group_index,
                    energy_mev: 0.0,
                    count: 1,
                    uncollided_flux_sum: uncollided_flux,
                    uncollided_flux_min: uncollided_flux,
                    uncollided_flux_max: uncollided_flux,
                    buildup_factor_sum: buildup,
                    buildup_factor_min: buildup,
                    buildup_factor_max: buildup,
                    uncollided_dose_rate_sum: uncollided_dose,
                    uncollided_dose_rate_min: uncollided_dose,
                    uncollided_dose_rate_max: uncollided_dose,
                    dose_rate_with_buildup_sum: total_dose,
                    dose_rate_with_buildup_min: total_dose,
                    dose_rate_with_buildup_max: total_dose,
                });
            }
            res.total_dose_rate_uncollided += uncollided_dose;
            res.total_dose_rate_with_buildup += total_dose;
        }
    }

    fn merge(&mut self, mut other: Self) {
        for other_res in other.report.results {
            if let Some(res) = self.report.results.iter_mut().find(|r| {
                (r.position[0] - other_res.position[0]).abs() < 1e-4
                    && (r.position[1] - other_res.position[1]).abs() < 1e-4
                    && (r.position[2] - other_res.position[2]).abs() < 1e-4
            }) {
                res.total_dose_rate_uncollided += other_res.total_dose_rate_uncollided;
                res.total_dose_rate_with_buildup += other_res.total_dose_rate_with_buildup;
                for (name, count) in other_res.buildup_material_frequencies {
                    *res.buildup_material_frequencies.entry(name).or_insert(0) += count;
                }
                for other_eg in other_res.energy_group_details {
                    if let Some(eg) = res
                        .energy_group_details
                        .iter_mut()
                        .find(|e| e.group_index == other_eg.group_index)
                    {
                        eg.count += other_eg.count;
                        eg.uncollided_flux_sum += other_eg.uncollided_flux_sum;
                        eg.uncollided_flux_min =
                            eg.uncollided_flux_min.min(other_eg.uncollided_flux_min);
                        eg.uncollided_flux_max =
                            eg.uncollided_flux_max.max(other_eg.uncollided_flux_max);
                        eg.buildup_factor_sum += other_eg.buildup_factor_sum;
                        eg.buildup_factor_min =
                            eg.buildup_factor_min.min(other_eg.buildup_factor_min);
                        eg.buildup_factor_max =
                            eg.buildup_factor_max.max(other_eg.buildup_factor_max);
                        eg.uncollided_dose_rate_sum += other_eg.uncollided_dose_rate_sum;
                        eg.uncollided_dose_rate_min = eg
                            .uncollided_dose_rate_min
                            .min(other_eg.uncollided_dose_rate_min);
                        eg.uncollided_dose_rate_max = eg
                            .uncollided_dose_rate_max
                            .max(other_eg.uncollided_dose_rate_max);
                        eg.dose_rate_with_buildup_sum += other_eg.dose_rate_with_buildup_sum;
                        eg.dose_rate_with_buildup_min = eg
                            .dose_rate_with_buildup_min
                            .min(other_eg.dose_rate_with_buildup_min);
                        eg.dose_rate_with_buildup_max = eg
                            .dose_rate_with_buildup_max
                            .max(other_eg.dose_rate_with_buildup_max);
                    } else {
                        res.energy_group_details.push(other_eg);
                    }
                }

                for other_seg in other_res.ray_path_summary {
                    if let Some(seg) = res
                        .ray_path_summary
                        .iter_mut()
                        .find(|s| s.material_id == other_seg.material_id)
                    {
                        seg.segments += other_seg.segments;
                        seg.physical_thickness_sum += other_seg.physical_thickness_sum;
                        seg.physical_thickness_min = seg
                            .physical_thickness_min
                            .min(other_seg.physical_thickness_min);
                        seg.physical_thickness_max = seg
                            .physical_thickness_max
                            .max(other_seg.physical_thickness_max);
                        seg.optical_thickness_sum += other_seg.optical_thickness_sum;
                        seg.optical_thickness_min = seg
                            .optical_thickness_min
                            .min(other_seg.optical_thickness_min);
                        seg.optical_thickness_max = seg
                            .optical_thickness_max
                            .max(other_seg.optical_thickness_max);
                    } else {
                        res.ray_path_summary.push(other_seg);
                    }
                }
            } else {
                self.report.results.push(other_res);
            }
        }
        self.report.warnings.append(&mut other.report.warnings);
    }
}
