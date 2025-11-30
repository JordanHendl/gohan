use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;
use std::fs;
use std::path::PathBuf;

#[test]
fn compiles_shader_via_cli() {
    let tmp_dir = tempfile::tempdir().unwrap();
    let requested_output = tmp_dir.path().join("simple_compute");
    let actual_output = requested_output.with_extension("bto");

    cargo_bin_cmd!("bentosc")
        .args([
            "tests/fixtures/simple_compute.glsl",
            "--stage",
            "compute",
            "--lang",
            "glsl",
            "--opt",
            "performance",
            "--output",
            requested_output.to_str().unwrap(),
            "--name",
            "simple_compute",
            "--verbose",
        ])
        .assert()
        .success();

    assert!(actual_output.exists());

    let result = bento::CompilationResult::load_from_disk(actual_output.to_str().unwrap()).unwrap();
    assert_eq!(result.stage, dashi::ShaderType::Compute);
    assert_eq!(result.lang, bento::ShaderLang::Glsl);
    assert!(result.variables.len() > 0);
    assert!(!result.spirv.is_empty());

    fs::remove_file(actual_output).ok();
}

#[test]
fn compiles_hlsl_shader_via_cli() {
    let tmp_dir = tempfile::tempdir().unwrap();
    let output = tmp_dir.path().join("simple_compute_hlsl.bto");

    cargo_bin_cmd!("bentosc")
        .args([
            "tests/fixtures/simple_compute.hlsl",
            "--stage",
            "compute",
            "--lang",
            "hlsl",
            "--opt",
            "performance",
            "--output",
            output.to_str().unwrap(),
            "--name",
            "simple_compute_hlsl",
            "--verbose",
        ])
        .assert()
        .success();

    assert!(output.exists());

    let result = bento::CompilationResult::load_from_disk(output.to_str().unwrap()).unwrap();
    assert_eq!(result.stage, dashi::ShaderType::Compute);
    assert_eq!(result.lang, bento::ShaderLang::Hlsl);
    assert!(result.variables.len() > 0);
    assert!(!result.spirv.is_empty());
}

#[test]
fn compiles_slang_shader_via_cli() {
    let tmp_dir = tempfile::tempdir().unwrap();
    let output = tmp_dir.path().join("simple_compute_slang.bto");

    cargo_bin_cmd!("bentosc")
        .args([
            "tests/fixtures/simple_compute.slang",
            "--stage",
            "compute",
            "--lang",
            "slang",
            "--opt",
            "performance",
            "--output",
            output.to_str().unwrap(),
            "--name",
            "simple_compute_slang",
            "--verbose",
        ])
        .assert()
        .success();

    assert!(output.exists());

    let result = bento::CompilationResult::load_from_disk(output.to_str().unwrap()).unwrap();
    assert_eq!(result.stage, dashi::ShaderType::Compute);
    assert_eq!(result.lang, bento::ShaderLang::Slang);
    assert!(result.variables.len() > 0);
    assert!(!result.spirv.is_empty());
}

#[test]
fn fails_gracefully_for_missing_shader() {
    cargo_bin_cmd!("bentosc")
        .args([
            "tests/fixtures/does_not_exist.glsl",
            "--stage",
            "compute",
            "--lang",
            "glsl",
            "--output",
            "target/should_not_exist.bto",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("does_not_exist.glsl"));
}

#[test]
fn defaults_output_to_out_bto() {
    let tmp_dir = tempfile::tempdir().unwrap();
    let shader = PathBuf::from("tests/fixtures/simple_compute.glsl")
        .canonicalize()
        .unwrap();
    let expected_output = tmp_dir.path().join("out.bto");

    cargo_bin_cmd!("bentosc")
        .current_dir(tmp_dir.path())
        .args([
            shader.to_str().unwrap(),
            "--stage",
            "compute",
            "--lang",
            "glsl",
        ])
        .assert()
        .success();

    assert!(expected_output.exists());
}

#[test]
fn coerces_output_extension_to_bto() {
    let tmp_dir = tempfile::tempdir().unwrap();
    let shader = PathBuf::from("tests/fixtures/simple_compute.glsl");
    let requested_output = tmp_dir.path().join("custom_output.bin");
    let expected_output = requested_output.with_extension("bto");

    cargo_bin_cmd!("bentosc")
        .args([
            shader.to_str().unwrap(),
            "--stage",
            "compute",
            "--lang",
            "glsl",
            "--output",
            requested_output.to_str().unwrap(),
        ])
        .assert()
        .success();

    assert!(expected_output.exists());
}

#[test]
fn inspects_saved_artifact() {
    let tmp_dir = tempfile::tempdir().unwrap();
    let path = tmp_dir.path().join("artifact.bin");

    let artifact = bento::CompilationResult {
        name: Some("example".to_string()),
        file: Some("shader.glsl".to_string()),
        lang: bento::ShaderLang::Glsl,
        stage: dashi::ShaderType::Compute,
        variables: vec![bento::ShaderVariable {
            name: "u_time".to_string(),
            set: 0,
            kind: dashi::BindGroupVariable {
                var_type: dashi::BindGroupVariableType::Uniform,
                binding: 0,
                count: 1,
            },
        }],
        metadata: bento::ShaderMetadata {
            entry_points: vec!["main".to_string()],
            inputs: vec![],
            outputs: vec![],
            workgroup_size: Some([1, 1, 1]),
            vertex: None,
        },
        spirv: vec![0x0723_0203, 1, 2],
    };

    artifact
        .save_to_disk(path.to_str().unwrap())
        .expect("failed to save artifact");

    cargo_bin_cmd!("bentoinspect")
        .arg(path.to_str().unwrap())
        .assert()
        .success()
        .stdout(predicate::str::contains("example"))
        .stdout(predicate::str::contains("Compute"))
        .stdout(predicate::str::contains("set 0, binding 0"))
        .stdout(predicate::str::contains("Output size: 12 bytes"));
}

#[test]
fn outputs_json_when_requested() {
    let tmp_dir = tempfile::tempdir().unwrap();
    let path = tmp_dir.path().join("artifact.bin");

    let artifact = bento::CompilationResult {
        name: Some("json_example".to_string()),
        file: None,
        lang: bento::ShaderLang::Hlsl,
        stage: dashi::ShaderType::Fragment,
        variables: vec![],
        metadata: bento::ShaderMetadata {
            entry_points: vec!["main".to_string()],
            inputs: vec![],
            outputs: vec![],
            workgroup_size: None,
            vertex: None,
        },
        spirv: vec![1, 2, 3, 4],
    };

    artifact
        .save_to_disk(path.to_str().unwrap())
        .expect("failed to save artifact");

    let output = cargo_bin_cmd!("bentoinspect")
        .args([path.to_str().unwrap(), "--json"])
        .output()
        .expect("failed to run bentoinspect");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout is not UTF-8");
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("stdout is not valid JSON");

    assert_eq!(value["name"], "json_example");
    assert_eq!(value["file"], serde_json::Value::Null);
    assert_eq!(value["lang"], "Hlsl");
    assert_eq!(value["stage"], "Fragment");
    assert_eq!(value["spirv"], serde_json::json!([1, 2, 3, 4]));
}
