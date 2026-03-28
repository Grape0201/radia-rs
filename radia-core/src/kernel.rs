use glam::Vec3A;
use rayon::prelude::*;

use crate::csg::World;
use crate::material::MaterialIndex;
use crate::primitive::Ray;
use crate::source::PointSource;

pub trait DoseCollector {
    fn begin_detector(&mut self, position: Vec3A);
    fn begin_source(&mut self, position: Vec3A, intensity: f32);
    fn record_ray_segment(
        &mut self,
        material_id: Option<MaterialIndex>,
        physical_thickness: f32,
        optical_thickness: f32,
    );
    fn record_buildup_material(&mut self, material_id: Option<MaterialIndex>);
    fn record_energy_group(
        &mut self,
        group_index: usize,
        uncollided_flux: f32,
        buildup: f32,
        uncollided_dose: f32,
        total_dose: f32,
    );
}

#[derive(Default)]
pub struct FastCollector;

impl DoseCollector for FastCollector {
    #[inline(always)]
    fn begin_detector(&mut self, _: Vec3A) {}
    #[inline(always)]
    fn begin_source(&mut self, _: Vec3A, _: f32) {}
    #[inline(always)]
    fn record_ray_segment(&mut self, _: Option<MaterialIndex>, _: f32, _: f32) {}
    #[inline(always)]
    fn record_buildup_material(&mut self, _: Option<MaterialIndex>) {}
    #[inline(always)]
    fn record_energy_group(&mut self, _: usize, _: f32, _: f32, _: f32, _: f32) {}
}
/// Determine the appropriate buildup material ID for a ray path.
///
/// Rule:
/// 1. Group segments into contiguous blocks of the same material.
/// 2. Compare the last block (closest to detector) with the previous distinct material block.
/// 3. If the previous block's optical thickness is larger than the last block's, adopt the previous material.
pub fn select_buildup_material(grouped_segments: &[(MaterialIndex, f32)], mus: &[f32], num_groups: usize, ig: usize) -> Option<MaterialIndex> {
    if grouped_segments.is_empty() {
        return None;
    }

    let last_idx = grouped_segments.len() - 1;
    let (last_mat, last_len) = grouped_segments[last_idx];
    let last_ot = mus[last_mat * num_groups + ig] * last_len;

    if grouped_segments.len() >= 2 {
        let (prev_mat, prev_len) = grouped_segments[last_idx - 1];
        let prev_ot = mus[prev_mat * num_groups + ig] * prev_len;
        if prev_ot > last_ot {
            return Some(prev_mat);
        }
    }

    Some(last_mat)
}

