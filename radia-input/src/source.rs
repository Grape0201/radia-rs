use glam::Vec3A;
use radia_core::source::{
    PointSource, generate_cuboid_source, generate_cylinder_source, generate_sphere_source,
};
use serde::{Deserialize, Serialize};

use crate::InputError;

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
pub enum SourceShapeInput {
    #[serde(alias = "Point")]
    Point { position: [f32; 3], intensity: f32 },
    #[serde(alias = "Cylinder")]
    Cylinder {
        start: [f32; 3],
        axis: [f32; 3],
        radius: f32,
        nd_c: usize,
        nd_h: usize,
        nd_r: usize,
        total_intensity: f32,
    },
    #[serde(alias = "Sphere")]
    Sphere {
        center: [f32; 3],
        radius: f32,
        nd_r: usize,
        nd_theta: usize,
        nd_phi: usize,
        total_intensity: f32,
    },
    #[serde(alias = "Cuboid")]
    Cuboid {
        min: [f32; 3],
        max: [f32; 3],
        nd_x: usize,
        nd_y: usize,
        nd_z: usize,
        total_intensity: f32,
    },
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SourceInput {
    pub energy_groups: Vec<f32>,
    pub intensity_by_group: Vec<f32>,
    #[serde(flatten)]
    pub shape: SourceShapeInput,
}

impl SourceShapeInput {
    pub fn build(self) -> Result<Vec<PointSource>, InputError> {
        match self {
            Self::Point {
                position,
                intensity,
            } => {
                if intensity < 0.0 {
                    return Err(InputError::InvalidSource(format!(
                        "Intensity cannot be negative, got {}",
                        intensity
                    )));
                }
                Ok(vec![PointSource {
                    position: Vec3A::from_array(position),
                    intensity,
                }])
            }
            Self::Cylinder {
                start,
                axis,
                radius,
                nd_c,
                nd_h,
                nd_r,
                total_intensity,
            } => {
                if radius <= 0.0 {
                    return Err(InputError::InvalidSource(format!(
                        "Cylinder radius must be positive, got {}",
                        radius
                    )));
                }
                if total_intensity < 0.0 {
                    return Err(InputError::InvalidSource(format!(
                        "Intensity cannot be negative, got {}",
                        total_intensity
                    )));
                }
                if nd_c == 0 || nd_h == 0 || nd_r == 0 {
                    return Err(InputError::InvalidSource(
                        "Cylinder subdivisions must be greater than 0".to_string(),
                    ));
                }
                let axis_vec = Vec3A::from_array(axis);
                if axis_vec.length_squared() < 1e-12 {
                    return Err(InputError::InvalidSource(
                        "Cylinder axis has zero length".to_string(),
                    ));
                }

                Ok(generate_cylinder_source(
                    Vec3A::from_array(start),
                    axis_vec,
                    radius,
                    nd_c,
                    nd_h,
                    nd_r,
                    total_intensity,
                ))
            }
            Self::Sphere {
                center,
                radius,
                nd_r,
                nd_theta,
                nd_phi,
                total_intensity,
            } => {
                if radius <= 0.0 {
                    return Err(InputError::InvalidSource(format!(
                        "Sphere radius must be positive, got {}",
                        radius
                    )));
                }
                if total_intensity < 0.0 {
                    return Err(InputError::InvalidSource(format!(
                        "Intensity cannot be negative, got {}",
                        total_intensity
                    )));
                }
                if nd_r == 0 || nd_theta == 0 || nd_phi == 0 {
                    return Err(InputError::InvalidSource(
                        "Sphere subdivisions must be greater than 0".to_string(),
                    ));
                }

                Ok(generate_sphere_source(
                    Vec3A::from_array(center),
                    radius,
                    nd_r,
                    nd_theta,
                    nd_phi,
                    total_intensity,
                ))
            }
            Self::Cuboid {
                min,
                max,
                nd_x,
                nd_y,
                nd_z,
                total_intensity,
            } => {
                if min[0] >= max[0] || min[1] >= max[1] || min[2] >= max[2] {
                    return Err(InputError::InvalidSource(format!(
                        "Cuboid min bounds must be strictly less than max bounds. min: {:?}, max: {:?}",
                        min, max
                    )));
                }
                if total_intensity < 0.0 {
                    return Err(InputError::InvalidSource(format!(
                        "Intensity cannot be negative, got {}",
                        total_intensity
                    )));
                }
                if nd_x == 0 || nd_y == 0 || nd_z == 0 {
                    return Err(InputError::InvalidSource(
                        "Cuboid subdivisions must be greater than 0".to_string(),
                    ));
                }

                Ok(generate_cuboid_source(
                    min[0],
                    max[0],
                    min[1],
                    max[1],
                    min[2],
                    max[2],
                    nd_x,
                    nd_y,
                    nd_z,
                    total_intensity,
                ))
            }
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
        let sources = input.shape.build().unwrap();

        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].position, Vec3A::new(1.0, 2.0, 3.0));
        assert_eq!(sources[0].intensity, 100.0);
        assert_eq!(input.energy_groups, vec![1.0]);
    }

    #[test]
    fn test_invalid_cylinder_source() {
        let json = r#"{
            "type": "Cylinder",
            "start": [0.0, 0.0, 0.0],
            "axis": [0.0, 0.0, 0.0],
            "radius": 5.0,
            "nd_c": 10,
            "nd_h": 10,
            "nd_r": 5,
            "total_intensity": 100.0,
            "energy_groups": [1.0],
            "intensity_by_group": [1.0]
        }"#;
        // Zero length axis should fail
        let input: SourceInput = serde_json::from_str(json).unwrap();
        assert!(input.shape.build().is_err());
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
        let sources = input.shape.build().unwrap();

        assert_eq!(sources.len(), 32); // 2 * 4 * 4
        let total_built_intensity: f32 = sources.iter().map(|s| s.intensity).sum();
        assert!((total_built_intensity - 1000.0).abs() < 1e-4);
    }
}
