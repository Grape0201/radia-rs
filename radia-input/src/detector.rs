use garde::Validate;
use glam::Vec3A;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Validate)]
pub struct DetectorInput {
    #[garde(length(min = 2))]
    pub name: String,
    #[garde(skip)]
    pub position: [f32; 3],
}

impl DetectorInput {
    pub fn build(self) -> (String, Vec3A) {
        (self.name, Vec3A::from_array(self.position))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detector_deserialization() {
        let json = r#"{
            "name": "Det1",
            "position": [10.0, 20.0, -5.5]
        }"#;

        let det: DetectorInput = serde_json::from_str(json).unwrap();
        let (name, pos) = det.build();
        assert_eq!(name, "Det1");
        assert_eq!(pos, Vec3A::new(10.0, 20.0, -5.5));
    }
}
