use bento::{BentoError, Compiler, OptimizationLevel, Request, ShaderLang};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

fn stage_from_extension(path: &Path) -> Option<dashi::ShaderType> {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("vert") => Some(dashi::ShaderType::Vertex),
        Some("frag") => Some(dashi::ShaderType::Fragment),
        _ => None,
    }
}

#[test]
fn compiles_repository_shaders() -> Result<(), BentoError> {
    let compiler = Compiler::new()?;
    let shader_dir = Path::new("shaders");

    for entry in fs::read_dir(shader_dir)? {
        let path = entry?.path();
        let Some(stage) = stage_from_extension(&path) else {
            continue;
        };

        let request = Request {
            name: path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .map(|stem| stem.to_string()),
            lang: ShaderLang::Glsl,
            stage,
            optimization: OptimizationLevel::Performance,
            debug_symbols: false,
            defines: HashMap::new(),
        };

        let shader_path = path.to_str().expect("Shader path should be valid UTF-8");
        let result = compiler.compile_from_file(shader_path, &request)?;

        assert_eq!(result.stage, stage, "Stage mismatch for {shader_path}");
        assert_eq!(result.lang, ShaderLang::Glsl);
        assert!(
            !result.spirv.is_empty(),
            "SPIR-V output should not be empty for {shader_path}"
        );
    }

    Ok(())
}
