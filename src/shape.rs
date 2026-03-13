use glam::Vec3A;

/// Only convex shapes are supported.
#[derive(Debug, Clone)]
pub enum Shape {
    Sphere {
        center: Vec3A,
        /// radius^2
        radius2: f32,
    },
    RectangularPrallelPiped {
        min: Vec3A,
        max: Vec3A,
    },
    FiniteCylinder {
        center: Vec3A,
        /// unit vector
        direction: Vec3A,
        /// radius^2
        radius2: f32,
        half_height: f32,
    },
}

impl std::fmt::Display for Shape {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Shape::Sphere { center, radius2 } => {
                write!(f, "Sphere {{ center: {:?}, radius2: {} }}", center, radius2)
            }
            Shape::RectangularPrallelPiped { min, max } => write!(
                f,
                "RectangularPrallelPiped {{ min: {:?}, max: {:?} }}",
                min, max
            ),
            Shape::FiniteCylinder {
                center,
                direction,
                radius2,
                half_height,
            } => write!(
                f,
                "FiniteCylinder {{ center: {:?}, direction: {:?}, radius2: {}, half_height: {} }}",
                center, direction, radius2, half_height
            ),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Ray {
    pub origin: Vec3A,
    /// from origin to the detector point
    pub vector: Vec3A,
}

const EPSILON: f32 = 1e-6;
const SEGMENT_MIN: f32 = EPSILON;
const SEGMENT_MAX: f32 = 1.0 - EPSILON;

pub struct IntersectionTs {
    pub count: usize,
    /// 0 < ts[0],ts[1] < 1
    pub ts: [f32; 2],
}

impl IntersectionTs {
    fn empty() -> Self {
        Self {
            count: 0,
            ts: [0.0, 0.0],
        }
    }

    fn push_candidate(&mut self, value: f32) {
        if value <= SEGMENT_MIN || value >= SEGMENT_MAX {
            return;
        }

        match self.count {
            0 => {
                self.ts[0] = value;
                self.count = 1;
            }
            1 => {
                self.ts[1] = value;
                self.count = 2;
            }
            _ => unreachable!(),
        }
    }
}

impl Shape {
    #[inline(always)]
    pub fn get_intersections(&self, ray: &Ray) -> IntersectionTs {
        let mut hits = IntersectionTs::empty();
        match self {
            Shape::Sphere { center, radius2 } => {
                let oc = ray.origin - *center;
                let a = ray.vector.length_squared();
                let b = oc.dot(ray.vector);
                let c = oc.length_squared() - radius2;
                let discriminant = b * b - a * c;
                if discriminant > EPSILON {
                    let sqrt_d = discriminant.sqrt();
                    hits.push_candidate((-b - sqrt_d) / a);
                    hits.push_candidate((-b + sqrt_d) / a);
                } else if discriminant.abs() <= EPSILON {
                    hits.push_candidate(-b / a);
                }
                hits
            }
            Shape::RectangularPrallelPiped { min, max } => {
                let inv_dir = 1.0 / ray.vector;
                let t0 = (*min - ray.origin) * inv_dir;
                let t1 = (*max - ray.origin) * inv_dir;

                let tmin_v = t0.min(t1);
                let tmax_v = t0.max(t1);

                let tmin = tmin_v.max_element();
                let tmax = tmax_v.min_element();

                if tmin > tmax + EPSILON {
                    hits
                } else {
                    hits.push_candidate(tmin);
                    hits.push_candidate(tmax);
                    hits
                }
            }
            Shape::FiniteCylinder {
                center,
                direction,
                radius2,
                half_height,
            } => {
                if *half_height <= EPSILON {
                    return hits;
                }
                let axis = *direction;

                let v = ray.vector;
                let w = ray.origin - *center;

                let v_cross_axis = v.cross(axis);
                let w_cross_axis = w.cross(axis);

                let a = v_cross_axis.length_squared();
                let b = w_cross_axis.dot(v_cross_axis);
                let c = w_cross_axis.length_squared() - *radius2;

                if a.abs() > EPSILON {
                    let discriminant = b * b - a * c;
                    if discriminant > EPSILON {
                        let sqrt_d = discriminant.sqrt();
                        for &t in &[(-b - sqrt_d) / a, (-b + sqrt_d) / a] {
                            let point = ray.origin + ray.vector * t;
                            let axial = (point - *center).dot(axis);
                            if axial.abs() <= *half_height + EPSILON {
                                hits.push_candidate(t);
                            }
                        }
                    } else if discriminant.abs() <= EPSILON {
                        let t = -b / a;
                        let point = ray.origin + ray.vector * t;
                        let axial = (point - *center).dot(axis);
                        if axial.abs() <= *half_height + EPSILON {
                            hits.push_candidate(t);
                        }
                    }
                }

                let axis_dot_dir = ray.vector.dot(axis);
                if axis_dot_dir.abs() > EPSILON {
                    for &sign in &[1.0f32, -1.0f32] {
                        let cap_center = *center + axis * (sign * *half_height);
                        let t = (cap_center - ray.origin).dot(axis) / axis_dot_dir;
                        let point = ray.origin + ray.vector * t;
                        let radial = point - cap_center;
                        let radial_proj = radial.dot(axis);
                        let radial_vec = radial - axis * radial_proj;
                        if radial_vec.length_squared() <= *radius2 + EPSILON {
                            hits.push_candidate(t);
                        }
                    }
                }

                hits
            }
        }
    }

