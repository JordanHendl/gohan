use bento::CompilationResult;
use dashi::builders::BindTableLayoutBuilder;
use dashi::{
    BindTableVariable, BindTable, BindTableInfo, BindTableLayout, Context, Handle,
    IndexedBindingInfo, IndexedResource, ShaderInfo,
};
use std::collections::HashMap;

use crate::{GPUState, error::FurikakeError, reservations::ReservedBinding, resolver::Resolver};

#[derive(Debug, Clone)]
pub struct IndexedBindingRecipe {
    pub bindings: Option<Vec<IndexedResource>>,
    pub var: bento::ShaderVariable,
}

#[derive(Debug, Clone)]
pub struct BindTableRecipe {
    pub bindings: Vec<IndexedBindingRecipe>,
    pub layout: Handle<BindTableLayout>,
}

pub struct RecipeBook {
    recipes: Vec<BindTableRecipe>,
}

impl BindTableRecipe {
    pub fn cook(&mut self, ctx: &mut Context) -> Result<Handle<BindTable>, FurikakeError> {
        let mut owned_resources: Vec<Vec<IndexedResource>> =
            Vec::with_capacity(self.bindings.len());

        for recipe in &mut self.bindings {
            owned_resources.push(recipe.bindings.take().ok_or_else(|| {
                FurikakeError::MissingReservedBinding {
                    name: recipe.var.name.clone(),
                }
            })?);
        }

        let mut bindings: Vec<IndexedBindingInfo> = Vec::with_capacity(self.bindings.len());
        for (recipe, resources) in self.bindings.iter().zip(owned_resources.iter()) {
            bindings.push(IndexedBindingInfo {
                resources: resources.as_slice(),
                binding: recipe.var.kind.binding,
            });
        }

        let set = self.bindings.first().map(|b| b.var.set).unwrap_or_default();

        ctx.make_bind_table(&BindTableInfo {
            debug_name: "[FURIKAKE] Bind Table",
            layout: self.layout,
            bindings: &bindings,
            set,
        })
        .map_err(FurikakeError::from)
    }
}
impl RecipeBook {
    pub fn new<T: GPUState>(
        ctx: &mut Context,
        state: &T,
        shaders: &[CompilationResult],
    ) -> Result<Self, FurikakeError> {
        let mut table_layout_vars: HashMap<u32, Vec<(dashi::ShaderType, Vec<BindTableVariable>)>> =
            HashMap::new();
        let mut table_recipes: HashMap<u32, HashMap<String, IndexedBindingRecipe>> = HashMap::new();

        for shader in shaders {
            Resolver::new(state, shader)?;

            for var in &shader.variables {
                let reserved = state.binding(&var.name)?.binding();
                let shader_vars = table_layout_vars.entry(var.set).or_default();

                if let Some((_stage, vars)) = shader_vars
                    .iter_mut()
                    .find(|(stage, _)| *stage == shader.stage)
                {
                    vars.push(var.kind.clone());
                } else {
                    shader_vars.push((shader.stage, vec![var.kind.clone()]));
                }

                let ReservedBinding::TableBinding { resources, .. } = reserved;

                table_recipes
                    .entry(var.set)
                    .or_default()
                    .entry(var.name.clone())
                    .or_insert_with(|| IndexedBindingRecipe {
                        bindings: Some(resources),
                        var: var.clone(),
                    });
            }
        }

        let mut recipes: Vec<BindTableRecipe> = Vec::new();

        let mut bt_sets: Vec<u32> = table_recipes.keys().copied().collect();
        bt_sets.sort_unstable();
        for set in bt_sets {
            let mut shader_vars = table_layout_vars.remove(&set).unwrap_or_default();
            let shader_info: Vec<ShaderInfo<'_>> = shader_vars
                .iter_mut()
                .map(|(stage, vars)| ShaderInfo {
                    shader_type: *stage,
                    variables: vars.as_slice(),
                })
                .collect();

            let mut builder = BindTableLayoutBuilder::new("[FURIKAKE] Recipe BTL");
            for info in shader_info {
                builder = builder.shader(info);
            }

            let layout = builder.build(ctx).map_err(FurikakeError::from)?;

            let mut bindings: Vec<IndexedBindingRecipe> = table_recipes
                .remove(&set)
                .into_iter()
                .flat_map(|m| m.into_values())
                .collect();
            bindings.sort_by_key(|b| b.var.kind.binding);

            recipes.push(BindTableRecipe { bindings, layout });
        }

        Ok(Self {
            recipes: recipes.into_iter().collect(),
        })
    }

