use std::{
    collections::{HashMap, HashSet, hash_map::Entry},
    ptr::NonNull,
};

use dashi::{
    BindTable, BindTableInfo, BindTableLayout, BindTableLayoutInfo, BindTableVariableType,
    BufferInfo, BufferUsage, BufferView, ComputePipeline, ComputePipelineInfo,
    ComputePipelineLayout, ComputePipelineLayoutInfo, Context, Format, GraphicsPipeline,
    GraphicsPipelineDetails, GraphicsPipelineInfo, GraphicsPipelineLayout,
    GraphicsPipelineLayoutInfo, Handle, ImageInfo, ImageView, IndexedBindingInfo, IndexedResource,
    MemoryVisibility, PipelineShaderInfo, SampleCount, SamplerInfo, ShaderInfo,
    ShaderPrimitiveType, ShaderResource, ShaderType, VertexDescriptionInfo, VertexEntryInfo,
};

pub use crate::PipelineBuildError;
use crate::{
    BentoError, CompilationResult, Compiler, MissingBinding, OptimizationLevel, Request, ShaderLang,
};

fn merge_stage_flags(lhs: dashi::ShaderType, rhs: dashi::ShaderType) -> dashi::ShaderType {
    if lhs == rhs {
        lhs
    } else {
        dashi::ShaderType::All
    }
}

fn resource_var_type(resource: &ShaderResource) -> BindTableVariableType {
    match resource {
        ShaderResource::Buffer(_) | ShaderResource::ConstBuffer(_) => {
            BindTableVariableType::Uniform
        }
        ShaderResource::Dynamic(_) => BindTableVariableType::DynamicUniform,
        ShaderResource::DynamicStorage(_) => BindTableVariableType::DynamicStorage,
        ShaderResource::StorageBuffer(_) => BindTableVariableType::Storage,
        ShaderResource::SampledImage(_, _) => BindTableVariableType::SampledImage,
        ShaderResource::Image(_) => BindTableVariableType::Image,
        ShaderResource::Sampler(_) => BindTableVariableType::Sampler,
    }
}

fn config_dynamic_type(config: &BindTableVariable) -> Option<BindTableVariableType> {
    let dynamic_type_from_resource = |resource: &ShaderResource| match resource_var_type(resource) {
        BindTableVariableType::DynamicUniform => Some(BindTableVariableType::DynamicUniform),
        BindTableVariableType::DynamicStorage => Some(BindTableVariableType::DynamicStorage),
        _ => None,
    };

    match config {
        BindTableVariable::Binding { resource } => dynamic_type_from_resource(resource),
        BindTableVariable::WithResources { resources } => resources
            .iter()
            .find_map(|resource| dynamic_type_from_resource(&resource.resource)),
        BindTableVariable::Empty { .. } => None,
    }
}

fn promoted_var_type(
    var_type: BindTableVariableType,
    config: Option<&BindTableVariable>,
) -> BindTableVariableType {
    let Some(dynamic_type) = config.and_then(config_dynamic_type) else {
        return var_type;
    };

    match (var_type, dynamic_type) {
        (BindTableVariableType::Uniform, BindTableVariableType::DynamicUniform) => {
            BindTableVariableType::DynamicUniform
        }
        (BindTableVariableType::Storage, BindTableVariableType::DynamicStorage) => {
            BindTableVariableType::DynamicStorage
        }
        _ => var_type,
    }
}

fn merge_variable_type(
    existing: BindTableVariableType,
    incoming: BindTableVariableType,
) -> BindTableVariableType {
    match (existing, incoming) {
        (BindTableVariableType::Uniform, BindTableVariableType::DynamicUniform)
        | (BindTableVariableType::DynamicUniform, BindTableVariableType::Uniform)
        | (BindTableVariableType::DynamicUniform, BindTableVariableType::DynamicUniform) => {
            BindTableVariableType::DynamicUniform
        }
        (BindTableVariableType::Storage, BindTableVariableType::DynamicStorage)
        | (BindTableVariableType::DynamicStorage, BindTableVariableType::Storage)
        | (BindTableVariableType::DynamicStorage, BindTableVariableType::DynamicStorage) => {
            BindTableVariableType::DynamicStorage
        }
        _ => existing,
    }
}

struct DefaultResources {
    uniform: Option<ShaderResource>,
    storage: Option<ShaderResource>,
    sampled_image: Option<ShaderResource>,
    image: Option<ShaderResource>,
    sampler: Option<ShaderResource>,
    storage_image: Option<ShaderResource>,
}

impl Default for DefaultResources {
    fn default() -> Self {
        Self {
            uniform: None,
            storage: None,
            sampled_image: None,
            image: None,
            sampler: None,
            storage_image: None,
        }
    }
}

