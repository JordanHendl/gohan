use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use bento::{CompilationResult, Compiler, OptimizationLevel, Request, ShaderLang};

pub(crate) fn build_define_map(defines: &[String]) -> HashMap<String, Option<String>> {
    let mut define_map: HashMap<String, Option<String>> = HashMap::new();
    for define in defines {
        if let Some((name, value)) = define.split_once('=') {
            define_map.insert(name.to_string(), Some(value.to_string()));
        } else {
            define_map.insert(define.to_string(), None);
        }
    }

    define_map
}

fn resolve_imports(
    path: &Path,
    include_dir: &Path,
    visited: &mut HashSet<PathBuf>,
) -> std::io::Result<String> {
    let source = fs::read_to_string(path)?;
    let display_path = path.display().to_string();
    let mut resolved = String::new();

    resolved.push_str(&format!("#line 1 \"{display_path}\"\n"));

    for (line_index, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("import ") {
            let module_name = rest.trim_end_matches(';').trim();
            let module_path = include_dir.join(format!("{module_name}.slang"));

            if visited.insert(module_path.clone()) {
                let inlined = resolve_imports(&module_path, include_dir, visited)?;
                resolved.push_str(&format!("\n// begin include {module_name}\n"));
                resolved.push_str(&inlined);
                resolved.push_str(&format!("\n// end include {module_name}\n"));
            }

            let next_line = line_index + 2;
            resolved.push_str(&format!("#line {next_line} \"{display_path}\"\n"));
            continue;
        }

        resolved.push_str(line);
        resolved.push('\n');
    }

    Ok(resolved)
}

fn resolve_with_includes_impl(source: &str, include: &str) -> String {
    let include_dir = include.trim().trim_start_matches("-I");
    let include_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(include_dir);
    let source_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(source);
    let mut visited = HashSet::new();

    resolve_imports(&source_path, &include_dir, &mut visited)
        .unwrap_or_else(|err| panic!("Failed to resolve includes for {source}: {err}"))
}

macro_rules! resolve_with_includes {
    ($a:expr, $b:expr) => {
        crate::resolve_with_includes_impl($a, $b)
    };
}

pub fn stddeferred(defines: &[String]) -> Vec<CompilationResult> {
    let vshader = resolve_with_includes!("src/slang/src/stdvert.slang", "-Isrc/slang/include/");
    let fshader = resolve_with_includes!("src/slang/src/stdfrag.slang", "-Isrc/slang/include/");
    let define_map = build_define_map(defines);

    let compiler = Compiler::new().expect("Failed to create shader compiler");
    let base_request = Request {
        name: Some("stddeferred".to_string()),
        lang: ShaderLang::Slang,
        stage: dashi::ShaderType::Vertex,
        optimization: OptimizationLevel::Performance,
        debug_symbols: true,
        defines: define_map,
    };

    let vertex = compiler
        .compile(
            vshader.as_bytes(),
            &Request {
                stage: dashi::ShaderType::Vertex,
                ..base_request.clone()
            },
        )
        .expect("Failed to compile std vertex shader");

    let fragment = compiler
        .compile(
            fshader.as_bytes(),
            &Request {
                stage: dashi::ShaderType::Fragment,
                ..base_request
            },
        )
        .expect("Failed to compile std fragment shader");

    vec![vertex, fragment]
}

pub fn gpudeferred(defines: &[String]) -> Vec<CompilationResult> {
    let vshader =
        resolve_with_includes!("src/slang/src/gpudeferred_vert.slang", "-Isrc/slang/include/");
    let fshader =
        resolve_with_includes!("src/slang/src/gpudeferred_frag.slang", "-Isrc/slang/include/");
    let define_map = build_define_map(defines);

    let compiler = Compiler::new().expect("Failed to create shader compiler");
    let base_request = Request {
        name: Some("gpu-deferred".to_string()),
        lang: ShaderLang::Slang,
        stage: dashi::ShaderType::Vertex,
        optimization: OptimizationLevel::Performance,
        debug_symbols: true,
        defines: define_map,
    };

    let vertex = compiler
        .compile(
            vshader.as_bytes(),
            &Request {
                stage: dashi::ShaderType::Vertex,
                ..base_request.clone()
            },
        )
        .expect("Failed to compile gpu deferred vertex shader");

    let fragment = compiler
        .compile(
            fshader.as_bytes(),
            &Request {
                stage: dashi::ShaderType::Fragment,
                ..base_request
            },
        )
        .expect("Failed to compile gpu deferred fragment shader");

    vec![vertex, fragment]
}

