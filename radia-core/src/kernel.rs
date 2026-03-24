use glam::Vec3A;
use rayon::prelude::*;

use crate::csg::World;
use crate::material::{GroupIndex, MaterialIndex};
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
pub fn select_buildup_material(segment_ots: &[(MaterialIndex, f32)]) -> Option<MaterialIndex> {
    if segment_ots.is_empty() {
        return None;
    }

    // Traverse from detector back to source to find the last two distinct layers
    let last_idx = segment_ots.len() - 1;
    let last_mat_id = segment_ots[last_idx].0;
    let mut last_layer_ot = 0.0;

    let mut i = segment_ots.len();
    // Sum the last contiguous block of the same material
    while i > 0 && segment_ots[i - 1].0 == last_mat_id {
        last_layer_ot += segment_ots[i - 1].1;
        i -= 1;
    }

    // Find the next distinct material block further back
    let mut prev_mat_id = None;
    let mut prev_layer_ot = 0.0;

    while i > 0 {
        let current_mat_id = segment_ots[i - 1].0;
        if prev_mat_id.is_none() {
            if current_mat_id != last_mat_id {
                prev_mat_id = Some(current_mat_id);
                prev_layer_ot += segment_ots[i - 1].1;
            }
        } else if prev_mat_id == Some(current_mat_id) {
            prev_layer_ot += segment_ots[i - 1].1;
        } else {
            // Already found the full previous block
            break;
        }
        i -= 1;
    }

    #[allow(clippy::collapsible_if)]
    if let Some(pmid) = prev_mat_id {
        if prev_layer_ot > last_layer_ot {
            return Some(pmid as MaterialIndex);
        }
    }

    Some(last_mat_id as MaterialIndex)
}

/// Calculate the total integrated dose rate from multiple point sources over an energy spectrum.
/// This loops over both energy divisions and source divisions, and is optimized by extracting
/// ray intersections once per point source instead of once per energy group.
///
/// * `get_mu` - A closure mapping `(material_index, group_index)` to linear attenuation [cm^-1].
/// * `get_buildup` - A closure mapping `(material_index, group_index, optical_thickness)`
///   returning the buildup factor.
/// * `world` - The CSG defined world.
/// * `conversion_factors` - A slice of factors (e.g. from flux to effective dose) with one element
///   per energy group.
/// * `detector_position` - The 3D coordinates `Vec3A` of the detector.
/// * `sources` - A slice of `PointSource` instances to integrate over.
pub fn calculate_dose_rate<F, B>(
    get_mu: &F,
    get_buildup: &B,
    world: &World,
    conversion_factors: &[f32],
    intensity_by_group: &[f32],
    detector_position: Vec3A,
    sources: &[PointSource],
    collector: &mut impl DoseCollector,
) -> f32
where
    F: Fn(MaterialIndex, GroupIndex) -> f32,
    B: Fn(MaterialIndex, GroupIndex, f32) -> f32,
{
    let mut total_dose = 0.0;
    let num_groups = conversion_factors.len();
    let mut segments_buffer = Vec::with_capacity(32); // pre-allocate for performance
    let mut buffer_ts = Vec::with_capacity(64); // pre-allocate for performance
    let mut buffer_merged_ts = Vec::with_capacity(64); // pre-allocate for performance
    let mut segment_ots_buffer: Vec<(MaterialIndex, f32)> = Vec::with_capacity(32); // (material_id, ot)

    collector.begin_detector(detector_position);

    // Loop over source division
    for source in sources {
        collector.begin_source(source.position, source.intensity);
        let diff = detector_position - source.position;

        let distance_sq = diff.length_squared();
        if distance_sq < 1e-10 {
            // Detector is exactly at a source position, avoid infinity
            continue;
        }

        let ray = Ray {
            origin: source.position,
            vector: diff,
        };

        // Get material segments once for this source (Optimized step for speed)
        world.get_ray_segments(
            &ray,
            &mut segments_buffer,
            &mut buffer_ts,
            &mut buffer_merged_ts,
        );

        let geometric_attenuation = 1.0 / (4.0 * std::f32::consts::PI * distance_sq);
        let mut source_dose = 0.0;

        // Loop over energy division
        for ig in 0..num_groups {
            let mut total_optical_thickness = 0.0;
            segment_ots_buffer.clear();

            // 1. Calculate optical thickness for each segment and total
            for &(mat_id, length) in &segments_buffer {
                if let Some(mat_id) = mat_id {
                    let ot = get_mu(mat_id as MaterialIndex, ig as GroupIndex) * length;
                    total_optical_thickness += ot;
                    segment_ots_buffer.push((mat_id, ot));
                }
                collector.record_ray_segment(mat_id, length, 0.0);
            }

            // 2. Determine the buildup material ID for this Ray and Energy Group using refined logic.
            let buildup_material_id = select_buildup_material(&segment_ots_buffer);
            collector.record_buildup_material(buildup_material_id);

            let buildup = if let Some(mat_id) = buildup_material_id {
                get_buildup(mat_id, ig as GroupIndex, total_optical_thickness)
            } else {
                1.0
            };
            let material_attenuation = (-total_optical_thickness).exp();

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

            // flux to dose conversion
            source_dose += group_total_dose;
        }

        total_dose += source.intensity * geometric_attenuation * source_dose;
    }

    total_dose
}

