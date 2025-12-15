use std::{
    collections::{HashMap, HashSet},
    ptr::NonNull,
};

use dashi::{
    BindGroup, BindGroupInfo, BindGroupLayout, BindGroupLayoutInfo, BindGroupVariableType,
    BindTable, BindTableInfo, BindTableLayout, BindTableLayoutInfo, BufferInfo, BufferUsage,
    BufferView, ComputePipeline, ComputePipelineInfo, ComputePipelineLayout,
    ComputePipelineLayoutInfo, Context, Format, GraphicsPipeline, GraphicsPipelineDetails,
    GraphicsPipelineInfo, GraphicsPipelineLayout, GraphicsPipelineLayoutInfo, Handle, ImageInfo,
    ImageView, IndexedBindingInfo, IndexedResource, MemoryVisibility, PipelineShaderInfo,
    SampleCount, SamplerInfo, ShaderInfo, ShaderResource, ShaderType, VertexDescriptionInfo,
    VertexEntryInfo,
};

use crate::{CompilationResult, Compiler, OptimizationLevel, Request, ShaderLang};

fn merge_stage_flags(lhs: dashi::ShaderType, rhs: dashi::ShaderType) -> dashi::ShaderType {
    if lhs == rhs {
        lhs
    } else {
        dashi::ShaderType::All
    }
}

struct DefaultResources {
    uniform: Option<ShaderResource>,
    storage: Option<ShaderResource>,
    sampled_image: Option<ShaderResource>,
    storage_image: Option<ShaderResource>,
}

impl Default for DefaultResources {
    fn default() -> Self {
        Self {
            uniform: None,
            storage: None,
            sampled_image: None,
            storage_image: None,
        }
    }
}

impl DefaultResources {
    fn make_uniform(ctx: &mut dashi::Context) -> Option<ShaderResource> {
        let buffer = ctx
            .make_buffer(&BufferInfo {
                debug_name: "bento_default_uniform",
                byte_size: 256,
                visibility: MemoryVisibility::CpuAndGpu,
                usage: BufferUsage::UNIFORM,
                initial_data: None,
            })
            .ok()?;

        Some(ShaderResource::Buffer(BufferView::new(buffer)))
    }

    fn make_storage(ctx: &mut dashi::Context) -> Option<ShaderResource> {
        let buffer = ctx
            .make_buffer(&BufferInfo {
                debug_name: "bento_default_storage",
                byte_size: 256,
                visibility: MemoryVisibility::CpuAndGpu,
                usage: BufferUsage::STORAGE,
                initial_data: None,
            })
            .ok()?;

        Some(ShaderResource::StorageBuffer(BufferView::new(buffer)))
    }

    fn make_sampled_image(ctx: &mut dashi::Context) -> Option<ShaderResource> {
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
            .ok()?;

        let sampler = ctx.make_sampler(&SamplerInfo::default()).ok()?;
        let view = ImageView {
            img: image,
            ..Default::default()
        };

        Some(ShaderResource::SampledImage(view, sampler))
    }

    fn get(
        &mut self,
        ctx: &mut dashi::Context,
        var_type: BindGroupVariableType,
    ) -> Option<ShaderResource> {
        match var_type {
            BindGroupVariableType::Uniform | BindGroupVariableType::DynamicUniform => {
                if self.uniform.is_none() {
                    self.uniform = Self::make_uniform(ctx);
                }

                self.uniform.clone()
            }
            BindGroupVariableType::Storage | BindGroupVariableType::DynamicStorage => {
                if self.storage.is_none() {
                    self.storage = Self::make_storage(ctx);
                }

                self.storage.clone()
            }
            BindGroupVariableType::SampledImage => {
                if self.sampled_image.is_none() {
                    self.sampled_image = Self::make_sampled_image(ctx);
                }

                self.sampled_image.clone()
            }
            BindGroupVariableType::StorageImage => {
                if self.storage_image.is_none() {
                    self.storage_image = Self::make_sampled_image(ctx);
                }

                self.storage_image.clone()
            }
        }
    }
}

fn default_resources_for_variable(
    defaults: &mut DefaultResources,
    ctx: &mut dashi::Context,
    var: &dashi::BindGroupVariable,
    size: u32,
) -> Option<Vec<IndexedResource>> {
    let default_resource = defaults.get(ctx, var.var_type)?;

    let mut defaults = Vec::with_capacity(size as usize);
    for slot in 0..size {
        defaults.push(IndexedResource {
            resource: default_resource.clone(),
            slot,
        });
    }

    Some(defaults)
}