pub fn calculate_dose_rate(
    physics: &crate::physics::MaterialPhysicsTable,
    world: &World,
    conversion_factors: &[f32],
    intensity_by_group: &[f32],
    detector_position: Vec3A,
    sources: &[PointSource],
    collector: &mut impl DoseCollector,
) -> f32 {
    let mut total_dose = 0.0;
    let num_groups = conversion_factors.len();
    
    // Thread-local buffers to avoid allocations
    let mut segments_buffer = Vec::with_capacity(32);
    let mut buffer_ts = Vec::with_capacity(64);
    let mut grouped_segments: Vec<(MaterialIndex, f32)> = Vec::with_capacity(32);
    let mut total_ots = vec![0.0; num_groups];

    collector.begin_detector(detector_position);

    let mut prefactors = vec![0.0; num_groups];
    for ig in 0..num_groups {
        prefactors[ig] = conversion_factors[ig] * intensity_by_group[ig];
    }

    // Process sources in batches to optimize cache efficiency and enable
    // vectorized intersection tests across multiple rays.
    let batch_size = 64;
    let n_prims = world.primitives.len();
    let mut rays = Vec::with_capacity(batch_size);
    let mut batch_ranges = vec![(f32::INFINITY, f32::NEG_INFINITY); batch_size * n_prims];

    for source_batch in sources.chunks(batch_size) {
        rays.clear();
        for source in source_batch {
            rays.push(Ray { 
                origin: source.position, 
                vector: detector_position - source.position 
            });
        }

        if n_prims > 0 {
            batch_ranges.resize(rays.len() * n_prims, (f32::INFINITY, f32::NEG_INFINITY));
            world.primitives.get_ranges_batched(&rays, &mut batch_ranges);
        }

        for (i, source) in source_batch.iter().enumerate() {
            collector.begin_source(source.position, source.intensity);
            let ray = &rays[i];
            let distance_sq = ray.vector.length_squared();
            if distance_sq < 1e-10 { continue; }

            if n_prims > 0 {
                let ranges = &batch_ranges[i * n_prims .. (i + 1) * n_prims];
                world.get_ray_segments_from_ranges(ray, ranges, &mut segments_buffer, &mut buffer_ts);
            } else {
                segments_buffer.clear();
                segments_buffer.push((None, ray.vector.length()));
            }

            // 1. Group contiguous segments and calculate total OTs for all energy groups at once
            let mu_data = physics.get_mu_data();
            grouped_segments.clear();
            total_ots.fill(0.0);
            for &(mat_id, length) in &segments_buffer {
                if let Some(mat_id) = mat_id {
                    let m_idx = mat_id as MaterialIndex;
                    if let Some(last) = grouped_segments.last_mut() {
                        if last.0 == m_idx {
                            last.1 += length;
                        } else {
                            grouped_segments.push((m_idx, length));
                        }
                    } else {
                        grouped_segments.push((m_idx, length));
                    }

                    let mus = &mu_data[m_idx * num_groups..(m_idx + 1) * num_groups];
                    for ig in 0..num_groups {
                        total_ots[ig] += mus[ig] * length;
                    }
                }
                collector.record_ray_segment(mat_id, length, 0.0);
            }

            let geometric_attenuation = 1.0 / (4.0 * std::f32::consts::PI * distance_sq);
            let mut source_dose = 0.0;

            // 2. Loop over energy division
            for ig in 0..num_groups {
                let buildup_material_id = select_buildup_material(&grouped_segments, mu_data, num_groups, ig);
                collector.record_buildup_material(buildup_material_id);

                let total_ot = total_ots[ig];
                let buildup = if let Some(mat_id) = buildup_material_id {
                    physics.get_buildup(mat_id, ig, total_ot)
                } else {
                    1.0
                };
                let material_attenuation = (-total_ot).exp();

                let uncollided_flux = material_attenuation * intensity_by_group[ig];
                let uncollided_dose = conversion_factors[ig] * uncollided_flux;
                let group_total_dose = uncollided_dose * buildup;

                collector.record_energy_group(
                    ig,
                    uncollided_flux * geometric_attenuation * source.intensity,
                    buildup,
                    uncollided_dose * geometric_attenuation * source.intensity,
                    group_total_dose * geometric_attenuation * source.intensity,
                );

                source_dose += prefactors[ig] * material_attenuation * buildup;
            }

            total_dose += source.intensity * geometric_attenuation * source_dose;
        }
    }

    total_dose
}

pub fn calculate_dose_rate_no_collector(
    physics: &crate::physics::MaterialPhysicsTable,
    world: &World,
    conversion_factors: &[f32],
    intensity_by_group: &[f32],
    detector_position: Vec3A,
    sources: &[PointSource],
) -> f32 {
    let mut total_dose = 0.0;
    let num_groups = conversion_factors.len();
    let mut segments_buffer = Vec::with_capacity(32);
    let mut buffer_ts = Vec::with_capacity(64);
    let mut grouped_segments: Vec<(MaterialIndex, f32)> = Vec::with_capacity(32);
    let mut total_ots = vec![0.0; num_groups];

    let mut prefactors = vec![0.0; num_groups];
    for ig in 0..num_groups {
        prefactors[ig] = conversion_factors[ig] * intensity_by_group[ig];
    }

    // Process sources in batches to optimize cache efficiency and enable
    // vectorized intersection tests across multiple rays.
    let batch_size = 64;
    let n_prims = world.primitives.len();
    let mut rays = Vec::with_capacity(batch_size);
    let mut batch_ranges = vec![(f32::INFINITY, f32::NEG_INFINITY); batch_size * n_prims];

    for source_batch in sources.chunks(batch_size) {
        rays.clear();
        for source in source_batch {
            rays.push(Ray { 
                origin: source.position, 
                vector: detector_position - source.position 
            });
        }

        if n_prims > 0 {
            batch_ranges.resize(rays.len() * n_prims, (f32::INFINITY, f32::NEG_INFINITY));
            world.primitives.get_ranges_batched(&rays, &mut batch_ranges);
        }

        for (i, source) in source_batch.iter().enumerate() {
            let ray = &rays[i];
            let distance_sq = ray.vector.length_squared();
            if distance_sq < 1e-10 { continue; }

            if n_prims > 0 {
                let ranges = &batch_ranges[i * n_prims .. (i + 1) * n_prims];
                world.get_ray_segments_from_ranges(ray, ranges, &mut segments_buffer, &mut buffer_ts);
            } else {
                segments_buffer.clear();
                segments_buffer.push((None, ray.vector.length()));
            }

            let mu_data = physics.get_mu_data();
            grouped_segments.clear();
            total_ots.fill(0.0);
            for &(mat_id, length) in &segments_buffer {
                if let Some(mat_id) = mat_id {
                    let m_idx = mat_id as MaterialIndex;
                    if let Some(last) = grouped_segments.last_mut() {
                        if last.0 == m_idx {
                            last.1 += length;
                        } else {
                            grouped_segments.push((m_idx, length));
                        }
                    } else {
                        grouped_segments.push((m_idx, length));
                    }
                    
                    let mus = &mu_data[m_idx * num_groups..(m_idx + 1) * num_groups];
                    for ig in 0..num_groups {
                        total_ots[ig] += mus[ig] * length;
                    }
                }
            }

            let geometric_attenuation = 1.0 / (4.0 * std::f32::consts::PI * distance_sq);
            let mut source_dose = 0.0;

            for ig in 0..num_groups {
                let buildup_material_id = select_buildup_material(&grouped_segments, mu_data, num_groups, ig);
                let total_ot = total_ots[ig];
                let buildup = if let Some(mat_id) = buildup_material_id {
                    physics.get_buildup(mat_id, ig, total_ot)
                } else {
                    1.0
                };

                source_dose += prefactors[ig] * (-total_ot).exp() * buildup;
            }

            total_dose += source.intensity * geometric_attenuation * source_dose;
        }
    }

    total_dose
}

