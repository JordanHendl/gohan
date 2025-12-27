use bento::{CompilationResult, Pipeline, ShaderLang, ShaderMetadata, ShaderVariable};

fn fake_result(stage: dashi::ShaderType, name: &str) -> CompilationResult {
    CompilationResult {
        name: Some(name.to_string()),
        file: None,
        lang: ShaderLang::Glsl,
        stage,
        variables: vec![ShaderVariable {
            name: "var".to_string(),
            set: 0,
            kind: dashi::BindTableVariable {
                var_type: dashi::BindTableVariableType::Uniform,
                binding: 0,
                count: 1,
            },
        }],
        metadata: ShaderMetadata {
            entry_points: vec!["main".to_string()],
            inputs: Vec::new(),
            outputs: Vec::new(),
            workgroup_size: None,
            vertex: None,
        },
        spirv: vec![0x07230203],
    }
}

#[test]
fn builds_compute_pipeline() {
    let compute_stage = fake_result(dashi::ShaderType::Compute, "compute");
    let pipeline = Pipeline::from_stages(vec![compute_stage.clone()]).unwrap();

    assert!(pipeline.compute().is_some());
    assert_eq!(pipeline.compute().unwrap().name.as_deref(), Some("compute"));
    assert_eq!(pipeline.kind(), bento::PipelineKind::Compute);
}

#[test]
fn builds_graphics_pipeline_with_required_fragment() {
    let vertex_stage = fake_result(dashi::ShaderType::Vertex, "vertex");
    let fragment_stage = fake_result(dashi::ShaderType::Fragment, "fragment");

    let graphics = Pipeline::from_stages(vec![vertex_stage, fragment_stage]).unwrap();
    assert_eq!(graphics.vertex().unwrap().name.as_deref(), Some("vertex"));
    assert_eq!(
        graphics.fragment().unwrap().name.as_deref(),
        Some("fragment")
    );
}

#[test]
fn rejects_invalid_stage_combinations() {
    let vertex = fake_result(dashi::ShaderType::Vertex, "vertex");
    let fragment = fake_result(dashi::ShaderType::Fragment, "fragment");
    let compute = fake_result(dashi::ShaderType::Compute, "compute");

    let mixed = Pipeline::from_stages(vec![vertex.clone(), compute.clone()]);
    assert!(mixed.is_err());

    let missing_fragment = Pipeline::from_stages(vec![vertex.clone()]);
    assert!(missing_fragment.is_err());

    let duplicate_fragment = Pipeline::from_stages(vec![fragment.clone(), fragment.clone()]);
    assert!(duplicate_fragment.is_err());

    let fragment_only = Pipeline::from_stages(vec![fragment.clone()]);
    assert!(fragment_only.is_err());

    let duplicate_compute = Pipeline::from_stages(vec![compute.clone(), compute]);
    assert!(duplicate_compute.is_err());

    let no_stages = Pipeline::from_stages(Vec::<CompilationResult>::new());
    assert!(no_stages.is_err());
}