impl DefaultResources {
    fn make_uniform(
        ctx: &mut dashi::Context,
        name: &str,
    ) -> Result<ShaderResource, PipelineBuildError> {
        let buffer = ctx
            .make_buffer(&BufferInfo {
                debug_name: "bento_default_uniform",
                byte_size: 256,
                visibility: MemoryVisibility::CpuAndGpu,
                usage: BufferUsage::UNIFORM,
                initial_data: None,
            })
            .map_err(|source| PipelineBuildError::DefaultResourceCreateFailed {
                name: name.to_string(),
                resource_type: "uniform buffer",
                source,
            })?;

        Ok(ShaderResource::Buffer(BufferView::new(buffer)))
    }

    fn make_storage(
        ctx: &mut dashi::Context,
        name: &str,
    ) -> Result<ShaderResource, PipelineBuildError> {
        let buffer = ctx
            .make_buffer(&BufferInfo {
                debug_name: "bento_default_storage",
                byte_size: 256,
                visibility: MemoryVisibility::CpuAndGpu,
                usage: BufferUsage::STORAGE,
                initial_data: None,
            })
            .map_err(|source| PipelineBuildError::DefaultResourceCreateFailed {
                name: name.to_string(),
                resource_type: "storage buffer",
                source,
            })?;

        Ok(ShaderResource::StorageBuffer(BufferView::new(buffer)))
    }

    fn make_sampled_image(
        ctx: &mut dashi::Context,
        name: &str,
    ) -> Result<ShaderResource, PipelineBuildError> {
        const BLACK_PIXEL: [u8; 4] = [0, 0, 0, 0];

        let image = ctx
            .make_image(&ImageInfo {
                debug_name: "bento_default_image",
                dim: [1, 1, 1],
                layers: 1,
                format: Format::RGBA8,
                mip_levels: 1,
                samples: SampleCount::S1,
                initial_data: Some(&BLACK_PIXEL),
            })
            .map_err(|source| PipelineBuildError::DefaultResourceCreateFailed {
                name: name.to_string(),
                resource_type: "sampled image",
                source,
            })?;

        let sampler = ctx
            .make_sampler(&SamplerInfo::default())
            .map_err(|source| PipelineBuildError::DefaultResourceCreateFailed {
                name: name.to_string(),
                resource_type: "sampler",
                source,
            })?;
        let view = ImageView {
            img: image,
            ..Default::default()
        };

        Ok(ShaderResource::SampledImage(view, sampler))
    }

    fn make_image(ctx: &mut dashi::Context, name: &str) -> Result<ShaderResource, PipelineBuildError> {
        const BLACK_PIXEL: [u8; 4] = [0, 0, 0, 0];

        let image = ctx
            .make_image(&ImageInfo {
                debug_name: "bento_default_image",
                dim: [1, 1, 1],
                layers: 1,
                format: Format::RGBA8,
                mip_levels: 1,
                samples: SampleCount::S1,
                initial_data: Some(&BLACK_PIXEL),
            })
            .map_err(|source| PipelineBuildError::DefaultResourceCreateFailed {
                name: name.to_string(),
                resource_type: "image",
                source,
            })?;

        let view = ImageView {
            img: image,
            ..Default::default()
        };

        Ok(ShaderResource::Image(view))
    }

    fn make_sampler(
        ctx: &mut dashi::Context,
        name: &str,
    ) -> Result<ShaderResource, PipelineBuildError> {
        let sampler = ctx
            .make_sampler(&SamplerInfo::default())
            .map_err(|source| PipelineBuildError::DefaultResourceCreateFailed {
                name: name.to_string(),
                resource_type: "sampler",
                source,
            })?;

        Ok(ShaderResource::Sampler(sampler))
    }

    fn get(
        &mut self,
        ctx: &mut dashi::Context,
        var_type: BindTableVariableType,
        name: &str,
    ) -> Result<ShaderResource, PipelineBuildError> {
        match var_type {
            BindTableVariableType::Uniform | BindTableVariableType::DynamicUniform => {
                if self.uniform.is_none() {
                    self.uniform = Some(Self::make_uniform(ctx, name)?);
                }

                Ok(self.uniform.clone().expect("uniform default"))
            }
            BindTableVariableType::Storage | BindTableVariableType::DynamicStorage => {
                if self.storage.is_none() {
                    self.storage = Some(Self::make_storage(ctx, name)?);
                }

                Ok(self.storage.clone().expect("storage default"))
            }
            BindTableVariableType::SampledImage => {
                if self.sampled_image.is_none() {
                    self.sampled_image = Some(Self::make_sampled_image(ctx, name)?);
                }

                Ok(self.sampled_image.clone().expect("sampled image default"))
            }
            BindTableVariableType::Image => {
                if self.image.is_none() {
                    self.image = Some(Self::make_image(ctx, name)?);
                }

                Ok(self.image.clone().expect("image default"))
            }
            BindTableVariableType::Sampler => {
                if self.sampler.is_none() {
                    self.sampler = Some(Self::make_sampler(ctx, name)?);
                }

                Ok(self.sampler.clone().expect("sampler default"))
            }
            BindTableVariableType::StorageImage => {
                if self.storage_image.is_none() {
                    self.storage_image = Some(Self::make_sampled_image(ctx, name)?);
                }

                Ok(self.storage_image.clone().expect("storage image default"))
            }
        }
    }
}

