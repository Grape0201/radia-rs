# Radia GUI

Experimental desktop application for editing and visualizing radiation shielding simulations.

## Tech Stack

### Backend (Rust / Tauri)
- **Framework**: [Tauri v2](https://v2.tauri.app/) for a lightweight, secure desktop environment.
- **Core Logic**: Integrates directly with the `radia-input` workspace crate for YAML parsing and multi-stage validation.
- **Commands**:
  - `validate_yaml`: Real-time validation of simulation input.
  - `parse_geometry`: Extraction of geometry primitives for 3D visualization.
  - Native file system integration via `tauri-plugin-dialog`.

### Frontend (React / TypeScript)
- **Framework**: [React 19](https://react.dev/) with TypeScript for a robust UI.
- **3D Visualization**: 
  - [Three.js](https://threejs.org/) for high-performance rendering.
  - [@react-three/fiber](https://r3f.docs.pmnd.rs/) & [@react-three/drei](https://github.com/pmndrs/drei) for declarative 3D components.
- **Editor**:
  - [Monaco Editor](https://microsoft.github.io/monaco-editor/) (via `@monaco-editor/react`) for a VS Code-like editing experience.
  - [monaco-yaml](https://github.com/remcohaszing/monaco-yaml) for YAML schema support and syntax highlighting.
- **Styling**: Vanilla CSS for layouts and interactive components.

### Build Tools
- **Package Manager**: [Bun](https://bun.sh/) for fast dependency management and execution.
- **Bundler**: [Vite](https://vitejs.dev/) for optimal frontend development performance.

## Getting Started

### Prerequisites
- [Rust](https://www.rust-lang.org/) (latest stable)
- [Bun](https://bun.sh/)

### Development
```bash
# Install dependencies
bun install

# Start the application in development mode
bun tauri dev
```

### Build
```bash
# Build the production application
bun tauri build
```
