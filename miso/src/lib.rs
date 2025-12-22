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
    let mut resolved = String::new();

    for line in source.lines() {
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
        name: None,
        lang: ShaderLang::Slang,
        stage: dashi::ShaderType::Vertex,
        optimization: OptimizationLevel::Performance,
        debug_symbols: false,
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

#[cfg(test)]
mod tests {
    use super::*;

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

        let vertex_sets: Vec<(u32, dashi::BindGroupVariableType)> = vertex
            .variables
            .iter()
            .map(|v| (v.set, v.kind.var_type))
            .collect();
        assert_eq!(vertex_sets.len(), 3);
        assert!(vertex_sets.contains(&(1, dashi::BindGroupVariableType::Storage)));
        assert!(vertex_sets.contains(&(2, dashi::BindGroupVariableType::Storage)));

        let fragment_sets: Vec<(u32, dashi::BindGroupVariableType)> = fragment
            .variables
            .iter()
            .map(|v| (v.set, v.kind.var_type))
            .collect();
        assert_eq!(fragment_sets.len(), 4);
        assert!(fragment_sets.contains(&(1, dashi::BindGroupVariableType::SampledImage)));
        assert!(fragment_sets.contains(&(1, dashi::BindGroupVariableType::Storage)));
        assert!(fragment_sets.contains(&(2, dashi::BindGroupVariableType::Storage)));
        assert!(fragment_sets.contains(&(1, dashi::BindGroupVariableType::Uniform)));
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

        assert_eq!(layout.stride, 64);
        assert!(matches!(layout.rate, dashi::VertexRate::Vertex));
        assert_eq!(locations.len(), 5);
        assert_eq!(locations[0], (0, &dashi::ShaderPrimitiveType::Vec3));
        assert_eq!(locations[1], (1, &dashi::ShaderPrimitiveType::Vec3));
        assert_eq!(locations[2], (2, &dashi::ShaderPrimitiveType::Vec4));
        assert_eq!(locations[3], (3, &dashi::ShaderPrimitiveType::Vec2));
        assert_eq!(locations[4], (4, &dashi::ShaderPrimitiveType::Vec4));

        assert_eq!(offsets, vec![0, 12, 24, 40, 48]);
    }
}