fn default_resources_for_variable(
    defaults: &mut DefaultResources,
    ctx: &mut dashi::Context,
    var: &dashi::BindTableVariable,
    name: &str,
    size: u32,
) -> Result<Vec<IndexedResource>, PipelineBuildError> {
    let default_resource = defaults.get(ctx, var.var_type, name)?;

    let mut defaults = Vec::with_capacity(size as usize);
    for slot in 0..size {
        defaults.push(IndexedResource {
            resource: default_resource.clone(),
            slot,
        });
    }

    Ok(defaults)
}

fn resources_from_config(
    defaults: &mut DefaultResources,
    ctx: &mut dashi::Context,
    var: &dashi::BindTableVariable,
    name: &str,
    config: &BindTableVariable,
    expected_count: u32,
) -> Result<(Vec<IndexedResource>, u32), PipelineBuildError> {
    match config {
        BindTableVariable::Empty { size } => {
            if *size != expected_count {
                return Err(PipelineBuildError::InvalidResourceCount {
                    name: name.to_string(),
                    expected: expected_count,
                    provided: *size,
                });
            }

            let defaults =
                default_resources_for_variable(defaults, ctx, var, name, expected_count)?;
            Ok((defaults, expected_count))
        }
        BindTableVariable::WithResources { resources } => {
            if resources.len() != expected_count as usize {
                return Err(PipelineBuildError::InvalidResourceCount {
                    name: name.to_string(),
                    expected: expected_count,
                    provided: resources.len() as u32,
                });
            }

            let mut used_slots = HashSet::new();
            if resources
                .iter()
                .any(|res| res.slot >= expected_count || !used_slots.insert(res.slot))
            {
                return Err(PipelineBuildError::InvalidResourceSlots {
                    name: name.to_string(),
                    expected: expected_count,
                });
            }

            Ok((resources.clone(), expected_count))
        }
        BindTableVariable::Binding { resource } => Ok((
            vec![IndexedResource {
                resource: resource.clone(),
                slot: 0,
            }],
            1,
        )),
    }
}

fn resolve_binding_count(
    var: &dashi::BindTableVariable,
    config: Option<&BindTableVariable>,
    name: &str,
    set: u32,
) -> Result<u32, PipelineBuildError> {
    let config = config.ok_or_else(|| PipelineBuildError::MissingBindings {
        bindings: vec![MissingBinding {
            name: name.to_string(),
            set,
            binding: var.binding,
        }],
    })?;
    let count = match config {
        BindTableVariable::Binding { resource: _ } => 1,
        BindTableVariable::Empty { size } => *size,
        BindTableVariable::WithResources { resources } => resources.len() as u32,
    };

    Ok(if count == 0 { 256 } else { count })
}

#[derive(Debug, Clone)]
pub struct PSO {
    pub layout: Handle<GraphicsPipelineLayout>,
    pub handle: Handle<GraphicsPipeline>,
    pub bind_table: [Option<Handle<BindTable>>; 4],
    pub ctx: NonNull<Context>,
    table_bindings: HashMap<String, TableBinding>,
}

impl PSO {
    pub fn table_binding(&self, key: &str) -> Option<BindTableUpdateTarget> {
        self.table_bindings.get(key).copied().map(|binding| {
            BindTableUpdateTarget {
                table: binding.table,
                binding: binding.binding,
                size: binding.size,
            }
        })
    }

    pub fn update_table(&mut self, key: &str, resource: IndexedResource) {
        self.update_table_slice(key, std::slice::from_ref(&resource));
    }

    pub fn update_table_slice(&mut self, key: &str, resources: &[IndexedResource]) {
        let Some(binding_info) = self.table_bindings.get(key).copied() else {
            return;
        };

        if resources
            .iter()
            .any(|resource| resource.slot >= binding_info.size)
        {
            return;
        }

        let bindings = [IndexedBindingInfo {
            resources,
            binding: binding_info.binding,
        }];

        // Safety: The PSO stores a NonNull pointer to the context it was
        // created with. Callers are responsible for ensuring the context
        // remains valid for the lifetime of the PSO.
        let ctx = unsafe { self.ctx.as_mut() };
        let _ = ctx.update_bind_table(&dashi::BindTableUpdateInfo {
            table: binding_info.table,
            bindings: &bindings,
        });
    }