    #[inline(always)]
    pub fn contains(&self, p: &Vec3A) -> bool {
        match self {
            Shape::Sphere { center, radius2 } => {
                let dist_sq = (*p - *center).length_squared();
                dist_sq <= *radius2 + EPSILON
            }
            Shape::RectangularPrallelPiped { min, max } => {
                p.cmpge(*min - EPSILON).all() && p.cmple(*max + EPSILON).all()
            }
            Shape::FiniteCylinder {
                center,
                direction,
                radius2,
                half_height,
            } => {
                if *half_height <= EPSILON {
                    return false;
                }
                let axis = *direction;
                let w = *p - *center;
                let axial = w.dot(axis);
                if axial.abs() > *half_height + EPSILON {
                    return false;
                }
                let radial = w - axis * axial;
                radial.length_squared() <= *radius2 + EPSILON
            }
        }
    }

    pub fn sdf(&self, p: &Vec3A) -> f32 {
        match self {
            Shape::Sphere { center, radius2 } => {
                let radius = radius2.sqrt();
                (*p - *center).length() - radius
            }
            Shape::RectangularPrallelPiped { min, max } => {
                let center = (*min + *max) * 0.5;
                let extents = (*max - *min) * 0.5;
                let q = (*p - center).abs() - extents;
                let outside = q.max(Vec3A::ZERO).length();
                let inside = q.max_element().min(0.0);
                outside + inside
            }
            Shape::FiniteCylinder {
                center,
                direction,
                radius2,
                half_height,
            } => {
                if *half_height <= EPSILON {
                    return f32::INFINITY;
                }
                let axis = *direction;
                let w = *p - *center;
                let axial = w.dot(axis);
                let radial = (w - axis * axial).length();
                let radius = radius2.sqrt();
                let dx = radial - radius;
                let dy = axial.abs() - *half_height;
                let outside = (dx.max(0.0).powi(2) + dy.max(0.0).powi(2)).sqrt();
                let inside = dx.max(dy).min(0.0);
                outside + inside
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
            vector: Vec3A::new(0.0, 0.0, 4.0),
        };
        let ts = sphere.get_intersections(&ray);
        assert_eq!(ts.count, 2);
        assert!((ts.ts[0] - 0.25).abs() < EPSILON);
        assert!((ts.ts[1] - 0.75).abs() < EPSILON);
    }

    #[test]
    fn test_sphere_tangent() {
        let sphere = Shape::Sphere {
            center: Vec3A::ZERO,
            radius2: 1.0,
        };
        let ray = Ray {
            origin: Vec3A::new(1.0, 0.0, -2.0),
            vector: Vec3A::new(0.0, 0.0, 4.0),
        };
        let ts = sphere.get_intersections(&ray);
        assert_eq!(ts.count, 1);
        assert!((ts.ts[0] - 0.5).abs() < EPSILON);
    }

    #[test]
    fn test_rect_intersections() {
        let rect = Shape::RectangularPrallelPiped {
            min: Vec3A::new(-1.0, -1.0, -1.0),
            max: Vec3A::new(1.0, 1.0, 1.0),
        };
        let ray = Ray {
            origin: Vec3A::new(0.0, 0.0, -2.0),
            vector: Vec3A::new(0.0, 0.0, 4.0),
        };
        let ts = rect.get_intersections(&ray);
        assert_eq!(ts.count, 2);
        assert!((ts.ts[0] - 0.25).abs() < EPSILON);
        assert!((ts.ts[1] - 0.75).abs() < EPSILON);
    }

    #[test]
    fn test_cylinder_intersections() {
        let cylinder = Shape::FiniteCylinder {
            center: Vec3A::ZERO,
            direction: Vec3A::Y,
            radius2: 1.0,
            half_height: 1.0,
        };
        // from -z to z
        let ray = Ray {
            origin: Vec3A::new(0.0, 0.0, -2.0),
            vector: Vec3A::new(0.0, 0.0, 4.0),
        };
        let ts = cylinder.get_intersections(&ray);
        assert_eq!(ts.count, 2);
        assert!((ts.ts[0] - 0.25).abs() < EPSILON);
        assert!((ts.ts[1] - 0.75).abs() < EPSILON);
        // from top to bottom
        let ray = Ray {
            origin: Vec3A::new(0.0, 2.0, 0.0),
            vector: Vec3A::new(0.0, -4.0, 0.0),
        };
        let ts = cylinder.get_intersections(&ray);
        assert_eq!(ts.count, 2);
        assert!((ts.ts[0] - 0.25).abs() < EPSILON);
        assert!((ts.ts[1] - 0.75).abs() < EPSILON);
        // from top to center
        let ray = Ray {
            origin: Vec3A::new(0.0, 2.0, 0.0),
            vector: Vec3A::new(0.0, -2.0, 0.0),
        };
        let ts = cylinder.get_intersections(&ray);
        assert_eq!(ts.count, 1);
        assert!((ts.ts[0] - 0.5).abs() < EPSILON);
    }

    #[test]
    fn test_shape_sdf() {
        let sphere = Shape::Sphere {
            center: Vec3A::ZERO,
            radius2: 4.0,
        };
        assert!((sphere.sdf(&Vec3A::ZERO) + 2.0).abs() < EPSILON);
        assert!((sphere.sdf(&Vec3A::new(3.0, 0.0, 0.0)) - 1.0).abs() < EPSILON);

        let rect = Shape::RectangularPrallelPiped {
            min: Vec3A::new(-1.0, -1.0, -1.0),
            max: Vec3A::new(1.0, 1.0, 1.0),
        };
        assert!(rect.sdf(&Vec3A::ZERO).is_sign_negative());
        assert!((rect.sdf(&Vec3A::new(2.0, 0.0, 0.0)) - 1.0).abs() < EPSILON);

        let cylinder = Shape::FiniteCylinder {
            center: Vec3A::ZERO,
            direction: Vec3A::Y,
            radius2: 1.0,
            half_height: 1.0,
        };
        assert!((cylinder.sdf(&Vec3A::ZERO) + 1.0).abs() < EPSILON);
        assert!((cylinder.sdf(&Vec3A::new(2.0, 0.0, 0.0)) - 1.0).abs() < EPSILON);
    }
}