pub fn calculate_dose_rate_parallel(
    physics: &crate::physics::MaterialPhysicsTable,
    world: &World,
    conversion_factors: &[f32],
    intensity_by_group: &[f32],
    detector_position: Vec3A,
    sources: &[PointSource],
    chunk_size: usize,
) -> f32 {
    sources
        .par_chunks(chunk_size)
        .map(|source_chunk| {
            let mut collector = FastCollector::default();
            calculate_dose_rate(
                physics,
                world,
                conversion_factors,
                intensity_by_group,
                detector_position,
                source_chunk,
                &mut collector,
            )
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::csg::PrimitiveStorage;
    use crate::physics::{MaterialPhysicsTable, BuildupModel};

    fn setup_dummy_physics() -> MaterialPhysicsTable {
        let mu_data = vec![0.0];
        let buildup_models = vec![BuildupModel::Constant(1.0)];
        MaterialPhysicsTable::generate_for_test(mu_data, buildup_models, 1, 1)
    }

    #[test]
    fn test_unshielded_point_source_spectrum() {
        let world = World {
            primitives: PrimitiveStorage::new(),
            cells: vec![],
        };

        let source = PointSource {
            position: Vec3A::ZERO,
            intensity: 100.0,
        };

        let detector = Vec3A::new(10.0, 0.0, 0.0);
        let physics = setup_dummy_physics();

        let conversion_factors = vec![1.0];
        let intensity_by_group = vec![1.0];

        let mut collector = FastCollector::default();
        let dose = calculate_dose_rate(
            &physics,
            &world,
            &conversion_factors,
            &intensity_by_group,
            detector,
            &[source],
            &mut collector,
        );

        let expected_geometric = 1.0 / (4.0 * std::f32::consts::PI * 100.0);
        let expected_dose = 100.0 * expected_geometric * 1.0;
        assert!((dose - expected_dose).abs() < 1e-6);
    }

    #[test]
    fn test_select_buildup_material() {
        let _num_groups = 1;
        let mu_data = vec![1.0, 2.0]; // Mat0: mu=1, Mat1: mu=2
        
        // Simple case: one material
        assert_eq!(select_buildup_material(&[(0, 10.0)], &mu_data, 1, 0), Some(0));

        // Two materials: last is thicker
        // Mat0: OT = 1*1 = 1, Mat1: OT = 2*10 = 20
        assert_eq!(select_buildup_material(&[(0, 1.0), (1, 10.0)], &mu_data, 1, 0), Some(1));

        // Two materials: previous is thicker
        // Mat0: OT = 1*10 = 10, Mat1: OT = 2*1 = 2
        assert_eq!(select_buildup_material(&[(0, 10.0), (1, 1.0)], &mu_data, 1, 0), Some(0));
    }
}
