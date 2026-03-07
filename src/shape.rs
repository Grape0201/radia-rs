#[derive(Debug, Clone)]
pub enum Shape {
    Sphere {
        center: Vec3,
        radius: f64,
    },
    RectangularPrallelPiped {
        xmin: f64,
        xmax: f64,
        ymin: f64,
        ymax: f64,
        zmin: f64,
        zmax: f64,
    },
    Cylinder {
        center: Vec3,
        direction: Vec3,
        radius: f64,
    },
}

#[derive(Debug, Clone)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Debug, Clone)]
pub struct Ray {
    pub origin: (f64, f64, f64),
    pub direction: (f64, f64, f64),
}

const EPSILON: f64 = 1e-8;

impl Shape {
    pub fn get_intersections(&self, ray: &Ray) -> Vec<f64> {
        match self {
            Shape::Sphere { center, radius } => {
                let oc_x = ray.origin.0 - center.x;
                let oc_y = ray.origin.1 - center.y;
                let oc_z = ray.origin.2 - center.z;

                let a = ray.direction.0 * ray.direction.0
                    + ray.direction.1 * ray.direction.1
                    + ray.direction.2 * ray.direction.2;
                let b = oc_x * ray.direction.0 + oc_y * ray.direction.1 + oc_z * ray.direction.2;
                let c = oc_x * oc_x + oc_y * oc_y + oc_z * oc_z - radius * radius;

                let discriminant = b * b - a * c;
                if discriminant > EPSILON {
                    let sqrt_d = discriminant.sqrt();
                    let t1 = (-b - sqrt_d) / a;
                    let t2 = (-b + sqrt_d) / a;
                    vec![t1.min(t2), t1.max(t2)]
                } else if discriminant.abs() <= EPSILON {
                    vec![-b / a]
                } else {
                    vec![]
                }
            }
            Shape::RectangularPrallelPiped {
                xmin,
                xmax,
                ymin,
                ymax,
                zmin,
                zmax,
            } => {
                let mut tmin = f64::NEG_INFINITY;
                let mut tmax = f64::INFINITY;

                for (o, d, min_val, max_val) in [
                    (ray.origin.0, ray.direction.0, *xmin, *xmax),
                    (ray.origin.1, ray.direction.1, *ymin, *ymax),
                    (ray.origin.2, ray.direction.2, *zmin, *zmax),
                ] {
                    if d.abs() < EPSILON {
                        if o < min_val || o > max_val {
                            return vec![];
                        }
                    } else {
                        let t1 = (min_val - o) / d;
                        let t2 = (max_val - o) / d;
                        tmin = tmin.max(t1.min(t2));
                        tmax = tmax.min(t1.max(t2));
                        if tmin > tmax + EPSILON {
                            return vec![];
                        }
                    }
                }
                if (tmax - tmin).abs() <= EPSILON {
                    vec![tmin]
                } else {
                    vec![tmin, tmax]
                }
            }
            Shape::Cylinder {
                center,
                direction,
                radius,
            } => {
                let wx = ray.origin.0 - center.x;
                let wy = ray.origin.1 - center.y;
                let wz = ray.origin.2 - center.z;

                let dc_x = direction.x;
                let dc_y = direction.y;
                let dc_z = direction.z;

                let cross_w_dc = (
                    wy * dc_z - wz * dc_y,
                    wz * dc_x - wx * dc_z,
                    wx * dc_y - wy * dc_x,
                );

                let v_x = ray.direction.0;
                let v_y = ray.direction.1;
                let v_z = ray.direction.2;

                let cross_v_dc = (
                    v_y * dc_z - v_z * dc_y,
                    v_z * dc_x - v_x * dc_z,
                    v_x * dc_y - v_y * dc_x,
                );

                let a = cross_v_dc.0 * cross_v_dc.0
                    + cross_v_dc.1 * cross_v_dc.1
                    + cross_v_dc.2 * cross_v_dc.2;
                let b = cross_w_dc.0 * cross_v_dc.0
                    + cross_w_dc.1 * cross_v_dc.1
                    + cross_w_dc.2 * cross_v_dc.2;
                let c = cross_w_dc.0 * cross_w_dc.0
                    + cross_w_dc.1 * cross_w_dc.1
                    + cross_w_dc.2 * cross_w_dc.2
                    - radius * radius * (dc_x * dc_x + dc_y * dc_y + dc_z * dc_z);

                if a.abs() < EPSILON {
                    vec![]
                } else {
                    let discriminant = b * b - a * c;
                    if discriminant > EPSILON {
                        let sqrt_d = discriminant.sqrt();
                        let t1 = (-b - sqrt_d) / a;
                        let t2 = (-b + sqrt_d) / a;
                        vec![t1.min(t2), t1.max(t2)]
                    } else if discriminant.abs() <= EPSILON {
                        vec![-b / a]
                    } else {
                        vec![]
                    }
                }
            }
        }
    }