    pub fn tables(&self) -> [Option<Handle<BindTable>>; 4] {
        self.bind_table
    }
}

#[derive(Debug, Clone, Copy)]
struct TableBinding {
    table: Handle<BindTable>,
    binding: u32,
    size: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct BindTableUpdateTarget {
    pub table: Handle<BindTable>,
    pub binding: u32,
    pub size: u32,
}

#[derive(Clone)]
pub enum BindTableVariable {
    Empty { size: u32 },
    Binding { resource: ShaderResource },
    WithResources { resources: Vec<IndexedResource> },
}

pub struct AttachmentDesc {
    pub samples: SampleCount,
    pub format: Format,
}

pub struct PSOBuilder {
    vertex: Option<CompilationResult>,
    fragment: Option<CompilationResult>,
    depth: Option<AttachmentDesc>,
    table_variables: HashMap<String, BindTableVariable>,
    attachment_formats: HashMap<u32, Format>,
    details: GraphicsPipelineDetails,
}

impl PSOBuilder {
    pub fn new() -> Self {
        Self {
            vertex: None,
            fragment: None,
            table_variables: HashMap::new(),
            attachment_formats: HashMap::new(),
            details: GraphicsPipelineDetails::default(),
            depth: None,
        }
    }

    pub fn vertex(self, shader: Option<&[u8]>) -> Self {
        let compiler = Compiler::new().unwrap();
        if let Some(bytes) = shader {
            return Self {
                #[cfg(debug_assertions)]
                vertex: Some(
                    compiler
                        .compile(
                            bytes,
                            &Request {
                                name: None,
                                lang: ShaderLang::Infer,
                                stage: ShaderType::Vertex,
                                optimization: OptimizationLevel::None,
                                debug_symbols: true,
                                defines: HashMap::new(),
                            },
                        )
                        .unwrap(),
                ),
                #[cfg(not(debug_assertions))]
                vertex: Some(
                    compiler
                        .compile(
                            bytes,
                            &Request {
                                name: None,
                                lang: ShaderLang::Infer,
                                stage: ShaderType::Vertex,
                                optimization: OptimizationLevel::Performance,
                                debug_symbols: false,
                                defines: HashMap::new(),
                            },
                        )
                        .unwrap(),
                ),

                ..self
            };
        }

        self
    }

    pub fn vertex_compiled(self, shader: Option<CompilationResult>) -> Self {
        if let Some(compiled) = shader {
            return Self {
                vertex: Some(compiled),
                ..self
            };
        }

        self
    }

    pub fn fragment(self, shader: Option<&[u8]>) -> Self {
        if let Some(bytes) = shader {
            if let Ok(result) = CompilationResult::from_bytes(bytes) {
                return Self {
                    fragment: Some(result),
                    ..self
                };
            }
        }

        self
    }

    pub fn fragment_compiled(self, shader: Option<CompilationResult>) -> Self {
        if let Some(compiled) = shader {
            return Self {
                fragment: Some(compiled),
                ..self
            };
        }

        self
    }

    pub fn add_depth_target(self, info: AttachmentDesc) -> Self {
        return Self {
            depth: Some(info),
            ..self
        };
    }
    // Adds a variable to this builder. The variable name is used to match the binding up with the
    // shader source bindings.
    pub fn add_table_variable(self, key: &str, size: u32) -> Self {
        let mut table_variables = self.table_variables;
        table_variables.insert(key.to_string(), BindTableVariable::Empty { size });

        Self {
            table_variables,
            ..self
        }
    }

    pub fn add_table_variable_with_resources(
        self,
        key: &str,
        resources: Vec<IndexedResource>,
    ) -> Self {
        let mut table_variables = self.table_variables;
        table_variables.insert(
            key.to_string(),
            BindTableVariable::WithResources { resources },
        );

        Self {
            table_variables,
            ..self
        }
    }

    pub fn add_variable(self, key: &str, variable: ShaderResource) -> Self {
        let mut table_variables = self.table_variables;
        table_variables.insert(
            key.to_string(),
            BindTableVariable::Binding { resource: variable },
        );

        Self {
            table_variables,
            ..self
        }
    }

    pub fn set_details(self, details: GraphicsPipelineDetails) -> Self {
        Self { details, ..self }
    }

    pub fn set_attachment_format(self, slot: u32, format: Format) -> Self {
        let mut attachment_formats = self.attachment_formats;
        attachment_formats.insert(slot, format);

        Self {
            attachment_formats,
            ..self
        }
    }

