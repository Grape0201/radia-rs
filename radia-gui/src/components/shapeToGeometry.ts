import * as THREE from "three";
import { PrimitiveShape } from "../types/geometry";

/**
 * Converts a PrimitiveShape into a THREE.BufferGeometry with its world transform baked in.
 * This is crucial for three-bvh-csg, as the Brush relies on the base geometry.
 */
export const shapeToGeometry = (shape: PrimitiveShape): THREE.BufferGeometry => {
  let geometry: THREE.BufferGeometry;

  if (shape.type === "Sphere") {
    geometry = new THREE.SphereGeometry(shape.radius, 32, 32);
    geometry.translate(shape.center[0], shape.center[1], shape.center[2]);
  } else if (shape.type === "Box") {
    const width = Math.abs(shape.max[0] - shape.min[0]);
    const height = Math.abs(shape.max[1] - shape.min[1]);
    const depth = Math.abs(shape.max[2] - shape.min[2]);
    geometry = new THREE.BoxGeometry(width, height, depth);
    const center = [
      (shape.max[0] + shape.min[0]) / 2,
      (shape.max[1] + shape.min[1]) / 2,
      (shape.max[2] + shape.min[2]) / 2,
    ];
    geometry.translate(center[0], center[1], center[2]);
  } else if (shape.type === "Cylinder") {
    const v = new THREE.Vector3(...shape.vector);
    const length = v.length() || 1e-6;
    geometry = new THREE.CylinderGeometry(shape.radius, shape.radius, length, 32);
    
    // Rotate the cylinder orientation to match the vector direction
    const dir = v.clone().normalize();
    const quaternion = new THREE.Quaternion().setFromUnitVectors(new THREE.Vector3(0, 1, 0), dir);
    geometry.applyQuaternion(quaternion);
    
    // Position the cylinder at its origin center
    geometry.translate(shape.center[0], shape.center[1], shape.center[2]);
  } else {
    throw new Error(`Unsupported shape type: ${(shape as any).type}`);
  }

  // Ensure attributes required for bvh operations exist and are populated
  geometry.computeVertexNormals();
  return geometry;
};
