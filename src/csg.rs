use crate::shape::{Ray, Shape};
use glam::Vec3A;
use std::collections::HashMap;

pub enum CSGNode {
    Union(Box<CSGNode>, Box<CSGNode>),
    Intersection(Box<CSGNode>, Box<CSGNode>),
    Difference(Box<CSGNode>, Box<CSGNode>),
    Primitive(usize), // shape_id
}

impl CSGNode {
    fn contains(&self, p: &Vec3A, shapes: &HashMap<usize, Shape>) -> bool {
        match self {
            CSGNode::Union(left, right) => left.contains(p, shapes) || right.contains(p, shapes),
            CSGNode::Intersection(left, right) => {
                left.contains(p, shapes) && right.contains(p, shapes)
            }
            CSGNode::Difference(left, right) => {
                left.contains(p, shapes) && !right.contains(p, shapes)
            }
            CSGNode::Primitive(id) => shapes.get(id).is_some_and(|s| s.contains(p)),
        }
    }
}

pub struct Cell {
    pub csg: CSGNode,
    pub material_id: u32,
}

pub struct World {
    pub shapes: HashMap<usize, Shape>,
    pub cells: Vec<Cell>,
}

const EPSILON: f32 = 1e-6;

impl World {
    pub fn get_ray_segments(&self, ray: &Ray) -> Vec<(u32, f32)> {
        let mut segments = Vec::new();

        let mut all_ts = vec![0.0, 1.0];
        // Collect intersections for all shapes
        for shape in self.shapes.values() {
            all_ts.extend(shape.get_intersections(ray));
        }

        all_ts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let mut merged_ts = Vec::new();
        for t in all_ts {
            if merged_ts.is_empty() {
                merged_ts.push(t);
            } else {
                let last = *merged_ts.last().unwrap();
                if t - last > EPSILON {
                    merged_ts.push(t);
                }
            }
        }

        for i in 0..merged_ts.len().saturating_sub(1) {
            let t0 = merged_ts[i];
            let t1 = merged_ts[i + 1];

            // Only care about points in front of the ray origin
            if t1 <= 0.0 {
                continue;
            }
            let t0 = t0.max(0.0);

            if t1 - t0 <= EPSILON {
                continue;
            }

            let t_mid = (t0 + t1) * 0.5;
            let p_mid = ray.origin + ray.direction * t_mid;

            for cell in &self.cells {
                if cell.csg.contains(&p_mid, &self.shapes) {
                    let length = t1 - t0;
                    segments.push((cell.material_id, length));
                    // Stop checking cells once we find the one containing this segment
                    // Assuming cells do not overlap
                    break;
                }
            }
        }

        segments
    }

    pub fn get_optical_thickness<F>(&self, ray: &Ray, get_mu: F) -> f32
    where
        F: Fn(u32) -> f32,
    {
        let segments = self.get_ray_segments(ray);
        let mut total_thickness = 0.0;
        for (mat_id, length) in segments {
            total_thickness += get_mu(mat_id) * length;
        }
        total_thickness
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_world() -> World {
        let mut shapes = HashMap::new();
        // Shape 0: Sphere at origin, r=1
        shapes.insert(
            0,
            Shape::Sphere {
                center: Vec3A::ZERO,
                radius: 1.0,
            },
        );
        // Shape 1: Sphere at x=1.5, r=1
        shapes.insert(
            1,
            Shape::Sphere {
                center: Vec3A::new(1.5, 0.0, 0.0),
                radius: 1.0,
            },
        );

        World {
            shapes,
            cells: vec![],
        }
    }

    #[test]
    fn test_midpoint_csg_union() {
        let mut world = setup_world();
        world.cells.push(Cell {
            csg: CSGNode::Union(
                Box::new(CSGNode::Primitive(0)),
                Box::new(CSGNode::Primitive(1)),
            ),
            material_id: 1, // mu = 1.0
        });

        let ray = Ray {
            origin: Vec3A::new(-2.0, 0.0, 0.0),
            direction: Vec3A::new(1.0, 0.0, 0.0), // Shoot along x-axis
        };

        // Union spans x from -1 to 2.5
        // Total distance inside: 3.5
        let th = world.get_optical_thickness(&ray, |_| 1.0);
        assert!((th - 3.5).abs() < 1e-6);
    }

    #[test]
    fn test_midpoint_csg_intersection() {
        let mut world = setup_world();
        world.cells.push(Cell {
            csg: CSGNode::Intersection(
                Box::new(CSGNode::Primitive(0)),
                Box::new(CSGNode::Primitive(1)),
            ),
            material_id: 1,
        });

        let ray = Ray {
            origin: Vec3A::new(-2.0, 0.0, 0.0),
            direction: Vec3A::new(1.0, 0.0, 0.0),
        };

        // Intersection spans x from 0.5 to 1.0
        // Total distance inside: 0.5
        let th = world.get_optical_thickness(&ray, |_| 1.0);
        assert!((th - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_midpoint_csg_difference() {
        let mut world = setup_world();
        world.cells.push(Cell {
            csg: CSGNode::Difference(
                Box::new(CSGNode::Primitive(0)),
                Box::new(CSGNode::Primitive(1)),
            ),
            material_id: 1,
        });

        let ray = Ray {
            origin: Vec3A::new(-2.0, 0.0, 0.0),
            direction: Vec3A::new(1.0, 0.0, 0.0),
        };

        // Difference (Shape 0 - Shape 1) spans x from -1.0 to 0.5
        // Total distance inside: 1.5
        let th = world.get_optical_thickness(&ray, |_| 1.0);
        assert!((th - 1.5).abs() < 1e-6);
    }
}
