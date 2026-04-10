import React, { useMemo } from "react";
import { Canvas } from "@react-three/fiber";
import { OrbitControls, Box, Sphere, Cylinder, Text, Line } from "@react-three/drei";
import * as THREE from "three";
import { GeometryData, PrimitiveShape } from "../types/geometry";

interface ViewerProps {
  geometry: GeometryData | null;
}

const computeMaxExtent = (geometry: GeometryData | null) => {
  let extent = 10;
  if (!geometry) return extent;

  const updateExtent = (val: number) => {
    if (val > extent) extent = val;
  };

  geometry.primitives.forEach(prim => {
    const s = prim.shape;
    if (s.type === "Sphere") {
      updateExtent(Math.abs(s.center[0]) + s.radius);
      updateExtent(Math.abs(s.center[1]) + s.radius);
      updateExtent(Math.abs(s.center[2]) + s.radius);
    } else if (s.type === "Box") {
      s.min.forEach(v => updateExtent(Math.abs(v)));
      s.max.forEach(v => updateExtent(Math.abs(v)));
    } else if (s.type === "Cylinder") {
      const len = Math.sqrt(s.vector[0]**2 + s.vector[1]**2 + s.vector[2]**2);
      updateExtent(Math.abs(s.center[0]) + len + s.radius);
      updateExtent(Math.abs(s.center[1]) + len + s.radius);
      updateExtent(Math.abs(s.center[2]) + len + s.radius);
    }
  });

  geometry.detectors.forEach(det => {
    det.position.forEach(v => updateExtent(Math.abs(v)));
  });

  if (geometry.source?.center) {
    geometry.source.center.forEach(v => updateExtent(Math.abs(v) + (geometry.source!.radius || 1.0)));
  }

  return extent;
};

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

const AxesWithTicks = ({ size }: { size: number }) => {
  // Determine step size (try to get roughly 5 ticks)
  const magnitude = Math.pow(10, Math.floor(Math.log10(size || 1)));
  let step = magnitude;
  if (size / step < 3) step /= 2;
  else if (size / step > 8) step *= 2;
  
  const tickLength = Math.max(0.1, size * 0.02);
  const fontSize = Math.max(0.5, size * 0.04);
  
  const ticks = [];
  
  // Align tick start/end with multiples of step so 0 is squarely hit
  const startTick = -Math.floor(size / step) * step;
  const endTick = Math.floor(size / step) * step;

  // +1e-6 to avoid floating point precision issues on the loop boundary
  for (let i = startTick; i <= endTick + 1e-6; i += step) {
    if (Math.abs(i) < 1e-6) continue; // Skip 0
    const val = parseFloat(i.toPrecision(3));
    // X axis ticks (red)
    ticks.push(
      <group key={`x-${i}`} position={[i, 0, 0]}>
        <mesh position={[0, -tickLength/2, 0]}>
          <boxGeometry args={[tickLength*0.1, tickLength, tickLength*0.1]} />
          <meshBasicMaterial color="#ff0000" />
        </mesh>
        <Text position={[0, -tickLength * 1.5, 0]} fontSize={fontSize} color="#ff0000" anchorY="top">{val}</Text>
      </group>
    );
    // Y axis ticks (green)
    ticks.push(
      <group key={`y-${i}`} position={[0, i, 0]}>
        <mesh position={[-tickLength/2, 0, 0]}>
          <boxGeometry args={[tickLength, tickLength*0.1, tickLength*0.1]} />
          <meshBasicMaterial color="#00ff00" />
        </mesh>
        <Text position={[-tickLength * 1.5, 0, 0]} fontSize={fontSize} color="#00ff00" anchorX="right">{val}</Text>
      </group>
    );
    // Z axis ticks (blue)
    ticks.push(
      <group key={`z-${i}`} position={[0, 0, i]}>
        <mesh position={[0, -tickLength/2, 0]}>
          <boxGeometry args={[tickLength*0.1, tickLength, tickLength*0.1]} />
          <meshBasicMaterial color="#0000ff" />
        </mesh>
        <Text position={[0, -tickLength * 1.5, 0]} fontSize={fontSize} color="#0000ff" anchorY="top">{val}</Text>
      </group>
    );
  }

  return (
    <group>
      <Line points={[[-size, 0, 0], [size, 0, 0]]} color="#ff0000" lineWidth={1} />
      <Line points={[[0, -size, 0], [0, size, 0]]} color="#00ff00" lineWidth={1} />
      <Line points={[[0, 0, -size], [0, 0, size]]} color="#0000ff" lineWidth={1} />
      {ticks}
    </group>
  );
};

export const Viewer3D: React.FC<ViewerProps> = ({ geometry }) => {
  const maxExtent = useMemo(() => computeMaxExtent(geometry), [geometry]);
  
  // Adjust grid and axes dynamically
  const gridSize = Math.ceil((maxExtent * 2.5) / 10) * 10;
  const axesSize = gridSize / 2;

  // Ensure camera isn't clipped and position covers the extent
  const camPos = maxExtent * 2;
  const farPlane = Math.max(1000, maxExtent * 10);

  return (
    <Canvas camera={{ position: [camPos, camPos, camPos], fov: 50, far: farPlane }}>
      {/* Lights */}
      <ambientLight intensity={0.5} />
      <directionalLight position={[maxExtent, maxExtent, maxExtent]} intensity={1} />
      <directionalLight position={[-maxExtent, maxExtent, -maxExtent]} intensity={0.5} />
      
      {/* Controls */}
      <OrbitControls makeDefault />
      <AxesWithTicks size={axesSize} />

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
