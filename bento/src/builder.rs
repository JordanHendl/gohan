use std::{collections::{HashMap, HashSet}, ptr::NonNull};

use dashi::{
    BindGroup, BindGroupInfo, BindGroupLayout, BindGroupLayoutInfo, BindTable, BindTableInfo,
    BindTableLayout, BindTableLayoutInfo, ComputePipeline, ComputePipelineInfo, ComputePipelineLayout,
    ComputePipelineLayoutInfo, Context, GraphicsPipeline, GraphicsPipelineDetails,
    GraphicsPipelineInfo, GraphicsPipelineLayout, GraphicsPipelineLayoutInfo, Handle,
    IndexedBindingInfo, IndexedResource, PipelineShaderInfo, ShaderInfo, ShaderResource,
    VertexDescriptionInfo, VertexEntryInfo,
};

use crate::CompilationResult;

fn merge_stage_flags(lhs: dashi::ShaderType, rhs: dashi::ShaderType) -> dashi::ShaderType {
    if lhs == rhs {
        lhs
    } else {
        dashi::ShaderType::All
    }
}

pub struct PSO {
    pub layout: Handle<GraphicsPipelineLayout>,
    pub handle: Handle<GraphicsPipeline>,
    pub bind_groups: Vec<Handle<BindGroup>>,
    pub bind_table: Vec<Handle<BindTable>>,
    table_bindings: HashMap<String, (Handle<BindTable>, u32)>,
    ctx: NonNull<Context>,
}

impl PSO {
    pub fn update_table(&mut self, key: &str, resource: IndexedResource) {
        if let Some((table, binding)) = self.table_bindings.get(key).copied() {
            let bindings = [IndexedBindingInfo {
                resources: std::slice::from_ref(&resource),
                binding,
            }];

            // Safety: The PSO stores a NonNull pointer to the context it was
            // created with. Callers are responsible for ensuring the context
            // remains valid for the lifetime of the PSO.
            let ctx = unsafe { self.ctx.as_mut() };
            let _ = ctx.update_bind_table(&dashi::BindTableUpdateInfo { table, bindings: &bindings });
        }
    }
}

