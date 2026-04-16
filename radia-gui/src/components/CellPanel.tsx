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
}

export const CellPanel: React.FC<Props> = ({ 
  geometry, 
  hiddenCells, 
  toggleCell,
  showSource,
  setShowSource,
  showDetectors,
  setShowDetectors
}) => {
  if (!geometry || !geometry.cells || geometry.cells.length === 0) return null;

  return (
    <div className="cell-panel">
      <div className="cell-panel-header">Cells</div>
      <ul className="cell-list">
        {geometry.cells.map((cell, idx) => {
          const isHidden = hiddenCells.has(idx);
          return (
            <li key={idx} className="cell-item">
              <label>
                <input
                  type="checkbox"
                  checked={!isHidden}
                  onChange={() => toggleCell(idx)}
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
