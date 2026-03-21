use serde::{Deserialize, Serialize};

pub const EPSILON2: f32 = 1e-12; // length^2 [cm^2]

pub(crate) fn is_vector_longer_than_epsilon(v: &[f32; 3], _ctx: &()) -> garde::Result {
    let length_sq: f32 = v.iter().map(|x| x * x).sum();
    if length_sq > EPSILON2 {
        Ok(())
    } else {
        Err(garde::Error::new("cylinder vector is too short"))
    }
}
#[derive(Serialize, Deserialize, Debug)]
pub struct MinMaxBounds {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

impl garde::Validate for MinMaxBounds {
    type Context = ();
    fn validate_into(
        &self,
        _cx: &Self::Context,
        parent: &mut dyn FnMut() -> garde::Path,
        report: &mut garde::Report,
    ) {
        let coords = ["X", "Y", "Z"];
        for (i, coord) in coords.iter().enumerate() {
            if self.min[i] >= self.max[i] {
                report.append(
                    parent(),
                    garde::Error::new(format!(
                        "min bounds must be strictly less than max bounds. min: {:?}, max: {:?} for coordinate {}",
                        self.min, self.max, coord
                    )),
                );
            }
        }
    }
}

pub(crate) fn is_all_zero_or_more(v: &[f32], _ctx: &()) -> garde::Result {
    if v.iter().all(|&x| x >= 0.0) {
        Ok(())
    } else {
        Err(garde::Error::new("all elements must be more than zero"))
    }
}

pub(crate) fn is_sorted(v: &[f32], _ctx: &()) -> garde::Result {
    if v.windows(2).all(|w| w[0] <= w[1]) {
        Ok(())
    } else {
        Err(garde::Error::new("elements must be sorted"))
    }
}