    pub fn build(self, ctx: &mut dashi::Context) -> Result<PSO, BentoError> {
        let PSOBuilder {
            vertex,
            fragment,
            table_variables,
            attachment_formats,
            details,
            depth,
        } = self;

        let vertex = vertex.ok_or(PipelineBuildError::MissingShader { stage: "vertex" })?;
        let fragment = fragment.ok_or(PipelineBuildError::MissingShader { stage: "fragment" })?;

        let mut missing_bindings = Vec::new();
        let mut seen = HashSet::new();
        for var in vertex.variables.iter().chain(fragment.variables.iter()) {
            if !table_variables.contains_key(&var.name) && seen.insert(var.name.clone()) {
                missing_bindings.push(MissingBinding {
                    name: var.name.clone(),
                    set: var.set,
                    binding: var.kind.binding,
                });
            }
        }

        if !missing_bindings.is_empty() {
            return Err(PipelineBuildError::MissingBindings {
                bindings: missing_bindings,
            }
            .into());
        }

        // Build bind table layouts and tables.
        let mut bt_layouts: [Option<Handle<BindTableLayout>>; 4] = [None; 4];
        let mut bind_tables: [Option<Handle<BindTable>>; 4] = [None; 4];
        let mut table_bindings = HashMap::new();
        let mut defaults = DefaultResources::default();

        for set in 0..4u32 {
            let mut merged_vars: HashMap<u32, (dashi::BindTableVariable, dashi::ShaderType)> =
                HashMap::new();

            let mut collect_vars = |stage: &CompilationResult,
                                    shader_stage: dashi::ShaderType|
             -> Result<(), PipelineBuildError> {
                for var in stage.variables.iter().filter(|var| var.set == set) {
                    let count = resolve_binding_count(
                        &var.kind,
                        table_variables.get(&var.name),
                        &var.name,
                        var.set,
                    )?;
                    let var_type =
                        promoted_var_type(var.kind.var_type, table_variables.get(&var.name));

                    match merged_vars.entry(var.kind.binding) {
                        Entry::Occupied(mut entry) => {
                            let (existing, stage_flags) = entry.get_mut();
                            if existing.count != count {
                                return Err(PipelineBuildError::MismatchedBindingCounts {
                                    set,
                                    binding: var.kind.binding,
                                    expected: existing.count,
                                    provided: count,
                                });
                            }
                            existing.var_type = merge_variable_type(existing.var_type, var_type);
                            *stage_flags = merge_stage_flags(*stage_flags, shader_stage);
                        }
                        Entry::Vacant(entry) => {
                            let mut var_with_count = var.kind.clone();
                            var_with_count.count = count;
                            var_with_count.var_type = var_type;
                            entry.insert((var_with_count, shader_stage));
                        }
                    }
                }

                Ok(())
            };

            collect_vars(&vertex, vertex.stage)?;
            collect_vars(&fragment, fragment.stage)?;

            if merged_vars.is_empty() {
                continue;
            }

            let mut merged_vars: Vec<(u32, (dashi::BindTableVariable, dashi::ShaderType))> =
                merged_vars.into_iter().collect();
            merged_vars.sort_by_key(|(_, (var, _))| var.binding);

            let mut vertex_vars = Vec::new();
            let mut fragment_vars = Vec::new();
            let mut shared_vars = Vec::new();

            for (_, (var, stage)) in merged_vars.iter() {
                match stage {
                    dashi::ShaderType::Vertex => vertex_vars.push(var.clone()),
                    dashi::ShaderType::Fragment => fragment_vars.push(var.clone()),
                    dashi::ShaderType::All => shared_vars.push(var.clone()),
                    _ => {}
                }
            }

            let mut shader_infos: Vec<ShaderInfo> = Vec::new();
            if !vertex_vars.is_empty() {
                shader_infos.push(ShaderInfo {
                    shader_type: dashi::ShaderType::Vertex,
                    variables: vertex_vars.as_slice(),
                });
            }

            if !fragment_vars.is_empty() {
                shader_infos.push(ShaderInfo {
                    shader_type: dashi::ShaderType::Fragment,
                    variables: fragment_vars.as_slice(),
                });
            }

            if !shared_vars.is_empty() {
                shader_infos.push(ShaderInfo {
                    shader_type: dashi::ShaderType::All,
                    variables: shared_vars.as_slice(),
                });
            }

            let layout =
                ctx.make_bind_table_layout(&BindTableLayoutInfo {
                    debug_name: "bento_bt_layout",
                    shaders: shader_infos.as_slice(),
                })
                .map_err(|source| {
                    PipelineBuildError::BindTableLayoutCreateFailed { set, source }
                })?;

            if (set as usize) < bt_layouts.len() {
                bt_layouts[set as usize] = Some(layout);
            }

            // Create bind table with any provided resources.
            let mut pending_bindings = Vec::new();
            let mut pending_names = Vec::new();
            let mut bound_indices = HashSet::new();
            let mut resources: Vec<Vec<IndexedResource>> = Vec::new();
            for var in vertex.variables.iter().chain(fragment.variables.iter()) {
                if var.set != set {
                    continue;
                }

                if let Some(resource) = table_variables.get(&var.name) {
                    let expected_count =
                        resolve_binding_count(&var.kind, Some(resource), &var.name, var.set)?;
                    if bound_indices.insert(var.kind.binding) {
                        let (initial_resources, size) = resources_from_config(
                            &mut defaults,
                            ctx,
                            &var.kind,
                            &var.name,
                            resource,
                            expected_count,
                        )?;
                        resources.push(initial_resources);
                        let resource_index = resources.len() - 1;

                        pending_bindings.push((var.kind.binding, resource_index));
                        pending_names.push((var.name.clone(), var.kind.binding, size));
                    }
                }
            }

            if !pending_bindings.is_empty() {
                let indexed_bindings: Vec<IndexedBindingInfo> = pending_bindings
                    .iter()
                    .map(|(binding, resource_index)| IndexedBindingInfo {
                        resources: resources[*resource_index].as_slice(),
                        binding: *binding,
                    })
                    .collect();

                let table = ctx
                    .make_bind_table(&BindTableInfo {
                        debug_name: "bento_bind_table",
                        layout,
                        bindings: indexed_bindings.as_slice(),
                        set,
                    })
                    .map_err(|source| PipelineBuildError::BindTableCreateFailed { set, source })?;
                for (name, binding, size) in pending_names {
                    table_bindings.insert(
                        name,
                        TableBinding {
                            table,
                            binding,
                            size,
                        },
                    );
                }
                if (set as usize) < bind_tables.len() {
                    bind_tables[set as usize] = Some(table);
                }
            }
        }

        let shader_infos = vec![
            PipelineShaderInfo {
                stage: vertex.stage,
                spirv: &vertex.spirv,
                specialization: &[],
            },
            PipelineShaderInfo {
                stage: fragment.stage,
                spirv: &fragment.spirv,
                specialization: &[],
            },
        ];

        let vertex_entries: Vec<VertexEntryInfo> = vertex
            .metadata
            .vertex
            .as_ref()
            .map(|layout| {
                layout
                    .entries
                    .iter()
                    .map(|entry| VertexEntryInfo {
                        format: entry.format,
                        location: entry.location,
                        offset: entry.offset,
                    })
                    .collect()
            })
            .unwrap_or_default();

        let vertex_info = VertexDescriptionInfo {
            entries: vertex_entries.as_slice(),
            stride: vertex
                .metadata
                .vertex
                .as_ref()
                .map(|v| v.stride)
                .unwrap_or_default(),
            rate: vertex
                .metadata
                .vertex
                .as_ref()
                .map(|v| v.rate)
                .unwrap_or(dashi::VertexRate::Vertex),
        };

        let sample_count = details.sample_count.clone();
        let layout = ctx
            .make_graphics_pipeline_layout(&GraphicsPipelineLayoutInfo {
                debug_name: "bento_graphics_layout",
                vertex_info,
                bt_layouts,
                shaders: shader_infos.as_slice(),
                details,
            })
            .map_err(|source| PipelineBuildError::PipelineLayoutCreateFailed {
                pipeline: "graphics",
                source,
            })?;

        let mut idx = 0;
        let attachments: Vec<Format> = fragment
            .metadata
            .outputs
            .iter()
            .map(|iv| {
                if let Some(format) = attachment_formats.get(&idx).copied() {
                    return format;
                }

                idx += 1;
                let fmt = iv.format.unwrap_or(ShaderPrimitiveType::Vec4);
                match fmt {
                    ShaderPrimitiveType::Vec2 => Format::RGB8,
                    ShaderPrimitiveType::Vec3 => Format::RGB8,
                    ShaderPrimitiveType::Vec4 => Format::RGBA8,
                    ShaderPrimitiveType::IVec4 => Format::RGBA8,
                    ShaderPrimitiveType::UVec4 => Format::RGBA8,
                }
            })
            .collect();

        let samples = attachments.iter().map(|_| sample_count).collect();

        let depth_format = match depth.as_ref() {
            Some(d) => Some(d.format),
            None => None,
        };

        let depth_sample = match depth.as_ref() {
            Some(d) => Some(d.samples),
            None => None,
        };

        let pipeline = ctx
            .make_graphics_pipeline(&GraphicsPipelineInfo {
                layout,
                attachment_formats: attachments,
                depth_format,
                subpass_samples: dashi::SubpassSampleInfo {
                    color_samples: samples,
                    depth_sample,
                },
                subpass_id: 0,
                debug_name: "bento_graphics_pipeline",
            })
            .map_err(|source| PipelineBuildError::PipelineCreateFailed {
                pipeline: "graphics",
                source,
            })?;

        Ok(PSO {
            layout,
            handle: pipeline,
            bind_table: bind_tables,
            table_bindings,
            ctx: NonNull::from(ctx),
        })
    }
}

////////////////////////////////////////////////////////////////////////////
///

#[derive(Debug, Clone)]
pub struct CSO {
    pub layout: Handle<ComputePipelineLayout>,
    pub handle: Handle<ComputePipeline>,
    pub bind_table: [Option<Handle<BindTable>>; 4],
    pub ctx: NonNull<Context>,
    table_bindings: HashMap<String, TableBinding>,
}

impl CSO {
    pub fn table_binding(&self, key: &str) -> Option<BindTableUpdateTarget> {
        self.table_bindings.get(key).copied().map(|binding| {
            BindTableUpdateTarget {
                table: binding.table,
                binding: binding.binding,
                size: binding.size,
            }
        })
    }