pub fn calculate_dose_rate_no_collector<F, B>(
    get_mu: &F,
    get_buildup: &B,
    world: &World,
    conversion_factors: &[f32],
    intensity_by_group: &[f32],
    detector_position: Vec3A,
    sources: &[PointSource],
) -> f32
where
    F: Fn(MaterialIndex, GroupIndex) -> f32,
    B: Fn(MaterialIndex, GroupIndex, f32) -> f32,
{
    let mut total_dose = 0.0;
    let num_groups = conversion_factors.len();
    let mut segments_buffer = Vec::with_capacity(32);
    let mut buffer_ts = Vec::with_capacity(64);
    let mut buffer_merged_ts = Vec::with_capacity(64);
    let mut segment_ots_buffer: Vec<(MaterialIndex, f32)> = Vec::with_capacity(32);

    for source in sources {
        let diff = detector_position - source.position;
        let distance_sq = diff.length_squared();
        if distance_sq < 1e-10 {
            continue;
        }

        let ray = Ray {
            origin: source.position,
            vector: diff,
        };

        world.get_ray_segments(
            &ray,
            &mut segments_buffer,
            &mut buffer_ts,
            &mut buffer_merged_ts,
        );

        let geometric_attenuation = 1.0 / (4.0 * std::f32::consts::PI * distance_sq);
        let mut source_dose = 0.0;

        for ig in 0..num_groups {
            let mut total_optical_thickness = 0.0;
            segment_ots_buffer.clear();

            for &(mat_id, length) in &segments_buffer {
                let Some(mat_id) = mat_id else {
                    continue;
                };
                let ot = get_mu(mat_id as MaterialIndex, ig as GroupIndex) * length;
                total_optical_thickness += ot;
                segment_ots_buffer.push((mat_id, ot));
            }

            let buildup_material_id = select_buildup_material(&segment_ots_buffer);

            let buildup = if let Some(mat_id) = buildup_material_id {
                get_buildup(mat_id, ig as GroupIndex, total_optical_thickness)
            } else {
                1.0
            };

            let material_attenuation = (-total_optical_thickness).exp();
            let uncollided_flux = material_attenuation * intensity_by_group[ig];
            let uncollided_dose = conversion_factors[ig] * uncollided_flux;
            let group_total_dose = uncollided_dose * buildup;

            source_dose += group_total_dose;
        }

        total_dose += source.intensity * geometric_attenuation * source_dose;
    }

    total_dose
}

