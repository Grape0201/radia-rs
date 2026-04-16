export interface ValidationResult {
  valid: boolean;
  errors: string[];
}

export type PrimitiveShape =
  | { type: "Sphere"; center: [number, number, number]; radius: number }
  | { type: "Box"; min: [number, number, number]; max: [number, number, number] }
  | { type: "Cylinder"; center: [number, number, number]; vector: [number, number, number]; radius: number };

export type InstructionJson =
  | { op: "push_primitive"; index: number }
  | { op: "union" }
  | { op: "intersection" }
  | { op: "difference" }
  | { op: "complement" };

export interface PrimitiveData {
  name: string;
  shape: PrimitiveShape;
}

export interface CellData {
  material_name: string;
  density?: number;
  csg_string: string;
  csg: { instructions: InstructionJson[] };
}

export interface DetectorData {
  name: string;
  position: [number, number, number];
}

export interface SourceData {
  shape: PrimitiveShape;
}

export interface GeometryData {
  primitives: PrimitiveData[];
  cells: CellData[];
  detectors: DetectorData[];
  source?: SourceData;
}