    pub fn update_table(&mut self, key: &str, resource: IndexedResource) {
        self.update_table_slice(key, std::slice::from_ref(&resource));
    }

    pub fn update_table_slice(&mut self, key: &str, resources: &[IndexedResource]) {
        let Some(binding_info) = self.table_bindings.get(key).copied() else {
            return;
        };

        if resources
            .iter()
            .any(|resource| resource.slot >= binding_info.size)
        {
            return;
        }

        let bindings = [IndexedBindingInfo {
            resources,
            binding: binding_info.binding,
        }];

        let ctx = unsafe { self.ctx.as_mut() };
        let _ = ctx.update_bind_table(&dashi::BindTableUpdateInfo {
            table: binding_info.table,
            bindings: &bindings,
        });
    }

    pub fn tables(&self) -> [Option<Handle<BindTable>>; 4] {
        self.bind_table
    }
}
pub struct CSOBuilder {
    shader: Option<CompilationResult>,
    table_variables: HashMap<String, BindTableVariable>,
}

impl CSOBuilder {
    pub fn new() -> Self {
        Self {
            shader: None,
            table_variables: HashMap::new(),
        }
    }

    pub fn shader(self, shader: Option<&[u8]>) -> Self {
        if let Some(shader) = shader {
            let compiler = Compiler::new().unwrap();
            return Self {
                #[cfg(debug_assertions)]
                shader: Some(
                    compiler
                        .compile(
                            shader,
                            &Request {
                                name: None,
                                lang: ShaderLang::Infer,
                                stage: ShaderType::Compute,
                                optimization: OptimizationLevel::None,
                                debug_symbols: true,
                                defines: HashMap::new(),
                            },
                        )
                        .unwrap(),
                ),
                #[cfg(not(debug_assertions))]
                shader: Some(
                    compiler
                        .compile(
                            shader,
                            &Request {
                                name: None,
                                lang: ShaderLang::Infer,
                                stage: ShaderType::Compute,
                                optimization: OptimizationLevel::Performance,
                                debug_symbols: false,
                                defines: HashMap::new(),
                            },
                        )
                        .unwrap(),
                ),
                ..self
            };
        }

        self
    }

