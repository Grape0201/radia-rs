import React from "react";
import { GeometryData } from "../types/geometry";

interface Props {
  geometry: GeometryData | null;
  hiddenCells: Set<number>;
  toggleCell: (index: number) => void;
  showSource: boolean;
  setShowSource: (val: boolean) => void;
  showDetectors: boolean;
  setShowDetectors: (val: boolean) => void;
  cellStyles: Record<number, {color: string, opacity: number}>;
  updateCellStyle: (idx: number, color: string, opacity: number) => void;
}

const stringToColor = (str: string) => {
  let hash = 0;
  for (let i = 0; i < str.length; i++) {
    hash = str.charCodeAt(i) + ((hash << 5) - hash);
  }
  const c = (hash & 0x00ffffff).toString(16).toUpperCase();
  return "#" + "00000".substring(0, 6 - c.length) + c;
};

export const CellPanel: React.FC<Props> = ({ 
  geometry, 
  hiddenCells, 
  toggleCell,
  showSource,
  setShowSource,
  showDetectors,
  setShowDetectors,
  cellStyles,
  updateCellStyle
}) => {
  if (!geometry || !geometry.cells || geometry.cells.length === 0) return null;

  return (
    <div className="cell-panel">
      <div className="cell-panel-header">Cells</div>
      <ul className="cell-list">
        {geometry.cells.map((cell, idx) => {
          const isHidden = hiddenCells.has(idx);
          const currentColor = cellStyles[idx]?.color ?? stringToColor(cell.material_name);
          const currentOpacity = cellStyles[idx]?.opacity ?? 0.5;

          return (
            <li key={idx} className="cell-item">
              <label>
                <input
                  type="checkbox"
                  checked={!isHidden}
                  onChange={() => toggleCell(idx)}
                />
                <input 
                  type="color" 
                  value={currentColor} 
                  onChange={(e) => updateCellStyle(idx, e.target.value, currentOpacity)} 
                  style={{ width: '20px', height: '20px', padding: 0, border: 'none', margin: '0 4px' }}
                />
                <input 
                  type="range" 
                  min="0" max="1" step="0.05" 
                  value={currentOpacity}
                  onChange={(e) => updateCellStyle(idx, currentColor, parseFloat(e.target.value))}
                  style={{ width: '50px', margin: '0 4px' }}
                  title={`Opacity: ${currentOpacity}`}
                />
                <span className="material-name">
                  {cell.material_name}
                  {cell.density !== undefined && cell.density !== null && (
                    <span className="cell-density"> ({cell.density} g/cm³)</span>
                  )}
                </span>
              </label>
            </li>
          );
        })}
      </ul>
      
      {(geometry.source || (geometry.detectors && geometry.detectors.length > 0)) && (
        <>
          <div className="cell-panel-header" style={{ marginTop: '12px' }}>Entities</div>
          <ul className="cell-list">
            {geometry.source && (
              <li className="cell-item">
                <label>
                  <input
                    type="checkbox"
                    checked={showSource}
                    onChange={(e) => setShowSource(e.target.checked)}
                  />
                  <span className="material-name">Source</span>
                </label>
              </li>
            )}
            {geometry.detectors && geometry.detectors.length > 0 && (
              <li className="cell-item">
                <label>
                  <input
                    type="checkbox"
                    checked={showDetectors}
                    onChange={(e) => setShowDetectors(e.target.checked)}
                  />
                  <span className="material-name">Detectors</span>
                </label>
              </li>
            )}
          </ul>
        </>
      )}
    </div>
  );
};
