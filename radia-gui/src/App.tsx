import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Editor } from "./components/Editor";
import { Viewer3D } from "./components/Viewer3D";
import { ValidationPanel } from "./components/ValidationPanel";
import { CellPanel } from "./components/CellPanel";
import { GeometryData, ValidationResult } from "./types/geometry";
import "./App.css";

const DEFAULT_YAML = `world:
  primitives:
    - name: "shield"
      type: "Sphere"
      center: [0.0, 0.0, 0.0]
      radius: 10.0
    - name: "core"
      type: "Sphere"
      center: [0.0, 0.0, 0.0]
      radius: 2.0
  cells:
    - material_name: "Water"
      csg: "shield - core"
    - material_name: "Iron"
      csg: "core"
user_defined_materials:
  Water:
    density: 1.0
    composition: { 1: 0.111, 8: 0.889 }
  Iron:
    density: 7.874
    composition: { 26: 1.0 }
dose_quantity:
  energy_groups: [1.0]
  conversion_factors: [1.0]
detectors:
  - name: "det1"
    position: [12.0, 0.0, 0.0]
source:
  type: "Point"
  position: [0.0, 0.0, 0.0]
  intensity: 100.0
  energy_groups: [1.0]
  intensity_by_group: [1.0]
`;

function App() {
  const [yaml, setYaml] = useState(DEFAULT_YAML);
  const [validation, setValidation] = useState<ValidationResult | null>(null);
  const [geometry, setGeometry] = useState<GeometryData | null>(null);
  const [currentPath, setCurrentPath] = useState<string | null>(null);
  const [hiddenCells, setHiddenCells] = useState<Set<number>>(new Set());
  const [showSource, setShowSource] = useState(true);
  const [showDetectors, setShowDetectors] = useState(true);

  const toggleCell = useCallback((idx: number) => {
    setHiddenCells(prev => {
      const next = new Set(prev);
      if (next.has(idx)) {
        next.delete(idx);
      } else {
        next.add(idx);
      }
      return next;
    });
  }, []);

  const parseAndValidate = useCallback(async (content: string) => {
    try {
      const vResult: ValidationResult = await invoke("validate_yaml", { yaml: content });
      setValidation(vResult);

      if (vResult.valid) {
        const gResult: GeometryData = await invoke("parse_geometry", { yaml: content });
        setGeometry(gResult);
      }
    } catch (err) {
      console.error("Backend error:", err);
    }
  }, []);

  useEffect(() => {
    const timeout = setTimeout(() => {
      parseAndValidate(yaml);
    }, 400); // Debounce editor updates
    return () => clearTimeout(timeout);
  }, [yaml, parseAndValidate]);

  const handleOpen = async () => {
    try {
      const res: [string, string] | null = await invoke("open_file_dialog");
      if (res) {
        setCurrentPath(res[0]);
        setYaml(res[1]);
      }
    } catch (err) {
      console.error(err);
    }
  };

  const handleSave = async () => {
    try {
      if (currentPath) {
        await invoke("save_file_to_path", { path: currentPath, content: yaml });
      } else {
        const newPath: string | null = await invoke("save_file_dialog", { content: yaml });
        if (newPath) setCurrentPath(newPath);
      }
    } catch (err) {
      console.error(err);
    }
  };

  return (
    <div className="layout">
      <header className="toolbar">
        <div className="toolbar-title">
          <span>Radia Geometry GUI</span>
          <span className="file-path">{currentPath ? currentPath : "Untitled.yaml"}</span>
        </div>
        <div className="toolbar-actions">
          <button onClick={handleOpen}>Open</button>
          <button onClick={handleSave}>Save</button>
        </div>
      </header>

      <div className="main-content">
        <div className="left-pane">
          <div className="editor-wrapper">
            <Editor value={yaml} onChange={setYaml} />
          </div>
          <div className="bottom-panels">
            <CellPanel 
              geometry={geometry} 
              hiddenCells={hiddenCells} 
              toggleCell={toggleCell} 
              showSource={showSource}
              setShowSource={setShowSource}
              showDetectors={showDetectors}
              setShowDetectors={setShowDetectors}
            />
            <ValidationPanel result={validation} />
          </div>
        </div>
        <div className="right-pane">
          <Viewer3D geometry={geometry} hiddenCells={hiddenCells} showSource={showSource} showDetectors={showDetectors} />
        </div>
      </div>
    </div>
  );
}

export default App;