    pub fn shader_compiled(self, shader: Option<CompilationResult>) -> Self {
        if let Some(shader) = shader {
            return Self {
                shader: Some(shader),
                ..self
            };
        }

        self
    }

    // Adds a bind table variable to this builder. The variable name is used to match the binding up with the
    // shader source bindings.
    pub fn add_table_variable(self, key: &str, size: u32) -> Self {
        let mut table_variables = self.table_variables;
        table_variables.insert(key.to_string(), BindTableVariable::Empty { size });

        Self {
            table_variables,
            ..self
        }
    }

    pub fn add_table_variable_with_resources(
        self,
        key: &str,
        resources: Vec<IndexedResource>,
    ) -> Self {
        let mut table_variables = self.table_variables;
        table_variables.insert(
            key.to_string(),
            BindTableVariable::WithResources { resources },
        );

        Self {
            table_variables,
            ..self
        }
    }

    // Adds a variable to this builder. The variable name is used to match the binding up with the
    // shader source bindings.
    pub fn add_variable(self, key: &str, variable: ShaderResource) -> Self {
        let mut table_variables = self.table_variables;
        table_variables.insert(
            key.to_string(),
            BindTableVariable::Binding { resource: variable },
        );

        Self {
            table_variables,
            ..self
        }
    }

