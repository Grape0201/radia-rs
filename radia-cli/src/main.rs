use clap::{Parser, ValueEnum};
use miette::{IntoDiagnostic, Result};
use radia_cli::{JsonMassAttenuationProvider, load_material_registry_from_file};
use radia_core::buildup::GPBuildupProvider;
use radia_core::kernel::{FastCollector, calculate_dose_rate_parallel};
use radia_core::mass_attenuation::{MaterialIndex, MaterialRegistry};
use radia_core::physics::MaterialPhysicsTable;
use radia_input::SimulationInput;
use radia_report::{DetailedCollector, PhysicsSummary};
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::info;
use tracing_subscriber::{EnvFilter, fmt};

#[derive(ValueEnum, Clone, Debug)]
enum CollectorSub {
    Fast,
    Detailed,
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(help = "The input YAML file for the simulation")]
    input: PathBuf,

    #[arg(
        short,
        long,
        value_enum,
        default_value_t = CollectorSub::Fast,
        help = "The data collector to use"
    )]
    collector: CollectorSub,

    #[arg(
        short,
        long,
        default_value = "report.md",
        help = "Output file for the detailed markdown report"
    )]
    output_report: PathBuf,
}

fn main() -> Result<()> {
    fmt()
        .with_timer(tracing_subscriber::fmt::time::uptime())
        .compact()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();
    info!("Loading input from {:?}", args.input);

    let sim_input = SimulationInput::from_yaml_file(&args.input).into_diagnostic()?;

    let conversion_factors = sim_input
        .get_interpolated_conversion_factors()
        .into_diagnostic()?;

    let SimulationInput {
        world,
        user_defined_materials,
        dose_quantity,
        detectors,
        source,
    } = sim_input;

    let radia_input::DoseQuantityInput {
        buildup_params,
        buildup_alias_map,
        energy_groups: _,
        conversion_factors: _,
    } = dose_quantity;

    let mut used_materials: Vec<String> = world
        .cells
        .iter()
        .map(|c| c.material_name.clone())
        .collect();
    used_materials.sort();
    used_materials.dedup();

    let material_map: HashMap<String, MaterialIndex> = used_materials
        .iter()
        .enumerate()
        .map(|(i, name)| (name.clone(), i as MaterialIndex))
        .collect();

    info!("Building world...");
    let world = world.build(&material_map).into_diagnostic()?;

    info!("Building materials...");
    let mut registry = match load_material_registry_from_file("data/compositions.json") {
        Ok(r) => r,
        Err(_) => MaterialRegistry::new(),
    };
    info!("Registering user defined materials...");
    for (name, mat_input) in user_defined_materials {
        let def = mat_input.build();
        registry.insert(name, def);
    }

    info!("Building buildup parameters...");
    let mut gp_provider = GPBuildupProvider::new();
    for (name, params) in buildup_params {
        gp_provider.insert_data(name, params.into_iter().map(|p| p.into()).collect());
    }

    info!("Loading physical datatables...");
    let provider = match JsonMassAttenuationProvider::from_file("data/elements.json") {
        Ok(p) => p,
        Err(_) => JsonMassAttenuationProvider::from_file("../data/elements.json").expect(
            "Failed to load elements.json (looked in 'data/elements.json' and '../data/elements.json')",
        ),
    };

    info!("Calculating dose rates...");
    let mut detector_doses = HashMap::new();

    let energy_groups = source.energy_groups;
    let intensity_by_group = source.intensity_by_group;
    let srcs = source.shape.build();
    info!("Number of sources: {}", srcs.len());

    info!("Generating material physics table for a source...");
    let physics_table = MaterialPhysicsTable::generate(
        &used_materials,
        &buildup_alias_map,
        &registry,
        &energy_groups,
        &provider,
        &gp_provider,
    )
    .into_diagnostic()?;

    let chunk_size = 1000;

    match args.collector {
        CollectorSub::Fast => {
            for det in &detectors {
                let mut collector = FastCollector::default();
                let dose_rate = calculate_dose_rate_parallel(
                    &physics_table,
                    &world,
                    &conversion_factors,
                    &intensity_by_group,
                    glam::Vec3A::from(det.position),
                    &srcs,
                    chunk_size,
                    &mut collector,
                );
                *detector_doses.entry(det.name.clone()).or_insert(0.0) += dose_rate;
            }
        }
        CollectorSub::Detailed => {
            let physics_summary = PhysicsSummary {
                cross_section_library: "NIST XCOM (JSON)".to_string(),
                buildup_library: "Geometric Progression (GP)".to_string(),
                conversion_factors: "Interpolated".to_string(),
            };

            // Attempt to read the original file to echo its contents, otherwise default to Null
            let input_echo = std::fs::read_to_string(&args.input)
                .ok()
                .map(serde_json::Value::String)
                .unwrap_or(serde_json::Value::Null);

            let mut global_collector = DetailedCollector::new(
                physics_summary,
                input_echo,
                args.input.to_string_lossy().to_string(),
            );

            for det in &detectors {
                let dose_rate = calculate_dose_rate_parallel(
                    &physics_table,
                    &world,
                    &conversion_factors,
                    &intensity_by_group,
                    glam::Vec3A::from(det.position),
                    &srcs,
                    chunk_size,
                    &mut global_collector,
                );
                *detector_doses.entry(det.name.clone()).or_insert(0.0) += dose_rate;
            }

            let markdown = global_collector.to_markdown();
            if let Err(e) = std::fs::write(&args.output_report, markdown) {
                tracing::error!("Failed to write report to {:?}: {}", args.output_report, e);
            } else {
                info!("Wrote detailed report to {:?}", args.output_report);
            }
        }
    }

    for det in detectors {
        let dose_rate = detector_doses.get(&det.name).unwrap_or(&0.0);
        info!(
            "Detector '{}' at {:?}: Dose Rate = {:.6e}",
            det.name, det.position, dose_rate
        );
    }

    Ok(())
}
