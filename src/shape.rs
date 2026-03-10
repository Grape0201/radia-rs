use glam::Vec3A;

#[derive(Debug, Clone)]
pub enum Shape {
    Sphere {
        center: Vec3A,
        radius2: f32, // radius^2
    },
    RectangularPrallelPiped {
        min: Vec3A,
        max: Vec3A,
    },
    Cylinder {
        center: Vec3A,
        direction: Vec3A,
        radius: f32,
    },
}

#[derive(Debug, Clone)]
pub struct Ray {
    pub origin: Vec3A,
    pub direction: Vec3A,
}

const EPSILON: f32 = 1e-6;

impl Shape {
    pub fn get_intersections(&self, ray: &Ray) -> Vec<f32> {
        match self {
            Shape::Sphere { center, radius2 } => {
                let oc = ray.origin - *center;
                let a = ray.direction.length_squared();
                let b = oc.dot(ray.direction);
                let c = oc.length_squared() - radius2;

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
            Shape::RectangularPrallelPiped { min, max } => {
                let inv_dir = 1.0 / ray.direction;
                let t0 = (*min - ray.origin) * inv_dir;
                let t1 = (*max - ray.origin) * inv_dir;

                let tmin_v = t0.min(t1);
                let tmax_v = t0.max(t1);

                let tmin = tmin_v.max_element();
                let tmax = tmax_v.min_element();

                if tmin > tmax + EPSILON || tmax < 0.0 {
                    vec![]
                } else if (tmax - tmin).abs() <= EPSILON {
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
                let w = ray.origin - *center;
                let v = ray.direction;
                let d = *direction;

                let v_cross_d = v.cross(d);
                let w_cross_d = w.cross(d);

                let a = v_cross_d.length_squared();
                let b = w_cross_d.dot(v_cross_d);
                let c = w_cross_d.length_squared() - radius * radius * d.length_squared();

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

    pub fn contains(&self, p: &Vec3A) -> bool {
        match self {
            Shape::Sphere { center, radius2 } => {
                let dist_sq = (*p - *center).length_squared();
                dist_sq <= *radius2 + EPSILON
            }
            Shape::RectangularPrallelPiped { min, max } => {
                p.cmpge(*min - EPSILON).all() && p.cmple(*max + EPSILON).all()
            }
            Shape::Cylinder {
                center,
                direction,
                radius,
            } => {
                let w = *p - *center;
                let cross = w.cross(*direction);
                let dist_sq = cross.length_squared() / direction.length_squared();
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
            center: Vec3A::ZERO,
            radius2: 1.0,
        };
        let ray = Ray {
            origin: Vec3A::new(0.0, 0.0, -2.0),
            direction: Vec3A::new(0.0, 0.0, 1.0),
        };
        let ts = sphere.get_intersections(&ray);
        assert_eq!(ts.len(), 2);
        assert!((ts[0] - 1.0).abs() < EPSILON);
        assert!((ts[1] - 3.0).abs() < EPSILON);
    }

    #[test]
    fn test_sphere_tangent() {
        let sphere = Shape::Sphere {
            center: Vec3A::ZERO,
            radius2: 1.0,
        };
        let ray = Ray {
            origin: Vec3A::new(1.0, 0.0, -2.0),
            direction: Vec3A::new(0.0, 0.0, 1.0),
        };
        let ts = sphere.get_intersections(&ray);
        assert_eq!(ts.len(), 1);
        assert!((ts[0] - 2.0).abs() < EPSILON);
    }

    #[test]
    fn test_rect_intersections() {
        let rect = Shape::RectangularPrallelPiped {
            min: Vec3A::new(-1.0, -1.0, -1.0),
            max: Vec3A::new(1.0, 1.0, 1.0),
        };
        let ray = Ray {
            origin: Vec3A::new(0.0, 0.0, -2.0),
            direction: Vec3A::new(0.0, 0.0, 1.0),
        };
        let ts = rect.get_intersections(&ray);
        assert_eq!(ts.len(), 2);
        assert!((ts[0] - 1.0).abs() < EPSILON);
        assert!((ts[1] - 3.0).abs() < EPSILON);
    }

    #[test]
    fn test_cylinder_intersections() {
        let cylinder = Shape::Cylinder {
            center: Vec3A::ZERO,
            direction: Vec3A::Y,
            radius: 1.0,
        };
        let ray = Ray {
            origin: Vec3A::new(0.0, 0.0, -2.0),
            direction: Vec3A::new(0.0, 0.0, 1.0),
        };
        let ts = cylinder.get_intersections(&ray);
        assert_eq!(ts.len(), 2);
        assert!((ts[0] - 1.0).abs() < EPSILON);
        assert!((ts[1] - 3.0).abs() < EPSILON);
    }
}