    // Will fail if shaders are not given, or if variables given do not
    pub fn build(self, ctx: &mut dashi::Context) -> Result<CSO, BentoError> {
        let CSOBuilder {
            shader,
            table_variables,
        } = self;

        let shader = shader.ok_or(PipelineBuildError::MissingShader { stage: "compute" })?;

        let mut missing_bindings = Vec::new();
        let mut seen = HashSet::new();
        for var in shader.variables.iter() {
            if !table_variables.contains_key(&var.name) && seen.insert(var.name.clone()) {
                missing_bindings.push(MissingBinding {
                    name: var.name.clone(),
                    set: var.set,
                    binding: var.kind.binding,
                });
            }
        }

        if !missing_bindings.is_empty() {
            return Err(PipelineBuildError::MissingBindings {
                bindings: missing_bindings,
            }
            .into());
        }

        let mut bt_layouts: [Option<Handle<BindTableLayout>>; 4] = [None; 4];
        let mut bind_tables: [Option<Handle<BindTable>>; 4] = [None; 4];
        let mut table_bindings = HashMap::new();
        let mut defaults = DefaultResources::default();

        for set in 0..4u32 {
            let vars: Vec<dashi::BindTableVariable> = shader
                .variables
                .iter()
                .filter(|var| var.set == set)
                .map(|var| {
                    let mut var_with_count = var.kind.clone();
                    var_with_count.var_type =
                        promoted_var_type(var.kind.var_type, table_variables.get(&var.name));
                    var_with_count.count = resolve_binding_count(
                        &var.kind,
                        table_variables.get(&var.name),
                        &var.name,
                        var.set,
                    )?;

                    Ok(var_with_count)
                })
                .collect::<Result<_, PipelineBuildError>>()?;

            if vars.is_empty() {
                continue;
            }

            let shader_info = ShaderInfo {
                shader_type: shader.stage,
                variables: vars.as_slice(),
            };

            let layout =
                ctx.make_bind_table_layout(&BindTableLayoutInfo {
                    debug_name: "bento_compute_bt_layout",
                    shaders: std::slice::from_ref(&shader_info),
                })
                .map_err(|source| {
                    PipelineBuildError::BindTableLayoutCreateFailed { set, source }
                })?;

            if (set as usize) < bt_layouts.len() {
                bt_layouts[set as usize] = Some(layout);
            }

            let mut pending_bindings = Vec::new();
            let mut pending_names = Vec::new();
            let mut resources: Vec<Vec<IndexedResource>> = Vec::new();
            for var in shader.variables.iter() {
                if var.set != set {
                    continue;
                }

                if let Some(res) = table_variables.get(&var.name) {
                    let expected_count =
                        resolve_binding_count(&var.kind, Some(res), &var.name, var.set)?;
                    let (initial_resources, size) = resources_from_config(
                        &mut defaults,
                        ctx,
                        &var.kind,
                        &var.name,
                        res,
                        expected_count,
                    )?;
                    resources.push(initial_resources);
                    let resource_index = resources.len() - 1;

                    pending_bindings.push((var.kind.binding, resource_index));
                    pending_names.push((var.name.clone(), var.kind.binding, size));
                }
            }

            if !pending_bindings.is_empty() {
                let indexed_bindings: Vec<IndexedBindingInfo> = pending_bindings
                    .iter()
                    .map(|(binding, resource_index)| IndexedBindingInfo {
                        resources: resources[*resource_index].as_slice(),
                        binding: *binding,
                    })
                    .collect();

                let table = ctx
                    .make_bind_table(&BindTableInfo {
                        debug_name: "bento_compute_bind_table",
                        layout,
                        bindings: indexed_bindings.as_slice(),
                        set,
                    })
                    .map_err(|source| PipelineBuildError::BindTableCreateFailed { set, source })?;
                for (name, binding, size) in pending_names {
                    table_bindings.insert(
                        name,
                        TableBinding {
                            table,
                            binding,
                            size,
                        },
                    );
                }
                if (set as usize) < bind_tables.len() {
                    bind_tables[set as usize] = Some(table);
                }
            }
        }

        let shader_info = PipelineShaderInfo {
            stage: shader.stage,
            spirv: &shader.spirv,
            specialization: &[],
        };

        let layout = ctx
            .make_compute_pipeline_layout(&ComputePipelineLayoutInfo {
                bt_layouts,
                shader: &shader_info,
            })
            .map_err(|source| PipelineBuildError::PipelineLayoutCreateFailed {
                pipeline: "compute",
                source,
            })?;

        let pipeline = ctx
            .make_compute_pipeline(&ComputePipelineInfo {
                debug_name: "bento_compute_pipeline",
                layout,
            })
            .map_err(|source| PipelineBuildError::PipelineCreateFailed {
                pipeline: "compute",
                source,
            })?;

        Ok(CSO {
            layout,
            handle: pipeline,
            bind_table: bind_tables,
            table_bindings,
            ctx: NonNull::from(ctx),
        })
    }
}