pub fn gpuforward(defines: &[String]) -> Vec<CompilationResult> {
    let vshader =
        resolve_with_includes!("src/slang/src/gpuforward_vert.slang", "-Isrc/slang/include/");
    let fshader =
        resolve_with_includes!("src/slang/src/gpuforward_frag.slang", "-Isrc/slang/include/");
    let define_map = build_define_map(defines);

    let compiler = Compiler::new().expect("Failed to create shader compiler");
    let base_request = Request {
        name: Some("gpu-forward".to_string()),
        lang: ShaderLang::Slang,
        stage: dashi::ShaderType::Vertex,
        optimization: OptimizationLevel::Performance,
        debug_symbols: true,
        defines: define_map,
    };

    let vertex = compiler
        .compile(
            vshader.as_bytes(),
            &Request {
                stage: dashi::ShaderType::Vertex,
                ..base_request.clone()
            },
        )
        .expect("Failed to compile gpu forward vertex shader");

    let fragment = compiler
        .compile(
            fshader.as_bytes(),
            &Request {
                stage: dashi::ShaderType::Fragment,
                ..base_request
            },
        )
        .expect("Failed to compile gpu forward fragment shader");

    vec![vertex, fragment]
}

pub fn stdforward(defines: &[String]) -> Vec<CompilationResult> {
    let vshader = resolve_with_includes!(
        "src/slang/src/stdforward_vert.slang",
        "-Isrc/slang/include/"
    );
    let fshader = resolve_with_includes!(
        "src/slang/src/stdforward_frag.slang",
        "-Isrc/slang/include/"
    );
    let define_map = build_define_map(defines);

    let compiler = Compiler::new().expect("Failed to create shader compiler");
    let base_request = Request {
        name: Some("stdforward".to_string()),
        lang: ShaderLang::Slang,
        stage: dashi::ShaderType::Vertex,
        optimization: OptimizationLevel::Performance,
        debug_symbols: true,
        defines: define_map,
    };

    let vertex = compiler
        .compile(
            vshader.as_bytes(),
            &Request {
                stage: dashi::ShaderType::Vertex,
                ..base_request.clone()
            },
        )
        .expect("Failed to compile std forward vertex shader");

    let fragment = compiler
        .compile(
            fshader.as_bytes(),
            &Request {
                stage: dashi::ShaderType::Fragment,
                ..base_request
            },
        )
        .expect("Failed to compile std forward fragment shader");

    vec![vertex, fragment]
}

pub fn stddeferred_combine(defines: &[String]) -> Vec<CompilationResult> {
    let vshader = resolve_with_includes!(
        "src/slang/src/stddeferred_combine_vert.slang",
        "-Isrc/slang/include/"
    );
    let fshader = resolve_with_includes!(
        "src/slang/src/stddeferred_combine_frag.slang",
        "-Isrc/slang/include/"
    );
    let define_map = build_define_map(defines);

    let compiler = Compiler::new().expect("Failed to create shader compiler");
    let base_request = Request {
        name: Some("stddeferred_combine".to_string()),
        lang: ShaderLang::Slang,
        stage: dashi::ShaderType::Vertex,
        optimization: OptimizationLevel::Performance,
        debug_symbols: true,
        defines: define_map,
    };

    let vertex = compiler
        .compile(
            vshader.as_bytes(),
            &Request {
                stage: dashi::ShaderType::Vertex,
                ..base_request.clone()
            },
        )
        .expect("Failed to compile std deferred combine vertex shader");

    let fragment = compiler
        .compile(
            fshader.as_bytes(),
            &Request {
                stage: dashi::ShaderType::Fragment,
                ..base_request
            },
        )
        .expect("Failed to compile std deferred combine fragment shader");

    vec![vertex, fragment]
}

