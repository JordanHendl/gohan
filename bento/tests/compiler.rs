use bento::{BentoError, Compiler, OptimizationLevel, Request, ShaderLang};
use std::collections::HashMap;

fn spirv_words_to_bytes(words: &[u32]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(words.as_ptr() as *const u8, words.len() * 4) }
}

fn binding_names_from_spirv(spirv: &[u32]) -> Vec<(u32, String)> {
    let reflection = rspirv_reflect::Reflection::new_from_spirv(spirv_words_to_bytes(spirv))
        .expect("failed to reflect SPIR-V");

    reflection
        .get_descriptor_sets()
        .expect("unable to read descriptor sets")
        .get(&0)
        .map(|set| {
            set.iter()
                .map(|(binding, info)| (*binding, info.name.clone()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn sample_request(lang: ShaderLang) -> Request {
    Request {
        name: Some("sample".to_string()),
        lang,
        stage: dashi::ShaderType::Compute,
        optimization: OptimizationLevel::None,
        debug_symbols: false,
        defines: HashMap::new(),
    }
}

#[test]
fn compiles_fixture_shader() -> Result<(), BentoError> {
    let compiler = Compiler::new()?;
    let request = sample_request(ShaderLang::Glsl);
    let path = "tests/fixtures/simple_compute.glsl";

    let result = compiler.compile_from_file(path, &request)?;

    assert_eq!(result.file.as_deref(), Some(path));
    assert_eq!(result.stage, dashi::ShaderType::Compute);
    assert_eq!(result.lang, ShaderLang::Glsl);
    assert!(!result.spirv.is_empty());
    assert!(!result.variables.is_empty());
    assert!(result.metadata.entry_points.contains(&"main".to_string()));
    assert_eq!(result.metadata.workgroup_size, Some([1, 1, 1]));

    Ok(())
}

#[test]
fn returns_missing_file_error() {
    let compiler = Compiler::new().unwrap();
    let request = sample_request(ShaderLang::Glsl);
    let missing_path = "tests/fixtures/not_real_shader.glsl";

    let err = compiler
        .compile_from_file(missing_path, &request)
        .unwrap_err();

    match err {
        BentoError::Io(io_err) => {
            assert!(io_err.to_string().contains(missing_path));
        }
        other => panic!("Unexpected error: {:?}", other),
    }
}

#[test]
fn fails_with_invalid_shader_source() {
    let compiler = Compiler::new().unwrap();
    let request = sample_request(ShaderLang::Glsl);

    let err = compiler
        .compile(b"#version 450\nvoid main() {", &request)
        .unwrap_err();

    assert!(matches!(err, BentoError::ShaderCompilation(_)));
}

#[test]
fn compiles_hlsl_shader() -> Result<(), BentoError> {
    let compiler = Compiler::new()?;
    let request = sample_request(ShaderLang::Hlsl);
    let path = "tests/fixtures/simple_compute.hlsl";

    let result = compiler.compile_from_file(path, &request)?;

    assert_eq!(result.file.as_deref(), Some(path));
    assert_eq!(result.stage, dashi::ShaderType::Compute);
    assert_eq!(result.lang, ShaderLang::Hlsl);
    assert!(!result.spirv.is_empty());
    assert!(!result.variables.is_empty());

    Ok(())
}

#[test]
fn compiles_slang_shader() -> Result<(), BentoError> {
    let compiler = Compiler::new()?;
    let request = sample_request(ShaderLang::Slang);
    let path = "tests/fixtures/simple_compute.slang";

    let result = compiler.compile_from_file(path, &request)?;

    assert_eq!(result.file.as_deref(), Some(path));
    assert_eq!(result.stage, dashi::ShaderType::Compute);
    assert_eq!(result.lang, ShaderLang::Slang);
    assert!(!result.spirv.is_empty());
    assert!(!result.variables.is_empty());

    Ok(())
}

#[test]
fn infers_glsl_shader_language() -> Result<(), BentoError> {
    let compiler = Compiler::new()?;
    let request = sample_request(ShaderLang::Infer);
    let path = "tests/fixtures/simple_compute.glsl";

    let result = compiler.compile_from_file(path, &request)?;

    assert_eq!(result.lang, ShaderLang::Glsl);

    Ok(())
}

#[test]
fn infers_hlsl_shader_language() -> Result<(), BentoError> {
    let compiler = Compiler::new()?;
    let request = sample_request(ShaderLang::Infer);
    let path = "tests/fixtures/simple_compute.hlsl";

    let result = compiler.compile_from_file(path, &request)?;

    assert_eq!(result.lang, ShaderLang::Hlsl);

    Ok(())
}

#[test]
fn infers_slang_shader_language() -> Result<(), BentoError> {
    let compiler = Compiler::new()?;
    let request = sample_request(ShaderLang::Infer);
    let path = "tests/fixtures/simple_compute.slang";

    let result = compiler.compile_from_file(path, &request)?;

    assert_eq!(result.lang, ShaderLang::Slang);

    Ok(())
}

#[test]
fn applies_preprocessor_definitions() -> Result<(), BentoError> {
    let compiler = Compiler::new()?;
    let mut request = sample_request(ShaderLang::Glsl);
    request
        .defines
        .insert("WORKGROUP_SIZE".into(), Some("4".into()));
    let path = "tests/fixtures/define_workgroup.glsl";

    let result = compiler.compile_from_file(path, &request)?;

    assert_eq!(result.metadata.workgroup_size, Some([4, 1, 1]));

    Ok(())
}

#[test]
fn hlsl_binding_names_follow_registers() -> Result<(), BentoError> {
    let compiler = Compiler::new()?;
    let request = sample_request(ShaderLang::Hlsl);
    let path = "tests/fixtures/hlsl_binding_map.hlsl";

    let result = compiler.compile_from_file(path, &request)?;

    let bindings: Vec<(u32, String)> = result
        .variables
        .iter()
        .map(|var| (var.kind.binding, var.name.clone()))
        .collect();
    let spirv_bindings = binding_names_from_spirv(&result.spirv);

    assert_eq!(bindings.len(), 4);
    assert_eq!(bindings, spirv_bindings);
    assert_eq!(bindings[0], (0, "colorTex".to_string()));
    assert_eq!(bindings[1], (1, "Params".to_string()));
    assert_eq!(bindings[2], (2, "outputData".to_string()));
    assert_eq!(bindings[3], (3, "linearSampler".to_string()));

    Ok(())
}

#[test]
fn hlsl_binding_names_follow_declaration_order() -> Result<(), BentoError> {
    let compiler = Compiler::new()?;
    let request = sample_request(ShaderLang::Hlsl);
    let path = "tests/fixtures/hlsl_binding_order.hlsl";

    let result = compiler.compile_from_file(path, &request)?;

    let bindings: Vec<(u32, String)> = result
        .variables
        .iter()
        .map(|var| (var.kind.binding, var.name.clone()))
        .collect();
    let spirv_bindings = binding_names_from_spirv(&result.spirv);

    assert_eq!(bindings.len(), 4);
    assert_eq!(bindings, spirv_bindings);
    assert_eq!(bindings[0], (0, "albedo".to_string()));
    assert_eq!(bindings[1], (1, "FrameData".to_string()));
    assert_eq!(bindings[2], (2, "outputData".to_string()));
    assert_eq!(bindings[3], (3, "pointSampler".to_string()));

    Ok(())
}

#[test]
fn slang_binding_names_follow_registers() -> Result<(), BentoError> {
    let compiler = Compiler::new()?;
    let request = sample_request(ShaderLang::Slang);
    let path = "tests/fixtures/slang_binding_map.slang";

    let result = compiler.compile_from_file(path, &request)?;

    let bindings: Vec<(u32, String)> = result
        .variables
        .iter()
        .map(|var| (var.kind.binding, var.name.clone()))
        .collect();
    let spirv_bindings = binding_names_from_spirv(&result.spirv);

    assert_eq!(bindings.len(), 4);
    assert_eq!(bindings, spirv_bindings);
    assert_eq!(bindings[0], (0, "colorTex".to_string()));
    assert_eq!(bindings[1], (1, "Params".to_string()));
    assert_eq!(bindings[2], (2, "outputData".to_string()));
    assert_eq!(bindings[3], (3, "linearSampler".to_string()));

    Ok(())
}

#[test]
fn slang_binding_names_follow_declaration_order() -> Result<(), BentoError> {
    let compiler = Compiler::new()?;
    let request = sample_request(ShaderLang::Slang);
    let path = "tests/fixtures/slang_binding_order.slang";

    let result = compiler.compile_from_file(path, &request)?;

    let bindings: Vec<(u32, String)> = result
        .variables
        .iter()
        .map(|var| (var.kind.binding, var.name.clone()))
        .collect();
    let spirv_bindings = binding_names_from_spirv(&result.spirv);

    assert_eq!(bindings.len(), 4);
    assert_eq!(bindings, spirv_bindings);
    assert_eq!(bindings[0], (0, "albedo".to_string()));
    assert_eq!(bindings[1], (1, "FrameData".to_string()));
    assert_eq!(bindings[2], (2, "outputData".to_string()));
    assert_eq!(bindings[3], (3, "pointSampler".to_string()));

    Ok(())
}