pub fn calculate_dose_rate_parallel<F, B>(
    get_mu: &F,
    get_buildup: &B,
    world: &World,
    conversion_factors: &[f32],
    intensity_by_group: &[f32],
    detector_position: Vec3A,
    sources: &[PointSource],
    chunk_size: usize,
) -> f32
where
    F: Fn(MaterialIndex, GroupIndex) -> f32 + Sync,
    B: Fn(MaterialIndex, GroupIndex, f32) -> f32 + Sync,
{
    tracing::info!(
        "Starting parallel dose rate calculation for {} sources with chunk size {}",
        sources.len(),
        chunk_size
    );
    sources
        .par_chunks(chunk_size)
        .map(|source_chunk| {
            let mut collector = FastCollector::default();
            calculate_dose_rate(
                get_mu,
                get_buildup,
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
    use crate::csg::{CSGNode, Cell};
    use crate::primitive::Primitive;

    #[test]
    fn test_unshielded_point_source_spectrum() {
        let world = World {
            primitives: vec![],
            cells: vec![],
        };

        let source = PointSource {
            position: Vec3A::ZERO,
            intensity: 100.0,
        };

        // Detector at r = 10.0
        let detector = Vec3A::new(10.0, 0.0, 0.0);

        let get_mu = |_, _| 0.0;
        let get_buildup = |_, _, _| 1.0;

        let conversion_factors = vec![1.0];
        let intensity_by_group = vec![1.0];

        let mut collector = FastCollector::default();
        let dose = calculate_dose_rate(
            &get_mu,
            &get_buildup,
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
        // Simple case: one material
        assert_eq!(select_buildup_material(&[(0, 10.0)]), Some(0));

        // Two materials: last is thicker
        assert_eq!(select_buildup_material(&[(0, 1.0), (1, 10.0)]), Some(1));

        // Two materials: previous is thicker
        assert_eq!(select_buildup_material(&[(0, 10.0), (1, 1.0)]), Some(0));

        // Contiguous segments grouping
        assert_eq!(
            select_buildup_material(&[(0, 5.0), (0, 6.0), (1, 5.0), (1, 5.0)]),
            Some(0) // OT1=11 > OT2=10
        );

        // Gap with same material: [M1, M2, M1]
        // Last layer is M1, previous layer is M2.
        assert_eq!(
            select_buildup_material(&[(0, 10.0), (1, 5.0), (0, 1.0)]),
            Some(1)
        ); // OT(M2)=5 > OT(M1-last)=1
    }

    #[test]
    fn test_multi_material_buildup_selection() {
        // Segments: [Material 0 (L=10.0), Material 1 (L=1.0)]
        // At mu_0 = 1.0, mu_1 = 1.0:
        // OT_0 = 10.0, OT_1 = 1.0.
        // Total OT = 11.0.
        // Rule: OT(Prev=0) = 10.0 > OT(Last=1) = 1.0. Adopt Material 0.

        let source = PointSource {
            position: Vec3A::ZERO,
            intensity: 1.0,
        };
        let detector = Vec3A::new(15.0, 0.0, 0.0);

        let get_mu = |mat_id, _| if mat_id == 0 { 1.0 } else { 1.0 };
        let get_buildup = |mat_id, _, ot: f32| 1.0 + (mat_id as f32) + ot;
        let conversion_factors = vec![1.0];
        let intensity_by_group = vec![1.0];

        let world = World {
            primitives: vec![
                Primitive::RectangularParallelPiped {
                    min: Vec3A::new(-1.0, -1.0, -1.0),
                    max: Vec3A::new(10.0, 1.0, 1.0),
                },
                Primitive::RectangularParallelPiped {
                    min: Vec3A::new(10.0, -1.0, -1.0),
                    max: Vec3A::new(11.0, 1.0, 1.0),
                },
            ],
            cells: vec![
                Cell {
                    csg: CSGNode::Primitive(0),
                    material_id: 0,
                },
                Cell {
                    csg: CSGNode::Primitive(1),
                    material_id: 1,
                },
            ],
        };

        let mut collector = FastCollector::default();
        let dose = calculate_dose_rate(
            &get_mu,
            &get_buildup,
            &world,
            &conversion_factors,
            &intensity_by_group,
            detector,
            &[source],
            &mut collector,
        );

        // Total OT = 10.0 + 1.0 = 11.0
        // Selected mat_id = 0
        // Buildup = 1.0 + 0 + 11.0 = 12.0
        // Expected Dose = 1.0 * (1/(4*PI*15^2)) * 12.0 * exp(-11.0)

        let expected_buildup = 12.0;
        let expected_dose = 1.0
            * (1.0 / (4.0 * std::f32::consts::PI * 225.0))
            * expected_buildup
            * (-11.0f32).exp();

        assert!(
            (dose - expected_dose).abs() < 1e-9,
            "Dose {} != expected {}",
            dose,
            expected_dose
        );
    }
}