pub fn stdsky(defines: &[String]) -> Vec<CompilationResult> {
    let vshader = resolve_with_includes!("src/slang/src/stdsky_vert.slang", "-Isrc/slang/include/");
    let fshader = resolve_with_includes!("src/slang/src/stdsky_frag.slang", "-Isrc/slang/include/");
    let define_map = build_define_map(defines);

    let compiler = Compiler::new().expect("Failed to create shader compiler");
    let base_request = Request {
        name: Some("stdsky".to_string()),
        lang: ShaderLang::Slang,
        stage: dashi::ShaderType::Vertex,
        optimization: OptimizationLevel::Performance,
        debug_symbols: true,
        defines: define_map,
    };

    let vertex = compiler
        .compile(
            vshader.as_bytes(),
            &Request {
                stage: dashi::ShaderType::Vertex,
                ..base_request.clone()
            },
        )
        .expect("Failed to compile std sky vertex shader");

    let fragment = compiler
        .compile(
            fshader.as_bytes(),
            &Request {
                stage: dashi::ShaderType::Fragment,
                ..base_request
            },
        )
        .expect("Failed to compile std sky fragment shader");

    vec![vertex, fragment]
}

pub fn stdocean(defines: &[String]) -> Vec<CompilationResult> {
    let vshader =
        resolve_with_includes!("src/slang/src/stdocean_vert.slang", "-Isrc/slang/include/");
    let fshader =
        resolve_with_includes!("src/slang/src/stdocean_frag.slang", "-Isrc/slang/include/");
    let define_map = build_define_map(defines);

    let compiler = Compiler::new().expect("Failed to create shader compiler");
    let base_request = Request {
        name: Some("stdocean".to_string()),
        lang: ShaderLang::Slang,
        stage: dashi::ShaderType::Vertex,
        optimization: OptimizationLevel::Performance,
        debug_symbols: true,
        defines: define_map,
    };

    let vertex = compiler
        .compile(
            vshader.as_bytes(),
            &Request {
                stage: dashi::ShaderType::Vertex,
                ..base_request.clone()
            },
        )
        .expect("Failed to compile std ocean vertex shader");

    let fragment = compiler
        .compile(
            fshader.as_bytes(),
            &Request {
                stage: dashi::ShaderType::Fragment,
                ..base_request
            },
        )
        .expect("Failed to compile std ocean fragment shader");

    vec![vertex, fragment]
}

pub fn stdbillboard(defines: &[String]) -> Vec<CompilationResult> {
    let vshader = resolve_with_includes!(
        "src/slang/src/stdbillboard_vert.slang",
        "-Isrc/slang/include/"
    );
    let fshader = resolve_with_includes!(
        "src/slang/src/stdbillboard_frag.slang",
        "-Isrc/slang/include/"
    );
    let define_map = build_define_map(defines);

    let compiler = Compiler::new().expect("Failed to create shader compiler");
    let base_request = Request {
        name: Some("stdbillboard".to_string()),
        lang: ShaderLang::Slang,
        stage: dashi::ShaderType::Vertex,
        optimization: OptimizationLevel::Performance,
        debug_symbols: true,
        defines: define_map,
    };

    let vertex = compiler
        .compile(
            vshader.as_bytes(),
            &Request {
                stage: dashi::ShaderType::Vertex,
                ..base_request.clone()
            },
        )
        .expect("Failed to compile std billboard vertex shader");

    let fragment = compiler
        .compile(
            fshader.as_bytes(),
            &Request {
                stage: dashi::ShaderType::Fragment,
                ..base_request
            },
        )
        .expect("Failed to compile std billboard fragment shader");

    vec![vertex, fragment]
}

pub fn stdparticle(defines: &[String]) -> Vec<CompilationResult> {
    let vshader = resolve_with_includes!(
        "src/slang/src/stdparticle_vert.slang",
        "-Isrc/slang/include/"
    );
    let fshader = resolve_with_includes!(
        "src/slang/src/stdparticle_frag.slang",
        "-Isrc/slang/include/"
    );
    let define_map = build_define_map(defines);

    let compiler = Compiler::new().expect("Failed to create shader compiler");
    let base_request = Request {
        name: Some("stdparticle".to_string()),
        lang: ShaderLang::Slang,
        stage: dashi::ShaderType::Vertex,
        optimization: OptimizationLevel::Performance,
        debug_symbols: true,
        defines: define_map,
    };

    let vertex = compiler
        .compile(
            vshader.as_bytes(),
            &Request {
                stage: dashi::ShaderType::Vertex,
                ..base_request.clone()
            },
        )
        .expect("Failed to compile std particle vertex shader");

    let fragment = compiler
        .compile(
            fshader.as_bytes(),
            &Request {
                stage: dashi::ShaderType::Fragment,
                ..base_request
            },
        )
        .expect("Failed to compile std particle fragment shader");

    vec![vertex, fragment]
}

