use clap::{Parser, ValueEnum};
use miette::{IntoDiagnostic, Result};
use radia_cli::{
    JsonMassAttenuationProvider, load_buildup_registry_from_file, load_material_registry_from_file,
};
use radia_core::buildup::{GPBuildupProvider, GPParams};
use radia_core::csg::World;
use radia_core::kernel::{DoseCollector, FastCollector, calculate_dose_rate_parallel};
use radia_core::mass_attenuation::{MaterialIndex, MaterialRegistry};
use radia_core::physics::MaterialPhysicsTable;
use radia_core::source::PointSource;
use radia_input::{DetectorInput, SimulationInput};
use radia_report::DetailedCollector;
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{info, warn};
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

    #[arg(
        long,
        default_value_t = 1000,
        help = "The number of source points to process in a single parallel chunk"
    )]
    chunk_size: usize,

    #[arg(
        long,
        env = "RADIA_MATERIAL_REGISTRY",
        help = "Path to the material registry JSON file (overrides default data/compositions.json)"
    )]
    material_registry: Option<PathBuf>,
    #[arg(
        long,
        env = "RADIA_BUILDUP_REGISTRY",
        help = "Path to the buildup registry JSON file"
    )]
    buildup_registry: Option<PathBuf>,
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

    let mut used_materials = sim_input
        .world
        .cells
        .iter()
        .map(|c| c.material_name.clone())
        .collect::<Vec<_>>();
    used_materials.sort();
    used_materials.dedup();

    let material_map: HashMap<String, MaterialIndex> = used_materials
        .iter()
        .enumerate()
        .map(|(i, name)| (name.clone(), i as MaterialIndex))
        .collect();

    info!("Building world...");
    // Clone world input to keep sim_input intact for the collector later
    let world = sim_input
        .world
        .clone()
        .build(&material_map)
        .into_diagnostic()?;

    info!("Building materials...");
    let mut registry = if let Some(path) = &args.material_registry {
        load_material_registry_from_file(path)?
    } else {
        match load_material_registry_from_file("data/compositions.json") {
            Ok(r) => r,
            Err(_) => {
                warn!("Material registry file not found, using empty registry.");
                MaterialRegistry::new()
            }
        }
    };
    info!("Registering user defined materials...");
    for (name, mat_input) in &sim_input.user_defined_materials {
        let def = mat_input.clone().build();
        registry.insert(name.clone(), def);
    }

    info!("Building buildup parameters...");
    let mut gp_provider = if let Some(path) = &args.buildup_registry {
        load_buildup_registry_from_file(path)?
    } else {
        match load_buildup_registry_from_file("data/buildup.json") {
            Ok(r) => r,
            Err(_) => GPBuildupProvider::new(),
        }
    };
    info!("Registering user defined buildup parameters...");
    for (name, params) in &sim_input.dose_quantity.buildup_params {
        gp_provider.insert_data(
            name.clone(),
            params.iter().map(|p| GPParams::from(p.clone())).collect(),
        );
    }

    info!("Loading physical datatables...");
    let provider = match JsonMassAttenuationProvider::from_file("data/elements.json") {
        Ok(p) => p,
        Err(_) => JsonMassAttenuationProvider::from_file("../data/elements.json").expect(
            "Failed to load elements.json (looked in 'data/elements.json' and '../data/elements.json')",
        ),
    };

    let energy_groups = &sim_input.source.energy_groups;
    let intensity_by_group = &sim_input.source.intensity_by_group;
    let srcs = sim_input.source.shape.clone().build();

    info!("Generating material physics table for a source...");
    let physics_table = MaterialPhysicsTable::generate(
        &used_materials,
        &sim_input.dose_quantity.buildup_alias_map,
        &registry,
        energy_groups,
        &provider,
        &gp_provider,
    )
    .into_diagnostic()?;

    info!("Calculating dose rates...");

    match args.collector {
        CollectorSub::Fast => {
            let mut collector = FastCollector::default();
            run_simulation(
                &mut collector,
                &physics_table,
                &world,
                &conversion_factors,
                intensity_by_group,
                &srcs,
                &sim_input.detectors,
                args.chunk_size,
            );
        }
        CollectorSub::Detailed => {
            let mut collector = DetailedCollector::new(
                &sim_input,
                &physics_table,
                &registry,
                &world,
                args.input.to_string_lossy().to_string(),
            );

            run_simulation(
                &mut collector,
                &physics_table,
                &world,
                &conversion_factors,
                intensity_by_group,
                &srcs,
                &sim_input.detectors,
                args.chunk_size,
            );

            let markdown = collector.to_markdown();
            if let Err(e) = std::fs::write(&args.output_report, markdown) {
                tracing::error!("Failed to write report to {:?}: {}", args.output_report, e);
            } else {
                info!("Wrote detailed report to {:?}", args.output_report);
            }
        }
    }

    Ok(())
}

/// Run the dose-rate calculation loop over all detectors using a generic collector.
fn run_simulation<C: DoseCollector + Send + Default>(
    collector: &mut C,
    physics_table: &MaterialPhysicsTable,
    world: &World,
    conversion_factors: &[f32],
    intensity_by_group: &[f32],
    sources: &[PointSource],
    detectors: &[DetectorInput],
    chunk_size: usize,
) {
    for det in detectors {
        let dose_rate = calculate_dose_rate_parallel(
            physics_table,
            world,
            conversion_factors,
            intensity_by_group,
            glam::Vec3A::from(det.position),
            sources,
            chunk_size,
            collector,
        );
        info!(
            "Detector '{}' at {:?}: Dose Rate = {:.6e}",
            det.name, det.position, dose_rate
        );
    }
}
