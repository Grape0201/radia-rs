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

    println!("Building world...");
    let world = sim_input.world.build().into_diagnostic()?;

    println!("Building materials...");
    let mut registry = MaterialRegistry::new();
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

    println!("Building sources...");
    let mut sources = Vec::new();
    for src_input in sim_input.sources {
        let srcs = src_input.build().into_diagnostic()?;
        sources.extend(srcs);
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

    // Default energy groups for now
    let energy_groups = vec![1.0];
    let conversion_factors = vec![1.0];

    println!("Generating material physics table...");
    let physics_table = MaterialPhysicsTable::generate(
        &sim_input.buildup_alias_map,
        &registry,
        &energy_groups,
        &provider,
        &gp_provider,
    )
    .into_diagnostic()?;

    let (get_mu, get_buildup) = physics_table.into_closures();

    println!("Calculating dose rates...");
    for (name, pos) in detectors {
        let chunk_size = 1000;
        let dose_rate = calculate_dose_rate_parallel(
            &get_mu,
            &get_buildup,
            &world,
            &conversion_factors,
            pos,
            &sources,
            chunk_size,
        );
        println!(
            "Detector '{}' at {:?}: Dose Rate = {:.6e}",
            name, pos, dose_rate
        );
    }

    Ok(())
}