    pub fn contains(&self, p: &(f64, f64, f64)) -> bool {
        match self {
            Shape::Sphere { center, radius } => {
                let dx = p.0 - center.x;
                let dy = p.1 - center.y;
                let dz = p.2 - center.z;
                dx * dx + dy * dy + dz * dz <= radius * radius + EPSILON
            }
            Shape::RectangularPrallelPiped {
                xmin,
                xmax,
                ymin,
                ymax,
                zmin,
                zmax,
            } => {
                p.0 >= *xmin - EPSILON
                    && p.0 <= *xmax + EPSILON
                    && p.1 >= *ymin - EPSILON
                    && p.1 <= *ymax + EPSILON
                    && p.2 >= *zmin - EPSILON
                    && p.2 <= *zmax + EPSILON
            }
            Shape::Cylinder {
                center,
                direction,
                radius,
            } => {
                let dx = p.0 - center.x;
                let dy = p.1 - center.y;
                let dz = p.2 - center.z;

                // Vector w = p - center
                // Distance to axis = |w x direction| / |direction|
                // Assuming direction is normalized
                let cross_x = dy * direction.z - dz * direction.y;
                let cross_y = dz * direction.x - dx * direction.z;
                let cross_z = dx * direction.y - dy * direction.x;

                let dist_sq = cross_x * cross_x + cross_y * cross_y + cross_z * cross_z;
                dist_sq <= radius * radius + EPSILON
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sphere_intersections() {
        let sphere = Shape::Sphere {
            center: Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            radius: 1.0,
        };
        let ray = Ray {
            origin: (0.0, 0.0, -2.0),
            direction: (0.0, 0.0, 1.0),
        };
        let ts = sphere.get_intersections(&ray);
        assert_eq!(ts.len(), 2);
        assert!((ts[0] - 1.0).abs() < EPSILON);
        assert!((ts[1] - 3.0).abs() < EPSILON);
    }

    #[test]
    fn test_sphere_tangent() {
        let sphere = Shape::Sphere {
            center: Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            radius: 1.0,
        };
        let ray = Ray {
            origin: (1.0, 0.0, -2.0),
            direction: (0.0, 0.0, 1.0),
        };
        let ts = sphere.get_intersections(&ray);
        assert_eq!(ts.len(), 1);
        assert!((ts[0] - 2.0).abs() < EPSILON);
    }

    #[test]
    fn test_rect_intersections() {
        let rect = Shape::RectangularPrallelPiped {
            xmin: -1.0,
            xmax: 1.0,
            ymin: -1.0,
            ymax: 1.0,
            zmin: -1.0,
            zmax: 1.0,
        };
        let ray = Ray {
            origin: (0.0, 0.0, -2.0),
            direction: (0.0, 0.0, 1.0),
        };
        let ts = rect.get_intersections(&ray);
        assert_eq!(ts.len(), 2);
        assert!((ts[0] - 1.0).abs() < EPSILON);
        assert!((ts[1] - 3.0).abs() < EPSILON);
    }

    #[test]
    fn test_cylinder_intersections() {
        let cylinder = Shape::Cylinder {
            center: Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            direction: Vec3 {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            },
            radius: 1.0,
        };
        let ray = Ray {
            origin: (0.0, 0.0, -2.0),
            direction: (0.0, 0.0, 1.0),
        };
        let ts = cylinder.get_intersections(&ray);
        assert_eq!(ts.len(), 2);
        assert!((ts[0] - 1.0).abs() < EPSILON);
        assert!((ts[1] - 3.0).abs() < EPSILON);
    }
}
