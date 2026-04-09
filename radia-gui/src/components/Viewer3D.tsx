import React from "react";
import { Canvas } from "@react-three/fiber";
import { OrbitControls, Box, Sphere, Cylinder } from "@react-three/drei";
import * as THREE from "three";
import { GeometryData, PrimitiveShape } from "../types/geometry";

interface ViewerProps {
  geometry: GeometryData | null;
}

// Generate consistent colour from primitive name
const stringToColor = (str: string) => {
  let hash = 0;
  for (let i = 0; i < str.length; i++) {
    hash = str.charCodeAt(i) + ((hash << 5) - hash);
  }
  const c = (hash & 0x00ffffff).toString(16).toUpperCase();
  return "#" + "00000".substring(0, 6 - c.length) + c;
};

const PrimitiveView = ({ shape, color, opacity = 0.5 }: { shape: PrimitiveShape, color: string, opacity?: number }) => {
  const material = <meshStandardMaterial color={color} transparent opacity={opacity} depthWrite={false} />;

  if (shape.type === "Sphere") {
    return (
      <Sphere args={[shape.radius, 32, 32]} position={shape.center}>
        {material}
      </Sphere>
    );
  } else if (shape.type === "Box") {
    const width = Math.abs(shape.max[0] - shape.min[0]);
    const height = Math.abs(shape.max[1] - shape.min[1]);
    const depth = Math.abs(shape.max[2] - shape.min[2]);
    const center: [number, number, number] = [
      (shape.max[0] + shape.min[0]) / 2,
      (shape.max[1] + shape.min[1]) / 2,
      (shape.max[2] + shape.min[2]) / 2,
    ];
    return (
      <Box args={[width, height, depth]} position={center}>
        {material}
      </Box>
    );
  } else if (shape.type === "Cylinder") {
    const v = new THREE.Vector3(...shape.vector);
    const length = v.length() || 1e-6; 
    const dir = v.clone().normalize();
    const quaternion = new THREE.Quaternion().setFromUnitVectors(new THREE.Vector3(0, 1, 0), dir);

    return (
      <Cylinder args={[shape.radius, shape.radius, length, 32]} position={shape.center} quaternion={quaternion}>
        {material}
      </Cylinder>
    );
  }
  return null;
};

export const Viewer3D: React.FC<ViewerProps> = ({ geometry }) => {
  return (
    <Canvas camera={{ position: [20, 20, 20], fov: 50 }}>
      {/* Lights */}
      <ambientLight intensity={0.5} />
      <directionalLight position={[10, 10, 10]} intensity={1} />
      <directionalLight position={[-10, 10, -10]} intensity={0.5} />
      
      {/* Controls */}
      <OrbitControls makeDefault />
      <axesHelper args={[10]} />
      <gridHelper args={[20, 20]} />

      {/* Geometry Models */}
      {geometry?.primitives.map((prim, i) => (
        <PrimitiveView 
          key={`prim-${prim.name}-${i}`} 
          shape={prim.shape} 
          color={stringToColor(prim.name)} 
        />
      ))}

      {/* Detectors */}
      {geometry?.detectors.map((det, i) => (
        <Sphere key={`det-${det.name}-${i}`} args={[0.5]} position={det.position}>
          <meshStandardMaterial color="red" />
        </Sphere>
      ))}

      {/* Source */}
      {geometry?.source?.center && (
        <Sphere args={[geometry.source.radius || 1.0]} position={geometry.source.center}>
          <meshStandardMaterial color="yellow" emissive="yellow" emissiveIntensity={0.8} />
        </Sphere>
      )}
    </Canvas>
  );
};
