pub struct PointSource {
    pub position: (f64, f64, f64),
    pub intensity: f64, // photons/sec or similar
}

pub fn generate_cylinder_source(
    start: (f64, f64, f64),
    axis: (f64, f64, f64), // vector from start to end
    radius: f64,
    nd_c: usize, // circumference division
    nd_h: usize, // height division
    nd_r: usize, // radius division
    total_intensity: f64,
) -> Vec<PointSource> {
    let mut sources = Vec::with_capacity(nd_c * nd_h * nd_r);

    let height = (axis.0 * axis.0 + axis.1 * axis.1 + axis.2 * axis.2).sqrt();
    if height < 1e-10 {
        return vec![];
    }
    let w = (axis.0 / height, axis.1 / height, axis.2 / height);

    // Create an orthogonal basis (u, v, w)
    let mut u = (w.1 - w.2, w.2 - w.0, w.0 - w.1);
    let u_norm = (u.0 * u.0 + u.1 * u.1 + u.2 * u.2).sqrt();
    if u_norm < 1e-10 {
        // w is parallel to (1,1,1), pick another vector
        u = (1.0, -1.0, 0.0);
    }
    let u_norm = (u.0 * u.0 + u.1 * u.1 + u.2 * u.2).sqrt();
    let u = (u.0 / u_norm, u.1 / u_norm, u.2 / u_norm);

    let v = (
        w.1 * u.2 - w.2 * u.1,
        w.2 * u.0 - w.0 * u.2,
        w.0 * u.1 - w.1 * u.0,
    );

    let dr = radius / nd_r as f64;
    let dtheta = 2.0 * std::f64::consts::PI / nd_c as f64;
    let dz = height / nd_h as f64;

    let mut total_weight = 0.0;

    for i in 0..nd_r {
        let r = (i as f64 + 0.5) * dr;
        let weight = r; // proportional to volume element r dr dtheta dz
        for j in 0..nd_c {
            let theta = (j as f64 + 0.5) * dtheta;
            for k in 0..nd_h {
                let z = (k as f64 + 0.5) * dz;

                let px = start.0 + w.0 * z + r * theta.cos() * u.0 + r * theta.sin() * v.0;
                let py = start.1 + w.1 * z + r * theta.cos() * u.1 + r * theta.sin() * v.1;
                let pz = start.2 + w.2 * z + r * theta.cos() * u.2 + r * theta.sin() * v.2;

                sources.push(PointSource {
                    position: (px, py, pz),
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

pub fn generate_cuboid_source(
    xmin: f64,
    xmax: f64,
    ymin: f64,
    ymax: f64,
    zmin: f64,
    zmax: f64,
    nd_x: usize,
    nd_y: usize,
    nd_z: usize,
    total_intensity: f64,
) -> Vec<PointSource> {
    let mut sources = Vec::with_capacity(nd_x * nd_y * nd_z);

    let dx = (xmax - xmin) / nd_x as f64;
    let dy = (ymax - ymin) / nd_y as f64;
    let dz = (zmax - zmin) / nd_z as f64;

    let intensity_per_point = total_intensity / (nd_x * nd_y * nd_z) as f64;

    for i in 0..nd_x {
        let x = xmin + (i as f64 + 0.5) * dx;
        for j in 0..nd_y {
            let y = ymin + (j as f64 + 0.5) * dy;
            for k in 0..nd_z {
                let z = zmin + (k as f64 + 0.5) * dz;

                sources.push(PointSource {
                    position: (x, y, z),
                    intensity: intensity_per_point,
                });
            }
        }
    }

    sources
}

pub fn generate_sphere_source(
    center: (f64, f64, f64),
    radius: f64,
    nd_r: usize,
    nd_theta: usize, // polar angle
    nd_phi: usize,   // azimuthal angle
    total_intensity: f64,
) -> Vec<PointSource> {
    let mut sources = Vec::with_capacity(nd_r * nd_theta * nd_phi);

    let dr = radius / nd_r as f64;
    let dtheta = std::f64::consts::PI / nd_theta as f64;
    let dphi = 2.0 * std::f64::consts::PI / nd_phi as f64;

    let mut total_weight = 0.0;

    for i in 0..nd_r {
        let r = (i as f64 + 0.5) * dr;
        for j in 0..nd_theta {
            let theta = (j as f64 + 0.5) * dtheta;
            let weight = r * r * theta.sin(); // proportional to volume element r^2 sin(theta) dr dtheta dphi
            for k in 0..nd_phi {
                let phi = (k as f64 + 0.5) * dphi;

                let px = center.0 + r * theta.sin() * phi.cos();
                let py = center.1 + r * theta.sin() * phi.sin();
                let pz = center.2 + r * theta.cos();

                sources.push(PointSource {
                    position: (px, py, pz),
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
