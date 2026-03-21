use crate::constants::{EPSILON, EPSILON2};
use glam::Vec3A;

pub struct PointSource {
    pub position: Vec3A,
    pub intensity: f32, // photons/sec or similar
}

pub fn generate_cylinder_source(
    start: Vec3A,
    axis: Vec3A, // vector from start to end
    radius: f32,
    nd_c: usize, // circumference division
    nd_h: usize, // height division
    nd_r: usize, // radius division
    total_intensity: f32,
) -> Vec<PointSource> {
    let mut sources = Vec::with_capacity(nd_c * nd_h * nd_r);

    let height = axis.length();
    if height < EPSILON2 {
        return vec![];
    }
    let w = axis / height;

    // Create an orthogonal basis (u, v, w)
    let mut u = Vec3A::new(w.y - w.z, w.z - w.x, w.x - w.y);
    let u_norm = u.length();
    if u_norm < EPSILON {
        // w is parallel to (1,1,1), pick another vector
        u = Vec3A::new(1.0, -1.0, 0.0);
    }
    let u = u.normalize();
    let v = w.cross(u);

    let dr = radius / nd_r as f32;
    let dtheta = 2.0 * std::f32::consts::PI / nd_c as f32;
    let dz = height / nd_h as f32;

    let mut total_weight = 0.0;

    for i in 0..nd_r {
        let r = (i as f32 + 0.5) * dr;
        let weight = r; // proportional to volume element r dr dtheta dz
        for j in 0..nd_c {
            let theta = (j as f32 + 0.5) * dtheta;
            for k in 0..nd_h {
                let z = (k as f32 + 0.5) * dz;

                let p = start + w * z + r * theta.cos() * u + r * theta.sin() * v;

                sources.push(PointSource {
                    position: p,
                    intensity: weight,
                });
                total_weight += weight;
            }
        }
    }

    // Normalize intensities
    for src in &mut sources {
        src.intensity = src.intensity / total_weight * total_intensity;
    }

    sources
}

#[allow(clippy::too_many_arguments)]
pub fn generate_cuboid_source(
    xmin: f32,
    xmax: f32,
    ymin: f32,
    ymax: f32,
    zmin: f32,
    zmax: f32,
    nd_x: usize,
    nd_y: usize,
    nd_z: usize,
    total_intensity: f32,
) -> Vec<PointSource> {
    let mut sources = Vec::with_capacity(nd_x * nd_y * nd_z);

    let dx = (xmax - xmin) / nd_x as f32;
    let dy = (ymax - ymin) / nd_y as f32;
    let dz = (zmax - zmin) / nd_z as f32;

    let intensity_per_point = total_intensity / (nd_x * nd_y * nd_z) as f32;

    for i in 0..nd_x {
        let x = xmin + (i as f32 + 0.5) * dx;
        for j in 0..nd_y {
            let y = ymin + (j as f32 + 0.5) * dy;
            for k in 0..nd_z {
                let z = zmin + (k as f32 + 0.5) * dz;

                sources.push(PointSource {
                    position: Vec3A::new(x, y, z),
                    intensity: intensity_per_point,
                });
            }
        }
    }

    sources
}

pub fn generate_sphere_source(
    center: Vec3A,
    radius: f32,
    nd_r: usize,
    nd_theta: usize, // polar angle
    nd_phi: usize,   // azimuthal angle
    total_intensity: f32,
) -> Vec<PointSource> {
    let mut sources = Vec::with_capacity(nd_r * nd_theta * nd_phi);

    let dr = radius / nd_r as f32;
    let dtheta = std::f32::consts::PI / nd_theta as f32;
    let dphi = 2.0 * std::f32::consts::PI / nd_phi as f32;

    let mut total_weight = 0.0;

    for i in 0..nd_r {
        let r = (i as f32 + 0.5) * dr;
        for j in 0..nd_theta {
            let theta = (j as f32 + 0.5) * dtheta;
            let weight = r * r * theta.sin(); // proportional to volume element r^2 sin(theta) dr dtheta dphi
            for k in 0..nd_phi {
                let phi = (k as f32 + 0.5) * dphi;

                let px = center.x + r * theta.sin() * phi.cos();
                let py = center.y + r * theta.sin() * phi.sin();
                let pz = center.z + r * theta.cos();

                sources.push(PointSource {
                    position: Vec3A::new(px, py, pz),
                    intensity: weight,
                });
                total_weight += weight;
            }
        }
    }

    // Normalize intensities
    for src in &mut sources {
        src.intensity = src.intensity / total_weight * total_intensity;
    }

    sources
}