pub struct GraphicsPipelineBuilder {
    vertex: Option<CompilationResult>,
    fragment: Option<CompilationResult>,
    variables: HashMap<String, ShaderResource>,
    table_variables: HashMap<String, IndexedResource>,
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
                return Self { vertex: Some(result), ..self };
            }
        }

        self
    }

    pub fn vertex_compiled(self, shader: Option<CompilationResult>) -> Self {
        if let Some(compiled) = shader {
            return Self { vertex: Some(compiled), ..self };
        }

        self
    }

    pub fn fragment(self, shader: Option<&[u8]>) -> Self {
        if let Some(bytes) = shader {
            if let Ok(result) = CompilationResult::from_bytes(bytes) {
                return Self { fragment: Some(result), ..self };
            }
        }

        self
    }

    pub fn fragment_compiled(self, shader: Option<CompilationResult>) -> Self {
        if let Some(compiled) = shader {
            return Self { fragment: Some(compiled), ..self };
        }

        self
    }

    // Adds a variable to this builder. The variable name is used to match the binding up with the
    // shader source bindings.
    pub fn add_table_variable(self, key: &str, variable: IndexedResource) -> Self {
        let mut table_variables = self.table_variables;
        table_variables.insert(key.to_string(), variable);

        Self { table_variables, ..self }
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
            let mut merged_vars: HashMap<String, (dashi::BindGroupVariable, dashi::ShaderType)> =
                HashMap::new();

            let mut collect_vars = |stage: &CompilationResult, shader_stage: dashi::ShaderType| {
                for var in stage.variables.iter().filter(|var| var.set == set) {
                    merged_vars
                        .entry(var.name.clone())
                        .and_modify(|(_existing, stage_flags)| {
                            *stage_flags = merge_stage_flags(*stage_flags, shader_stage);
                        })
                        .or_insert((var.kind.clone(), shader_stage));
                }
            };

            collect_vars(&vertex, vertex.stage);
            collect_vars(&fragment, fragment.stage);

            if merged_vars.is_empty() {
                continue;
            }

            let mut merged_vars: Vec<(String, (dashi::BindGroupVariable, dashi::ShaderType))> =
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
            for (name, (var, _)) in merged_vars.iter() {
                if let Some(res) = variables.get(name) {
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

        for set in 0..4u32 {
            let mut combined_vars: HashMap<u32, dashi::BindGroupVariable> = HashMap::new();

            for var in vertex.variables.iter().chain(fragment.variables.iter()) {
                if var.set != set {
                    continue;
                }

                combined_vars.entry(var.kind.binding).or_insert_with(|| var.kind.clone());
            }

            if combined_vars.is_empty() {
                continue;
            }

            let mut merged_vars: Vec<dashi::BindGroupVariable> = combined_vars.into_values().collect();
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
            let mut indexed_bindings: Vec<IndexedBindingInfo> = Vec::new();
            let mut pending_names = Vec::new();
            let mut bound_indices = HashSet::new();
            for var in vertex.variables.iter().chain(fragment.variables.iter()) {
                if var.set != set {
                    continue;
                }

                if let Some(resource) = table_variables.get(&var.name) {
                    if bound_indices.insert(var.kind.binding) {
                        indexed_bindings.push(IndexedBindingInfo {
                            resources: std::slice::from_ref(resource),
                            binding: var.kind.binding,
                        });
                    }
                    pending_names.push((var.name.clone(), var.kind.binding));
                }
            }

            if !indexed_bindings.is_empty() {
                let table = ctx
                    .make_bind_table(&BindTableInfo {
                        debug_name: "bento_bind_table",
                        layout,
                        bindings: indexed_bindings.as_slice(),
                        set,
                    })
                    .ok()?;
                for (name, binding) in pending_names {
                    table_bindings.insert(name, (table, binding));
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
            stride: vertex.metadata.vertex.as_ref().map(|v| v.stride).unwrap_or_default(),
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

pub struct CSO {
    layout: Handle<ComputePipelineLayout>,
    handle: Handle<ComputePipeline>,
    bind_groups: Vec<Handle<BindGroup>>,
    bind_table: Vec<Handle<BindTable>>,
    table_bindings: HashMap<String, (Handle<BindTable>, u32)>,
    ctx: NonNull<Context>,
}

impl CSO {
    pub fn update_table(&mut self, key: &str, resource: IndexedResource) {
        if let Some((table, binding)) = self.table_bindings.get(key).copied() {
            let bindings = [IndexedBindingInfo {
                resources: std::slice::from_ref(&resource),
                binding,
            }];

            let ctx = unsafe { self.ctx.as_mut() };
            let _ = ctx.update_bind_table(&dashi::BindTableUpdateInfo { table, bindings: &bindings });
        }
    }
}
pub struct ComputePipelineBuilder {
    shader: Option<CompilationResult>,
    variables: HashMap<String, ShaderResource>,
    table_variables: HashMap<String, IndexedResource>,
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
        if let Some(bytes) = shader {
            if let Ok(result) = CompilationResult::from_bytes(bytes) {
                return Self { shader: Some(result), ..self };
            }
        }

        self
    }

    pub fn shader_compiled(self, shader: Option<CompilationResult>) -> Self {
        if let Some(shader) = shader {
            return Self { shader: Some(shader), ..self };
        }

        self
    }

    // Adds a bind table variable to this builder. The variable name is used to match the binding up with the
    // shader source bindings.
    pub fn add_table_variable(self, key: &str, variable: IndexedResource) -> Self {
        let mut table_variables = self.table_variables;
        table_variables.insert(key.to_string(), variable);

        Self { table_variables, ..self }
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
                .map(|var| var.kind.clone())
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

        for set in 0..4u32 {
            let vars: Vec<dashi::BindGroupVariable> = shader
                .variables
                .iter()
                .filter(|var| var.set == set)
                .map(|var| var.kind.clone())
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

            let mut indexed_bindings = Vec::new();
            let mut pending_names = Vec::new();
            for var in shader.variables.iter() {
                if var.set != set {
                    continue;
                }

                if let Some(res) = table_variables.get(&var.name) {
                    indexed_bindings.push(IndexedBindingInfo {
                        resources: std::slice::from_ref(res),
                        binding: var.kind.binding,
                    });
                    pending_names.push((var.name.clone(), var.kind.binding));
                }
            }

            if !indexed_bindings.is_empty() {
                let table = ctx
                    .make_bind_table(&BindTableInfo {
                        debug_name: "bento_compute_bind_table",
                        layout,
                        bindings: indexed_bindings.as_slice(),
                        set,
                    })
                    .ok()?;
                for (name, binding) in pending_names {
                    table_bindings.insert(name, (table, binding));
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
