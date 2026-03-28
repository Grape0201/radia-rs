use miette::{IntoDiagnostic, Result};
use radia_cli::{JsonMassAttenuationProvider, load_material_registry_from_file};
use radia_core::kernel::calculate_dose_rate_parallel;
use radia_core::material::{MaterialIndex, MaterialRegistry};
use radia_core::physics::{GPBuildupProvider, MaterialPhysicsTable};
use radia_input::SimulationInput;
use std::env;
use tracing::info;
use tracing_subscriber::{EnvFilter, fmt};

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: radia-cli <input.yaml>");
        std::process::exit(1);
    }

    fmt()
        .with_timer(tracing_subscriber::fmt::time::uptime())
        .compact()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let input_path = &args[1];
    info!("Loading input from {}", input_path);

    let sim_input = SimulationInput::from_yaml_file(input_path).into_diagnostic()?;
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

    let material_map: std::collections::HashMap<String, MaterialIndex> = used_materials
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
        Err(_) => JsonMassAttenuationProvider::from_file("../data/elements.json")
            .expect("Failed to load elements.json (looked in 'data/elements.json' and '../data/elements.json')"),
    };

    info!("Calculating dose rates...");
    let mut detector_doses = std::collections::HashMap::new();

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

    for det in &detectors {
        let chunk_size = 1000;
        let dose_rate = calculate_dose_rate_parallel(
            &physics_table,
            &world,
            &conversion_factors,
            &intensity_by_group,
            glam::Vec3A::from(det.position),
            &srcs,
            chunk_size,
        );
        *detector_doses.entry(det.name.clone()).or_insert(0.0) += dose_rate;
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
