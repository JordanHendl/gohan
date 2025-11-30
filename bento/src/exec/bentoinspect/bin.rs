use bento::CompilationResult;
use clap::{ArgAction, Parser};

/// CLI arguments for inspecting Bento Files and their metadata.
#[derive(Debug, Parser)]
#[command(author, version, about = "Inspect Bento shader artifacts", long_about = None)]
struct Args {
    /// Path to the Bento artifact to inspect
    file: String,

    /// Emit the artifact contents as pretty-printed JSON
    #[arg(long, action = ArgAction::SetTrue)]
    json: bool,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("Error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let artifact = CompilationResult::load_from_disk(&args.file)?;

    if args.json {
        let json = serde_json::to_string_pretty(&artifact)?;
        println!("{json}");
    } else {
        print_summary(&artifact);
    }

    Ok(())
}

fn print_summary(result: &CompilationResult) {
    println!(
        "Entry: {}",
        result.name.as_deref().unwrap_or("<unnamed shader>")
    );

    if let Some(file) = &result.file {
        println!("Source: {file}");
    }

    println!("Language: {:?}", result.lang);
    println!("Stage: {:?}", result.stage);
    println!("Variables:");
    for var in &result.variables {
        println!(
            "  {} -> set {}, binding {} ({:?}), count {}",
            var.name, var.set, var.kind.binding, var.kind.var_type, var.kind.count
        );
    }

    if !result.metadata.entry_points.is_empty() {
        println!("Entry points:");
        for entry in &result.metadata.entry_points {
            println!("  {entry}");
        }
    }

    if !result.metadata.inputs.is_empty() {
        println!("Inputs:");
        for input in &result.metadata.inputs {
            match input.location {
                Some(location) => println!("  @location({location}) {}", input.name),
                None => println!("  {}", input.name),
            }
        }
    }

    if !result.metadata.outputs.is_empty() {
        println!("Outputs:");
        for output in &result.metadata.outputs {
            match output.location {
                Some(location) => println!("  @location({location}) {}", output.name),
                None => println!("  {}", output.name),
            }
        }
    }

    if let Some(vertex) = &result.metadata.vertex {
        println!("Vertex layout (stride {} bytes):", vertex.stride);
        for entry in &vertex.entries {
            println!(
                "  @location({}) offset {} -> {:?}",
                entry.location, entry.offset, entry.format
            );
        }
    }

    if let Some([x, y, z]) = result.metadata.workgroup_size {
        println!("Workgroup size: {x} x {y} x {z}");
    }

    println!("SPIR-V words: {}", result.spirv.len());
    let byte_size = result.spirv.len() * std::mem::size_of::<u32>();
    println!("Output size: {} bytes", byte_size);
}
