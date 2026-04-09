import React from "react";
import { ValidationResult } from "../types/geometry";

interface Props {
  result: ValidationResult | null;
}

export const ValidationPanel: React.FC<Props> = ({ result }) => {
  if (!result) return null;

  const validClass = result.valid ? "valid" : "invalid";
  
  return (
    <div className={`validation-panel ${validClass}`}>
      <div className="validation-header">
        {result.valid ? "✅ YAML conforms to schema and bounds" : "❌ Validation Errors"}
      </div>
      {!result.valid && (
        <ul className="validation-errors">
          {result.errors.map((error, idx) => (
            <li key={idx}>{error}</li>
          ))}
        </ul>
      )}
    </div>
  );
};
