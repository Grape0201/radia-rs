import React, { useEffect } from "react";
import MonacoEditor, { useMonaco } from "@monaco-editor/react";
import { configureMonacoYaml } from "monaco-yaml";
import schema from "../assets/radia-schema.json";

interface EditorProps {
  value: string;
  onChange: (value: string) => void;
}

export const Editor: React.FC<EditorProps> = ({ value, onChange }) => {
  const monaco = useMonaco();

  useEffect(() => {
    if (monaco) {
      configureMonacoYaml(monaco, {
        enableSchemaRequest: true,
        schemas: [
          {
            uri: "http://radia-schema/schema.json",
            // apply this schema to all yaml documents
            fileMatch: ["*"],
            schema: schema,
          },
        ],
      });
    }
  }, [monaco]);

  return (
    <MonacoEditor
      language="yaml"
      theme="vs-dark"
      value={value}
      onChange={(val) => onChange(val || "")}
      options={{
        minimap: { enabled: false },
        wordWrap: "on",
      }}
    />
  );
};