fn resources_from_config(
    defaults: &mut DefaultResources,
    ctx: &mut dashi::Context,
    var: &dashi::BindGroupVariable,
    config: &BindTableVariable,
    expected_count: u32,
) -> Option<(Vec<IndexedResource>, u32)> {
    match config {
        BindTableVariable::Empty { size } => {
            if *size != expected_count {
                return None;
            }

            let defaults = default_resources_for_variable(defaults, ctx, var, expected_count)?;
            Some((defaults, expected_count))
        }
        BindTableVariable::WithResources { resources } => {
            if resources.len() != expected_count as usize {
                return None;
            }

            let mut used_slots = HashSet::new();
            if resources
                .iter()
                .any(|res| res.slot >= expected_count || !used_slots.insert(res.slot))
            {
                return None;
            }

            Some((resources.clone(), expected_count))
        }
    }
}

fn resolve_binding_count(
    var: &dashi::BindGroupVariable,
    config: Option<&BindTableVariable>,
) -> u32 {
    let count = match config {
        Some(BindTableVariable::Empty { size }) => *size,
        Some(BindTableVariable::WithResources { resources }) => resources.len() as u32,
        None => var.count,
    };

    if count == 0 { 256 } else { count }
}

#[derive(Clone)]
pub struct PSO {
    pub layout: Handle<GraphicsPipelineLayout>,
    pub handle: Handle<GraphicsPipeline>,
    pub bind_groups: Vec<Handle<BindGroup>>,
    pub bind_table: Vec<Handle<BindTable>>,
    pub ctx: NonNull<Context>,
    table_bindings: HashMap<String, TableBinding>,
}

impl PSO {
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
}

#[derive(Clone, Copy)]
struct TableBinding {
    table: Handle<BindTable>,
    binding: u32,
    size: u32,
}

#[derive(Clone)]
pub enum BindTableVariable {
    Empty { size: u32 },
    WithResources { resources: Vec<IndexedResource> },
}

pub struct GraphicsPipelineBuilder {
    vertex: Option<CompilationResult>,
    fragment: Option<CompilationResult>,
    variables: HashMap<String, ShaderResource>,
    table_variables: HashMap<String, BindTableVariable>,
    details: GraphicsPipelineDetails,
}

impl GraphicsPipelineBuilder {
    pub fn new() -> Self {
        Self {
            vertex: None,
            fragment: None,
            variables: HashMap::new(),
            table_variables: HashMap::new(),
            details: GraphicsPipelineDetails::default(),
        }
    }