    pub fn recipes(&self) -> Vec<BindTableRecipe> {
        self.recipes.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reservations::{ReservedBinding, ReservedItem};
    use crate::{DefaultState, GPUState, ReservedMetadata};
    use dashi::cmd::Executable;
    use dashi::{
        BindTableVariableType, BufferInfo, BufferView, CommandStream, ContextInfo, MemoryVisibility, ShaderResource, ShaderType
    };

    fn make_shader_variable(
        name: &str,
        set: u32,
        var_type: BindTableVariableType,
        binding: u32,
    ) -> bento::ShaderVariable {
        bento::ShaderVariable {
            name: name.to_string(),
            set,
            kind: dashi::BindTableVariable {
                var_type,
                binding,
                count: 1,
            },
        }
    }

    fn empty_metadata() -> bento::ShaderMetadata {
        bento::ShaderMetadata {
            entry_points: Vec::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            workgroup_size: None,
            vertex: Default::default(),
        }
    }

    #[test]
    fn creates_bind_table_recipes_and_cooks() {
        let mut ctx = Context::headless(&ContextInfo::default()).expect("create context");
        let state = DefaultState::new(&mut ctx);

        let shader = CompilationResult {
            name: None,
            file: None,
            lang: bento::ShaderLang::Glsl,
            stage: ShaderType::Vertex,
            variables: vec![make_shader_variable(
                "meshi_timing",
                0,
                BindTableVariableType::Uniform,
                0,
            )],
            metadata: empty_metadata(),
            spirv: Vec::new(),
        };

        let book = RecipeBook::new(&mut ctx, &state, &[shader]).expect("build recipes");
        let mut recipes = book.recipes();

        assert_eq!(recipes.len(), 1);

        let mut recipe = recipes.pop().unwrap();
        let handle = recipe.cook(&mut ctx).expect("cook bind table");
        assert!(handle.valid());
    }

    struct BindlessItem {
        resources: Vec<IndexedResource>,
    }

    impl BindlessItem {
        fn new(ctx: &mut Context, binding: u32) -> Self {
            let buffer = ctx
                .make_buffer(&BufferInfo {
                    debug_name: "[FURIKAKE] Test Buffer",
                    byte_size: 64,
                    visibility: MemoryVisibility::CpuAndGpu,
                    ..Default::default()
                })
                .expect("make buffer");

            Self {
                resources: vec![IndexedResource {
                    resource: ShaderResource::StorageBuffer(BufferView::new(buffer)),
                    slot: binding,
                }],
            }
        }
    }

    impl ReservedItem for BindlessItem {
        fn name(&self) -> String {
            "bindless_test".to_string()
        }

        fn update(&mut self) -> Result<CommandStream<Executable>, FurikakeError> {
            Ok(CommandStream::new().begin().end())
        }

        fn binding(&self) -> ReservedBinding {
            ReservedBinding::TableBinding {
                binding: 0,
                resources: self.resources.clone(),
            }
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }

    struct BindlessState {
        item: BindlessItem,
    }

    impl BindlessState {
        fn new(ctx: &mut Context) -> Self {
            Self {
                item: BindlessItem::new(ctx, 0),
            }
        }
    }

    impl GPUState for BindlessState {
        fn reserved_names() -> &'static [&'static str] {
            &["bindless_test"]
        }

        fn reserved_metadata() -> &'static [ReservedMetadata] {
            &[ReservedMetadata {
                name: "bindless_test",
                kind: BindTableVariableType::Storage,
            }]
        }

        fn binding(&self, _key: &str) -> Result<&dyn ReservedItem, FurikakeError> {
            Ok(&self.item)
        }
    }

    #[test]
    fn creates_bind_table_recipes_and_cooks_bindless() {
        let mut ctx = Context::headless(&ContextInfo::default()).expect("create context");
        let state = BindlessState::new(&mut ctx);

        let shader = CompilationResult {
            name: None,
            file: None,
            lang: bento::ShaderLang::Glsl,
            stage: ShaderType::Compute,
            variables: vec![make_shader_variable(
                "bindless_test",
                1,
                BindTableVariableType::Storage,
                0,
            )],
            metadata: empty_metadata(),
            spirv: Vec::new(),
        };

        let book = RecipeBook::new(&mut ctx, &state, &[shader]).expect("build recipes");
        let mut recipes = book.recipes();

        assert_eq!(recipes.len(), 1);

        let mut recipe = recipes.pop().unwrap();
        let handle = recipe.cook(&mut ctx).expect("cook bind table");
        assert!(handle.valid());
    }
}
