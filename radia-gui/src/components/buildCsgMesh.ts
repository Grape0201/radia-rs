import * as THREE from "three";
import { Brush, Evaluator, ADDITION, SUBTRACTION, INTERSECTION } from "three-bvh-csg";
import { InstructionJson } from "../types/geometry";

const evaluator = new Evaluator();

/**
 * Builds a final CSG mesh by executing an RPN stack over a set of geometries.
 */
export const buildCsgMesh = (
  instructions: InstructionJson[],
  geometries: THREE.BufferGeometry[]
): THREE.Mesh | null => {
  if (!instructions || instructions.length === 0) return null;

  const stack: Brush[] = [];

  for (const inst of instructions) {
    if (inst.op === "push_primitive") {
      const geom = geometries[inst.index];
      if (!geom) throw new Error(`Invalid shape index: ${inst.index}`);
      
      const brush = new Brush(geom.clone());
      brush.updateMatrixWorld();
      stack.push(brush);
      continue;
    }

    if (inst.op === "complement") {
      throw new Error("Complement is not supported");
    }

    // All remaining ops are binary CSG operations
    const right = stack.pop();
    const left = stack.pop();

    if (!left || !right) {
        throw new Error(`Invalid RPN instruction stack state on op ${inst.op}`);
    }

    let operation;
    switch (inst.op) {
      case "union":
        operation = ADDITION;
        break;
      case "intersection":
        operation = INTERSECTION;
        break;
      case "difference":
        operation = SUBTRACTION;
        break;
      default:
        throw new Error(`Unknown CSG operation: ${(inst as any).op}`);
    }

    const resultBrush = evaluator.evaluate(left, right, operation);
    stack.push(resultBrush);
  }

  if (stack.length !== 1) {
    throw new Error("Invalid CSG RPN: final stack size must be 1.");
  }

  const finalBrush = stack[0];
  const mesh = new THREE.Mesh(finalBrush.geometry.clone());
  
  // Cleanup original brushes
  return mesh;
};