#[cfg(test)]
mod tests {
    use super::*;
    use bento::builder::PSOBuilder;
    use dashi::Context;
    use std::collections::{HashMap, HashSet};

    #[test]
    fn resolve_with_includes_inlines_imports() {
        let resolved =
            resolve_with_includes_impl("src/slang/src/stdvert.slang", "-Isrc/slang/include/");

        assert!(resolved.contains("// begin include bindless"));
        assert!(!resolved.contains("import bindless;"));
    }

    #[test]
    fn build_define_map_parses_values_and_flags() {
        let defines = vec!["FOO=bar".to_string(), "BAZ".to_string()];

        let map = build_define_map(&defines);

        assert_eq!(map.get("FOO"), Some(&Some("bar".to_string())));
        assert_eq!(map.get("BAZ"), Some(&None));
    }

    #[test]
    fn stddeferred_compiles_vertex_and_fragment_shaders() {
        let results = stddeferred(&[]);

        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|r| r.stage == dashi::ShaderType::Vertex));
        assert!(results
            .iter()
            .any(|r| r.stage == dashi::ShaderType::Fragment));
        assert!(results.iter().all(|r| !r.spirv.is_empty()));
    }

    #[test]
    fn gpudeferred_compiles_vertex_and_fragment_shaders() {
        let results = gpudeferred(&[]);

        assert_eq!(results.len(), 2);
        let vertex = results
            .iter()
            .find(|r| r.stage == dashi::ShaderType::Vertex)
            .expect("vertex stage missing");
        assert!(vertex.metadata.inputs.is_empty());
        assert!(vertex.metadata.vertex.is_none());
        assert!(results
            .iter()
            .any(|r| r.stage == dashi::ShaderType::Fragment));
        assert!(results.iter().all(|r| !r.spirv.is_empty()));
    }

    #[test]
    fn stdforward_compiles_vertex_and_fragment_shaders() {
        let results = stdforward(&[]);

        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|r| r.stage == dashi::ShaderType::Vertex));
        assert!(results
            .iter()
            .any(|r| r.stage == dashi::ShaderType::Fragment));
        assert!(results.iter().all(|r| !r.spirv.is_empty()));
    }

    #[test]
    fn stddeferred_combine_compiles_vertex_and_fragment_shaders() {
        let results = stddeferred_combine(&[]);

        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|r| r.stage == dashi::ShaderType::Vertex));
        assert!(results
            .iter()
            .any(|r| r.stage == dashi::ShaderType::Fragment));
        assert!(results.iter().all(|r| !r.spirv.is_empty()));
    }

    #[test]
    fn stdsky_compiles_vertex_and_fragment_shaders() {
        let results = stdsky(&[]);

        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|r| r.stage == dashi::ShaderType::Vertex));
        assert!(results
            .iter()
            .any(|r| r.stage == dashi::ShaderType::Fragment));
        assert!(results.iter().all(|r| !r.spirv.is_empty()));
    }

    #[test]
    fn stdocean_compiles_vertex_and_fragment_shaders() {
        let results = stdocean(&[]);

        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|r| r.stage == dashi::ShaderType::Vertex));
        assert!(results
            .iter()
            .any(|r| r.stage == dashi::ShaderType::Fragment));
        assert!(results.iter().all(|r| !r.spirv.is_empty()));
    }

    #[test]
    fn stdbillboard_compiles_vertex_and_fragment_shaders() {
        let results = stdbillboard(&[]);

        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|r| r.stage == dashi::ShaderType::Vertex));
        assert!(results
            .iter()
            .any(|r| r.stage == dashi::ShaderType::Fragment));
        assert!(results.iter().all(|r| !r.spirv.is_empty()));
    }

    #[test]
    fn stdparticle_compiles_vertex_and_fragment_shaders() {
        let results = stdparticle(&[]);

        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|r| r.stage == dashi::ShaderType::Vertex));
        assert!(results
            .iter()
            .any(|r| r.stage == dashi::ShaderType::Fragment));
        assert!(results.iter().all(|r| !r.spirv.is_empty()));
    }

    #[test]
    fn stddeferred_bindings_match_bindless_state() {
        let results = stddeferred(&[]);
        let vertex = results
            .iter()
            .find(|r| r.stage == dashi::ShaderType::Vertex)
            .expect("vertex stage missing");
        let fragment = results
            .iter()
            .find(|r| r.stage == dashi::ShaderType::Fragment)
            .expect("fragment stage missing");

        let vertex_sets: Vec<(u32, dashi::BindTableVariableType)> = vertex
            .variables
            .iter()
            .map(|v| (v.set, v.kind.var_type))
            .collect();
        assert_eq!(vertex_sets.len(), 4);
        assert!(vertex_sets.contains(&(0, dashi::BindTableVariableType::Storage)));
        assert!(vertex_sets.contains(&(1, dashi::BindTableVariableType::Storage)));

        let fragment_sets: Vec<(u32, dashi::BindTableVariableType)> = fragment
            .variables
            .iter()
            .map(|v| (v.set, v.kind.var_type))
            .collect();
        assert_eq!(fragment_sets.len(), 4);
        assert!(fragment_sets.contains(&(0, dashi::BindTableVariableType::Image)));
        assert!(fragment_sets.contains(&(0, dashi::BindTableVariableType::Sampler)));
        assert!(fragment_sets.contains(&(0, dashi::BindTableVariableType::Storage)));
        assert!(fragment_sets.contains(&(1, dashi::BindTableVariableType::Storage)));
    }

    #[test]
    fn stddeferred_vertex_layout_matches_noren() {
        let results = stddeferred(&[]);
        let vertex = results
            .iter()
            .find(|r| r.stage == dashi::ShaderType::Vertex)
            .expect("vertex stage missing");

        let layout = vertex
            .metadata
            .vertex
            .as_ref()
            .expect("vertex layout missing");

        let locations: Vec<_> = layout
            .entries
            .iter()
            .map(|e| (e.location, &e.format))
            .collect();
        let offsets: Vec<_> = layout.entries.iter().map(|e| e.offset).collect();

        assert_eq!(layout.stride, 96);
        assert!(matches!(layout.rate, dashi::VertexRate::Vertex));
        assert_eq!(locations.len(), 7);
        assert_eq!(locations[0], (0, &dashi::ShaderPrimitiveType::Vec3));
        assert_eq!(locations[1], (1, &dashi::ShaderPrimitiveType::Vec3));
        assert_eq!(locations[2], (2, &dashi::ShaderPrimitiveType::Vec4));
        assert_eq!(locations[3], (3, &dashi::ShaderPrimitiveType::Vec2));
        assert_eq!(locations[4], (4, &dashi::ShaderPrimitiveType::Vec4));

        assert_eq!(offsets, vec![0, 12, 24, 40, 48, 64, 80]);
    }

    fn expected_binding_count(var: &dashi::BindTableVariable) -> u32 {
        if var.count == 0 { 256 } else { var.count }
    }

    #[test]
    fn stddeferred_builds_graphics_pso_with_tables_per_set() {
        let results = stddeferred(&[]);
        let mut vertex = None;
        let mut fragment = None;
        for result in results {
            match result.stage {
                dashi::ShaderType::Vertex => vertex = Some(result),
                dashi::ShaderType::Fragment => fragment = Some(result),
                _ => {}
            }
        }

        let vertex = vertex.expect("vertex stage missing");
        let fragment = fragment.expect("fragment stage missing");

        let mut table_sizes = HashMap::new();
        let mut used_sets = HashSet::new();
        for var in vertex.variables.iter().chain(fragment.variables.iter()) {
            let expected_count = expected_binding_count(&var.kind);
            if let Some(existing) = table_sizes.insert(var.name.clone(), expected_count) {
                assert_eq!(
                    existing, expected_count,
                    "binding counts for {} differ between stages",
                    var.name
                );
            }
            used_sets.insert(var.set);
        }

        let mut ctx = Context::headless(&dashi::ContextInfo::default()).expect("headless context");
        let mut builder = PSOBuilder::new()
            .vertex_compiled(Some(vertex))
            .fragment_compiled(Some(fragment));
        for (name, size) in table_sizes {
            assert!(!name.is_empty());
            builder = builder.add_table_variable(&name, size);
        }

        let pso = builder.build(&mut ctx);
        assert!(pso.is_ok(), "pipeline build failed: {pso:?}");
        let pso = pso.expect("pso");
        let tables = pso.tables();
        for set in 0..4u32 {
            if used_sets.contains(&set) {
                assert!(
                    tables[set as usize].is_some(),
                    "expected bind table for set {}",
                    set
                );
            } else {
                assert!(
                    tables[set as usize].is_none(),
                    "unexpected bind table for unused set {}",
                    set
                );
            }
        }

        ctx.destroy();
    }
}
