use bento::{Compiler, OptimizationLevel, Request, ShaderLang};
use clap::{ArgAction, Parser, ValueEnum};
use std::{collections::HashMap, path::PathBuf};

/// Command-line representation of supported shader languages.
#[derive(Debug, Clone, ValueEnum)]
enum LangArg {
    Slang,
    Glsl,
    Hlsl,
    Other,
}

impl From<LangArg> for ShaderLang {
    fn from(value: LangArg) -> Self {
        match value {
            LangArg::Slang => ShaderLang::Slang,
            LangArg::Glsl => ShaderLang::Glsl,
            LangArg::Hlsl => ShaderLang::Hlsl,
            LangArg::Other => ShaderLang::Other,
        }
    }
}

/// Command-line representation of shader stages.
#[derive(Debug, Clone, ValueEnum)]
enum StageArg {
    Vertex,
    Fragment,
    Compute,
}

impl From<StageArg> for dashi::ShaderType {
    fn from(value: StageArg) -> Self {
        match value {
            StageArg::Vertex => dashi::ShaderType::Vertex,
            StageArg::Fragment => dashi::ShaderType::Fragment,
            StageArg::Compute => dashi::ShaderType::Compute,
        }
    }
}

/// Command-line optimization levels for the Bento compiler.
#[derive(Debug, Clone, ValueEnum)]
enum OptArg {
    None,
    #[value(alias = "size")]
    FileSize,
    Performance,
}

impl From<OptArg> for OptimizationLevel {
    fn from(value: OptArg) -> Self {
        match value {
            OptArg::None => OptimizationLevel::None,
            OptArg::FileSize => OptimizationLevel::FileSize,
            OptArg::Performance => OptimizationLevel::Performance,
        }
    }
}

/// CLI surface for converting shader sources into Bento Files.
#[derive(Debug, Parser)]
#[command(author, version, about = "Compile shaders into Bento artifacts", long_about = None)]
struct Args {
    /// Path to the shader source file
    shader: String,

    /// Source language of the shader
    #[arg(short, long, value_enum, default_value = "glsl")]
    lang: LangArg,

    /// Shader stage to compile
    #[arg(short, long, value_enum)]
    stage: StageArg,

    /// Optimization level for the compiler
    #[arg(
        short = 'O',
        long = "optimization",
        value_enum,
        default_value = "none",
        alias = "opt"
    )]
    optimization: OptArg,

    /// Include debug symbols in the compiled output
    #[arg(long, action = ArgAction::SetTrue)]
    debug_symbols: bool,

    /// Output path for the compiled artifact
    #[arg(short, long, value_name = "PATH", default_value = "out.bto")]
    output: String,

    /// Optional name for the shader entry
    #[arg(short, long)]
    name: Option<String>,

    /// Print verbose compilation metadata
    #[arg(short, long, action = ArgAction::SetTrue)]
    verbose: bool,

    /// Preprocessor definitions passed to the compiler
    #[arg(short = 'D', long = "define", value_name = "NAME[=VALUE]")]
    defines: Vec<String>,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("Error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let defines = parse_defines(&args.defines)
        .map_err(|err| format!("Invalid preprocessor definition: {err}"))?;

    let request = Request {
        name: args.name.clone(),
        lang: args.lang.into(),
        stage: args.stage.into(),
        optimization: args.optimization.into(),
        debug_symbols: args.debug_symbols,
        defines,
    };

    let compiler = Compiler::new()?;
    let result = compiler.compile_from_file(&args.shader, &request)?;

    if args.verbose {
        print_metadata(&result);
    }

    let output_path = ensure_bto_extension(&args.output);
    result.save_to_disk(output_path.to_str().unwrap())?;

    Ok(())
}

fn parse_defines(raw_defines: &[String]) -> Result<HashMap<String, Option<String>>, String> {
    let mut defines = HashMap::new();

    for raw in raw_defines {
        if raw.is_empty() {
            return Err("definition cannot be empty".into());
        }

        let (name, value) = if let Some((name, value)) = raw.split_once('=') {
            if name.trim().is_empty() {
                return Err("definition is missing a name".into());
            }

            (name.trim().to_string(), Some(value.trim().to_string()))
        } else {
            (raw.trim().to_string(), None)
        };

        defines.insert(name, value);
    }

    Ok(defines)
}

fn print_metadata(result: &bento::CompilationResult) {
    println!(
        "Entry: {}",
        result.name.as_deref().unwrap_or("<unnamed shader>")
    );
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
    let byte_size = result.spirv.len() * std::mem::size_of::<u32>();
    println!("Output size: {} bytes", byte_size);
}

fn ensure_bto_extension(path: &str) -> PathBuf {
    let mut path = PathBuf::from(path);
    path.set_extension("bto");
    path
}
