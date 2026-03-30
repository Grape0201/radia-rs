use garde::Validate;
use glam::Vec3A;
use radia_core::source::{
    PointSource, generate_cuboid_source, generate_cylinder_source, generate_sphere_source,
};
use serde::{Deserialize, Serialize};

use crate::common::{MinMaxBounds, is_all_zero_or_more, is_sorted, is_vector_longer_than_epsilon};

#[derive(Serialize, Deserialize, Debug, Validate, Clone)]
#[serde(tag = "type")]
pub enum SourceShapeInput {
    #[serde(alias = "Point")]
    Point {
        #[garde(skip)]
        position: [f32; 3],
        #[garde(range(min = 0.0))]
        intensity: f32,
    },
    #[serde(alias = "Cylinder")]
    Cylinder {
        #[garde(skip)]
        start: [f32; 3],
        #[garde(custom(is_vector_longer_than_epsilon))]
        axis: [f32; 3],
        #[garde(range(min = 0.0))]
        radius: f32,
        #[garde(range(min = 1))]
        nd_c: usize,
        #[garde(range(min = 1))]
        nd_h: usize,
        #[garde(range(min = 1))]
        nd_r: usize,
        #[garde(range(min = 0.0))]
        total_intensity: f32,
    },
    #[serde(alias = "Sphere")]
    Sphere {
        #[garde(skip)]
        center: [f32; 3],
        #[garde(range(min = 0.0))]
        radius: f32,
        #[garde(range(min = 1))]
        nd_r: usize,
        #[garde(range(min = 1))]
        nd_theta: usize,
        #[garde(range(min = 1))]
        nd_phi: usize,
        #[garde(range(min = 0.0))]
        total_intensity: f32,
    },
    #[serde(alias = "Cuboid")]
    Cuboid {
        #[garde(dive)]
        #[serde(flatten)]
        bounds: MinMaxBounds,
        #[garde(range(min = 1))]
        nd_x: usize,
        #[garde(range(min = 1))]
        nd_y: usize,
        #[garde(range(min = 1))]
        nd_z: usize,
        #[garde(range(min = 0.0))]
        total_intensity: f32,
    },
}

#[derive(Serialize, Deserialize, Debug, Validate, Clone)]
pub struct SourceInput {
    #[garde(custom(is_all_zero_or_more), custom(is_sorted), length(min = 1))]
    pub energy_groups: Vec<f32>,
    #[garde(custom(is_all_zero_or_more), length(min = 1))]
    pub intensity_by_group: Vec<f32>,
    #[garde(dive)]
    #[serde(flatten)]
    pub shape: SourceShapeInput,
}

impl SourceShapeInput {
    pub fn build(self) -> Vec<PointSource> {
        match self {
            Self::Point {
                position,
                intensity,
            } => vec![PointSource {
                position: Vec3A::from_array(position),
                intensity,
            }],
            Self::Cylinder {
                start,
                axis,
                radius,
                nd_c,
                nd_h,
                nd_r,
                total_intensity,
            } => generate_cylinder_source(
                Vec3A::from_array(start),
                Vec3A::from_array(axis),
                radius,
                nd_c,
                nd_h,
                nd_r,
                total_intensity,
            ),
            Self::Sphere {
                center,
                radius,
                nd_r,
                nd_theta,
                nd_phi,
                total_intensity,
            } => generate_sphere_source(
                Vec3A::from_array(center),
                radius,
                nd_r,
                nd_theta,
                nd_phi,
                total_intensity,
            ),
            Self::Cuboid {
                bounds,
                nd_x,
                nd_y,
                nd_z,
                total_intensity,
            } => generate_cuboid_source(
                bounds.min[0],
                bounds.max[0],
                bounds.min[1],
                bounds.max[1],
                bounds.min[2],
                bounds.max[2],
                nd_x,
                nd_y,
                nd_z,
                total_intensity,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_source() {
        let json = r#"{
            "type": "Point",
            "position": [1.0, 2.0, 3.0],
            "intensity": 100.0,
            "energy_groups": [1.0],
            "intensity_by_group": [1.0]
        }"#;

        let input: SourceInput = serde_json::from_str(json).unwrap();
        let sources = input.shape.build();

        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].position, Vec3A::new(1.0, 2.0, 3.0));
        assert_eq!(sources[0].intensity, 100.0);
        assert_eq!(input.energy_groups, vec![1.0]);
    }

    #[test]
    fn test_invalid_cylinder_source() {
        let yaml = r#"type: Cylinder
start: [0.0, 0.0, 0.0]
axis: [0.0, 0.0, 0.0]
radius: 5.0
nd_c: 10
nd_h: 10
nd_r: 5
total_intensity: 100.0
energy_groups: [1.0]
intensity_by_group: [1.0]
        "#;
        // Zero length axis should fail
        let input: Result<SourceInput, _> = serde_saphyr::from_str_valid(yaml);
        assert!(input.is_err());
    }

    #[test]
    fn test_sphere_source() {
        let json = r#"{
            "type": "Sphere",
            "center": [0.0, 0.0, 0.0],
            "radius": 10.0,
            "nd_r": 2,
            "nd_theta": 4,
            "nd_phi": 4,
            "total_intensity": 1000.0,
            "energy_groups": [1.0],
            "intensity_by_group": [1.0]
        }"#;

        let input: SourceInput = serde_json::from_str(json).unwrap();
        let sources = input.shape.build();

        assert_eq!(sources.len(), 32); // 2 * 4 * 4
        let total_built_intensity: f32 = sources.iter().map(|s| s.intensity).sum();
        assert!((total_built_intensity - 1000.0).abs() < 1e-4);
    }
}
