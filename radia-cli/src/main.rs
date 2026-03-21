use miette::{IntoDiagnostic, Result};
use radia_cli::kernel::calculate_dose_rate_parallel;
use radia_core::material::{JsonMassAttenuationProvider, MaterialRegistry};
use radia_core::physics::{GPBuildupProvider, MaterialPhysicsTable};
use radia_input::SimulationInput;
use std::env;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: radia-cli <input.yaml>");
        std::process::exit(1);
    }

    let input_path = &args[1];
    println!("Loading input from {}", input_path);

    let sim_input = SimulationInput::from_yaml_file(input_path).into_diagnostic()?;

    let mut used_materials: Vec<String> = sim_input
        .world
        .cells
        .iter()
        .map(|c| c.material_name.clone())
        .collect();
    used_materials.sort();
    used_materials.dedup();

    let material_map: std::collections::HashMap<String, u32> = used_materials
        .iter()
        .enumerate()
        .map(|(i, name)| (name.clone(), i as u32))
        .collect();

    println!("Building world...");
    let world = sim_input.world.build(&material_map).into_diagnostic()?;

    println!("Building materials...");
    let mut registry = match MaterialRegistry::from_file("data/elements.json") {
        Ok(r) => r,
        Err(_) => MaterialRegistry::new(),
    };
    for mat_input in sim_input.materials {
        let (name, def) = mat_input.build().into_diagnostic()?;
        registry.insert(name, def);
    }

    println!("Building buildup parameters...");
    let mut gp_provider = GPBuildupProvider::new();
    for buildup_input in sim_input.buildup_params {
        let (name, params) = buildup_input.build().into_diagnostic()?;
        gp_provider.insert_data(name, params);
    }

    let mut detectors = Vec::new();
    for det_input in sim_input.detectors {
        let (name, pos) = det_input.build();
        detectors.push((name, pos));
    }

    println!("Loading physical datatables...");
    let provider = match JsonMassAttenuationProvider::from_file("data/elements.json") {
        Ok(p) => p,
        Err(_) => JsonMassAttenuationProvider::from_file("../data/elements.json")
            .expect("Failed to load elements.json (looked in 'data/elements.json' and '../data/elements.json')"),
    };

    println!("Calculating dose rates...");
    let mut detector_doses = std::collections::HashMap::new();

    let energy_groups = sim_input.source.energy_groups;
    let intensity_by_group = sim_input.source.intensity_by_group;
    let srcs = sim_input.source.shape.build().into_diagnostic()?;

    println!("Generating material physics table for a source...");
    let physics_table = MaterialPhysicsTable::generate(
        &used_materials,
        &sim_input.buildup_alias_map,
        &registry,
        &energy_groups,
        &provider,
        &gp_provider,
    )
    .into_diagnostic()?;

    let (get_mu, get_buildup) = physics_table.into_closures();

    for (name, pos) in &detectors {
        let chunk_size = 1000;
        let dose_rate = calculate_dose_rate_parallel(
            &get_mu,
            &get_buildup,
            &world,
            &sim_input.conversion_factors,
            &intensity_by_group,
            *pos,
            &srcs,
            chunk_size,
        );
        *detector_doses.entry(name.clone()).or_insert(0.0) += dose_rate;
    }

    for (name, pos) in detectors {
        let dose_rate = detector_doses.get(&name).unwrap_or(&0.0);
        println!(
            "Detector '{}' at {:?}: Dose Rate = {:.6e}",
            name, pos, dose_rate
        );
    }

    Ok(())
}
