use crate::{GPUState, ReservedMetadata};

#[cfg(test)]
use dashi::BindGroupVariableType;

#[derive(Default, Debug)]
pub struct ResolveResult {
    pub name: String,
    pub exists: bool,
    pub binding: dashi::BindGroupVariable,
    pub set: u32,
}

#[derive(Debug)]
pub struct Resolver {
    resolved: Vec<ResolveResult>,
}
impl Resolver {
    pub fn new<T: GPUState>(
        _state: &T,
        result: &bento::CompilationResult,
    ) -> Result<Self, crate::error::FurikakeError> {
        let names = T::reserved_metadata();

        Ok(Self {
            resolved: Self::reflect_bindings(names, result)?,
        })
    }

    pub fn resolved(&self) -> &[ResolveResult] {
        self.resolved.as_slice()
    }

    fn reflect_bindings(
        names: &[ReservedMetadata],
        res: &bento::CompilationResult,
    ) -> Result<Vec<ResolveResult>, crate::error::FurikakeError> {
        let mut results = Vec::new();
        for meta in names.iter() {
            if let Some(found) = res.variables.iter().find(|b| b.name == meta.name) {
                if found.kind.var_type != meta.kind {
                    return Err(crate::error::FurikakeError::ResolverReflection {
                        source: format!(
                            "reserved binding `{}` expected {:?} but shader reported {:?}",
                            meta.name, meta.kind, found.kind.var_type
                        ),
                    });
                }

                results.push(ResolveResult {
                    name: found.name.clone(),
                    exists: true,
                    binding: found.kind.clone(),
                    set: found.set,
                });
            } else {
                return Err(crate::error::FurikakeError::MissingReservedBinding {
                    name: meta.name.to_string(),
                });
            }
        }
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::FurikakeError;

    struct TestState;

    impl GPUState for TestState {
        fn reserved_names() -> &'static [&'static str] {
            &["meshi_timing"]
        }

        fn reserved_metadata() -> &'static [ReservedMetadata] {
            &[ReservedMetadata {
                name: "meshi_timing",
                kind: BindGroupVariableType::Uniform,
            }]
        }

        fn binding(
            &self,
            key: &str,
        ) -> Result<&dyn crate::reservations::ReservedItem, crate::error::FurikakeError> {
            Err(crate::error::FurikakeError::MissingReservedBinding {
                name: key.to_string(),
            })
        }
    }

    fn make_result(variables: Vec<bento::ShaderVariable>) -> bento::CompilationResult {
        bento::CompilationResult {
            name: None,
            file: None,
            lang: bento::ShaderLang::Glsl,
            stage: dashi::ShaderType::Vertex,
            variables,
            metadata: bento::ShaderMetadata {
                entry_points: Vec::new(),
                inputs: Vec::new(),
                outputs: Vec::new(),
                workgroup_size: None,
                vertex: Default::default(),
            },
            spirv: Vec::new(),
        }
    }

    #[test]
    fn reports_missing_reserved_binding() {
        let res = make_result(vec![]);
        let err = Resolver::new(&TestState, &res).unwrap_err();

        match err {
            FurikakeError::MissingReservedBinding { name } => {
                assert_eq!(name, "meshi_timing");
            }
            other => panic!("unexpected error {other:?}", other = other),
        }
    }

    #[test]
    fn reports_type_mismatch() {
        let res = make_result(vec![bento::ShaderVariable {
            name: "meshi_timing".to_string(),
            set: 0,
            kind: dashi::BindGroupVariable {
                var_type: BindGroupVariableType::Storage,
                binding: 0,
                count: 1,
            },
        }]);

        let err = Resolver::new(&TestState, &res).unwrap_err();
        match err {
            FurikakeError::ResolverReflection { source } => {
                assert!(source.contains("expected Uniform"));
                assert!(source.contains("Storage"));
            }
            other => panic!("unexpected error {other:?}", other = other),
        }
    }
}
