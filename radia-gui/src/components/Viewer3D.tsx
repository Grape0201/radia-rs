import React, { useMemo } from "react";
import { Canvas } from "@react-three/fiber";
import { OrbitControls, Sphere, Text, Line } from "@react-three/drei";
import * as THREE from "three";
import { GeometryData } from "../types/geometry";
import { shapeToGeometry } from "./shapeToGeometry";
import { buildCsgMesh } from "./buildCsgMesh";

interface ViewerProps {
  geometry: GeometryData | null;
  hiddenCells: Set<number>;
  showSource: boolean;
  showDetectors: boolean;
  cellStyles: Record<number, { color: string, opacity: number }>;
}

const computeMaxExtent = (geometry: GeometryData | null) => {
  let extent = 10;
  if (!geometry) return extent;

  const updateExtent = (val: number) => {
    if (val > extent) extent = val;
  };

  try {
    geometry.primitives?.forEach(prim => {
      const s = prim.shape;
      if (s.type === "Sphere") {
        updateExtent(Math.abs(s.center[0]) + s.radius);
        updateExtent(Math.abs(s.center[1]) + s.radius);
        updateExtent(Math.abs(s.center[2]) + s.radius);
      } else if (s.type === "Box") {
        s.min.forEach(v => updateExtent(Math.abs(v)));
        s.max.forEach(v => updateExtent(Math.abs(v)));
      } else if (s.type === "Cylinder") {
        const len = Math.sqrt(s.vector[0] ** 2 + s.vector[1] ** 2 + s.vector[2] ** 2);
        updateExtent(Math.abs(s.center[0]) + len + s.radius);
        updateExtent(Math.abs(s.center[1]) + len + s.radius);
        updateExtent(Math.abs(s.center[2]) + len + s.radius);
      }
    });

    geometry.detectors.forEach(det => {
      det.position.forEach(v => updateExtent(Math.abs(v)));
    });

    if (geometry.source?.shape) {
      if (geometry.source.shape.type === "Sphere") {
        updateExtent(Math.abs(geometry.source.shape.center[0]) + geometry.source.shape.radius);
        updateExtent(Math.abs(geometry.source.shape.center[1]) + geometry.source.shape.radius);
        updateExtent(Math.abs(geometry.source.shape.center[2]) + geometry.source.shape.radius);
      } else if (geometry.source.shape.type === "Box") {
        geometry.source.shape.max.forEach(v => updateExtent(Math.abs(v)));
        geometry.source.shape.min.forEach(v => updateExtent(Math.abs(v)));
      } else if (geometry.source.shape.type === "Cylinder") {
        const len = Math.sqrt(geometry.source.shape.vector[0] ** 2 + geometry.source.shape.vector[1] ** 2 + geometry.source.shape.vector[2] ** 2);
        updateExtent(Math.abs(geometry.source.shape.center[0]) + len + geometry.source.shape.radius);
        updateExtent(Math.abs(geometry.source.shape.center[1]) + len + geometry.source.shape.radius);
        updateExtent(Math.abs(geometry.source.shape.center[2]) + len + geometry.source.shape.radius);
      }
    }
  } catch (e) {
    console.error("Failed to compute extent:", e);
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

const CsgCellView = ({
  cell,
  baseGeometries,
  opacity,
  color,
}: {
  cell: GeometryData["cells"][0];
  baseGeometries: THREE.BufferGeometry[];
  opacity?: number;
  color?: string;
}) => {
  const mesh = useMemo(() => {
    try {
      if (!cell.csg?.instructions || cell.csg.instructions.length === 0) return null;
      return buildCsgMesh(cell.csg.instructions, baseGeometries);
    } catch (e) {
      console.error(`Failed to build CSG mesh for cell ${cell.material_name}:`, e);
      return null;
    }
  }, [cell, baseGeometries]);

  if (!mesh) return null;

  return (
    <mesh geometry={mesh.geometry}>
      <meshStandardMaterial color={color ?? stringToColor(cell.material_name)} transparent={true} opacity={opacity ?? 0.5} depthWrite={(opacity ?? 0.5) >= 1.0} side={THREE.DoubleSide} />
    </mesh>
  );
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
        <mesh position={[0, -tickLength / 2, 0]}>
          <boxGeometry args={[tickLength * 0.1, tickLength, tickLength * 0.1]} />
          <meshBasicMaterial color="#ff0000" />
        </mesh>
        <Text position={[0, -tickLength * 1.5, 0]} fontSize={fontSize} color="#ff0000" anchorY="top">{val}</Text>
      </group>
    );
    // Y axis ticks (green)
    ticks.push(
      <group key={`y-${i}`} position={[0, i, 0]}>
        <mesh position={[-tickLength / 2, 0, 0]}>
          <boxGeometry args={[tickLength, tickLength * 0.1, tickLength * 0.1]} />
          <meshBasicMaterial color="#00ff00" />
        </mesh>
        <Text position={[-tickLength * 1.5, 0, 0]} fontSize={fontSize} color="#00ff00" anchorX="right">{val}</Text>
      </group>
    );
    // Z axis ticks (blue)
    ticks.push(
      <group key={`z-${i}`} position={[0, 0, i]}>
        <mesh position={[0, -tickLength / 2, 0]}>
          <boxGeometry args={[tickLength * 0.1, tickLength, tickLength * 0.1]} />
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

export const Viewer3D: React.FC<ViewerProps> = ({ geometry, hiddenCells, showSource, showDetectors, cellStyles }) => {
  const maxExtent = useMemo(() => computeMaxExtent(geometry), [geometry]);

  // Create shared geometries array for all cells
  const baseGeometries = useMemo(() => {
    if (!geometry?.primitives) return [];
    try {
      return geometry.primitives.map(prim => shapeToGeometry(prim.shape));
    } catch (e) {
      console.error("Failed mapping base geometries:", e);
      return [];
    }
  }, [geometry?.primitives]);

  const sourceGeometry = useMemo(() => {
    if (!geometry?.source?.shape) return null;
    try {
      return shapeToGeometry(geometry.source.shape);
    } catch (e) {
      console.error("Failed mapping source geometry:", e);
      return null;
    }
  }, [geometry?.source]);

  // Adjust grid and axes dynamically
  const gridSize = Math.ceil((maxExtent * 2.5) / 10) * 10;
  const axesSize = gridSize / 2;
  const magnitude = Math.pow(10, Math.floor(Math.log10(axesSize || 1)));
  const detectorRadius = Math.max(0.1, magnitude * 0.05);

  // Ensure camera isn't clipped and position covers the extent.
  // Use a large far plane (maxExtent * 1000) so zooming out does not cause far-plane clipping.
  // logarithmicDepthBuffer (set on Canvas) preserves depth precision at large far/near ratios.
  const camPos = maxExtent * 2;
  const nearPlane = Math.max(0.01, maxExtent * 0.0001);
  const farPlane = Math.max(100000, maxExtent * 1000);

  return (
    <Canvas
      camera={{ position: [camPos, camPos, camPos], fov: 50, near: nearPlane, far: farPlane }}
      gl={{ logarithmicDepthBuffer: true }}
    >
      {/* Lights */}
      <ambientLight intensity={0.5} />
      <directionalLight position={[maxExtent, maxExtent, maxExtent]} intensity={1} />
      <directionalLight position={[-maxExtent, maxExtent, -maxExtent]} intensity={0.5} />

      {/* Controls */}
      <OrbitControls makeDefault maxDistance={farPlane * 0.9} />
      <AxesWithTicks size={axesSize} />

      {/* CSG Cells */}
      {geometry?.cells?.map((cell, i) => {
        if (hiddenCells.has(i)) return null;
        return (
          <CsgCellView
            key={`cell-${cell.material_name}-${i}`}
            cell={cell}
            baseGeometries={baseGeometries}
            color={cellStyles[i]?.color}
            opacity={cellStyles[i]?.opacity}
          />
        );
      })}

      {/* Detectors */}
      {showDetectors && geometry?.detectors.map((det, i) => (
        <Sphere key={`det-${det.name}-${i}`} args={[detectorRadius]} position={det.position}>
          <meshStandardMaterial color="red" />
        </Sphere>
      ))}

      {/* Source */}
      {showSource && sourceGeometry && (
        <mesh geometry={sourceGeometry}>
          <meshStandardMaterial color="yellow" emissive="yellow" emissiveIntensity={0.8} transparent opacity={0.6} depthWrite={false} side={THREE.DoubleSide} />
        </mesh>
      )}
    </Canvas>
  );
};