    pub fn vertex(self, shader: Option<&[u8]>) -> Self {
        if let Some(bytes) = shader {
            if let Ok(result) = CompilationResult::from_bytes(bytes) {
                return Self {
                    vertex: Some(result),
                    ..self
                };
            }
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
        let mut variables = self.variables;
        variables.insert(key.to_string(), variable);

        Self { variables, ..self }
    }

    pub fn set_details(self, details: GraphicsPipelineDetails) -> Self {
        Self { details, ..self }
    }

    pub fn build(self, ctx: &mut dashi::Context) -> Option<PSO> {
        let GraphicsPipelineBuilder {
            vertex,
            fragment,
            variables,
            table_variables,
            details,
        } = self;

        let vertex = vertex?;
        let fragment = fragment?;

        // Build bind group layouts for up to 4 sets.
        let mut bg_layouts: [Option<Handle<BindGroupLayout>>; 4] = [None; 4];
        let mut bind_groups = Vec::new();

        for set in 0..4u32 {
            let mut merged_vars: HashMap<
                u32,
                (dashi::BindGroupVariable, dashi::ShaderType, Vec<String>),
            > = HashMap::new();

            let mut collect_vars = |stage: &CompilationResult, shader_stage: dashi::ShaderType| {
                for var in stage.variables.iter().filter(|var| var.set == set) {
                    merged_vars
                        .entry(var.kind.binding)
                        .and_modify(|(existing, stage_flags, names)| {
                            *stage_flags = merge_stage_flags(*stage_flags, shader_stage);
                            if !names.contains(&var.name) {
                                names.push(var.name.clone());
                            }
                            *existing = var.kind.clone();
                        })
                        .or_insert((var.kind.clone(), shader_stage, vec![var.name.clone()]));
                }
            };

            collect_vars(&vertex, vertex.stage);
            collect_vars(&fragment, fragment.stage);

            if merged_vars.is_empty() {
                continue;
            }

            let mut merged_vars: Vec<(
                u32,
                (dashi::BindGroupVariable, dashi::ShaderType, Vec<String>),
            )> = merged_vars.into_iter().collect();
            merged_vars.sort_by_key(|(_, (var, _, _))| var.binding);

            let mut vertex_vars = Vec::new();
            let mut fragment_vars = Vec::new();
            let mut shared_vars = Vec::new();

            for (_, (var, stage, _)) in merged_vars.iter() {
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

            if shader_infos.is_empty() {
                continue;
            }

            let layout = ctx
                .make_bind_group_layout(&BindGroupLayoutInfo {
                    debug_name: "bento_bg_layout",
                    shaders: shader_infos.as_slice(),
                })
                .ok()?;

            if (set as usize) < bg_layouts.len() {
                bg_layouts[set as usize] = Some(layout);
            }

            // Build bind group if resources were provided.
            let mut bindings = Vec::new();
            for (_, (var, _, names)) in merged_vars.iter() {
                if let Some(res) = names.iter().find_map(|name| variables.get(name)) {
                    bindings.push(dashi::BindingInfo {
                        binding: var.binding,
                        resource: res.clone(),
                    });
                }
            }

            if !bindings.is_empty() {
                let bind_group = ctx
                    .make_bind_group(&BindGroupInfo {
                        debug_name: "bento_bind_group",
                        layout,
                        bindings: bindings.as_slice(),
                        set,
                    })
                    .ok()?;
                bind_groups.push(bind_group);
            }
        }

        // Build bind table layouts and tables.
        let mut bt_layouts: [Option<Handle<BindTableLayout>>; 4] = [None; 4];
        let mut bind_tables = Vec::new();
        let mut table_bindings = HashMap::new();
        let mut defaults = DefaultResources::default();

        for set in 0..4u32 {
            let mut combined_vars: HashMap<u32, dashi::BindGroupVariable> = HashMap::new();

            for var in vertex.variables.iter().chain(fragment.variables.iter()) {
                if var.set != set {
                    continue;
                }

                let count = resolve_binding_count(&var.kind, table_variables.get(&var.name));

                if let Some(existing) = combined_vars.get(&var.kind.binding) {
                    if existing.count != count {
                        return None;
                    }
                } else {
                    let mut var_with_count = var.kind.clone();
                    var_with_count.count = count;
                    combined_vars.insert(var.kind.binding, var_with_count);
                }
            }

            if combined_vars.is_empty() {
                continue;
            }

            let mut merged_vars: Vec<dashi::BindGroupVariable> =
                combined_vars.into_values().collect();
            merged_vars.sort_by_key(|var| var.binding);

            let shader_infos = [ShaderInfo {
                shader_type: dashi::ShaderType::All,
                variables: merged_vars.as_slice(),
            }];

            let layout = ctx
                .make_bind_table_layout(&BindTableLayoutInfo {
                    debug_name: "bento_bt_layout",
                    shaders: shader_infos.as_slice(),
                })
                .ok()?;

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
                    let expected_count = resolve_binding_count(&var.kind, Some(resource));
                    if bound_indices.insert(var.kind.binding) {
                        let (initial_resources, size) = resources_from_config(
                            &mut defaults,
                            ctx,
                            &var.kind,
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
                    .ok()?;
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
                bind_tables.push(table);
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

        let layout = ctx
            .make_graphics_pipeline_layout(&GraphicsPipelineLayoutInfo {
                debug_name: "bento_graphics_layout",
                vertex_info,
                bg_layouts,
                bt_layouts,
                shaders: shader_infos.as_slice(),
                details,
            })
            .ok()?;

        let pipeline = ctx
            .make_graphics_pipeline(&GraphicsPipelineInfo {
                layout,
                attachment_formats: Vec::new(),
                depth_format: None,
                subpass_samples: dashi::SubpassSampleInfo::default(),
                subpass_id: 0,
                debug_name: "bento_graphics_pipeline",
            })
            .ok()?;

        Some(PSO {
            layout,
            handle: pipeline,
            bind_groups,
            bind_table: bind_tables,
            table_bindings,
            ctx: NonNull::from(ctx),
        })
    }
}

////////////////////////////////////////////////////////////////////////////
///

#[derive(Clone)]
pub struct CSO {
    pub layout: Handle<ComputePipelineLayout>,
    pub handle: Handle<ComputePipeline>,
    pub bind_groups: Vec<Handle<BindGroup>>,
    pub bind_table: Vec<Handle<BindTable>>,
    pub ctx: NonNull<Context>,
    table_bindings: HashMap<String, TableBinding>,
}

impl CSO {
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

    pub fn bindings(&self) -> [Option<Handle<BindGroup>>; 4] {
        let mut out: [Option<Handle<BindGroup>>; 4] = [None; 4];
        for (i, x) in self.bind_groups.iter().take(4).enumerate() {
            out[i] = Some(*x);
        }

        out
    }

    pub fn tables(&self) -> [Option<Handle<BindTable>>; 4] {
        let mut out: [Option<Handle<BindTable>>; 4] = [None; 4];
        for (i, x) in self.bind_table.iter().take(4).enumerate() {
            out[i] = Some(*x);
        }

        out
    }
}
pub struct ComputePipelineBuilder {
    shader: Option<CompilationResult>,
    variables: HashMap<String, ShaderResource>,
    table_variables: HashMap<String, BindTableVariable>,
}

impl ComputePipelineBuilder {
    pub fn new() -> Self {
        Self {
            shader: None,
            variables: HashMap::new(),
            table_variables: HashMap::new(),
        }
    }

    pub fn shader(self, shader: Option<&[u8]>) -> Self {
        if let Some(shader) = shader {
            let compiler = Compiler::new().unwrap();
            return Self {
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
        let mut variables = self.variables;
        variables.insert(key.to_string(), variable);

        Self { variables, ..self }
    }

    // Will fail if shaders are not given, or if variables given do not
    pub fn build(self, ctx: &mut dashi::Context) -> Option<CSO> {
        let ComputePipelineBuilder {
            shader,
            variables,
            table_variables,
        } = self;

        let shader = shader?;

        let mut bg_layouts: [Option<Handle<BindGroupLayout>>; 4] = [None; 4];
        let mut bind_groups = Vec::new();

        for set in 0..4u32 {
            let vars: Vec<dashi::BindGroupVariable> = shader
                .variables
                .iter()
                .filter(|var| var.set == set)
                .map(|var| {
                    let mut var_with_count = var.kind.clone();
                    var_with_count.count = resolve_binding_count(&var.kind, table_variables.get(&var.name));

                    var_with_count
                })
                .collect();

            if vars.is_empty() {
                continue;
            }

            let shader_info = ShaderInfo {
                shader_type: shader.stage,
                variables: vars.as_slice(),
            };

            let layout = ctx
                .make_bind_group_layout(&BindGroupLayoutInfo {
                    debug_name: "bento_compute_bg_layout",
                    shaders: std::slice::from_ref(&shader_info),
                })
                .ok()?;

            if (set as usize) < bg_layouts.len() {
                bg_layouts[set as usize] = Some(layout);
            }

            let mut bindings = Vec::new();
            for var in shader.variables.iter() {
                if var.set != set {
                    continue;
                }

                if let Some(res) = variables.get(&var.name) {
                    println!("res: {}", var.name);
                    bindings.push(dashi::BindingInfo {
                        binding: var.kind.binding,
                        resource: res.clone(),
                    });
                }
            }

            if !bindings.is_empty() {
                let bind_group = ctx
                    .make_bind_group(&BindGroupInfo {
                        debug_name: "bento_compute_bind_group",
                        layout,
                        bindings: bindings.as_slice(),
                        set,
                    })
                    .ok()?;
                bind_groups.push(bind_group);
            }
        }

        let mut bt_layouts: [Option<Handle<BindTableLayout>>; 4] = [None; 4];
        let mut bind_tables = Vec::new();
        let mut table_bindings = HashMap::new();
        let mut defaults = DefaultResources::default();

        for set in 0..4u32 {
            let vars: Vec<dashi::BindGroupVariable> = shader
                .variables
                .iter()
                .filter(|var| var.set == set)
                .map(|var| {
                    let mut var_with_count = var.kind.clone();
                    var_with_count.count =
                        resolve_binding_count(&var.kind, table_variables.get(&var.name));

                    var_with_count
                })
                .collect();

            if vars.is_empty() {
                continue;
            }

            let shader_info = ShaderInfo {
                shader_type: shader.stage,
                variables: vars.as_slice(),
            };

            let layout = ctx
                .make_bind_table_layout(&BindTableLayoutInfo {
                    debug_name: "bento_compute_bt_layout",
                    shaders: std::slice::from_ref(&shader_info),
                })
                .ok()?;

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
                    println!("res2: {}", var.name);
                    let expected_count = resolve_binding_count(&var.kind, Some(res));
                    let (initial_resources, size) =
                        resources_from_config(&mut defaults, ctx, &var.kind, res, expected_count)?;
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
                    .ok()?;
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
                bind_tables.push(table);
            }
        }

        let shader_info = PipelineShaderInfo {
            stage: shader.stage,
            spirv: &shader.spirv,
            specialization: &[],
        };

        let layout = ctx
            .make_compute_pipeline_layout(&ComputePipelineLayoutInfo {
                bg_layouts,
                bt_layouts,
                shader: &shader_info,
            })
            .ok()?;

        let pipeline = ctx
            .make_compute_pipeline(&ComputePipelineInfo {
                debug_name: "bento_compute_pipeline",
                layout,
            })
            .ok()?;

        Some(CSO {
            layout,
            handle: pipeline,
            bind_groups,
            bind_table: bind_tables,
            table_bindings,
            ctx: NonNull::from(ctx),
        })
    }
}
