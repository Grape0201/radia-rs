use radia_input::source::SourceShapeInput;
use radia_input::world::PrimitiveInput;
use radia_input::SimulationInput;
use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;
use tokio::sync::oneshot;

// ── Data types sent to the frontend ──────────────────────────────────────────

#[derive(Debug, Serialize, Clone)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
}

/// Serialised primitive shape forwarded to the Three.js renderer.
#[derive(Debug, Serialize, Clone)]
#[serde(tag = "type")]
pub enum PrimitiveShape {
    Sphere {
        center: [f32; 3],
        radius: f32,
    },
    Box {
        min: [f32; 3],
        max: [f32; 3],
    },
    Cylinder {
        center: [f32; 3],
        /// Un-normalised axis vector; its magnitude equals the cylinder height.
        vector: [f32; 3],
        radius: f32,
    },
}

#[derive(Debug, Serialize, Clone)]
pub struct PrimitiveData {
    pub name: String,
    pub shape: PrimitiveShape,
}

#[derive(Debug, Serialize, Clone)]
pub struct CellData {
    pub material_name: String,
    pub csg: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct DetectorData {
    pub name: String,
    pub position: [f32; 3],
}

#[derive(Debug, Serialize, Clone)]
pub struct SourceData {
    pub shape_type: String,
    pub center: Option<[f32; 3]>,
    pub radius: Option<f32>,
}

#[derive(Debug, Serialize, Clone)]
pub struct GeometryData {
    pub primitives: Vec<PrimitiveData>,
    pub cells: Vec<CellData>,
    pub detectors: Vec<DetectorData>,
    pub source: Option<SourceData>,
}

// ── Tauri commands ────────────────────────────────────────────────────────────

/// Validate YAML text using `radia-input` and return structured error info.
#[tauri::command]
fn validate_yaml(yaml: String) -> ValidationResult {
    match SimulationInput::from_yaml_str(&yaml) {
        Ok(_) => ValidationResult {
            valid: true,
            errors: vec![],
        },
        Err(e) => {
            let raw = e.to_string();
            let errors: Vec<String> = raw
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| l.to_string())
                .collect();
            ValidationResult {
                valid: false,
                errors: if errors.is_empty() { vec![raw] } else { errors },
            }
        }
    }
}

/// Parse validated YAML and extract geometry data for the 3-D viewer.
#[tauri::command]
fn parse_geometry(yaml: String) -> Result<GeometryData, String> {
    let input = SimulationInput::from_yaml_str(&yaml).map_err(|e| e.to_string())?;

    let primitives: Vec<PrimitiveData> = input
        .world
        .primitives
        .iter()
        .map(|p| {
            let (name, shape) = match p {
                PrimitiveInput::Sphere { name, center, radius } => (
                    name.clone(),
                    PrimitiveShape::Sphere {
                        center: *center,
                        radius: *radius,
                    },
                ),
                PrimitiveInput::RectangularParallelPiped { name, bounds } => (
                    name.clone(),
                    PrimitiveShape::Box {
                        min: bounds.min,
                        max: bounds.max,
                    },
                ),
                PrimitiveInput::FiniteCylinder {
                    name,
                    center,
                    vector,
                    radius,
                } => (
                    name.clone(),
                    PrimitiveShape::Cylinder {
                        center: *center,
                        vector: *vector,
                        radius: *radius,
                    },
                ),
            };
            PrimitiveData { name, shape }
        })
        .collect();

    let cells: Vec<CellData> = input
        .world
        .cells
        .iter()
        .map(|c| CellData {
            material_name: c.material_name.clone(),
            csg: c.csg.clone(),
        })
        .collect();

    let detectors: Vec<DetectorData> = input
        .detectors
        .iter()
        .map(|d| DetectorData {
            name: d.name.clone(),
            position: d.position,
        })
        .collect();

    // Extract a representative source position / radius for visualisation.
    let source = Some(match &input.source.shape {
        SourceShapeInput::Point { position, .. } => SourceData {
            shape_type: "Point".to_string(),
            center: Some(*position),
            radius: Some(2.0), // fixed marker size
        },
        SourceShapeInput::Sphere { center, radius, .. } => SourceData {
            shape_type: "Sphere".to_string(),
            center: Some(*center),
            radius: Some(*radius),
        },
        SourceShapeInput::Cylinder { start, axis, radius, .. } => SourceData {
            shape_type: "Cylinder".to_string(),
            center: Some([
                start[0] + axis[0] / 2.0,
                start[1] + axis[1] / 2.0,
                start[2] + axis[2] / 2.0,
            ]),
            radius: Some(*radius),
        },
        SourceShapeInput::Cuboid { bounds, .. } => SourceData {
            shape_type: "Cuboid".to_string(),
            center: Some([
                (bounds.min[0] + bounds.max[0]) / 2.0,
                (bounds.min[1] + bounds.max[1]) / 2.0,
                (bounds.min[2] + bounds.max[2]) / 2.0,
            ]),
            radius: None,
        },
    });

    Ok(GeometryData {
        primitives,
        cells,
        detectors,
        source,
    })
}

/// Show a native open-file dialog and return the chosen path and its contents.
#[tauri::command]
async fn open_file_dialog(app: AppHandle) -> Result<Option<(String, String)>, String> {
    let (tx, rx) = oneshot::channel();

    app.dialog()
        .file()
        .add_filter("YAML files", &["yaml", "yml"])
        .pick_file(move |path| {
            let _ = tx.send(path);
        });

    match rx.await.map_err(|_| "Dialog closed unexpectedly".to_string())? {
        Some(tauri_plugin_dialog::FilePath::Path(p)) => {
            let path_str = p.to_string_lossy().to_string();
            let content = std::fs::read_to_string(&p).map_err(|e| e.to_string())?;
            Ok(Some((path_str, content)))
        }
        _ => Ok(None),
    }
}

/// Show a native save-file dialog and write `content` to the chosen path.
#[tauri::command]
async fn save_file_dialog(app: AppHandle, content: String) -> Result<Option<String>, String> {
    let (tx, rx) = oneshot::channel();

    app.dialog()
        .file()
        .add_filter("YAML files", &["yaml", "yml"])
        .save_file(move |path| {
            let _ = tx.send(path);
        });

    match rx.await.map_err(|_| "Dialog closed unexpectedly".to_string())? {
        Some(tauri_plugin_dialog::FilePath::Path(p)) => {
            std::fs::write(&p, &content).map_err(|e| e.to_string())?;
            Ok(Some(p.to_string_lossy().to_string()))
        }
        _ => Ok(None),
    }
}

/// Overwrite a file at a known path (used for Ctrl+S without re-opening dialog).
#[tauri::command]
async fn save_file_to_path(path: String, content: String) -> Result<(), String> {
    std::fs::write(&path, content).map_err(|e| e.to_string())
}

// ── App entry point ───────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            validate_yaml,
            parse_geometry,
            open_file_dialog,
            save_file_dialog,
            save_file_to_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
